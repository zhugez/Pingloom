use std::path::PathBuf;
use std::ptr::null;

use lolping_core::model::{PingId, Trigger, EMOTE_ASSETS, PING_ASSETS};
use lolping_core::settings::{Hotkey, Settings, ToggleMods};
use windows_sys::core::w;
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls::CB_SETMINVISIBLE;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowTextW, LoadCursorW, MessageBoxW, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, ShowWindow, BN_CLICKED, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON,
    CBS_DROPDOWNLIST, CB_ADDSTRING, CB_ERR, CB_GETCURSEL, CB_SETCURSEL, CREATESTRUCTW,
    GWLP_USERDATA, MB_ICONWARNING, MB_OK, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW,
    SW_SHOWNORMAL, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NCCREATE, WNDCLASSW, WS_CAPTION,
    WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, SetWindowPos};
use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER};

use super::{APP_SETTINGS_CLOSED, APP_SETTINGS_MESSAGE, CONTROL_FONT};

const PAGE_TRIGGERS: usize = 0;
const PAGE_PINGS: usize = 1;
const PAGE_EMOTES: usize = 2;
const BTN_TRIGGER_PAGE: i32 = 100;
const BTN_PING_PAGE: i32 = 101;
const BTN_EMOTE_PAGE: i32 = 102;
const BTN_SAVE: i32 = 103;
const BTN_CANCEL: i32 = 104;
const EDIT_PING_SIZE: i32 = 200;
const EDIT_DURATION: i32 = 201;
const EDIT_VOLUME: i32 = 202;
const EDIT_EMOTE_SIZE: i32 = 203;
const EDIT_THRESHOLD: i32 = 204;
const CHECK_MUTE: i32 = 205;
const CHECK_TICK: i32 = 206;
const CHECK_GAME: i32 = 207;
const CHECK_PING_CLICK: i32 = 208;
const CHECK_EMOTE_CLICK: i32 = 209;
const CHECK_EMOTE_SOUND: i32 = 210;
const CHECK_ENABLED_START: i32 = 211;
const CHECK_CTRL: i32 = 212;
const CHECK_ALT: i32 = 213;
const CHECK_SHIFT: i32 = 214;
const CHECK_WIN: i32 = 215;
const COMBO_PING_TRIGGER: i32 = 216;
const COMBO_EMOTE_TRIGGER: i32 = 217;
const COMBO_TOGGLE_KEY: i32 = 218;
const COMBO_CLICK_PING: i32 = 219;
const COMBO_CLICK_EMOTE: i32 = 220;
const COMBO_PING_SLOT_BASE: i32 = 300;
const COMBO_EMOTE_SLOT_BASE: i32 = 400;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Choice {
    Trigger(Trigger),
    TriggerOff,
    Key(u8),
    Ping(PingId),
    Emote(String),
}

#[derive(Clone)]
enum FieldKind {
    Edit,
    Check,
    Combo(Vec<Choice>),
}

struct Field {
    id: i32,
    hwnd: HWND,
    kind: FieldKind,
}

#[derive(Clone, Copy)]
struct Child {
    page: Option<usize>,
    hwnd: HWND,
}

pub struct SettingsWindow {
    hwnd: HWND,
    parent: HWND,
    path: PathBuf,
    settings: Settings,
    fields: Vec<Field>,
    children: Vec<Child>,
}

