#[cfg(any(windows, test))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(windows, test))]
use lolping_core::gesture::ReplayInput;
use lolping_core::model::{Button, Trigger};
use lolping_core::settings::Settings;

#[cfg(any(windows, test))]
const MAX_REPLAY_INSERT_ATTEMPTS: usize = 3;

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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
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

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Eq, PartialEq)]
enum PressedControl {
    Button(Button),
    Key(u8),
}

#[cfg(any(windows, test))]
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
