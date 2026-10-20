//! Versioned compositor bridge. The compositor runs the same gesture engine before delivery.
//! No raw device access, pointer polling, or GUI round trips occur on the input path.
use crate::{gesture::InputDecision, model::Action, GestureEngine, InputEvent, Settings};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const LEASE_MS: u64 = 1_500;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub version: u32,
    pub session: String,
    pub operation: Operation,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Operation {
    Connect {
        settings: Settings,
        enabled: bool,
        socket: String,
    },
    Configure(Settings),
    Enable(bool),
    Heartbeat,
    Disconnect,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub version: u32,
    pub session: String,
    pub sequence: u64,
    /// Origin-display physical pixels per logical desktop pixel for these actions.
    pub scale: f64,
    pub enabled: bool,
    pub actions: Vec<Action>,
}

impl Packet {
    /// The native GTK painter uses logical desktop coordinates; hit testing stays physical.
    pub fn logical_actions(self) -> Result<Vec<Action>, String> {
        if !self.scale.is_finite() || !(0.25..=8.0).contains(&self.scale) || self.actions.len() > 64
        {
            return Err("Invalid compositor coordinate scale/action bounds".into());
        }
        let point = |p: crate::Point| -> Result<crate::Point, String> {
            let x = f64::from(p.x) / self.scale;
            let y = f64::from(p.y) / self.scale;
            if x.abs() > 1_000_000.0 || y.abs() > 1_000_000.0 {
                return Err("Compositor coordinate exceeds desktop bounds".into());
            }
            Ok(crate::Point::new(x.round() as i32, y.round() as i32))
        };
        self.actions
            .into_iter()
            .map(|a| {
                Ok(match a {
                    Action::WheelOpen { wheel, origin } => Action::WheelOpen {
                        wheel,
                        origin: point(origin)?,
                    },
                    Action::WheelMove {
                        wheel,
                        point: p,
                        slot,
                    } => {
                        if slot.is_some_and(|s| s >= 8) {
                            return Err("Invalid wheel slice".into());
                        }
                        Action::WheelMove {
                            wheel,
                            point: point(p)?,
                            slot,
                        }
                    }
                    Action::Select {
                        wheel,
                        point: p,
                        slot,
                    } => {
                        if slot >= 8 {
                            return Err("Invalid wheel slice".into());
                        }
                        Action::Select {
                            wheel,
                            point: point(p)?,
                            slot,
                        }
                    }
                    Action::Click { wheel, point: p } => Action::Click {
                        wheel,
                        point: point(p)?,
                    },
                    other => other,
                })
            })
            .collect()
    }
}

pub struct Session {
    pub engine: GestureEngine,
    session: Option<String>,
    heartbeat_ms: u64,
    sequence: u64,
    pub scale: f64,
}

impl Default for Session {
    fn default() -> Self {
        let mut engine = GestureEngine::new(Settings::default()).expect("valid built-in settings");
        engine.set_enabled(false);
        engine.set_suspended(true);
        Self {
            engine,
            session: None,
            heartbeat_ms: 0,
            sequence: 0,
            scale: 1.0,
        }
    }
}

