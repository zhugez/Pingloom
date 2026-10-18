#[cfg(any(feature = "desktop", test))]
use pingloom_core::gesture::ReplayInput;
use pingloom_core::model::{Button, Trigger};
use pingloom_core::settings::Settings;

#[cfg(any(feature = "desktop", test))]
const MAX_REPLAY_INSERT_ATTEMPTS: usize = 3;

/// FlagsChanged arrives before CGEventSourceKeyState reflects that change.
/// Use the event's per-side NX_DEVICE*KEYMASK bits (IOKit/hidsystem/IOLLEvent.h),
/// not the previous HID state or the aggregate flag shared by both modifier keys.
pub fn macos_modifier_down(keycode: u16, flags: u64) -> Option<bool> {
    let mask = match keycode {
        59 => 0x0001, // left Control
        56 => 0x0002, // left Shift
        60 => 0x0004, // right Shift
        55 => 0x0008, // left Command
        54 => 0x0010, // right Command
        58 => 0x0020, // left Option
        61 => 0x0040, // right Option
        62 => 0x2000, // right Control
        _ => return None,
    };
    Some(flags & mask != 0)
}

/// A small, continuous overshoot that settles at exactly the configured size.
pub fn effect_pop(age: f64, enter_seconds: f64) -> f64 {
    let t = (age / enter_seconds).clamp(0.0, 1.0) - 1.0;
    1.0 + 0.6 * (2.0 * t.powi(3) + t.powi(2))
}

/// Bundled WebP already contains its entrance, movement and exit (lolPing's sheet).
/// Only a still fallback needs an external envelope; reduced motion uses that still.
pub fn emote_envelope(age: f64, baked: bool, reduced: bool, pending: bool) -> (f64, f64) {
    if pending && !reduced {
        // Static PNG thumbnails can use a different canvas/crop from WebP frames.
        // Never display them before decode readiness: that creates a false zoom.
        (1.0, 0.0)
    } else if baked && !reduced {
        (1.0, 1.0)
    } else {
        (
            if reduced { 1.0 } else { effect_pop(age, 0.25) },
            effect_fade(age, 3.0),
        )
    }
}

/// Normalized art offsets, scale and entrance alpha, shared by both native renderers.
/// Two-part pings converge diagonally; ordinary pings pop, then gently bob.
pub fn ping_art_motion(
    age: f64,
    layer: usize,
    layers: usize,
    reduced: bool,
) -> (f64, f64, f64, f64) {
    if reduced {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let bob_age = (age - if layers == 2 { 0.5 } else { 0.3 }).max(0.0);
    let bob = -0.0225 * (1.0 - (bob_age * std::f64::consts::TAU / 1.6).cos());
    if layers == 2 {
        let t = (age / 0.22).clamp(0.0, 1.0);
        let slide = (1.0 - t * t) * 0.64;
        (
            if layer == 0 { slide } else { -slide },
            -slide + bob,
            1.0,
            t,
        )
    } else {
        (
            0.0,
            bob,
            effect_pop(age, 0.28),
            (age / 0.08).clamp(0.0, 1.0),
        )
    }
}

/// One-pixel analytical edge coverage, avoiding repeated alpha stamping around rings.
pub fn ring_coverage(distance: f64, radius: f64, thickness: f64) -> f64 {
    (thickness * 0.5 + 0.5 - (distance - radius).abs()).clamp(0.0, 1.0)
}

pub fn effect_fade(age: f64, lifetime: f64) -> f64 {
    ((lifetime - age) / (lifetime * 0.13).min(0.35)).clamp(0.0, 1.0)
}

pub fn selection_blend(current: f64, selected: bool, elapsed: f64) -> f64 {
    let target = if selected { 1.0 } else { 0.0 };
    target + (current - target) * (-elapsed.max(0.0) / 0.035).exp()
}

pub fn caption_mask_usable_after_flush(usable: bool, flushed: bool) -> bool {
    usable && flushed
}

/// Blends straight-alpha RGBA into the premultiplied BGRA format used by layered-window DIBs.
pub fn composite_premultiplied_bgra(destination: [u8; 4], rgba: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(rgba[3]);
    let inverse = 255 - alpha;
    [
        ((u32::from(rgba[2]) * alpha + u32::from(destination[0]) * inverse) / 255) as u8,
        ((u32::from(rgba[1]) * alpha + u32::from(destination[1]) * inverse) / 255) as u8,
        ((u32::from(rgba[0]) * alpha + u32::from(destination[2]) * inverse) / 255) as u8,
        (alpha + u32::from(destination[3]) * inverse / 255).min(255) as u8,
    ]
}

#[cfg(any(feature = "desktop", test))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReplayInsertReport {
    pub(crate) inserted: usize,
    pub(crate) attempts: usize,
    pub(crate) complete: bool,
    pub(crate) balance_attempted: Vec<ReplayInput>,
    pub(crate) balance_inserted: usize,
    pub(crate) balance_attempts: usize,
    pub(crate) balance_complete: bool,
}

