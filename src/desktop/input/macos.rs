use super::Shared;
use core_foundation::{
    array::CFArrayGetValueAtIndex,
    base::TCFType,
    dictionary::{CFDictionaryGetValue, CFDictionaryRef},
    number::{kCFNumberSInt64Type, CFNumberGetValue, CFNumberRef},
    runloop::{kCFRunLoopDefaultMode, CFRunLoop},
};
use core_graphics::{
    display::CGDisplay,
    event::{
        CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventType, CGMouseButton, CallbackResult, EventField,
    },
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::{CGPoint, CGRect},
};
use pingloom_core::gesture::ReplayInput;
use pingloom_core::{Button, InputEvent, Point};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
const MARKER: i64 = 0x50494e474c4f4f4d;
const KEYS: &[(u8, u16)] = &[
    (0x41, 0),
    (0x53, 1),
    (0x44, 2),
    (0x46, 3),
    (0x48, 4),
    (0x47, 5),
    (0x5a, 6),
    (0x58, 7),
    (0x43, 8),
    (0x56, 9),
    (0x42, 11),
    (0x51, 12),
    (0x57, 13),
    (0x45, 14),
    (0x52, 15),
    (0x59, 16),
    (0x54, 17),
    (0x31, 18),
    (0x32, 19),
    (0x33, 20),
    (0x34, 21),
    (0x36, 22),
    (0x35, 23),
    (0xbb, 24),
    (0x39, 25),
    (0x37, 26),
    (0xbd, 27),
    (0x38, 28),
    (0x30, 29),
    (0xdd, 30),
    (0x4f, 31),
    (0x55, 32),
    (0xdb, 33),
    (0x49, 34),
    (0x50, 35),
    (0x0d, 36),
    (0x4c, 37),
    (0x4a, 38),
    (0xde, 39),
    (0x4b, 40),
    (0xba, 41),
    (0xdc, 42),
    (0xbc, 43),
    (0xbf, 44),
    (0x4e, 45),
    (0x4d, 46),
    (0xbe, 47),
    (0x09, 48),
    (0x20, 49),
    (0xc0, 50),
    (0x08, 51),
    (0x1b, 53),
    (0x5c, 54),
    (0x5b, 55),
    (0xa0, 56),
    (0xa4, 58),
    (0xa2, 59),
    (0xa1, 60),
    (0xa5, 61),
    (0xa3, 62),
    (0x70, 122),
    (0x71, 120),
    (0x72, 99),
    (0x73, 118),
    (0x74, 96),
    (0x75, 97),
    (0x76, 98),
    (0x77, 100),
    (0x78, 101),
    (0x79, 109),
    (0x7a, 103),
    (0x7b, 111),
    (0x24, 115),
    (0x23, 119),
    (0x21, 116),
    (0x22, 121),
    (0x2e, 117),
    (0x25, 123),
    (0x27, 124),
    (0x28, 125),
    (0x26, 126),
];
fn code(vk: u8) -> Option<u16> {
    let vk = match vk {
        0x10 => 0xa0,
        0x11 => 0xa2,
        0x12 => 0xa4,
        _ => vk,
    };
    KEYS.iter().find(|(v, _)| *v == vk).map(|(_, code)| *code)
}
pub(super) fn supported_vk(vk: u8) -> bool {
    code(vk).is_some()
}
fn vk(code: u16) -> u8 {
    KEYS.iter()
        .find(|(_, c)| *c == code)
        .map_or(0xff, |(vk, _)| *vk)
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceKeyState(state_id: i32, keycode: u16) -> bool;
    fn AXIsProcessTrusted() -> bool;
    fn CGRectMakeWithDictionaryRepresentation(
        dictionary: CFDictionaryRef,
        rect: *mut CGRect,
    ) -> bool;
}
fn button(kind: CGEventType, event: &CGEvent) -> Option<(Button, bool)> {
    match kind {
        CGEventType::LeftMouseDown => Some((Button::Left, true)),
        CGEventType::LeftMouseUp => Some((Button::Left, false)),
        CGEventType::RightMouseDown => Some((Button::Right, true)),
        CGEventType::RightMouseUp => Some((Button::Right, false)),
        CGEventType::OtherMouseDown | CGEventType::OtherMouseUp => {
            let button = match event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER)
            {
                2 => Button::Middle,
                3 => Button::X1,
                4 => Button::X2,
                _ => return None,
            };
            Some((button, matches!(kind, CGEventType::OtherMouseDown)))
        }
        _ => None,
    }
}
fn fullscreen_at(point: Point) -> bool {
    use core_graphics::window::*;
    let Some(windows) = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        0,
    ) else {
        return false;
    };
    for index in 0..windows.len() {
        unsafe {
            let dict =
                CFArrayGetValueAtIndex(windows.as_concrete_TypeRef(), index) as CFDictionaryRef;
            let layer = CFDictionaryGetValue(dict, kCGWindowLayer.cast());
            let pid = CFDictionaryGetValue(dict, kCGWindowOwnerPID.cast());
            if layer.is_null() || pid.is_null() {
                continue;
            }
            let mut layer_number = 0i64;
            let mut owner = 0i64;
            if !CFNumberGetValue(
                layer as CFNumberRef,
                kCFNumberSInt64Type,
                (&mut layer_number as *mut i64).cast(),
            ) || !CFNumberGetValue(
                pid as CFNumberRef,
                kCFNumberSInt64Type,
                (&mut owner as *mut i64).cast(),
            ) {
                continue;
            }
            if layer_number != 0 || owner == std::process::id() as i64 {
                continue;
            }
            let bounds = CFDictionaryGetValue(dict, kCGWindowBounds.cast());
            if bounds.is_null() {
                continue;
            }
            let mut rect = CGRect::new(
                &CGPoint::new(0.0, 0.0),
                &core_graphics::geometry::CGSize::new(0.0, 0.0),
            );
            if !CGRectMakeWithDictionaryRepresentation(bounds as CFDictionaryRef, &mut rect) {
                continue;
            }
            if !(point.x as f64 >= rect.origin.x
                && point.y as f64 >= rect.origin.y
                && (point.x as f64) < rect.origin.x + rect.size.width
                && (point.y as f64) < rect.origin.y + rect.size.height)
            {
                continue;
            }
            return CGDisplay::active_displays()
                .unwrap_or_default()
                .into_iter()
                .any(|id| {
                    let display = CGDisplay::new(id).bounds();
                    (rect.origin.x - display.origin.x).abs() < 2.0
                        && (rect.origin.y - display.origin.y).abs() < 2.0
                        && (rect.size.width - display.size.width).abs() < 2.0
                        && (rect.size.height - display.size.height).abs() < 2.0
                });
        }
    }
    false
}
pub(super) fn run(shared: Arc<Shared>) -> Result<(), String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err("Grant Pingloom Accessibility and Input Monitoring in System Settings, then quit and reopen the app.".into());
    }
    let settings = shared.state.lock().unwrap().engine.settings().clone();
    super::validate(&settings).map_err(|e| e.message)?;
    let events = vec![
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::FlagsChanged,
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDragged,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGEventType::RightMouseDown,
        CGEventType::RightMouseUp,
        CGEventType::OtherMouseDown,
        CGEventType::OtherMouseUp,
    ];
    let callback = Arc::clone(&shared);
    CGEventTap::with_enabled(CGEventTapLocation::HID,CGEventTapPlacement::HeadInsertEventTap,CGEventTapOptions::Default,events,move |_proxy,kind,event|{
        if matches!(kind,CGEventType::TapDisabledByTimeout|CGEventType::TapDisabledByUserInput){callback.fail("macOS disabled the input event tap. Gestures paused; restart Pingloom after checking Accessibility/Input Monitoring.".into());return CallbackResult::Keep;}
        if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)==MARKER || event.get_integer_value_field(EventField::EVENT_SOURCE_UNIX_PROCESS_ID)>0{return CallbackResult::Keep;}
        let location=event.location();let point=Point::new(location.x.round() as i32,location.y.round() as i32);let time_ms=callback.time_ms();
        let native=match kind{
            CGEventType::KeyDown|CGEventType::KeyUp=>Some(if matches!(kind,CGEventType::KeyDown){InputEvent::KeyDown{vk:vk(event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16),injected:false}}else{InputEvent::KeyUp{vk:vk(event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16),injected:false}}),
            CGEventType::FlagsChanged=>{
                let code=event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                if code==57{return CallbackResult::Keep;}
                Some(if unsafe{CGEventSourceKeyState(CGEventSourceStateID::HIDSystemState as i32,code)}{InputEvent::KeyDown{vk:vk(code),injected:false}}else{InputEvent::KeyUp{vk:vk(code),injected:false}})
            }
            CGEventType::MouseMoved|CGEventType::LeftMouseDragged|CGEventType::RightMouseDragged|CGEventType::OtherMouseDragged=>Some(InputEvent::PointerMove{point,time_ms,injected:false}),
            _=>button(kind,event).map(|(button,down)|if down{InputEvent::PointerDown{button,point,time_ms,injected:false,fullscreen_app:fullscreen_at(point),dpi_scale:callback.scale_at(point)}}else{InputEvent::PointerUp{button,point,time_ms,injected:false}}),
        };
        if native.is_some_and(|native|callback.input(native)){CallbackResult::Drop}else{CallbackResult::Keep}
    },||{
        shared.ready();
        while shared.running.load(Ordering::Acquire){CFRunLoop::run_in_mode(unsafe{kCFRunLoopDefaultMode},Duration::from_millis(100),true);}
    }).map_err(|_|"Allow Pingloom in System Settings → Privacy & Security → Accessibility and Input Monitoring, then quit and reopen the app. Native gestures are paused until permissions are available.".into())
}
pub(super) fn replay(input: ReplayInput) -> Result<(), ()> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(());
    }
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)?;
    let event = match input {
        ReplayInput::KeyDown(vk) => CGEvent::new_keyboard_event(source, code(vk).ok_or(())?, true)?,
        ReplayInput::KeyUp(vk) => CGEvent::new_keyboard_event(source, code(vk).ok_or(())?, false)?,
        ReplayInput::ButtonDown(button) | ReplayInput::ButtonUp(button) => {
            let location = CGEvent::new(source.clone())?.location();
            let down = matches!(input, ReplayInput::ButtonDown(_));
            let (kind, mouse, number) = match button {
                Button::Left => (
                    if down {
                        CGEventType::LeftMouseDown
                    } else {
                        CGEventType::LeftMouseUp
                    },
                    CGMouseButton::Left,
                    0,
                ),
                Button::Right => (
                    if down {
                        CGEventType::RightMouseDown
                    } else {
                        CGEventType::RightMouseUp
                    },
                    CGMouseButton::Right,
                    1,
                ),
                Button::Middle | Button::X1 | Button::X2 => (
                    if down {
                        CGEventType::OtherMouseDown
                    } else {
                        CGEventType::OtherMouseUp
                    },
                    CGMouseButton::Center,
                    match button {
                        Button::Middle => 2,
                        Button::X1 => 3,
                        _ => 4,
                    },
                ),
            };
            let event = CGEvent::new_mouse_event(source, kind, location, mouse)?;
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, number);
            event
        }
    };
    event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, MARKER);
    event.post(CGEventTapLocation::HID);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_keys_round_trip() {
        for &(vk, native) in KEYS {
            assert_eq!(super::vk(native), vk);
            assert_eq!(code(vk), Some(native));
        }
        assert_eq!(code(0x12), Some(58));
        assert!(!supported_vk(0x14));
    }
}