impl Session {
    pub fn control(
        &mut self,
        control: &Control,
        now: u64,
        safe: bool,
    ) -> Result<InputDecision, String> {
        if control.version != VERSION
            || control.session.len() != 32
            || !control.session.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Unsupported Pingloom bridge protocol/session".into());
        }
        let mut decision = InputDecision::default();
        match &control.operation {
            Operation::Connect {
                settings, enabled, ..
            } => {
                if !safe {
                    return Err("Compositor session is not available for gestures".into());
                }
                settings.validate().map_err(|e| e.to_string())?;
                // A replacement UI must not inherit a pending gesture.
                decision = self.engine.set_enabled(false);
                let _ = self
                    .engine
                    .set_settings(settings.clone())
                    .map_err(|e| e.to_string())?;
                self.engine.set_suspended(false);
                self.engine.set_enabled(*enabled);
                self.session = Some(control.session.clone());
                self.heartbeat_ms = now;
            }
            operation => {
                if self.session.as_deref() != Some(control.session.as_str()) {
                    return Err("Stale Pingloom session".into());
                }
                match operation {
                    Operation::Configure(settings) => {
                        decision = self
                            .engine
                            .set_settings(settings.clone())
                            .map_err(|e| e.to_string())?;
                    }
                    Operation::Enable(enabled) => {
                        if *enabled && !safe {
                            return Err("Compositor session is not available for gestures".into());
                        }
                        decision = self.engine.set_enabled(*enabled);
                    }
                    Operation::Heartbeat => {}
                    Operation::Disconnect => {
                        decision = self.engine.set_enabled(false);
                        self.session = None;
                    }
                    Operation::Connect { .. } => unreachable!(),
                }
                self.heartbeat_ms = now;
            }
        }
        Ok(decision)
    }

    pub fn guard(&mut self, now: u64, safe: bool) -> InputDecision {
        let expired = now.saturating_sub(self.heartbeat_ms) > LEASE_MS;
        if expired {
            self.session = None;
        }
        let available = safe && self.session.is_some();
        let mut decision = self.engine.set_suspended(!available);
        let next = if available {
            self.engine.tick(now)
        } else {
            // A key-up may be delivered on another VT or the lock surface. Do not
            // carry forwarded modifiers into the next admitted gesture session.
            self.engine.resync_key_states(|_| false);
            self.engine.set_enabled(false)
        };
        decision.actions.extend(next.actions);
        decision.replay.extend(next.replay);
        decision
    }

    pub fn event(&mut self, event: InputEvent) -> InputDecision {
        self.engine.on_event(event)
    }

    pub fn packet(&mut self, actions: Vec<Action>) -> Option<Packet> {
        let session = self.session.clone()?;
        self.sequence = self.sequence.checked_add(1)?;
        Some(Packet {
            version: VERSION,
            session,
            sequence: self.sequence,
            scale: self.scale,
            enabled: self.engine.enabled(),
            actions,
        })
    }

    pub fn disconnect(&mut self) -> InputDecision {
        self.session = None;
        let mut decision = self.engine.set_suspended(true);
        let next = self.engine.set_enabled(false);
        decision.actions.extend(next.actions);
        decision.replay.extend(next.replay);
        decision
    }
}

/// Linux evdev key codes, independent of layout. Compositor XKB codes include an extra 8.
pub fn vk(code: u32) -> u8 {
    const LETTERS: &[(u32, u8)] = &[
        (30, b'A'),
        (48, b'B'),
        (46, b'C'),
        (32, b'D'),
        (18, b'E'),
        (33, b'F'),
        (34, b'G'),
        (35, b'H'),
        (23, b'I'),
        (36, b'J'),
        (37, b'K'),
        (38, b'L'),
        (50, b'M'),
        (49, b'N'),
        (24, b'O'),
        (25, b'P'),
        (16, b'Q'),
        (19, b'R'),
        (31, b'S'),
        (20, b'T'),
        (22, b'U'),
        (47, b'V'),
        (17, b'W'),
        (45, b'X'),
        (21, b'Y'),
        (44, b'Z'),
    ];
    match code {
        1 => 0x1b,
        14 => 0x08,
        15 => 0x09,
        28 => 0x0d,
        29 => 0xa2,
        42 => 0xa0,
        54 => 0xa1,
        56 => 0xa4,
        57 => 0x20,
        58 => 0x14,
        97 => 0xa3,
        100 => 0xa5,
        125 => 0x5b,
        126 => 0x5c,
        2..=10 => b'1' + (code - 2) as u8,
        11 => b'0',
        59..=68 => 0x70 + (code - 59) as u8,
        87 => 0x7a,
        88 => 0x7b,
        102 => 0x24,
        103 => 0x26,
        104 => 0x21,
        105 => 0x25,
        106 => 0x27,
        107 => 0x23,
        108 => 0x28,
        109 => 0x22,
        110 => 0x2d,
        111 => 0x2e,
        12 => 0xbd,
        13 => 0xbb,
        26 => 0xdb,
        27 => 0xdd,
        39 => 0xba,
        40 => 0xde,
        41 => 0xc0,
        43 => 0xdc,
        51 => 0xbc,
        52 => 0xbe,
        53 => 0xbf,
        _ => LETTERS
            .iter()
            .find(|(key, _)| *key == code)
            .map_or(0xff, |(_, vk)| *vk),
    }
}

