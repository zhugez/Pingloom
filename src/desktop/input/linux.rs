use super::Shared;
use pingloom_core::gesture::ReplayInput;
use pingloom_core::{Button, InputEvent, Point};
use rdev::{EventType, Key};
use std::{
    fs::{File, OpenOptions},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt},
    rust_connection::RustConnection,
};

const KEYS: &[(u8, Key)] = &[
    (0xa4, Key::Alt),
    (0xa5, Key::AltGr),
    (0xa2, Key::ControlLeft),
    (0xa3, Key::ControlRight),
    (0xa0, Key::ShiftLeft),
    (0xa1, Key::ShiftRight),
    (0x5b, Key::MetaLeft),
    (0x5c, Key::MetaRight),
    (0x14, Key::CapsLock),
    (0x08, Key::Backspace),
    (0x09, Key::Tab),
    (0x0d, Key::Return),
    (0x1b, Key::Escape),
    (0x20, Key::Space),
    (0x21, Key::PageUp),
    (0x22, Key::PageDown),
    (0x23, Key::End),
    (0x24, Key::Home),
    (0x25, Key::LeftArrow),
    (0x26, Key::UpArrow),
    (0x27, Key::RightArrow),
    (0x28, Key::DownArrow),
    (0x2d, Key::Insert),
    (0x2e, Key::Delete),
    (0x30, Key::Num0),
    (0x31, Key::Num1),
    (0x32, Key::Num2),
    (0x33, Key::Num3),
    (0x34, Key::Num4),
    (0x35, Key::Num5),
    (0x36, Key::Num6),
    (0x37, Key::Num7),
    (0x38, Key::Num8),
    (0x39, Key::Num9),
    (0x41, Key::KeyA),
    (0x42, Key::KeyB),
    (0x43, Key::KeyC),
    (0x44, Key::KeyD),
    (0x45, Key::KeyE),
    (0x46, Key::KeyF),
    (0x47, Key::KeyG),
    (0x48, Key::KeyH),
    (0x49, Key::KeyI),
    (0x4a, Key::KeyJ),
    (0x4b, Key::KeyK),
    (0x4c, Key::KeyL),
    (0x4d, Key::KeyM),
    (0x4e, Key::KeyN),
    (0x4f, Key::KeyO),
    (0x50, Key::KeyP),
    (0x51, Key::KeyQ),
    (0x52, Key::KeyR),
    (0x53, Key::KeyS),
    (0x54, Key::KeyT),
    (0x55, Key::KeyU),
    (0x56, Key::KeyV),
    (0x57, Key::KeyW),
    (0x58, Key::KeyX),
    (0x59, Key::KeyY),
    (0x5a, Key::KeyZ),
    (0x70, Key::F1),
    (0x71, Key::F2),
    (0x72, Key::F3),
    (0x73, Key::F4),
    (0x74, Key::F5),
    (0x75, Key::F6),
    (0x76, Key::F7),
    (0x77, Key::F8),
    (0x78, Key::F9),
    (0x79, Key::F10),
    (0x7a, Key::F11),
    (0x7b, Key::F12),
    (0xba, Key::SemiColon),
    (0xbb, Key::Equal),
    (0xbc, Key::Comma),
    (0xbd, Key::Minus),
    (0xbe, Key::Dot),
    (0xbf, Key::Slash),
    (0xc0, Key::BackQuote),
    (0xdb, Key::LeftBracket),
    (0xdc, Key::BackSlash),
    (0xdd, Key::RightBracket),
    (0xde, Key::Quote),
    (0x60, Key::Kp0),
    (0x61, Key::Kp1),
    (0x62, Key::Kp2),
    (0x63, Key::Kp3),
    (0x64, Key::Kp4),
    (0x65, Key::Kp5),
    (0x66, Key::Kp6),
    (0x67, Key::Kp7),
    (0x68, Key::Kp8),
    (0x69, Key::Kp9),
    (0x6a, Key::KpMultiply),
    (0x6b, Key::KpPlus),
    (0x6d, Key::KpMinus),
    (0x6f, Key::KpDivide),
];
fn key(vk: u8) -> Option<Key> {
    let vk = match vk {
        0x10 => 0xa0,
        0x11 => 0xa2,
        0x12 => 0xa4,
        _ => vk,
    };
    KEYS.iter()
        .find(|(code, _)| *code == vk)
        .map(|(_, key)| *key)
}
pub(super) fn supported_vk(vk: u8) -> bool {
    key(vk).is_some()
}
fn vk(key: Key) -> u8 {
    KEYS.iter()
        .find(|(_, k)| *k == key)
        .map_or(0xff, |(code, _)| *code)
}
fn button(button: rdev::Button) -> Option<Button> {
    match button {
        rdev::Button::Left => Some(Button::Left),
        rdev::Button::Right => Some(Button::Right),
        rdev::Button::Middle => Some(Button::Middle),
        _ => None,
    }
}
struct Display {
    connection: RustConnection,
    root: u32,
}
impl Display {
    fn new() -> Result<Self, String> {
        let (connection, screen) =
            x11rb::connect(None).map_err(|e| format!("X11 display unavailable: {e}"))?;
        let root = connection.setup().roots[screen].root;
        Ok(Self { connection, root })
    }
    fn pointer(&self) -> Option<Point> {
        let reply = self
            .connection
            .query_pointer(self.root)
            .ok()?
            .reply()
            .ok()?;
        Some(Point::new(reply.root_x.into(), reply.root_y.into()))
    }
    fn fullscreen(&self, point: Point) -> bool {
        self.fullscreen_checked(point).unwrap_or(false)
    }
    fn fullscreen_checked(&self, point: Point) -> Option<bool> {
        let atom = |name: &[u8]| {
            self.connection
                .intern_atom(true, name)
                .ok()?
                .reply()
                .ok()
                .map(|reply| reply.atom)
        };
        let active = atom(b"_NET_ACTIVE_WINDOW")?;
        let state = atom(b"_NET_WM_STATE")?;
        let full = atom(b"_NET_WM_STATE_FULLSCREEN")?;
        if active == 0 || state == 0 || full == 0 {
            return Some(false);
        }
        let property = self
            .connection
            .get_property(false, self.root, active, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let window = property.value32()?.next()?;
        let property = self
            .connection
            .get_property(false, window, state, AtomEnum::ATOM, 0, 64)
            .ok()?
            .reply()
            .ok()?;
        if !property.value32()?.any(|value| value == full) {
            return Some(false);
        }
        // A closing foreign window is a normal protocol error, not a process-global Xlib error.
        let geometry = self.connection.get_geometry(window).ok()?.reply().ok()?;
        let origin = self
            .connection
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some(
            point.x >= origin.dst_x as i32
                && point.y >= origin.dst_y as i32
                && point.x < origin.dst_x as i32 + geometry.width as i32
                && point.y < origin.dst_y as i32 + geometry.height as i32,
        )
    }
}
pub(super) fn run(shared: Arc<Shared>) -> Result<(), String> {
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Err("Wayland does not support this global gesture backend. Sign into an X11 session; Settings remains available.".into());
    }
    OpenOptions::new().write(true).open("/dev/uinput").map_err(|_|"Native gestures need user access to /dev/uinput and the input devices. See docs/PLATFORMS.md; Pingloom never elevates privileges automatically.".to_owned())?;
    let entries =
        std::fs::read_dir("/dev/input").map_err(|e| format!("Input devices unavailable: {e}"))?;
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("event") {
            File::open(entry.path()).map_err(|_| {
                "User cannot read input devices; gestures remain paused. See docs/PLATFORMS.md."
                    .to_owned()
            })?;
        }
    }
    let display = Display::new()?;
    let initial = shared.state.lock().unwrap().engine.settings().clone();
    super::validate(&initial).map_err(|e| e.message)?;
    shared.ready();
    rdev::grab(move |event| {
        let time_ms = shared.time_ms();
        // rdev's evdev coordinates ignore acceleration. Keyboard input needs no X11 query.
        let point = if matches!(
            event.event_type,
            EventType::KeyPress(_) | EventType::KeyRelease(_)
        ) {
            Point::new(0, 0)
        } else {
            let Some(point) = display.pointer() else {
                shared.fail("X11 pointer connection lost; gestures paused.".into());
                return Some(event);
            };
            point
        };
        let native = match event.event_type {
            EventType::KeyPress(key) => Some(InputEvent::KeyDown {
                vk: vk(key),
                injected: false,
            }),
            EventType::KeyRelease(key) => Some(InputEvent::KeyUp {
                vk: vk(key),
                injected: false,
            }),
            EventType::MouseMove { .. } => Some(InputEvent::PointerMove {
                point,
                time_ms,
                injected: false,
            }),
            EventType::ButtonPress(b) => button(b).map(|button| InputEvent::PointerDown {
                button,
                point,
                time_ms,
                injected: false,
                fullscreen_app: display.fullscreen(point),
                dpi_scale: shared.scale_at(point),
            }),
            EventType::ButtonRelease(b) => {
                // The grab callback precedes X11 motion forwarding. Catch up at release,
                // including a very fast drag containing only one motion packet.
                if b == rdev::Button::Left {
                    shared.input(InputEvent::PointerMove {
                        point,
                        time_ms,
                        injected: false,
                    });
                }
                button(b).map(|button| InputEvent::PointerUp {
                    button,
                    point,
                    time_ms,
                    injected: false,
                })
            }
            _ => None,
        };
        if native.is_some_and(|native| shared.input(native)) {
            None
        } else {
            Some(event)
        }
    })
    .map_err(|e| {
        format!("X11 input grab stopped: {e:?}. Gestures paused; check device permissions.")
    })
}
static MENU_CODE: AtomicU8 = AtomicU8::new(0);
fn neutral_key(min: u8, width: u8, symbols: &[u32]) -> Option<u8> {
    if width == 0 {
        return None;
    }
    let rows = symbols.chunks(width as usize);
    let offset = rows
        .clone()
        .position(|row| row.contains(&0xffd5))
        .or_else(|| {
            rows.enumerate()
                .filter(|(_, row)| row.iter().all(|&symbol| symbol == 0))
                .map(|(index, _)| index)
                .next_back()
        })?;
    min.checked_add(u8::try_from(offset).ok()?)
}
// ponytail: F24 can reach custom application shortcuts; replace with WM-specific menu suppression if reported.
fn menu_key() -> Result<Key, ()> {
    let mut code = MENU_CODE.load(Ordering::Acquire);
    if code == 0 {
        let display = Display::new().map_err(|_| ())?;
        let setup = display.connection.setup();
        let count = setup
            .max_keycode
            .checked_sub(setup.min_keycode)
            .and_then(|n| n.checked_add(1))
            .ok_or(())?;
        let reply = display
            .connection
            .get_keyboard_mapping(setup.min_keycode, count)
            .map_err(|_| ())?
            .reply()
            .map_err(|_| ())?;
        code =
            neutral_key(setup.min_keycode, reply.keysyms_per_keycode, &reply.keysyms).ok_or(())?;
        MENU_CODE.store(code, Ordering::Release);
    }
    Ok(Key::Unknown(code.into()))
}
pub(super) fn replay(event: ReplayInput) -> Result<(), ()> {
    let button = |button| match button {
        Button::Left => Some(rdev::Button::Left),
        Button::Right => Some(rdev::Button::Right),
        Button::Middle => Some(rdev::Button::Middle),
        _ => None,
    };
    let clear_menu = matches!(event, ReplayInput::KeyUp(0xfc));
    let event = match event {
        ReplayInput::KeyDown(0xfc) => EventType::KeyPress(menu_key()?),
        ReplayInput::KeyUp(0xfc) => EventType::KeyRelease(menu_key()?),
        ReplayInput::KeyDown(vk) => EventType::KeyPress(key(vk).ok_or(())?),
        ReplayInput::KeyUp(vk) => EventType::KeyRelease(key(vk).ok_or(())?),
        ReplayInput::ButtonDown(b) => EventType::ButtonPress(button(b).ok_or(())?),
        ReplayInput::ButtonUp(b) => EventType::ButtonRelease(button(b).ok_or(())?),
    };
    rdev::simulate(&event).map_err(|_| ())?;
    if clear_menu {
        MENU_CODE.store(0, Ordering::Release);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_mask_prefers_f24_and_falls_back_to_an_unassigned_key() {
        assert_eq!(neutral_key(8, 2, &[1, 0, 0, 0, 0xffd5, 0]), Some(10));
        assert_eq!(neutral_key(8, 2, &[1, 0, 0, 0]), Some(9));
        assert_eq!(neutral_key(8, 2, &[1, 2]), None);
        assert_eq!(neutral_key(8, 0, &[]), None);
    }
    #[test]
    fn native_keys_round_trip_without_collision() {
        for &(code, key) in KEYS {
            assert_eq!(vk(key), code);
            assert_eq!(super::key(code), Some(key));
        }
        assert_eq!(super::key(0x11), Some(Key::ControlLeft));
    }
}