impl SettingsWindow {
    pub fn new(path: PathBuf) -> Self {
        Self {
            hwnd: null_mut(),
            parent: null_mut(),
            path,
            settings: Settings::default(),
            fields: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn open(&mut self, parent: HWND, settings: &Settings, instance: HINSTANCE) -> bool {
        if !self.hwnd.is_null() {
            unsafe {
                ShowWindow(self.hwnd, SW_SHOW);
                SetForegroundWindow(self.hwnd);
            }
            return true;
        }
        self.parent = parent;
        self.settings = settings.clone();
        self.fields.clear();
        self.children.clear();
        let x = (unsafe { GetSystemMetrics(SM_CXSCREEN) } - 880) / 2;
        let y = (unsafe { GetSystemMetrics(SM_CYSCREEN) } - 740) / 2;
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                w!("lolping-settings"),
                w!("lolPing settings"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                x,
                y,
                880,
                740,
                null_mut(),
                null_mut(),
                instance,
                (self as *mut Self).cast(),
            );
            if hwnd.is_null() {
                return false;
            }
            self.hwnd = hwnd;
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
        }
        true
    }

    unsafe fn build(&mut self, hwnd: HWND) {
        let tabs = [
            (BTN_TRIGGER_PAGE, "Triggers and startup", 24),
            (BTN_PING_PAGE, "Pings", 236),
            (BTN_EMOTE_PAGE, "Emotes", 448),
        ];
        for (id, text, x) in tabs {
            let tab = create_control(
                hwnd,
                w!("BUTTON"),
                text,
                WS_TABSTOP
                    | BS_AUTORADIOBUTTON as u32
                    | WS_CHILD
                    | WS_VISIBLE
                    | if id == BTN_TRIGGER_PAGE { WS_GROUP } else { 0 },
                x,
                14,
                196,
                32,
                id,
            );
            self.children.push(Child {
                page: None,
                hwnd: tab,
            });
        }
        add_label(self, hwnd, PAGE_TRIGGERS, "Trigger keys", 34, 68);
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_PING_TRIGGER,
            "Ping wheel trigger",
            34,
            98,
            trigger_choices(false),
            trigger_choice(self.settings.trigger),
        );
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_EMOTE_TRIGGER,
            "Emote wheel trigger",
            34,
            158,
            trigger_choices(true),
            self.settings
                .emote_trigger
                .map_or(Choice::TriggerOff, trigger_choice),
        );
        add_label(
            self,
            hwnd,
            PAGE_TRIGGERS,
            "Toggle shortcut modifiers (choose at least one of Ctrl, Alt or Win)",
            34,
            218,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_CTRL,
            "Ctrl",
            34,
            246,
            self.settings.toggle_hotkey.mods.0 & ToggleMods::CTRL.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_ALT,
            "Alt",
            140,
            246,
            self.settings.toggle_hotkey.mods.0 & ToggleMods::ALT.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_SHIFT,
            "Shift",
            246,
            246,
            self.settings.toggle_hotkey.mods.0 & ToggleMods::SHIFT.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_WIN,
            "Win",
            352,
            246,
            self.settings.toggle_hotkey.mods.0 & ToggleMods::WIN.0 != 0,
        );
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_TOGGLE_KEY,
            "Toggle key",
            34,
            298,
            key_choices(),
            Choice::Key(self.settings.toggle_hotkey.vk),
        );
        self.add_edit(
            hwnd,
            PAGE_TRIGGERS,
            EDIT_THRESHOLD,
            "Drag threshold (2–40 physical pixels)",
            34,
            360,
            self.settings.drag_threshold_px.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_GAME,
            "Pause gestures on the display occupied by a fullscreen app",
            34,
            424,
            self.settings.game_check,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_PING_CLICK,
            "Trigger tap plays the selected click ping",
            34,
            462,
            self.settings.click_ping,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_EMOTE_CLICK,
            "Trigger tap plays the selected click emote",
            34,
            500,
            self.settings.emote_click,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_ENABLED_START,
            "Enable gestures when lolPing starts",
            34,
            538,
            self.settings.enabled_on_start,
        );
        add_label(
            self,
            hwnd,
            PAGE_TRIGGERS,
            "Short trigger taps and unrelated shortcuts are passed through.",
            34,
            590,
        );

        add_label(
            self,
            hwnd,
            PAGE_PINGS,
            "Ping visual, audio and eight distinct wheel slots (north, then clockwise)",
            34,
            68,
        );
        self.add_edit(
            hwnd,
            PAGE_PINGS,
            EDIT_PING_SIZE,
            "Ping size (60–220 px)",
            34,
            98,
            self.settings.ping_size_px.to_string(),
        );
        self.add_edit(
            hwnd,
            PAGE_PINGS,
            EDIT_DURATION,
            "Ping duration (1–8 seconds)",
            34,
            154,
            self.settings.ping_duration_s.to_string(),
        );
        self.add_edit(
            hwnd,
            PAGE_PINGS,
            EDIT_VOLUME,
            "Volume (0–100)",
            34,
            210,
            self.settings.volume.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_PINGS,
            CHECK_MUTE,
            "Mute all sounds",
            34,
            266,
            self.settings.muted,
        );
        self.add_check(
            hwnd,
            PAGE_PINGS,
            CHECK_TICK,
            "Play sound when changing wheel selection",
            34,
            304,
            self.settings.tick_sound,
        );
        self.add_combo(
            hwnd,
            PAGE_PINGS,
            COMBO_CLICK_PING,
            "Click ping asset",
            34,
            352,
            ping_choices(),
            Choice::Ping(self.settings.click_ping_id),
        );
        let slots = [
            "North",
            "North-east",
            "East",
            "South-east",
            "South",
            "South-west",
            "West",
            "North-west",
        ];
        for (slot, label) in slots.iter().enumerate() {
            self.add_combo(
                hwnd,
                PAGE_PINGS,
                COMBO_PING_SLOT_BASE + slot as i32,
                label,
                34,
                390 + slot as i32 * 32,
                ping_choices(),
                Choice::Ping(self.settings.wheel[slot]),
            );
        }

        add_label(
            self,
            hwnd,
            PAGE_EMOTES,
            "Built-in animated emotes and their bundled sounds",
            34,
            68,
        );
        self.add_edit(
            hwnd,
            PAGE_EMOTES,
            EDIT_EMOTE_SIZE,
            "Emote size (80–300 px)",
            34,
            98,
            self.settings.emote_size_px.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_EMOTES,
            CHECK_EMOTE_SOUND,
            "Play the selected emote's sound",
            34,
            154,
            self.settings.emote_sound,
        );
        self.add_combo(
            hwnd,
            PAGE_EMOTES,
            COMBO_CLICK_EMOTE,
            "Click emote asset",
            34,
            202,
            emote_choices(),
            Choice::Emote(self.settings.click_emote_id.clone()),
        );
        for (slot, label) in slots.iter().enumerate() {
            self.add_combo(
                hwnd,
                PAGE_EMOTES,
                COMBO_EMOTE_SLOT_BASE + slot as i32,
                label,
                34,
                262 + slot as i32 * 42,
                emote_choices(),
                Choice::Emote(self.settings.emote_wheel[slot].clone()),
            );
        }
        add_label(
            self,
            hwnd,
            PAGE_EMOTES,
            "Animated WebP art and sounds play from bundled assets.",
            34,
            616,
        );

        let save = create_control(
            hwnd,
            w!("BUTTON"),
            "Save",
            WS_TABSTOP | WS_CHILD | WS_VISIBLE,
            634,
            654,
            100,
            34,
            BTN_SAVE,
        );
        let cancel = create_control(
            hwnd,
            w!("BUTTON"),
            "Cancel",
            WS_TABSTOP | WS_CHILD | WS_VISIBLE,
            744,
            654,
            100,
            34,
            BTN_CANCEL,
        );
        self.children.push(Child {
            page: None,
            hwnd: save,
        });
        self.children.push(Child {
            page: None,
            hwnd: cancel,
        });
        self.show_page(PAGE_TRIGGERS);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_edit(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        x: i32,
        y: i32,
        value: String,
    ) {
        add_label(self, parent, page, label, x, y);
        let hwnd = create_control(
            parent,
            w!("EDIT"),
            &value,
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL as u32,
            x + 320,
            y - 5,
            210,
            28,
            id,
        );
        self.add_field(page, id, hwnd, FieldKind::Edit, x + 320, y - 5, 210, 28);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_check(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        x: i32,
        y: i32,
        checked: bool,
    ) {
        let hwnd = create_control(
            parent,
            w!("BUTTON"),
            label,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32 | WS_CHILD | WS_VISIBLE,
            x,
            y,
            760,
            30,
            id,
        );
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            if checked {
                BST_CHECKED as usize
            } else {
                BST_UNCHECKED as usize
            },
            0,
        );
        self.add_field(page, id, hwnd, FieldKind::Check, x, y, 760, 30);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_combo(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        x: i32,
        y: i32,
        choices: Vec<(String, Choice)>,
        selected: Choice,
    ) {
        add_label(self, parent, page, label, x, y);
        let hwnd = create_control(
            parent,
            w!("COMBOBOX"),
            "",
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            x + 320,
            y - 5,
            360,
            28,
            id,
        );
        let mut values = Vec::with_capacity(choices.len());
        let mut selected_index = 0;
        for (index, (label, choice)) in choices.into_iter().enumerate() {
            let text: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(hwnd, CB_ADDSTRING, 0, text.as_ptr() as LPARAM);
            if choice == selected {
                selected_index = index;
            }
            values.push(choice);
        }
        SendMessageW(hwnd, CB_SETCURSEL, selected_index, 0);
        SendMessageW(hwnd, CB_SETMINVISIBLE, 10, 0);
        self.add_field(
            page,
            id,
            hwnd,
            FieldKind::Combo(values),
            x + 320,
            y - 5,
            360,
            28,
        );
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_field(
        &mut self,
        page: usize,
        id: i32,
        hwnd: HWND,
        kind: FieldKind,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        SendMessageW(hwnd, WM_SETFONT, CONTROL_FONT as usize, 1);
        self.fields.push(Field { id, hwnd, kind });
        self.children.push(Child {
            page: Some(page),
            hwnd,
        });
        SetWindowPos(
            hwnd,
            null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }

    unsafe fn show_page(&mut self, page: usize) {
        for child in &self.children {
            if let Some(child_page) = child.page {
                ShowWindow(
                    child.hwnd,
                    if child_page == page { SW_SHOW } else { SW_HIDE },
                );
            }
        }
        let selected = match page {
            PAGE_TRIGGERS => BTN_TRIGGER_PAGE,
            PAGE_PINGS => BTN_PING_PAGE,
            _ => BTN_EMOTE_PAGE,
        };
        CheckRadioButton(self.hwnd, BTN_TRIGGER_PAGE, BTN_EMOTE_PAGE, selected);
    }

    unsafe fn save(&mut self) {
        let mut settings = self.settings.clone();
        for field in &self.fields {
            match (&field.kind, field.id) {
                (FieldKind::Edit, EDIT_PING_SIZE) => match edit_value(field.hwnd).parse() {
                    Ok(value) => settings.ping_size_px = value,
                    Err(_) => return message_box(self.hwnd, "Ping size must be a number."),
                },
                (FieldKind::Edit, EDIT_EMOTE_SIZE) => match edit_value(field.hwnd).parse() {
                    Ok(value) => settings.emote_size_px = value,
                    Err(_) => return message_box(self.hwnd, "Emote size must be a number."),
                },
                (FieldKind::Edit, EDIT_DURATION) => match edit_value(field.hwnd).parse() {
                    Ok(value) => settings.ping_duration_s = value,
                    Err(_) => return message_box(self.hwnd, "Ping duration must be a number."),
                },
                (FieldKind::Edit, EDIT_VOLUME) => match edit_value(field.hwnd).parse() {
                    Ok(value) => settings.volume = value,
                    Err(_) => return message_box(self.hwnd, "Volume must be a number."),
                },
                (FieldKind::Edit, EDIT_THRESHOLD) => match edit_value(field.hwnd).parse() {
                    Ok(value) => settings.drag_threshold_px = value,
                    Err(_) => return message_box(self.hwnd, "Drag threshold must be a number."),
                },
                (FieldKind::Check, CHECK_CTRL) => set_mod(
                    &mut settings.toggle_hotkey,
                    ToggleMods::CTRL,
                    is_checked(field.hwnd),
                ),
                (FieldKind::Check, CHECK_ALT) => set_mod(
                    &mut settings.toggle_hotkey,
                    ToggleMods::ALT,
                    is_checked(field.hwnd),
                ),
                (FieldKind::Check, CHECK_SHIFT) => set_mod(
                    &mut settings.toggle_hotkey,
                    ToggleMods::SHIFT,
                    is_checked(field.hwnd),
                ),
                (FieldKind::Check, CHECK_WIN) => set_mod(
                    &mut settings.toggle_hotkey,
                    ToggleMods::WIN,
                    is_checked(field.hwnd),
                ),
                (FieldKind::Check, CHECK_MUTE) => settings.muted = is_checked(field.hwnd),
                (FieldKind::Check, CHECK_TICK) => settings.tick_sound = is_checked(field.hwnd),
                (FieldKind::Check, CHECK_GAME) => settings.game_check = is_checked(field.hwnd),
                (FieldKind::Check, CHECK_PING_CLICK) => {
                    settings.click_ping = is_checked(field.hwnd)
                }
                (FieldKind::Check, CHECK_EMOTE_CLICK) => {
                    settings.emote_click = is_checked(field.hwnd)
                }
                (FieldKind::Check, CHECK_EMOTE_SOUND) => {
                    settings.emote_sound = is_checked(field.hwnd)
                }
                (FieldKind::Check, CHECK_ENABLED_START) => {
                    settings.enabled_on_start = is_checked(field.hwnd)
                }
                (FieldKind::Combo(choices), COMBO_PING_TRIGGER) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Trigger(value)) => settings.trigger = value,
                        _ => return message_box(self.hwnd, "Choose a ping trigger."),
                    }
                }
                (FieldKind::Combo(choices), COMBO_EMOTE_TRIGGER) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Trigger(value)) => settings.emote_trigger = Some(value),
                        Some(Choice::TriggerOff) => settings.emote_trigger = None,
                        _ => return message_box(self.hwnd, "Choose an emote trigger."),
                    }
                }
                (FieldKind::Combo(choices), COMBO_TOGGLE_KEY) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Key(value)) => settings.toggle_hotkey.vk = value,
                        _ => return message_box(self.hwnd, "Choose a toggle key."),
                    }
                }
                (FieldKind::Combo(choices), COMBO_CLICK_PING) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Ping(value)) => settings.click_ping_id = value,
                        _ => return message_box(self.hwnd, "Choose a click ping."),
                    }
                }
                (FieldKind::Combo(choices), COMBO_CLICK_EMOTE) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Emote(value)) => settings.click_emote_id = value,
                        _ => return message_box(self.hwnd, "Choose a click emote."),
                    }
                }
                (FieldKind::Combo(choices), id)
                    if (COMBO_PING_SLOT_BASE..COMBO_PING_SLOT_BASE + 8).contains(&id) =>
                {
                    let slot = (id - COMBO_PING_SLOT_BASE) as usize;
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Ping(value)) => settings.wheel[slot] = value,
                        _ => return message_box(self.hwnd, "Choose a ping for each slot."),
                    }
                }
                (FieldKind::Combo(choices), id)
                    if (COMBO_EMOTE_SLOT_BASE..COMBO_EMOTE_SLOT_BASE + 8).contains(&id) =>
                {
                    let slot = (id - COMBO_EMOTE_SLOT_BASE) as usize;
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Emote(value)) => settings.emote_wheel[slot] = value,
                        _ => return message_box(self.hwnd, "Choose an emote for each slot."),
                    }
                }
                _ => {}
            }
        }
        if settings.emote_trigger == Some(settings.trigger) {
            return message_box(self.hwnd, "Ping and emote triggers must be different.");
        }
        if let Err(error) = settings.validate() {
            return message_box(self.hwnd, &error.to_string());
        }
        if let Err(error) = settings.save_atomic(&self.path) {
            return message_box(self.hwnd, &format!("Could not save settings: {error}"));
        }
        let posted = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
            self.parent,
            APP_SETTINGS_MESSAGE,
            0,
            Box::into_raw(Box::new(settings)) as LPARAM,
        );
        if posted == 0 {
            return message_box(
                self.hwnd,
                "Settings were saved but could not be applied until restart.",
            );
        }
        DestroyWindow(self.hwnd);
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        if !self.hwnd.is_null() {
            unsafe { DestroyWindow(self.hwnd) };
            self.hwnd = null_mut();
        }
    }
}

