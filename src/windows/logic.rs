use pingloom_core::model::{Button, Trigger};
use pingloom_core::settings::Settings;

pub fn trigger_starts(
    trigger: Trigger,
    button: Button,
    mut key_down: impl FnMut(u8) -> bool,
) -> bool {
    match trigger {
        Trigger::Mouse4 => button == Button::X1,
        Trigger::Mouse5 => button == Button::X2,
        Trigger::Alt => button == Button::Left && any_key(&mut key_down, &[0x12, 0xa4, 0xa5]),
        Trigger::Ctrl => button == Button::Left && any_key(&mut key_down, &[0x11, 0xa2, 0xa3]),
        Trigger::Shift => button == Button::Left && any_key(&mut key_down, &[0x10, 0xa0, 0xa1]),
        Trigger::Win => button == Button::Left && any_key(&mut key_down, &[0x5b, 0x5c]),
        Trigger::CapsLock => button == Button::Left && key_down(0x14),
        Trigger::Key { vk } => button == Button::Left && key_down(vk),
    }
}

pub fn gesture_candidate(
    settings: &Settings,
    button: Button,
    key_down: impl FnMut(u8) -> bool + Copy,
) -> bool {
    trigger_starts(settings.trigger, button, key_down)
        || settings
            .emote_trigger
            .is_some_and(|trigger| trigger_starts(trigger, button, key_down))
}

pub fn swallowed_trigger_key(settings: &Settings, vk: u8) -> bool {
    matches!(settings.trigger, Trigger::CapsLock | Trigger::Key { .. })
        && trigger_vk(settings.trigger) == Some(vk)
        || settings.emote_trigger.is_some_and(|trigger| {
            matches!(trigger, Trigger::CapsLock | Trigger::Key { .. })
                && trigger_vk(trigger) == Some(vk)
        })
}

fn trigger_vk(trigger: Trigger) -> Option<u8> {
    match trigger {
        Trigger::CapsLock => Some(0x14),
        Trigger::Key { vk } => Some(vk),
        _ => None,
    }
}

fn any_key(key_down: &mut impl FnMut(u8) -> bool, keys: &[u8]) -> bool {
    keys.iter().copied().any(key_down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_probe_candidates_follow_keyboard_and_mouse_triggers() {
        let mut settings = Settings::default();
        let mut keys = [false; 256];

        assert!(!gesture_candidate(&settings, Button::Left, |vk| keys[usize::from(vk)]));
        keys[usize::from(0xa4u8)] = true;
        assert!(gesture_candidate(&settings, Button::Left, |vk| keys[usize::from(vk)]));
        assert!(!gesture_candidate(&settings, Button::Right, |vk| keys[usize::from(vk)]));

        settings.trigger = Trigger::Mouse4;
        settings.emote_trigger = None;
        assert!(gesture_candidate(&settings, Button::X1, |_| false));
        assert!(!gesture_candidate(&settings, Button::Left, |_| true));

        settings.trigger = Trigger::Key { vk: b'Q' };
        assert!(gesture_candidate(&settings, Button::Left, |vk| vk == b'Q'));
        assert!(swallowed_trigger_key(&settings, b'Q'));
        assert!(!swallowed_trigger_key(&settings, b'W'));
    }
}
