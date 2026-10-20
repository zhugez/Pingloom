//! Pingloom integration for pinned niri. Gesture decisions stay on niri's input loop.
//! The UI receives semantic actions only; it cannot delay native input delivery.
use crate::{backend::Backend, layout::LayoutElement as _, niri::State, utils::get_monotonic_time};
use pingloom_core::{
    gesture::{GestureState, InputDecision, ReplayInput},
    model::Button,
    niri_bridge::{Control, Operation, Session},
    InputEvent, Point,
};
use smithay::{
    backend::input::{ButtonState, KeyState, Keycode},
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
};
use std::{
    os::unix::{fs::MetadataExt, net::UnixDatagram},
    path::PathBuf,
    time::Instant,
};

pub struct Bridge {
    pub session: Session,
    sink: Option<UnixDatagram>,
    clock: Instant,
    topology: Vec<(String, (i32, i32), (i32, i32), f64)>,
    buttons: [u32; 5],
}
impl Default for Bridge {
    fn default() -> Self {
        Self {
            session: Session::default(),
            sink: None,
            clock: Instant::now(),
            topology: Vec::new(),
            buttons: [0x110, 0x111, 0x112, 0x113, 0x114],
        }
    }
}
impl Bridge {
    fn now(&self) -> u64 {
        self.clock.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn emit(&mut self, actions: Vec<pingloom_core::Action>) -> Result<(), ()> {
        let Some(packet) = self.session.packet(actions) else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(&packet).map_err(|_| ())?;
        if bytes.len() > 16384 {
            return Err(());
        }
        self.sink.as_ref().ok_or(())?.send(&bytes).map_err(|_| ())?;
        Ok(())
    }
}
fn core_button(code: u32) -> Option<Button> {
    match code {
        0x110 => Some(Button::Left),
        0x111 => Some(Button::Right),
        0x112 => Some(Button::Middle),
        0x113 | 0x116 => Some(Button::X1),
        0x114 | 0x115 => Some(Button::X2),
        _ => None,
    }
}
// A physical modifier-up must still update Smithay after lock/seat loss. Its
// synthetic menu mask must NOT be replayed into the newly focused lock surface.
fn forwarded_release(state: KeyState, vk: u8, decision: &InputDecision) -> bool {
    state == KeyState::Released && decision.replay.contains(&ReplayInput::KeyUp(vk))
}
fn button_index(button: Button) -> usize {
    match button {
        Button::Left => 0,
        Button::Right => 1,
        Button::Middle => 2,
        Button::X1 => 3,
        Button::X2 => 4,
    }
}

impl State {
    fn pingloom_safe(&self) -> bool {
        let active = match &self.backend {
            Backend::Tty(tty) => tty.pingloom_session_active(),
            Backend::Winit(_) => true,
            Backend::Headless(_) => false,
        };
        active
            && self.niri.monitors_active
            && !self.niri.is_locked()
            && !self.niri.layout.is_overview_open()
    }
    pub fn pingloom_control(&mut self, value: serde_json::Value) -> Result<(), String> {
        if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > 65536 {
            return Err("Pingloom command too large".into());
        }
        let control: Control = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let safe = self.pingloom_safe();
        let now = self.niri.pingloom.now();
        let time = get_monotonic_time().as_millis() as u32;
        let recovery = self.niri.pingloom.session.guard(now, safe);
        self.pingloom_apply(recovery, time);
        let mut replacement_sink = None;
        if let Operation::Connect { socket, .. } = &control.operation {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("Missing runtime directory")?;
            let runtime = PathBuf::from(runtime);
            let path = PathBuf::from(socket);
            let parent = runtime.join("pingloom");
            if path.parent() != Some(parent.as_path())
                || path.file_name().and_then(|n| n.to_str()) != Some("niri-events.sock")
            {
                return Err(
                    "Pingloom event socket must be in its private runtime directory".into(),
                );
            }
            let meta = std::fs::symlink_metadata(&parent).map_err(|e| e.to_string())?;
            let owner = std::fs::metadata(&runtime)
                .map_err(|e| e.to_string())?
                .uid();
            if !meta.is_dir() || meta.uid() != owner || meta.mode() & 0o077 != 0 {
                return Err("Unsafe Pingloom runtime directory".into());
            }
            let sink = UnixDatagram::unbound().map_err(|e| e.to_string())?;
            sink.set_nonblocking(true).map_err(|e| e.to_string())?;
            sink.connect(&path).map_err(|e| e.to_string())?;
            // Validate before replacing an established sink/session.
            if control.version != pingloom_core::niri_bridge::VERSION || !safe {
                return Err("Unsupported or inactive compositor bridge".into());
            }
            if let Operation::Connect { settings, .. } = &control.operation {
                settings.validate().map_err(|e| e.to_string())?;
            }
            replacement_sink = Some(sink);
        }
        let decision = self.niri.pingloom.session.control(&control, now, safe)?;
        if let Some(sink) = replacement_sink {
            self.niri.pingloom.sink = Some(sink);
        }
        self.pingloom_apply(decision, time);
        if self.niri.pingloom.emit(Vec::new()).is_err() {
            let recovery = self.niri.pingloom.session.disconnect();
            self.pingloom_apply(recovery, time);
            return Err("Pingloom event receiver unavailable".into());
        }
        Ok(())
    }
    pub fn pingloom_refresh(&mut self) {
        let safe = self.pingloom_safe();
        let topology: Vec<_> = self
            .niri
            .global_space
            .outputs()
            .filter_map(|o| {
                self.niri.global_space.output_geometry(o).map(|g| {
                    (
                        o.name(),
                        (g.loc.x, g.loc.y),
                        (g.size.w, g.size.h),
                        o.current_scale().fractional_scale(),
                    )
                })
            })
            .collect();
        let changed =
            !self.niri.pingloom.topology.is_empty() && self.niri.pingloom.topology != topology;
        self.niri.pingloom.topology = topology;
        let now = self.niri.pingloom.now();
        let decision = self.niri.pingloom.session.guard(now, safe && !changed);
        if !decision.actions.is_empty() || !decision.replay.is_empty() {
            self.pingloom_apply(decision, get_monotonic_time().as_millis() as u32);
        }
    }
    fn pingloom_apply(&mut self, mut decision: InputDecision, time: u32) -> bool {
        if !decision.actions.is_empty()
            && self.niri.pingloom.emit(decision.actions.clone()).is_err()
        {
            let recovery = self.niri.pingloom.session.disconnect();
            self.niri.pingloom.sink = None;
            decision.replay.extend(recovery.replay);
        }
        // Never replay a pre-lock event into the lock surface or an inactive seat.
        if !self.pingloom_safe() {
            return decision.swallow;
        }
        for replay in decision.replay {
            match replay {
                ReplayInput::ButtonDown(button) | ReplayInput::ButtonUp(button) => {
                    let pressed = matches!(replay, ReplayInput::ButtonDown(_));
                    let code = self.niri.pingloom.buttons[button_index(button)];
                    self.pingloom_native_button(
                        code,
                        if pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                        time,
                        true,
                    );
                }
                ReplayInput::KeyDown(vk) | ReplayInput::KeyUp(vk) => {
                    let pressed = matches!(replay, ReplayInput::KeyDown(_));
                    let state = if pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    if vk == 0xfc {
                        // An unmapped key masks a bare modifier release without running compositor bindings.
                        let keyboard = self.niri.seat.get_keyboard().unwrap();
                        keyboard.input::<(), _>(
                            self,
                            Keycode::from(248u32),
                            state,
                            SERIAL_COUNTER.next_serial(),
                            time,
                            |_, _, _| FilterResult::Forward,
                        );
                    } else if let Some(code) = pingloom_core::niri_bridge::evdev(vk) {
                        self.pingloom_native_key(
                            Keycode::from(code + 8),
                            state,
                            time,
                            &mut false,
                            true,
                        );
                    }
                }
            }
        }
        decision.swallow
    }
    pub fn pingloom_key(&mut self, code: Keycode, state: KeyState, time: u32) -> bool {
        self.pingloom_refresh();
        let vk = pingloom_core::niri_bridge::vk(u32::from(code).saturating_sub(8));
        let event = if state == KeyState::Pressed {
            InputEvent::KeyDown {
                vk,
                injected: false,
            }
        } else {
            InputEvent::KeyUp {
                vk,
                injected: false,
            }
        };
        let decision = self.niri.pingloom.session.event(event);
        let physical_release = !self.pingloom_safe() && forwarded_release(state, vk, &decision);
        self.pingloom_apply(decision, time) && !physical_release
    }
    pub fn pingloom_motion(&mut self, time: u32) {
        self.pingloom_refresh();
        let location = self.niri.seat.get_pointer().unwrap().current_location();
        let scale = self.niri.pingloom.session.scale;
        let event = InputEvent::PointerMove {
            point: Point::new(
                (location.x * scale).round() as i32,
                (location.y * scale).round() as i32,
            ),
            time_ms: self.niri.pingloom.now(),
            injected: false,
        };
        let decision = self.niri.pingloom.session.event(event);
        self.pingloom_apply(decision, time);
    }
    pub fn pingloom_button(&mut self, code: u32, state: ButtonState, time: u32) -> bool {
        self.pingloom_refresh();
        let Some(button) = core_button(code) else {
            return false;
        };
        self.niri.pingloom.buttons[button_index(button)] = code;
        let location = self.niri.seat.get_pointer().unwrap().current_location();
        if self.niri.pingloom.session.engine.state() == GestureState::Idle
            && state == ButtonState::Pressed
        {
            if let Some(output) = self.niri.global_space.output_under(location).next() {
                self.niri.pingloom.session.scale = output.current_scale().fractional_scale();
            }
        }
        let scale = self.niri.pingloom.session.scale;
        let point = Point::new(
            (location.x * scale).round() as i32,
            (location.y * scale).round() as i32,
        );
        let fullscreen_app = self
            .niri
            .window_under_cursor()
            .is_some_and(|w| w.sizing_mode().is_fullscreen());
        let time_ms = self.niri.pingloom.now();
        let event = if state == ButtonState::Pressed {
            InputEvent::PointerDown {
                button,
                point,
                time_ms,
                injected: false,
                fullscreen_app,
                dpi_scale: scale,
            }
        } else {
            InputEvent::PointerUp {
                button,
                point,
                time_ms,
                injected: false,
            }
        };
        let decision = self.niri.pingloom.session.event(event);
        self.pingloom_apply(decision, time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_discards_mask_but_forwards_the_real_modifier_release() {
        let mut session = Session::default();
        session
            .control(
                &Control {
                    version: pingloom_core::niri_bridge::VERSION,
                    session: "0123456789abcdef0123456789abcdef".into(),
                    operation: Operation::Connect {
                        settings: pingloom_core::Settings::default(),
                        enabled: true,
                        socket: String::new(),
                    },
                },
                0,
                true,
            )
            .unwrap();
        session.event(InputEvent::KeyDown {
            vk: 0xa4,
            injected: false,
        });
        assert!(
            session
                .event(InputEvent::PointerDown {
                    button: Button::Left,
                    point: Point::new(100, 100),
                    time_ms: 1,
                    injected: false,
                    fullscreen_app: false,
                    dpi_scale: 1.0,
                })
                .swallow
        );
        session.guard(2, false);
        let decision = session.event(InputEvent::KeyUp {
            vk: 0xa4,
            injected: false,
        });
        assert!(decision.swallow);
        assert!(forwarded_release(KeyState::Released, 0xa4, &decision));
        assert!(!forwarded_release(KeyState::Pressed, 0xa4, &decision));
        assert!(!forwarded_release(KeyState::Released, b'Q', &decision));
    }
}