pub fn register(instance: HINSTANCE) -> Result<(), String> {
    let class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(settings_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: unsafe { LoadCursorW(null_mut(), w!("IDC_ARROW")) },
        hbrBackground: (COLOR_WINDOW + 1) as _,
        lpszMenuName: null(),
        lpszClassName: w!("lolping-settings"),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("could not register settings window class".into());
    }
    Ok(())
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    let pointer = if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        let pointer = create.lpCreateParams as *mut SettingsWindow;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, pointer as isize);
        pointer
    } else {
        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow
    };
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let window = &mut *pointer;
    match message {
        WM_CREATE => {
            window.hwnd = hwnd;
            window.build(hwnd);
            0
        }
        WM_COMMAND if (wparam & 0xffff) as i32 == BTN_TRIGGER_PAGE => {
            window.show_page(PAGE_TRIGGERS);
            0
        }
        WM_COMMAND if (wparam & 0xffff) as i32 == BTN_PING_PAGE => {
            window.show_page(PAGE_PINGS);
            0
        }
        WM_COMMAND if (wparam & 0xffff) as i32 == BTN_EMOTE_PAGE => {
            window.show_page(PAGE_EMOTES);
            0
        }
        WM_COMMAND
            if (wparam & 0xffff) as i32 == BTN_SAVE && (wparam >> 16) as u32 == BN_CLICKED =>
        {
            window.save();
            0
        }
        WM_COMMAND
            if (wparam & 0xffff) as i32 == BTN_CANCEL && (wparam >> 16) as u32 == BN_CLICKED =>
        {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            window.hwnd = null_mut();
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                window.parent,
                APP_SETTINGS_CLOSED,
                0,
                0,
            );
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    parent: HWND,
    class: windows_sys::core::PCWSTR,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
) -> HWND {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = CreateWindowExW(
        0,
        class,
        text.as_ptr(),
        style,
        x,
        y,
        width,
        height,
        parent,
        id as usize as _,
        null_mut(),
        null(),
    );
    if !hwnd.is_null() && CONTROL_FONT != 0 {
        SendMessageW(hwnd, WM_SETFONT, CONTROL_FONT as usize, 1);
    }
    hwnd
}

