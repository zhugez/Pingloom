use crate::geometry::select_slot_scaled;
use crate::model::{Action, Button, InputEvent, Point, Trigger, WheelKind};
use crate::settings::{Settings, SettingsError};

pub const VK_MASK_MENU: u8 = 0xfc;
pub const GESTURE_WATCHDOG_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GestureState {
    Idle,
    Pending,
    Wheel,
    SwallowUp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayInput {
    ButtonDown(Button),
    ButtonUp(Button),
    KeyDown(u8),
    KeyUp(u8),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputDecision {
    /// True means the native hook must suppress the original event.
    pub swallow: bool,
    /// Semantic events for the overlay/settings owner to consume.
    pub actions: Vec<Action>,
    /// Ordered synthetic inputs to replay after suppressing the original event.
    pub replay: Vec<ReplayInput>,
}

impl InputDecision {
    fn action(&mut self, action: Action) {
        self.actions.push(action);
    }

    fn replay(&mut self, input: ReplayInput) {
        self.replay.push(input);
    }
}

pub struct GestureEngine {
    settings: Settings,
    enabled: bool,
    suspended: bool,
    state: GestureState,
    keys_down: [bool; 256],
    swallow_key_ups: [bool; 256],
    ignore_trigger_until_up: [bool; 256],
    swallow_right_up: bool,
    need_menu_mask: u8,
    foreign_key: bool,
    owner: WheelKind,
    drag_button: Button,
    origin: Point,
    last_pointer_ms: u64,
    selection_scale: f64,
}

impl GestureEngine {
    pub fn new(settings: Settings) -> Result<Self, SettingsError> {
        settings.validate()?;
        Ok(Self {
            enabled: settings.enabled_on_start,
            settings,
            suspended: false,
            state: GestureState::Idle,
            keys_down: [false; 256],
            swallow_key_ups: [false; 256],
            ignore_trigger_until_up: [false; 256],
            swallow_right_up: false,
            need_menu_mask: 0,
            foreign_key: false,
            owner: WheelKind::Ping,
            drag_button: Button::Left,
            origin: Point::new(0, 0),
            last_pointer_ms: 0,
            selection_scale: 1.0,
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn state(&self) -> GestureState {
        self.state
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_settings(&mut self, settings: Settings) -> Result<InputDecision, SettingsError> {
        settings.validate()?;
        let mut decision = InputDecision::default();
        self.cancel_gesture(&mut decision);
        self.settings = settings;
        Ok(decision)
    }

    /// Disabling is an explicit recovery point for swallowed keyboard triggers: their state is
    /// cleared, but repeats and the eventual key-up remain swallowed until release.
    pub fn set_enabled(&mut self, enabled: bool) -> InputDecision {
        let mut decision = InputDecision::default();
        if !enabled {
            self.cancel_gesture(&mut decision);
            self.recover_swallowed_triggers();
        }
        self.enabled = enabled;
        decision
    }

    /// Safety suspension (for shutdown) uses the same recovery as an explicit pause.
    /// Opening Settings does not use this operation.
    pub fn set_suspended(&mut self, suspended: bool) -> InputDecision {
        let mut decision = InputDecision::default();
        if suspended {
            self.cancel_gesture(&mut decision);
            self.recover_swallowed_triggers();
        }
        self.suspended = suspended;
        decision
    }

    pub fn on_event(&mut self, event: InputEvent) -> InputDecision {
        let mut decision = InputDecision::default();
        match event {
            InputEvent::KeyDown { vk, injected } => {
                if !injected {
                    self.on_key_down(vk, &mut decision);
                }
            }
            InputEvent::KeyUp { vk, injected } => {
                if !injected {
                    self.on_key_up(vk, &mut decision);
                }
            }
            InputEvent::PointerDown {
                button,
                point,
                time_ms,
                injected,
                fullscreen_app,
                dpi_scale,
            } => {
                if !injected {
                    self.last_pointer_ms = time_ms;
                    self.on_pointer_down(
                        button,
                        point,
                        time_ms,
                        fullscreen_app,
                        dpi_scale,
                        &mut decision,
                    );
                }
            }
            InputEvent::PointerUp {
                button,
                point,
                time_ms,
                injected,
            } => {
                if !injected {
                    self.last_pointer_ms = time_ms;
                    self.on_pointer_up(button, point, &mut decision);
                }
            }
            InputEvent::PointerMove {
                point,
                time_ms,
                injected,
            } => {
                if !injected {
                    self.last_pointer_ms = time_ms;
                    self.on_pointer_move(point, &mut decision);
                }
            }
        }
        decision
    }

    pub fn tick(&mut self, now_ms: u64) -> InputDecision {
        let mut decision = InputDecision::default();
        let busy = matches!(self.state, GestureState::Pending | GestureState::Wheel);
        if busy
            && now_ms > self.last_pointer_ms
            && now_ms - self.last_pointer_ms > GESTURE_WATCHDOG_MS
        {
            decision.action(Action::Cancel);
            self.state = GestureState::SwallowUp;
        }
        decision
    }

    /// Clears only keys whose down reached the OS and which the OS now reports as released.
    pub fn resync_key_states(&mut self, mut is_down: impl FnMut(u8) -> bool) {
        for vk in 0..=u8::MAX {
            let index = usize::from(vk);
            if self.keys_down[index] && !self.swallow_key_ups[index] && !is_down(vk) {
                self.keys_down[index] = false;
            }
        }
        if !self.trigger_held(WheelKind::Ping) && !self.trigger_held(WheelKind::Emote) {
            self.foreign_key = false;
        }
    }

    fn active(&self) -> bool {
        self.enabled && !self.suspended
    }

    fn cancel_gesture(&mut self, decision: &mut InputDecision) {
        if matches!(self.state, GestureState::Pending | GestureState::Wheel) {
            decision.action(Action::Cancel);
            self.state = GestureState::SwallowUp;
        }
    }

    fn recover_swallowed_triggers(&mut self) {
        // A pause is the explicit policy boundary: OS state cannot distinguish a lost key-up
        // from a deliberately held key whose down was swallowed.
        for vk in 0..=u8::MAX {
            let index = usize::from(vk);
            let trigger = self.key_trigger_vk(WheelKind::Ping) == Some(vk)
                || self.key_trigger_vk(WheelKind::Emote) == Some(vk);
            if trigger && self.swallow_key_ups[index] {
                self.keys_down[index] = false;
                self.ignore_trigger_until_up[index] = true;
            }
        }
        if !self.trigger_held(WheelKind::Ping) && !self.trigger_held(WheelKind::Emote) {
            self.foreign_key = false;
        }
    }

    fn modifier_bits(&self) -> u8 {
        let mut modifiers = 0;
        for vk in 0..=u8::MAX {
            if self.keys_down[usize::from(vk)] {
                modifiers |= modifier_bit(vk);
            }
        }
        modifiers
    }

    fn trigger(&self, wheel: WheelKind) -> Option<Trigger> {
        match wheel {
            WheelKind::Ping => Some(self.settings.trigger),
            WheelKind::Emote => self.settings.emote_trigger,
        }
    }

    fn trigger_held(&self, wheel: WheelKind) -> bool {
        match self.trigger(wheel) {
            Some(Trigger::Alt) => self.modifier_bits() & MOD_ALT != 0,
            Some(Trigger::Ctrl) => self.modifier_bits() & MOD_CTRL != 0,
            Some(Trigger::Shift) => self.modifier_bits() & MOD_SHIFT != 0,
            Some(Trigger::Win) => self.modifier_bits() & MOD_WIN != 0,
            Some(Trigger::CapsLock) => self.keys_down[0x14],
            Some(Trigger::Key { vk }) => self.keys_down[usize::from(vk)],
            Some(Trigger::Mouse4 | Trigger::Mouse5) | None => false,
        }
    }

    fn is_trigger_vk(&self, wheel: WheelKind, vk: u8) -> bool {
        match self.trigger(wheel) {
            Some(Trigger::Alt) => modifier_bit(vk) == MOD_ALT,
            Some(Trigger::Ctrl) => modifier_bit(vk) == MOD_CTRL,
            Some(Trigger::Shift) => modifier_bit(vk) == MOD_SHIFT,
            Some(Trigger::Win) => modifier_bit(vk) == MOD_WIN,
            Some(Trigger::CapsLock) => vk == 0x14,
            Some(Trigger::Key { vk: key }) => vk == key,
            Some(Trigger::Mouse4 | Trigger::Mouse5) | None => false,
        }
    }

    fn is_key_trigger(&self, wheel: WheelKind) -> bool {
        matches!(
            self.trigger(wheel),
            Some(Trigger::CapsLock | Trigger::Key { .. })
        )
    }

    fn key_trigger_vk(&self, wheel: WheelKind) -> Option<u8> {
        match self.trigger(wheel) {
            Some(Trigger::CapsLock) => Some(0x14),
            Some(Trigger::Key { vk }) => Some(vk),
            _ => None,
        }
    }

    fn is_mouse_trigger(&self, wheel: WheelKind) -> bool {
        matches!(self.trigger(wheel), Some(Trigger::Mouse4 | Trigger::Mouse5))
    }

    fn drag_button(&self, wheel: WheelKind) -> Button {
        match self.trigger(wheel) {
            Some(Trigger::Mouse4) => Button::X1,
            Some(Trigger::Mouse5) => Button::X2,
            _ => Button::Left,
        }
    }

    fn click_on(&self, wheel: WheelKind) -> bool {
        match wheel {
            WheelKind::Ping => self.settings.click_ping,
            WheelKind::Emote => self.settings.emote_click,
        }
    }

    fn starts_gesture(&self, wheel: WheelKind, button: Button) -> bool {
        button == self.drag_button(wheel)
            && (self.is_mouse_trigger(wheel) || self.trigger_held(wheel))
    }

    fn mark_menu_mask(&mut self) {
        self.need_menu_mask |= self.modifier_bits() & (MOD_ALT | MOD_WIN);
    }

    fn on_key_down(&mut self, vk: u8, decision: &mut InputDecision) {
        let index = usize::from(vk);
        if self.ignore_trigger_until_up[index] {
            decision.swallow = true;
            return;
        }
        let repeat = self.keys_down[index];
        self.keys_down[index] = true;

        if self.state == GestureState::Idle
            && modifier_bit(vk) == 0
            && !self.is_trigger_vk(WheelKind::Ping, vk)
            && !self.is_trigger_vk(WheelKind::Emote, vk)
            && (self.trigger_held(WheelKind::Ping) || self.trigger_held(WheelKind::Emote))
        {
            self.foreign_key = true;
        }

        let hotkey = self.settings.toggle_hotkey;
        if !self.suspended && vk == hotkey.vk && self.modifier_bits() == hotkey.mods.0 {
            decision.swallow = true;
            self.swallow_key_ups[index] = true;
            self.mark_menu_mask();
            if !repeat {
                if self.enabled {
                    self.cancel_gesture(decision);
                }
                self.enabled = !self.enabled;
                if !self.enabled {
                    self.recover_swallowed_triggers();
                }
                decision.action(Action::Toggled {
                    enabled: self.enabled,
                });
            }
            return;
        }

        if self.active() {
            if vk == 0x1b && matches!(self.state, GestureState::Pending | GestureState::Wheel) {
                decision.swallow = true;
                self.swallow_key_ups[index] = true;
                self.mark_menu_mask();
                self.cancel_gesture(decision);
                return;
            }
            let own_key =
                |wheel| self.is_key_trigger(wheel) && self.key_trigger_vk(wheel) == Some(vk);
            if own_key(WheelKind::Ping) || own_key(WheelKind::Emote) {
                decision.swallow = true;
                self.swallow_key_ups[index] = true;
                return;
            }
        }
        self.swallow_key_ups[index] = false;
    }

    fn on_key_up(&mut self, vk: u8, decision: &mut InputDecision) {
        let index = usize::from(vk);
        if self.ignore_trigger_until_up[index] {
            self.keys_down[index] = false;
            self.swallow_key_ups[index] = false;
            self.ignore_trigger_until_up[index] = false;
            decision.swallow = true;
            return;
        }
        let held_before = self.trigger_held(self.owner);
        self.keys_down[index] = false;
        if !self.trigger_held(WheelKind::Ping) && !self.trigger_held(WheelKind::Emote) {
            self.foreign_key = false;
        }
        let trigger_released =
            held_before && !self.trigger_held(self.owner) && self.is_trigger_vk(self.owner, vk);

        if self.swallow_key_ups[index] {
            decision.swallow = true;
            self.swallow_key_ups[index] = false;
        }
        let released_modifier = modifier_bit(vk);
        if self.need_menu_mask & released_modifier != 0
            && self.modifier_bits() & released_modifier == 0
        {
            self.need_menu_mask &= !released_modifier;
            decision.swallow = true;
            decision.replay.extend([
                ReplayInput::KeyDown(VK_MASK_MENU),
                ReplayInput::KeyUp(VK_MASK_MENU),
                ReplayInput::KeyUp(vk),
            ]);
        }
        if !trigger_released {
            return;
        }
        if self.state == GestureState::Pending {
            if self.click_on(self.owner) {
                decision.action(Action::Click {
                    wheel: self.owner,
                    point: self.origin,
                });
                self.state = GestureState::SwallowUp;
            } else {
                decision.replay(ReplayInput::ButtonDown(self.drag_button));
                self.state = GestureState::Idle;
            }
        } else if self.state == GestureState::Wheel {
            decision.action(Action::Cancel);
            self.state = GestureState::SwallowUp;
        }
    }

    fn on_pointer_down(
        &mut self,
        button: Button,
        point: Point,
        time_ms: u64,
        fullscreen_app: bool,
        dpi_scale: f64,
        decision: &mut InputDecision,
    ) {
        if self.state == GestureState::SwallowUp && button == self.drag_button {
            self.state = GestureState::Idle;
        }
        if button == Button::Right {
            self.swallow_right_up = false;
        }
        if !self.active() {
            return;
        }
        if matches!(self.state, GestureState::Pending | GestureState::Wheel)
            && button == Button::Right
        {
            decision.swallow = true;
            self.swallow_right_up = true;
            self.mark_menu_mask();
            self.cancel_gesture(decision);
            return;
        }
        if self.state != GestureState::Idle || self.foreign_key {
            return;
        }
        let ping = self.starts_gesture(WheelKind::Ping, button);
        let emote = self.starts_gesture(WheelKind::Emote, button);
        if ping == emote {
            return;
        }
        if (self.settings.game_check && fullscreen_app)
            || !dpi_scale.is_finite()
            || dpi_scale <= 0.0
        {
            return;
        }
        decision.swallow = true;
        self.mark_menu_mask();
        self.state = GestureState::Pending;
        self.owner = if ping {
            WheelKind::Ping
        } else {
            WheelKind::Emote
        };
        self.drag_button = button;
        self.origin = point;
        self.last_pointer_ms = time_ms;
        self.selection_scale = dpi_scale;
    }

    fn on_pointer_up(&mut self, button: Button, point: Point, decision: &mut InputDecision) {
        if button == Button::Right && self.swallow_right_up {
            self.swallow_right_up = false;
            decision.swallow = true;
            return;
        }
        if self.state == GestureState::Idle || button != self.drag_button {
            return;
        }
        decision.swallow = true;
        if self.state == GestureState::Pending {
            if self.click_on(self.owner) {
                decision.action(Action::Click {
                    wheel: self.owner,
                    point: self.origin,
                });
            } else {
                decision.replay.extend([
                    ReplayInput::ButtonDown(button),
                    ReplayInput::ButtonUp(button),
                ]);
            }
        } else if self.state == GestureState::Wheel {
            if let Some(slot) = select_slot_scaled(self.origin, point, self.selection_scale) {
                decision.action(Action::Select {
                    wheel: self.owner,
                    slot,
                    point,
                });
            } else {
                decision.action(Action::Cancel);
            }
        }
        self.state = GestureState::Idle;
    }

    fn on_pointer_move(&mut self, point: Point, decision: &mut InputDecision) {
        if self.state == GestureState::Pending {
            let dx = i64::from(point.x) - i64::from(self.origin.x);
            let dy = i64::from(point.y) - i64::from(self.origin.y);
            if (dx as f64).hypot(dy as f64) > f64::from(self.settings.drag_threshold_px) {
                self.state = GestureState::Wheel;
                decision.action(Action::WheelOpen {
                    wheel: self.owner,
                    origin: self.origin,
                });
                decision.action(Action::WheelMove {
                    wheel: self.owner,
                    point,
                    slot: select_slot_scaled(self.origin, point, self.selection_scale),
                });
            }
        } else if self.state == GestureState::Wheel {
            decision.action(Action::WheelMove {
                wheel: self.owner,
                point,
                slot: select_slot_scaled(self.origin, point, self.selection_scale),
            });
        }
    }
}

fn modifier_bit(vk: u8) -> u8 {
    match vk {
        0x11 | 0xa2 | 0xa3 => MOD_CTRL,
        0x12 | 0xa4 | 0xa5 => MOD_ALT,
        0x10 | 0xa0 | 0xa1 => MOD_SHIFT,
        0x5b | 0x5c => MOD_WIN,
        _ => 0,
    }
}

const MOD_CTRL: u8 = 1;
const MOD_ALT: u8 = 2;
const MOD_SHIFT: u8 = 4;
const MOD_WIN: u8 = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Hotkey, ToggleMods};

    const ALT: u8 = 0xa4;
    const CTRL: u8 = 0xa2;
    const ESC: u8 = 0x1b;
    const TAB: u8 = 0x09;

    fn engine() -> GestureEngine {
        GestureEngine::new(Settings::default()).unwrap()
    }

    fn key_down(vk: u8) -> InputEvent {
        InputEvent::KeyDown {
            vk,
            injected: false,
        }
    }

    fn key_up(vk: u8) -> InputEvent {
        InputEvent::KeyUp {
            vk,
            injected: false,
        }
    }

    fn down(button: Button, x: i32, y: i32, time_ms: u64) -> InputEvent {
        InputEvent::PointerDown {
            button,
            point: Point::new(x, y),
            time_ms,
            injected: false,
            fullscreen_app: false,
            dpi_scale: 1.0,
        }
    }

    fn up(button: Button, x: i32, y: i32, time_ms: u64) -> InputEvent {
        InputEvent::PointerUp {
            button,
            point: Point::new(x, y),
            time_ms,
            injected: false,
        }
    }

    fn movement(x: i32, y: i32, time_ms: u64) -> InputEvent {
        InputEvent::PointerMove {
            point: Point::new(x, y),
            time_ms,
            injected: false,
        }
    }

    fn open_wheel(engine: &mut GestureEngine) {
        engine.on_event(key_down(ALT));
        assert!(engine.on_event(down(Button::Left, 100, 100, 100)).swallow);
        let decision = engine.on_event(movement(120, 100, 110));
        assert_eq!(
            decision.actions[0],
            Action::WheelOpen {
                wheel: WheelKind::Ping,
                origin: Point::new(100, 100)
            }
        );
    }

    #[test]
    fn drag_selects_slice_and_center_release_cancels() {
        let mut engine = engine();
        open_wheel(&mut engine);
        let decision = engine.on_event(up(Button::Left, 100, 0, 120));
        assert!(decision.swallow);
        assert_eq!(
            decision.actions,
            [Action::Select {
                wheel: WheelKind::Ping,
                slot: 0,
                point: Point::new(100, 0)
            }]
        );

        open_wheel(&mut engine);
        let decision = engine.on_event(up(Button::Left, 105, 100, 120));
        assert_eq!(decision.actions, [Action::Cancel]);
        assert_eq!(engine.state(), GestureState::Idle);
    }

    #[test]
    fn taps_and_unrelated_shortcuts_are_replayed_or_pass_through() {
        let mut engine = engine();
        assert!(!engine.on_event(down(Button::Left, 0, 0, 1)).swallow);
        engine.on_event(key_down(ALT));
        let tap = engine.on_event(down(Button::Left, 10, 20, 2));
        assert!(tap.swallow);
        let released = engine.on_event(up(Button::Left, 10, 20, 3));
        assert_eq!(
            released.replay,
            [
                ReplayInput::ButtonDown(Button::Left),
                ReplayInput::ButtonUp(Button::Left)
            ]
        );

        engine.on_event(key_up(ALT));
        engine.on_event(key_down(ALT));
        assert!(!engine.on_event(key_down(TAB)).swallow);
        assert!(!engine.on_event(down(Button::Left, 20, 20, 4)).swallow);
        assert!(!engine.on_event(up(Button::Left, 20, 20, 5)).swallow);
        assert!(!engine.on_event(key_up(ALT)).swallow);
    }

    #[test]
    fn emote_trigger_uses_its_own_wheel_and_emits_a_selection() {
        let engine_settings = Settings {
            emote_trigger: Some(Trigger::Ctrl),
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(engine_settings).unwrap();
        engine.on_event(key_down(CTRL));
        assert!(engine.on_event(down(Button::Left, 100, 100, 10)).swallow);
        let opened = engine.on_event(movement(200, 100, 20));
        assert_eq!(
            opened.actions[0],
            Action::WheelOpen {
                wheel: WheelKind::Emote,
                origin: Point::new(100, 100),
            }
        );
        let selected = engine.on_event(up(Button::Left, 200, 100, 30));
        assert_eq!(
            selected.actions,
            [Action::Select {
                wheel: WheelKind::Emote,
                slot: 2,
                point: Point::new(200, 100),
            }]
        );
    }

    #[test]
    fn selection_uses_dpi_scale_of_the_origin_monitor() {
        let mut engine = engine();
        engine.on_event(key_down(ALT));
        let down_at_scale = |time_ms| InputEvent::PointerDown {
            button: Button::Left,
            point: Point::new(100, 100),
            time_ms,
            injected: false,
            fullscreen_app: false,
            dpi_scale: 1.5,
        };
        assert!(engine.on_event(down_at_scale(1)).swallow);
        let move_inside = engine.on_event(movement(220, 100, 2));
        assert_eq!(
            move_inside.actions[1],
            Action::WheelMove {
                wheel: WheelKind::Ping,
                point: Point::new(220, 100),
                slot: None,
            }
        );
        assert_eq!(
            engine.on_event(up(Button::Left, 220, 100, 3)).actions,
            [Action::Cancel]
        );

        assert!(engine.on_event(down_at_scale(4)).swallow);
        engine.on_event(movement(229, 100, 5));
        assert_eq!(
            engine.on_event(up(Button::Left, 229, 100, 6)).actions,
            [Action::Select {
                wheel: WheelKind::Ping,
                slot: 2,
                point: Point::new(229, 100),
            }]
        );
    }

    #[test]
    fn mouse_trigger_taps_replay_the_side_button() {
        let settings = Settings {
            trigger: Trigger::Mouse4,
            emote_trigger: None,
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(settings).unwrap();
        assert!(engine.on_event(down(Button::X1, 10, 10, 1)).swallow);
        let tap = engine.on_event(up(Button::X1, 10, 10, 2));
        assert_eq!(
            tap.replay,
            [
                ReplayInput::ButtonDown(Button::X1),
                ReplayInput::ButtonUp(Button::X1)
            ]
        );
    }

    #[test]
    fn alternate_trigger_conflict_and_fullscreen_keep_input() {
        let mut engine = engine();
        engine.on_event(key_down(ALT));
        engine.on_event(key_down(CTRL));
        assert!(!engine.on_event(down(Button::Left, 1, 2, 10)).swallow);

        engine.on_event(key_up(CTRL));
        let fullscreen = InputEvent::PointerDown {
            button: Button::Left,
            point: Point::new(3, 4),
            time_ms: 20,
            injected: false,
            fullscreen_app: true,
            dpi_scale: 1.0,
        };
        assert!(!engine.on_event(fullscreen).swallow);
        assert_eq!(engine.state(), GestureState::Idle);
    }

    #[test]
    fn escape_right_click_disable_and_watchdog_cancel_without_stuck_button() {
        let mut engine = engine();
        open_wheel(&mut engine);
        assert!(engine.on_event(key_down(ESC)).swallow);
        assert!(engine.on_event(key_up(ESC)).swallow);
        assert!(engine.on_event(up(Button::Left, 120, 100, 121)).swallow);
        assert_eq!(engine.state(), GestureState::Idle);

        open_wheel(&mut engine);
        assert!(engine.on_event(down(Button::Right, 120, 100, 200)).swallow);
        assert!(engine.on_event(up(Button::Right, 120, 100, 201)).swallow);
        assert!(engine.on_event(up(Button::Left, 120, 100, 202)).swallow);
        assert_eq!(engine.state(), GestureState::Idle);

        open_wheel(&mut engine);
        assert_eq!(
            engine.tick(110 + GESTURE_WATCHDOG_MS + 1).actions,
            [Action::Cancel]
        );
        assert!(engine.on_event(up(Button::Left, 120, 100, 202)).swallow);
        assert_eq!(engine.state(), GestureState::Idle);
    }

    #[test]
    fn saving_active_settings_cancels_balances_and_keeps_manual_pause() {
        let mut engine = engine();
        open_wheel(&mut engine);
        let mut settings = engine.settings().clone();
        settings.volume = 37;
        let saved = engine.set_settings(settings.clone()).unwrap();
        assert_eq!(saved.actions, [Action::Cancel]);
        assert!(engine.enabled());
        assert!(engine.on_event(up(Button::Left, 120, 100, 120)).swallow);
        engine.on_event(key_up(ALT));
        open_wheel(&mut engine);
        assert!(matches!(
            engine.on_event(up(Button::Left, 100, 0, 130)).actions[0],
            Action::Select { .. }
        ));
        engine.on_event(key_up(ALT));
        engine.set_enabled(false);
        engine.set_settings(settings).unwrap();
        assert!(!engine.enabled());
        assert!(!engine.on_event(down(Button::Left, 100, 100, 140)).swallow);
    }

    #[test]
    fn trigger_release_replays_tap_and_masks_alt_menu_keyup() {
        let mut engine = engine();
        engine.on_event(key_down(ALT));
        engine.on_event(down(Button::Left, 10, 20, 1));
        let release = engine.on_event(key_up(ALT));
        assert!(release.swallow);
        assert_eq!(
            release.replay,
            [
                ReplayInput::KeyDown(VK_MASK_MENU),
                ReplayInput::KeyUp(VK_MASK_MENU),
                ReplayInput::KeyUp(ALT),
                ReplayInput::ButtonDown(Button::Left),
            ]
        );
        assert_eq!(engine.state(), GestureState::Idle);
        assert!(!engine.on_event(up(Button::Left, 10, 20, 2)).swallow);
    }

    #[test]
    fn both_alt_keys_are_masked_only_on_the_final_release() {
        let mut engine = engine();
        engine.on_event(key_down(0xa4));
        engine.on_event(key_down(0xa5));
        engine.on_event(down(Button::Left, 10, 20, 1));
        assert!(!engine.on_event(key_up(0xa4)).swallow);
        let final_release = engine.on_event(key_up(0xa5));
        assert!(final_release.swallow);
        assert_eq!(
            final_release.replay,
            [
                ReplayInput::KeyDown(VK_MASK_MENU),
                ReplayInput::KeyUp(VK_MASK_MENU),
                ReplayInput::KeyUp(0xa5),
                ReplayInput::ButtonDown(Button::Left),
            ]
        );
    }

    #[test]
    fn resync_does_not_release_a_deliberately_held_swallowed_trigger() {
        let settings = Settings {
            trigger: Trigger::Key { vk: b'Q' },
            emote_trigger: None,
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(settings).unwrap();
        assert!(engine.on_event(key_down(b'Q')).swallow);
        engine.resync_key_states(|_| false);
        assert!(engine.on_event(down(Button::Left, 100, 100, 10)).swallow);
        let opened = engine.on_event(movement(130, 100, 20));
        assert_eq!(
            opened.actions[0],
            Action::WheelOpen {
                wheel: WheelKind::Ping,
                origin: Point::new(100, 100),
            }
        );
        assert!(engine.on_event(key_down(b'Q')).swallow);
        let released = engine.on_event(key_up(b'Q'));
        assert!(released.swallow);
        assert_eq!(released.actions, [Action::Cancel]);
        assert!(engine.on_event(up(Button::Left, 130, 100, 30)).swallow);
    }

    #[test]
    fn safety_suspension_recovers_lost_trigger_release_safely() {
        let settings = Settings {
            trigger: Trigger::Key { vk: b'Q' },
            emote_trigger: None,
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(settings).unwrap();
        assert!(engine.on_event(key_down(b'Q')).swallow);
        assert!(engine.on_event(down(Button::Left, 100, 100, 10)).swallow);
        assert_eq!(
            engine.on_event(movement(130, 100, 20)).actions[0],
            Action::WheelOpen {
                wheel: WheelKind::Ping,
                origin: Point::new(100, 100),
            }
        );

        // The physical key-up is lost; a safety suspension cancels the wheel.
        assert_eq!(engine.set_suspended(true).actions, [Action::Cancel]);
        assert_eq!(engine.state(), GestureState::SwallowUp);
        engine.set_suspended(false);
        engine.resync_key_states(|_| false); // OS async state may still report a swallowed hold as up
        assert!(engine.on_event(key_down(b'Q')).swallow); // held-key repeats stay suppressed
        assert!(engine.on_event(up(Button::Left, 130, 100, 30)).swallow);
        assert!(!engine.on_event(down(Button::Left, 100, 100, 40)).swallow);
        assert!(!engine.on_event(up(Button::Left, 100, 100, 41)).swallow);
        assert!(engine.on_event(key_up(b'Q')).swallow); // eventual key-up stays suppressed

        assert!(engine.on_event(key_down(b'Q')).swallow); // next fresh press is a trigger again
        assert!(engine.on_event(down(Button::Left, 100, 100, 50)).swallow);
        assert!(engine.on_event(key_up(b'Q')).swallow);
        assert!(!engine.on_event(up(Button::Left, 100, 100, 51)).swallow);
    }

    #[test]
    fn explicit_disable_recovers_a_lost_custom_trigger_release() {
        let settings = Settings {
            trigger: Trigger::Key { vk: b'Q' },
            emote_trigger: None,
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(settings).unwrap();
        assert!(engine.on_event(key_down(b'Q')).swallow);
        engine.set_enabled(false);
        engine.set_enabled(true);
        assert!(engine.on_event(key_down(b'Q')).swallow);
        assert!(!engine.on_event(down(Button::Left, 1, 1, 1)).swallow);
        assert!(engine.on_event(key_up(b'Q')).swallow);
    }

    #[test]
    fn toggle_pause_recovers_a_lost_custom_trigger_release() {
        let settings = Settings {
            trigger: Trigger::Key { vk: b'Q' },
            emote_trigger: None,
            ..Settings::default()
        };
        let mut engine = GestureEngine::new(settings).unwrap();
        engine.on_event(key_down(b'Q'));
        engine.on_event(key_down(CTRL));
        engine.on_event(key_down(ALT));
        assert_eq!(
            engine.on_event(key_down(b'P')).actions,
            [Action::Toggled { enabled: false }]
        );
        assert!(engine.on_event(key_down(b'Q')).swallow);
        assert!(!engine.enabled());
        engine.set_enabled(true);
        assert!(!engine.on_event(down(Button::Left, 1, 1, 1)).swallow);
        assert!(engine.on_event(key_up(b'Q')).swallow);
    }

    #[test]
    fn toggle_autorepeat_and_swallowed_keyups_are_balanced() {
        let mut engine = engine();
        engine.on_event(key_down(CTRL));
        engine.on_event(key_down(ALT));
        let toggle = engine.on_event(key_down(b'P'));
        assert_eq!(toggle.actions, [Action::Toggled { enabled: false }]);
        assert!(toggle.swallow);
        assert!(engine.on_event(key_down(b'P')).actions.is_empty());
        assert!(engine.on_event(key_up(b'P')).swallow);
        assert!(!engine.enabled());

        engine.on_event(key_up(CTRL));
        engine.on_event(key_up(ALT));
        engine.on_event(key_down(CTRL));
        engine.on_event(key_down(ALT));
        assert_eq!(
            engine.on_event(key_down(b'P')).actions,
            [Action::Toggled { enabled: true }]
        );
        assert!(engine.enabled());

        let settings = Settings {
            toggle_hotkey: Hotkey {
                mods: ToggleMods::CTRL,
                vk: b'K',
            },
            ..Settings::default()
        };
        assert!(GestureEngine::new(settings).is_ok());
    }

    #[test]
    fn injected_events_are_ignored_and_lost_keys_can_be_resynced() {
        let mut engine = engine();
        let injected = InputEvent::PointerDown {
            button: Button::Left,
            point: Point::new(1, 2),
            time_ms: 1,
            injected: true,
            fullscreen_app: false,
            dpi_scale: 1.0,
        };
        assert!(!engine.on_event(injected).swallow);
        engine.on_event(key_down(ALT));
        engine.resync_key_states(|_| false);
        assert!(!engine.on_event(down(Button::Left, 1, 1, 2)).swallow);
    }
}
