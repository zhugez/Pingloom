#[cfg(any(windows, test))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(feature = "desktop", test))]
use pingloom_core::gesture::ReplayInput;
use pingloom_core::model::{Button, Trigger};
use pingloom_core::settings::{Settings, ToggleMods};

#[cfg(any(feature = "desktop", test))]
const MAX_REPLAY_INSERT_ATTEMPTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl UiRect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsIssue {
    TriggerConflict,
    ToggleModifiers,
    ToggleKeyConflict,
    DragThreshold,
    PingSize,
    PingDuration,
    Volume,
    EmoteSize,
    DuplicatePingSlot(usize),
    DuplicateEmoteSlot(usize),
}

pub fn scale_dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi.max(1)) + 48) / 96) as i32
}

pub fn fit_window_to_work_area(work: UiRect, width: i32, height: i32, margin: i32) -> UiRect {
    let margin = margin
        .max(0)
        .min(work.width().max(0) / 2)
        .min(work.height().max(0) / 2);
    let width = width.max(1).min((work.width() - margin * 2).max(1));
    let height = height.max(1).min((work.height() - margin * 2).max(1));
    let left = work.left + (work.width() - width) / 2;
    let top = work.top + (work.height() - height) / 2;
    UiRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

pub fn constrain_rect_to_work_area(work: UiRect, proposed: UiRect) -> UiRect {
    let width = proposed.width().max(1).min(work.width().max(1));
    let height = proposed.height().max(1).min(work.height().max(1));
    let left = proposed.left.clamp(work.left, work.right - width);
    let top = proposed.top.clamp(work.top, work.bottom - height);
    UiRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

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

pub fn wheel_boundary_angle_degrees(boundary: usize) -> f64 {
    boundary as f64 * 45.0 - 112.5
}

pub fn wheel_wedge_outline(slot: usize, inner: f64, outer: f64) -> Option<Vec<(f64, f64)>> {
    if slot >= 8 || !inner.is_finite() || !outer.is_finite() || inner < 0.0 || outer <= inner {
        return None;
    }
    let start = wheel_boundary_angle_degrees(slot).to_radians();
    let end = wheel_boundary_angle_degrees(slot + 1).to_radians();
    let mut points = Vec::with_capacity(26);
    for step in 0..=12 {
        let angle = start + (end - start) * step as f64 / 12.0;
        points.push((angle.cos() * outer, angle.sin() * outer));
    }
    for step in (0..=12).rev() {
        let angle = start + (end - start) * step as f64 / 12.0;
        points.push((angle.cos() * inner, angle.sin() * inner));
    }
    Some(points)
}

/// Maps settings validation failures to their editable fields without interpreting error text.
pub fn reveal_scroll_delta(top: i32, bottom: i32, viewport_height: i32) -> i32 {
    if top < 0 {
        top
    } else if bottom > viewport_height {
        bottom - viewport_height
    } else {
        0
    }
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

/// A dark radial wheel with a restrained rose selection, independent of hit testing.
pub fn wheel_surface(radial: f64, selected: f64) -> [u8; 4] {
    let r = radial.clamp(0.0, 1.0);
    let b = selected.clamp(0.0, 1.0) * 0.55;
    let base = [42.0 + 32.0 * r, 45.0 + 33.0 * r, 49.0 + 35.0 * r];
    let hot = [226.0, 180.0, 189.0];
    [
        (base[0] + (hot[0] - base[0]) * b) as u8,
        (base[1] + (hot[1] - base[1]) * b) as u8,
        (base[2] + (hot[2] - base[2]) * b) as u8,
        235,
    ]
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

pub fn composite_rgba_over_rgb(pixel: [u8; 4], background: [u8; 3]) -> [u8; 3] {
    let alpha = u32::from(pixel[3]);
    let inverse = 255 - alpha;
    std::array::from_fn(|channel| {
        ((u32::from(pixel[channel]) * alpha + u32::from(background[channel]) * inverse) / 255) as u8
    })
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

pub fn settings_issue(settings: &Settings) -> Option<SettingsIssue> {
    if settings
        .emote_trigger
        .is_some_and(|emote| same_trigger(settings.trigger, emote))
    {
        return Some(SettingsIssue::TriggerConflict);
    }

    let mods = settings.toggle_hotkey.mods.0;
    if mods == 0
        || mods & !0x0f != 0
        || mods & (ToggleMods::CTRL.0 | ToggleMods::ALT.0 | ToggleMods::WIN.0) == 0
    {
        return Some(SettingsIssue::ToggleModifiers);
    }
    if trigger_uses_vk(settings.trigger, settings.toggle_hotkey.vk)
        || settings
            .emote_trigger
            .is_some_and(|trigger| trigger_uses_vk(trigger, settings.toggle_hotkey.vk))
    {
        return Some(SettingsIssue::ToggleKeyConflict);
    }
    if !(2..=40).contains(&settings.drag_threshold_px) {
        return Some(SettingsIssue::DragThreshold);
    }
    if !(60..=220).contains(&settings.ping_size_px) {
        return Some(SettingsIssue::PingSize);
    }
    if !settings.ping_duration_s.is_finite() || !(1.0..=8.0).contains(&settings.ping_duration_s) {
        return Some(SettingsIssue::PingDuration);
    }
    if settings.volume > 100 {
        return Some(SettingsIssue::Volume);
    }
    if !(80..=300).contains(&settings.emote_size_px) {
        return Some(SettingsIssue::EmoteSize);
    }
    for (slot, value) in settings.wheel.iter().enumerate() {
        if settings.wheel[..slot].contains(value) {
            return Some(SettingsIssue::DuplicatePingSlot(slot));
        }
    }
    for (slot, value) in settings.emote_wheel.iter().enumerate() {
        if settings.emote_wheel[..slot].contains(value) {
            return Some(SettingsIssue::DuplicateEmoteSlot(slot));
        }
    }
    None
}

fn same_trigger(left: Trigger, right: Trigger) -> bool {
    trigger_vk(left).is_some() && trigger_vk(left) == trigger_vk(right) || left == right
}

fn trigger_uses_vk(trigger: Trigger, vk: u8) -> bool {
    trigger_vk(trigger) == Some(vk)
}

fn trigger_vk(trigger: Trigger) -> Option<u8> {
    match trigger {
        Trigger::CapsLock => Some(0x14),
        Trigger::Key { vk } => Some(vk),
        _ => None,
    }
}

#[cfg(any(windows, test))]
pub(crate) struct CallbackGate(AtomicBool);

#[cfg(any(windows, test))]
pub(crate) struct CallbackPermit<'a>(&'a AtomicBool);

#[cfg(any(windows, test))]
impl CallbackGate {
    pub(crate) const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(crate) fn enter(&self) -> Option<CallbackPermit<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| CallbackPermit(&self.0))
    }

    pub(crate) fn is_held(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(any(windows, test))]
impl Drop for CallbackPermit<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
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

#[cfg(any(windows, test))]
pub(crate) fn take_deferred<T>(pending: &mut Vec<T>) -> Vec<T> {
    std::mem::take(pending)
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
    fn deferred_messages_are_taken_once_in_fifo_order() {
        let mut pending = vec!["settings", "resume", "inject"];
        assert_eq!(
            take_deferred(&mut pending),
            ["settings", "resume", "inject"]
        );
        assert!(pending.is_empty());
        assert!(take_deferred(&mut pending).is_empty());
    }

    #[test]
    fn callback_gate_blocks_reentrant_mutation_until_the_outer_call_returns() {
        let gate = CallbackGate::new();
        let outer = gate.enter().unwrap();
        assert!(gate.is_held());
        assert!(gate.enter().is_none());
        drop(outer);
        assert!(!gate.is_held());
        assert!(gate.enter().is_some());
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
    fn dpi_layout_scales_and_centers_within_negative_monitor_work_areas() {
        let work = UiRect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        let rect = fit_window_to_work_area(work, scale_dip(1000, 144), scale_dip(800, 144), 24);
        assert_eq!(rect.width(), 1500);
        assert_eq!(rect.height(), 992);
        assert!(rect.left >= work.left + 24);
        assert!(rect.right <= work.right - 24);
        assert!(rect.top >= work.top + 24);
        assert!(rect.bottom <= work.bottom - 24);
        assert_eq!(scale_dip(10, 144), 15);
    }

    #[test]
    fn wheel_preview_and_overlay_share_slice_boundaries() {
        assert_eq!(wheel_boundary_angle_degrees(0), -112.5);
        assert_eq!(wheel_boundary_angle_degrees(1), -67.5);
        assert_eq!(wheel_boundary_angle_degrees(8), 247.5);
    }

    #[test]
    fn focused_controls_scroll_only_when_they_leave_the_viewport() {
        assert_eq!(reveal_scroll_delta(12, 40, 100), 0);
        assert_eq!(reveal_scroll_delta(-8, 20, 100), -8);
        assert_eq!(reveal_scroll_delta(88, 118, 100), 18);
    }

    #[test]
    fn preview_rgba_is_composited_over_native_background() {
        assert_eq!(
            composite_rgba_over_rgb([240, 120, 20, 0], [10, 20, 30]),
            [10, 20, 30]
        );
        assert_eq!(
            composite_rgba_over_rgb([200, 100, 50, 128], [10, 20, 30]),
            [105, 60, 40]
        );
        assert_eq!(
            composite_rgba_over_rgb([200, 100, 50, 255], [10, 20, 30]),
            [200, 100, 50]
        );
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
        assert!(wheel_surface(1.0, 0.0)[0] > wheel_surface(0.0, 0.0)[0]);
        assert!(wheel_surface(0.5, 1.0)[0] > wheel_surface(0.5, 0.0)[0]);
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
    fn wheel_highlight_outline_tracks_the_north_clockwise_slice_geometry() {
        let north = wheel_wedge_outline(0, 86.0, 220.0).unwrap();
        assert_eq!(north.len(), 26);
        let top = north
            .iter()
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .unwrap();
        assert!(top.1 < -219.0);
        assert_eq!(
            pingloom_core::geometry::pick_slice(0.0, -220.0, 0.0),
            Some(0)
        );
        assert_eq!(
            pingloom_core::geometry::pick_slice(220.0, 0.0, 0.0),
            Some(2)
        );
        assert!(wheel_wedge_outline(8, 86.0, 220.0).is_none());
        assert!(wheel_wedge_outline(0, 220.0, 86.0).is_none());
    }

    #[test]
    fn dpi_suggested_rect_is_clamped_to_work_area_without_losing_access() {
        let work = UiRect {
            left: 0,
            top: 40,
            right: 800,
            bottom: 600,
        };
        let proposed = UiRect {
            left: 550,
            top: 500,
            right: 1100,
            bottom: 900,
        };
        assert_eq!(
            constrain_rect_to_work_area(work, proposed),
            UiRect {
                left: 250,
                top: 200,
                right: 800,
                bottom: 600
            }
        );
    }

    #[test]
    fn settings_issues_point_at_trigger_shortcut_range_and_duplicate_fields() {
        let mut settings = Settings {
            emote_trigger: Some(Trigger::Alt),
            ..Settings::default()
        };
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::TriggerConflict)
        );

        settings = Settings::default();
        settings.toggle_hotkey.mods = ToggleMods(0);
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::ToggleModifiers)
        );

        settings = Settings {
            trigger: Trigger::Key { vk: b'A' },
            ..Settings::default()
        };
        settings.toggle_hotkey.vk = b'A';
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::ToggleKeyConflict)
        );

        settings = Settings::default();
        settings.drag_threshold_px = 1;
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::DragThreshold)
        );

        settings = Settings::default();
        settings.wheel[3] = settings.wheel[1];
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::DuplicatePingSlot(3))
        );

        settings = Settings::default();
        settings.emote_wheel[4] = settings.emote_wheel[0].clone();
        assert_eq!(
            settings_issue(&settings),
            Some(SettingsIssue::DuplicateEmoteSlot(4))
        );
        assert_eq!(settings_issue(&Settings::default()), None);
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