unsafe fn add_label(
    window: &mut SettingsWindow,
    parent: HWND,
    page: usize,
    text: &str,
    x: i32,
    y: i32,
) {
    let hwnd = create_control(
        parent,
        w!("STATIC"),
        text,
        WS_CHILD | WS_VISIBLE,
        x,
        y,
        310,
        32,
        0,
    );
    SendMessageW(hwnd, WM_SETFONT, CONTROL_FONT as usize, 1);
    window.children.push(Child {
        page: Some(page),
        hwnd,
    });
}

fn trigger_choices(include_off: bool) -> Vec<(String, Choice)> {
    let mut values = Vec::new();
    if include_off {
        values.push(("Off".into(), Choice::TriggerOff));
    }
    values.extend([
        ("Alt".into(), Choice::Trigger(Trigger::Alt)),
        ("Ctrl".into(), Choice::Trigger(Trigger::Ctrl)),
        ("Shift".into(), Choice::Trigger(Trigger::Shift)),
        ("Win".into(), Choice::Trigger(Trigger::Win)),
        ("Caps Lock".into(), Choice::Trigger(Trigger::CapsLock)),
        ("Mouse 4".into(), Choice::Trigger(Trigger::Mouse4)),
        ("Mouse 5".into(), Choice::Trigger(Trigger::Mouse5)),
    ]);
    values.extend(key_choices().into_iter().map(|(name, choice)| {
        let Choice::Key(vk) = choice else {
            unreachable!()
        };
        (name, Choice::Trigger(Trigger::Key { vk }))
    }));
    values
}