#[cfg(any(feature = "desktop", test))]
pub(crate) fn insert_replay_batch(
    replay: &[ReplayInput],
    mut insert: impl FnMut(&[ReplayInput]) -> usize,
) -> ReplayInsertReport {
    let (inserted, attempts) = insert_prefix(replay, &mut insert);
    let complete = inserted == replay.len();
    let balance_attempted = if complete {
        Vec::new()
    } else {
        unbalanced_releases(&replay[..inserted])
    };
    let (balance_inserted, balance_attempts) = insert_prefix(&balance_attempted, &mut insert);
    ReplayInsertReport {
        inserted,
        attempts,
        complete,
        balance_complete: balance_inserted == balance_attempted.len(),
        balance_attempted,
        balance_inserted,
        balance_attempts,
    }
}

#[cfg(any(feature = "desktop", test))]
fn insert_prefix(
    inputs: &[ReplayInput],
    insert: &mut impl FnMut(&[ReplayInput]) -> usize,
) -> (usize, usize) {
    let mut inserted = 0;
    let mut attempts = 0;
    while inserted < inputs.len() && attempts < MAX_REPLAY_INSERT_ATTEMPTS {
        attempts += 1;
        let remaining = &inputs[inserted..];
        let count = insert(remaining).min(remaining.len());
        inserted += count;
    }
    (inserted, attempts)
}

#[cfg(any(feature = "desktop", test))]
#[derive(Clone, Copy, Eq, PartialEq)]
enum PressedControl {
    Button(Button),
    Key(u8),
}

#[cfg(any(feature = "desktop", test))]
fn unbalanced_releases(accepted: &[ReplayInput]) -> Vec<ReplayInput> {
    let mut pressed = Vec::new();
    for event in accepted {
        let (control, down) = match event {
            ReplayInput::ButtonDown(button) => (PressedControl::Button(*button), true),
            ReplayInput::ButtonUp(button) => (PressedControl::Button(*button), false),
            ReplayInput::KeyDown(vk) => (PressedControl::Key(*vk), true),
            ReplayInput::KeyUp(vk) => (PressedControl::Key(*vk), false),
        };
        if down {
            if !pressed.contains(&control) {
                pressed.push(control);
            }
        } else {
            pressed.retain(|held| *held != control);
        }
    }
    pressed
        .into_iter()
        .map(|control| match control {
            PressedControl::Button(button) => ReplayInput::ButtonUp(button),
            PressedControl::Key(vk) => ReplayInput::KeyUp(vk),
        })
        .collect()
}

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

fn trigger_vk(trigger: Trigger) -> Option<u8> {
    match trigger {
        Trigger::CapsLock => Some(0x14),
        Trigger::Key { vk } => Some(vk),
        _ => None,
    }
}