pub fn evdev(vk_code: u8) -> Option<u32> {
    (1..=126).find(|&code| vk(code) == vk_code && vk_code != 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Button, Point};
    fn control(session: &str, operation: Operation) -> Control {
        Control {
            version: VERSION,
            session: session.into(),
            operation,
        }
    }
    #[test]
    fn lease_cancels_pending_and_drains_its_suppressed_release() {
        let mut s = Session::default();
        let id = "0123456789abcdef0123456789abcdef";
        s.control(
            &control(
                id,
                Operation::Connect {
                    settings: Settings::default(),
                    enabled: true,
                    socket: String::new(),
                },
            ),
            0,
            true,
        )
        .unwrap();
        s.event(InputEvent::KeyDown {
            vk: 0xa4,
            injected: false,
        });
        assert!(
            s.event(InputEvent::PointerDown {
                button: Button::Left,
                point: Point::new(100, 100),
                time_ms: 1,
                injected: false,
                fullscreen_app: false,
                dpi_scale: 1.0,
            })
            .swallow
        );
        let d = s.guard(LEASE_MS + 1, true);
        assert_eq!(d.actions, [Action::Cancel]);
        assert!(d.replay.is_empty());
        assert!(!s.engine.enabled());
        assert!(
            s.event(InputEvent::PointerUp {
                button: Button::Left,
                point: Point::new(100, 100),
                time_ms: LEASE_MS + 2,
                injected: false
            })
            .swallow
        );
        assert!(s.packet(Vec::new()).is_none());
        assert!(s
            .control(&control(id, Operation::Heartbeat), LEASE_MS + 2, true)
            .is_err());
    }
    #[test]
    fn unsafe_boundary_forgets_forwarded_modifiers() {
        let id = "0123456789abcdef0123456789abcdef";
        let mut s = Session::default();
        s.control(
            &control(
                id,
                Operation::Connect {
                    settings: Settings::default(),
                    enabled: true,
                    socket: String::new(),
                },
            ),
            0,
            true,
        )
        .unwrap();
        s.event(InputEvent::KeyDown {
            vk: 0xa4,
            injected: false,
        });
        s.guard(1, false);
        s.guard(2, true);
        s.control(&control(id, Operation::Enable(true)), 3, true)
            .unwrap();
        assert!(
            !s.event(InputEvent::PointerDown {
                button: Button::Left,
                point: Point::new(100, 100),
                time_ms: 4,
                injected: false,
                fullscreen_app: false,
                dpi_scale: 1.0,
            })
            .swallow
        );
    }

    #[test]
    fn physical_actions_project_once_into_logical_desktop_coordinates() {
        let packet = Packet {
            version: VERSION,
            session: String::new(),
            sequence: 1,
            scale: 1.5,
            enabled: true,
            actions: vec![Action::Click {
                wheel: crate::WheelKind::Ping,
                point: Point::new(-300, 150),
            }],
        };
        assert_eq!(
            packet.logical_actions().unwrap(),
            [Action::Click {
                wheel: crate::WheelKind::Ping,
                point: Point::new(-200, 100)
            }]
        );
    }

    #[test]
    fn locked_session_cannot_be_enabled_and_stale_ui_cannot_control_replacement() {
        let id = "0123456789abcdef0123456789abcdef";
        let mut s = Session::default();
        s.control(
            &control(
                id,
                Operation::Connect {
                    settings: Settings::default(),
                    enabled: true,
                    socket: String::new(),
                },
            ),
            0,
            true,
        )
        .unwrap();
        s.guard(1, false);
        assert!(!s.engine.enabled());
        assert!(s
            .control(&control(id, Operation::Enable(true)), 2, false)
            .is_err());
        assert!(s
            .control(
                &control(
                    "ffffffffffffffffffffffffffffffff",
                    Operation::Configure(Settings::default())
                ),
                3,
                true
            )
            .is_err());
        for code in [1, 29, 42, 54, 56, 97, 100, 125, 126, 16, 30, 111] {
            assert_eq!(evdev(vk(code)), Some(code));
        }
    }
}