fn key_choices() -> Vec<(String, Choice)> {
    let mut keys = (b'A'..=b'Z')
        .map(|key| (char::from(key).to_string(), key))
        .collect::<Vec<_>>();
    keys.extend((b'0'..=b'9').map(|key| (char::from(key).to_string(), key)));
    keys.extend((1..=24).map(|n| (format!("F{n}"), 0x6f + n)));
    keys.into_iter()
        .map(|(label, vk)| (label, Choice::Key(vk)))
        .collect()
}

fn ping_choices() -> Vec<(String, Choice)> {
    PING_ASSETS
        .iter()
        .map(|asset| (asset.name.to_owned(), Choice::Ping(asset.id)))
        .collect()
}

fn emote_choices() -> Vec<(String, Choice)> {
    EMOTE_ASSETS
        .iter()
        .map(|slug| (slug.replace('_', " "), Choice::Emote((*slug).to_owned())))
        .collect()
}

fn trigger_choice(trigger: Trigger) -> Choice {
    Choice::Trigger(trigger)
}

unsafe fn combo_value(hwnd: HWND, choices: &[Choice]) -> Option<Choice> {
    let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0);
    (index != CB_ERR as isize)
        .then(|| choices.get(index as usize).cloned())
        .flatten()
}

unsafe fn edit_value(hwnd: HWND) -> String {
    let length = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut text = vec![0u16; length + 1];
    let read = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&text[..read]).trim().to_owned()
}

unsafe fn is_checked(hwnd: HWND) -> bool {
    SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
}

fn set_mod(hotkey: &mut Hotkey, flag: ToggleMods, enabled: bool) {
    if enabled {
        hotkey.mods.0 |= flag.0
    } else {
        hotkey.mods.0 &= !flag.0
    }
}

unsafe fn message_box(hwnd: HWND, text: &str) {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    MessageBoxW(
        hwnd,
        text.as_ptr(),
        w!("lolPing settings"),
        MB_OK | MB_ICONWARNING,
    );
}

use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
use windows_sys::Win32::UI::Controls::{CheckRadioButton, BST_CHECKED, BST_UNCHECKED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, ES_AUTOHSCROLL, WM_SETFONT, WS_BORDER, WS_GROUP,
};