pub fn swallowed_trigger_key(settings: &Settings, vk: u8) -> bool {
    matches!(settings.trigger, Trigger::CapsLock | Trigger::Key { .. })
        && trigger_vk(settings.trigger) == Some(vk)
        || settings.emote_trigger.is_some_and(|trigger| {
            matches!(trigger, Trigger::CapsLock | Trigger::Key { .. })
                && trigger_vk(trigger) == Some(vk)
        })
}

fn any_key(key_down: &mut impl FnMut(u8) -> bool, keys: &[u8]) -> bool {
    keys.iter().copied().any(key_down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_modifiers_use_event_flags_not_the_previous_hid_state() {
        // Physical Mac trace: Option press has flags=true/HID=false; release is reversed.
        for (flags, old_hid, expected) in [(0x80020, false, true), (0, true, false)] {
            assert_eq!(macos_modifier_down(58, flags), Some(expected));
            assert_ne!(expected, old_hid);
        }
        for (left, right, lmask, rmask, aggregate) in [
            (58, 61, 0x20, 0x40, 0x80000),
            (56, 60, 0x2, 0x4, 0x20000),
            (59, 62, 0x1, 0x2000, 0x40000),
            (55, 54, 0x8, 0x10, 0x100000),
        ] {
            assert_eq!(
                macos_modifier_down(left, aggregate | lmask | rmask),
                Some(true)
            );
            assert_eq!(
                macos_modifier_down(right, aggregate | lmask | rmask),
                Some(true)
            );
            // Releasing one side must not keep it down just because the sibling is held.
            assert_eq!(macos_modifier_down(left, aggregate | rmask), Some(false));
            assert_eq!(macos_modifier_down(right, aggregate | rmask), Some(true));
            assert_eq!(macos_modifier_down(right, 0), Some(false));
        }
        assert_eq!(macos_modifier_down(57, 0x10000), None); // Caps Lock is not a macOS trigger.
        assert_eq!(macos_modifier_down(0, u64::MAX), None);
    }

    #[test]
    fn macos_option_physical_trace_opens_selects_and_balances_the_trigger() {
        use pingloom_core::{Action, GestureEngine, InputEvent, Point, WheelKind};
        let mut engine = GestureEngine::new(Settings::default()).unwrap();
        let modifier = |flags| {
            if macos_modifier_down(58, flags).unwrap() {
                InputEvent::KeyDown {
                    vk: 0xa4,
                    injected: false,
                }
            } else {
                InputEvent::KeyUp {
                    vk: 0xa4,
                    injected: false,
                }
            }
        };
        engine.on_event(modifier(0x80020));
        let down = engine.on_event(InputEvent::PointerDown {
            button: Button::Left,
            point: Point::new(100, 100),
            time_ms: 10,
            injected: false,
            fullscreen_app: false,
            dpi_scale: 1.0,
        });
        assert!(down.swallow);
        let drag = engine.on_event(InputEvent::PointerMove {
            point: Point::new(300, 100),
            time_ms: 30,
            injected: false,
        });
        assert!(drag.actions.iter().any(|a| matches!(
            a,
            Action::WheelOpen {
                wheel: WheelKind::Ping,
                ..
            }
        )));
        let up = engine.on_event(InputEvent::PointerUp {
            button: Button::Left,
            point: Point::new(300, 100),
            time_ms: 40,
            injected: false,
        });
        assert!(up.actions.iter().any(|a| matches!(
            a,
            Action::Select {
                wheel: WheelKind::Ping,
                slot: 2,
                ..
            }
        )));
        engine.on_event(modifier(0));
        let ordinary = engine.on_event(InputEvent::PointerDown {
            button: Button::Left,
            point: Point::new(100, 100),
            time_ms: 60,
            injected: false,
            fullscreen_app: false,
            dpi_scale: 1.0,
        });
        assert!(!ordinary.swallow);
    }

    #[test]
    fn replay_retries_only_the_uninserted_suffix() {
        let replay = [ReplayInput::KeyDown(b'Q'), ReplayInput::KeyUp(b'Q')];
        let mut calls = Vec::new();
        let mut results = [1, 1].into_iter();
        let report = insert_replay_batch(&replay, |suffix| {
            calls.push(suffix.to_vec());
            results.next().unwrap_or(0)
        });

        assert_eq!(calls, [replay.to_vec(), vec![ReplayInput::KeyUp(b'Q')]]);
        assert_eq!(report.inserted, 2);
        assert_eq!(report.attempts, 2);
        assert!(report.complete);
        assert!(report.balance_complete);
        assert!(report.balance_attempted.is_empty());
    }

    #[test]
    fn zero_and_partial_replay_failures_are_bounded_and_balance_accepted_downs() {
        let zero = insert_replay_batch(&[ReplayInput::ButtonDown(Button::Left)], |_| 0);
        assert_eq!(zero.inserted, 0);
        assert_eq!(zero.attempts, MAX_REPLAY_INSERT_ATTEMPTS);
        assert!(!zero.complete);
        assert!(zero.balance_attempted.is_empty());

        let replay = [
            ReplayInput::ButtonDown(Button::Left),
            ReplayInput::ButtonDown(Button::Right),
            ReplayInput::ButtonUp(Button::Right),
        ];
        let mut calls = Vec::new();
        let mut results = [1, 0, 0, 1].into_iter();
        let report = insert_replay_batch(&replay, |suffix| {
            calls.push(suffix.to_vec());
            results.next().unwrap_or(0)
        });

        assert_eq!(report.inserted, 1);
        assert_eq!(report.attempts, MAX_REPLAY_INSERT_ATTEMPTS);
        assert!(!report.complete);
        assert_eq!(
            report.balance_attempted,
            [ReplayInput::ButtonUp(Button::Left)]
        );
        assert_eq!(report.balance_inserted, 1);
        assert_eq!(report.balance_attempts, 1);
        assert!(report.balance_complete);
        assert_eq!(calls[1], replay[1..]);
        assert_eq!(calls[2], replay[1..]);
        assert_eq!(calls[3], [ReplayInput::ButtonUp(Button::Left)]);

        let mut results = [1, 0, 0, 0, 0, 0].into_iter();
        let unbalanced = insert_replay_batch(
            &[
                ReplayInput::ButtonDown(Button::Left),
                ReplayInput::ButtonUp(Button::Left),
            ],
            |_| results.next().unwrap_or(0),
        );
        assert_eq!(unbalanced.inserted, 1);
        assert_eq!(
            unbalanced.balance_attempted,
            [ReplayInput::ButtonUp(Button::Left)]
        );
        assert_eq!(unbalanced.balance_attempts, MAX_REPLAY_INSERT_ATTEMPTS);
        assert!(!unbalanced.balance_complete);
    }

    #[test]
    fn partial_progress_stops_after_bounded_attempts() {
        let replay = [
            ReplayInput::KeyDown(1),
            ReplayInput::KeyDown(2),
            ReplayInput::KeyDown(3),
            ReplayInput::KeyDown(4),
        ];
        let mut calls = 0;
        let report = insert_replay_batch(&replay, |_| {
            calls += 1;
            1
        });

        assert_eq!(calls, MAX_REPLAY_INSERT_ATTEMPTS * 2);
        assert_eq!(report.inserted, MAX_REPLAY_INSERT_ATTEMPTS);
        assert_eq!(report.attempts, MAX_REPLAY_INSERT_ATTEMPTS);
        assert!(!report.complete);
        assert_eq!(report.balance_attempted.len(), MAX_REPLAY_INSERT_ATTEMPTS);
        assert!(report.balance_complete);
    }

    #[test]
    fn motion_is_continuous_time_based_and_fades_all_the_way_out() {
        for duration in [0.25, 0.28] {
            assert!((effect_pop(0.0, duration) - 0.4).abs() < 1e-9);
            assert!((effect_pop(duration - 1e-6, duration) - 1.0).abs() < 1e-5);
            assert_eq!(effect_pop(duration, duration), 1.0);
            assert_eq!(effect_pop(duration + 1e-6, duration), 1.0);
            for step in 0..=100 {
                assert!((0.4 - 1e-9..=1.03)
                    .contains(&effect_pop(duration * step as f64 / 100.0, duration)));
            }
        }
        for duration in [1.0, 3.0, 8.0] {
            assert_eq!(effect_fade(0.0, duration), 1.0);
            assert_eq!(effect_fade(duration, duration), 0.0);
            assert_eq!(effect_fade(duration + 1.0, duration), 0.0);
        }
        let single_frame = selection_blend(0.0, true, 0.10);
        let split_frames = selection_blend(selection_blend(0.0, true, 0.04), true, 0.06);
        assert!((single_frame - split_frames).abs() < 1e-9);
        assert!(selection_blend(single_frame, false, 0.05) < single_frame);
        assert_eq!(selection_blend(0.5, true, 0.0), 0.5);
    }

    #[test]
    fn pending_animation_never_flashes_a_differently_sized_still() {
        for age in [0.0, 0.1, 0.25, 0.8, 2.5] {
            assert_eq!(
                emote_envelope(age, false, false, true),
                (1.0, 0.0),
                "PNG fallback must stay hidden until animated frames are ready"
            );
        }
        assert_eq!(
            emote_envelope(0.0, true, false, false),
            (1.0, 1.0),
            "decode-ready animation must retain its own scale/motion"
        );
        assert_eq!(
            emote_envelope(0.0, false, true, true),
            (1.0, 1.0),
            "reduced motion may immediately use its still"
        );
        assert!(
            emote_envelope(0.1, false, false, false).1 > 0.0,
            "static images and failed decode must keep their fallback"
        );
    }

    #[test]
    fn lolping_motion_preserves_baked_animation_and_shared_ping_geometry() {
        for age in [0.0, 0.1, 0.25, 1.4, 2.9] {
            assert_eq!(emote_envelope(age, true, false, false), (1.0, 1.0));
            assert_eq!(ping_art_motion(age, 0, 2, true), (0.0, 0.0, 1.0, 1.0));
            assert_eq!(emote_envelope(age, true, true, false).0, 1.0);
        }
        assert_eq!(emote_envelope(3.0, false, false, false).1, 0.0);
        let left = ping_art_motion(0.1, 0, 2, false);
        let right = ping_art_motion(0.1, 1, 2, false);
        assert_eq!(left.0, -right.0);
        assert_eq!(left.1, right.1);
        assert!(left.1 < 0.0 && left.3 > 0.0 && left.3 < 1.0);
        assert_eq!(ping_art_motion(0.22, 0, 2, false), (0.0, 0.0, 1.0, 1.0));
        let before = ping_art_motion(0.3 - 1e-6, 0, 1, false);
        let after = ping_art_motion(0.3 + 1e-6, 0, 1, false);
        assert!((before.1 - after.1).abs() < 1e-8);
        assert_eq!(ring_coverage(20.0, 20.0, 3.0), 1.0);
        assert_eq!(ring_coverage(21.5, 20.0, 3.0), 0.5);
        assert_eq!(ring_coverage(18.5, 20.0, 3.0), 0.5);
        assert_eq!(ring_coverage(22.0, 20.0, 3.0), 0.0);
    }

    #[test]
    fn failed_caption_flush_permanently_disables_mask_access() {
        let usable = caption_mask_usable_after_flush(true, false);
        assert!(!usable);
        assert!(!caption_mask_usable_after_flush(usable, true));
        assert!(caption_mask_usable_after_flush(true, true));
    }

    #[test]
    fn caption_coverage_uses_premultiplied_overlay_compositing() {
        assert_eq!(
            composite_premultiplied_bgra([0; 4], [248, 248, 248, 128]),
            [124, 124, 124, 128]
        );
        assert_eq!(
            composite_premultiplied_bgra([10, 20, 30, 128], [248, 248, 248, 0]),
            [10, 20, 30, 128]
        );
    }

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
