use std::collections::HashMap;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Arc;

use crate::logic::CallbackGate;
use crate::logic::{
    composite_rgba_over_rgb, constrain_rect_to_work_area, fit_window_to_work_area,
    reveal_scroll_delta, scale_dip, settings_issue, wheel_boundary_angle_degrees, SettingsIssue,
    UiRect,
};

use pingloom_core::model::{PingId, Trigger, WheelKind, EMOTE_ASSETS, PING_ASSETS};
use pingloom_core::settings::{Hotkey, Settings, ToggleMods};
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, DeleteObject, Ellipse, FillRect, GetStockObject, GetSysColor,
    GetSysColorBrush, LineTo, MoveToEx, SelectObject, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, COLOR_WINDOW, COLOR_WINDOWTEXT, DEFAULT_GUI_FONT, DIB_RGB_COLORS, DT_CENTER,
    DT_END_ELLIPSIS, DT_NOPREFIX, DT_VCENTER, DT_WORDBREAK, PS_SOLID, SRCCOPY,
};
use windows_sys::Win32::UI::Controls::{
    CheckRadioButton, SetScrollInfo, BST_CHECKED, BST_UNCHECKED, CB_SETMINVISIBLE, DRAWITEMSTRUCT,
    EM_SETSEL,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetParent,
    GetScrollInfo, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    LoadCursorW, RegisterClassW, SendMessageW, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, BN_CLICKED, BN_SETFOCUS, BS_AUTOCHECKBOX,
    BS_AUTORADIOBUTTON, BS_DEFPUSHBUTTON, BS_MULTILINE, BS_NOTIFY, BS_PUSHBUTTON, CBN_SELCHANGE,
    CBN_SETFOCUS, CB_ADDSTRING, CB_ERR, CB_GETCURSEL, CB_GETDROPPEDSTATE, CB_SETCURSEL,
    EN_SETFOCUS, ES_AUTOHSCROLL, GWLP_HINSTANCE, GWLP_USERDATA, MB_ICONWARNING, MB_OK, SB_BOTTOM,
    SB_ENDSCROLL, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK,
    SB_VERT, SCROLLINFO, SIF_ALL, SIF_DISABLENOSCROLL, SIF_PAGE, SIF_POS, SIF_RANGE,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNORMAL, WM_CLOSE, WM_COMMAND,
    WM_DPICHANGED, WM_DRAWITEM, WM_GETMINMAXINFO, WM_MOUSEWHEEL, WM_NCCREATE, WM_SETFONT, WM_SIZE,
    WM_VSCROLL, WNDCLASSW, WS_BORDER, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_EX_CONTROLPARENT, WS_GROUP, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use super::assets::ASSETS;
use super::{queue_main_message, APP_SETTINGS_CLOSED, APP_SETTINGS_MESSAGE};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MapWindowPoints, MonitorFromPoint, MonitorFromWindow,
    MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::SystemServices::{SS_ETCHEDHORZ, SS_OWNERDRAW};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{BM_GETCHECK, BM_SETCHECK, CBS_DROPDOWNLIST};

static SETTINGS_CALLBACK_GATE: CallbackGate = CallbackGate::new();

const PAGE_TRIGGERS: usize = 0;
const PAGE_PINGS: usize = 1;
const PAGE_EMOTES: usize = 2;
const PAGE_TOP: i32 = 82;
const FOOTER_HEIGHT: i32 = 58;
const BTN_TRIGGER_PAGE: i32 = 100;
const BTN_PING_PAGE: i32 = 101;
const BTN_EMOTE_PAGE: i32 = 102;
const BTN_SAVE: i32 = 1;
const BTN_CANCEL: i32 = 2;
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
const PREVIEW_PING: i32 = 500;
const PREVIEW_EMOTE: i32 = 501;
const SLOTS: [&str; 8] = [
    "North",
    "North-east",
    "East",
    "South-east",
    "South",
    "South-west",
    "West",
    "North-west",
];

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
    page: usize,
    hwnd: HWND,
    kind: FieldKind,
    label_hwnd: Option<HWND>,
    label: String,
}

struct FieldError {
    page: usize,
    id: i32,
    label: String,
    recovery: &'static str,
}

#[derive(Clone, Copy)]
struct LayoutRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy)]
enum ChildPosition {
    Content(LayoutRect),
    Navigation(usize),
    FooterRule,
    Save,
    Cancel,
    Check(i32, LayoutRect),
    TriggerLabel(i32),
    TriggerInput(i32),
    TriggerHelp,
    NumericLabel(i32),
    NumericInput(i32),
    ClickLabel,
    ClickCombo,
    Preview,
    SlotLabel(i32),
    SlotCombo(i32),
}

struct Child {
    page: Option<usize>,
    hwnd: HWND,
    position: ChildPosition,
}

pub struct SettingsWindow {
    hwnd: HWND,
    viewport: HWND,
    path: PathBuf,
    settings: Settings,
    fields: Vec<Field>,
    children: Vec<Child>,
    dpi: u32,
    font: isize,
    page: usize,
    scroll_dip: i32,
    preview_cache: HashMap<String, Arc<image::RgbaImage>>,
}

impl SettingsWindow {
    pub fn new(path: PathBuf) -> Self {
        Self {
            hwnd: null_mut(),
            viewport: null_mut(),
            path,
            settings: Settings::default(),
            fields: Vec::new(),
            children: Vec::new(),
            dpi: 96,
            font: 0,
            page: PAGE_TRIGGERS,
            scroll_dip: 0,
            preview_cache: HashMap::new(),
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn callback_busy() -> bool {
        SETTINGS_CALLBACK_GATE.is_held()
    }

    pub fn open(&mut self, settings: &Settings, instance: HINSTANCE) -> bool {
        let Some(permit) = SETTINGS_CALLBACK_GATE.enter() else {
            return false;
        };
        let opened = if !self.hwnd.is_null() {
            unsafe {
                ShowWindow(self.hwnd, SW_SHOW);
                SetForegroundWindow(self.hwnd);
                let first_field = self.first_field(self.page);
                self.ensure_field_visible(first_field);
                SetFocus(first_field);
            }
            true
        } else {
            self.settings = settings.clone();
            self.fields.clear();
            self.children.clear();
            self.preview_cache.clear();
            self.page = PAGE_TRIGGERS;
            self.scroll_dip = 0;
            let (work, dpi) = unsafe { work_area_at_cursor() };
            self.dpi = dpi;
            self.font = unsafe { create_ui_font(dpi) } as isize;
            let rect = fit_window_to_work_area(
                work,
                scale_dip(1000, dpi),
                scale_dip(820, dpi),
                scale_dip(16, dpi),
            );
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    w!("pingloom-settings"),
                    w!("Pingloom settings"),
                    WS_OVERLAPPEDWINDOW | WS_VSCROLL,
                    rect.left,
                    rect.top,
                    rect.width(),
                    rect.height(),
                    null_mut(),
                    null_mut(),
                    instance,
                    (self as *mut Self).cast(),
                )
            };
            if hwnd.is_null() {
                unsafe { self.delete_font() };
                false
            } else {
                self.hwnd = hwnd;
                if unsafe { self.build(hwnd) } {
                    unsafe {
                        ShowWindow(hwnd, SW_SHOWNORMAL);
                        self.layout_client();
                        SetForegroundWindow(hwnd);
                        SetFocus(self.first_field(PAGE_TRIGGERS));
                    }
                    true
                } else {
                    if !unsafe { self.destroy_window(false) } {
                        super::output_debug_string(
                            "could not destroy settings window after viewport creation failed",
                        );
                    }
                    false
                }
            }
        };
        drop(permit);
        opened
    }

    unsafe fn build(&mut self, hwnd: HWND) -> bool {
        let root = hwnd;
        for (index, (id, text)) in [
            (BTN_TRIGGER_PAGE, "&Triggers"),
            (BTN_PING_PAGE, "P&ings"),
            (BTN_EMOTE_PAGE, "E&motes"),
        ]
        .into_iter()
        .enumerate()
        {
            let button = self.create_control(
                hwnd,
                w!("BUTTON"),
                text,
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | BS_AUTORADIOBUTTON as u32
                    | if index == 0 { WS_GROUP } else { 0 },
                id,
            );
            self.children.push(Child {
                page: None,
                hwnd: button,
                position: ChildPosition::Navigation(index),
            });
        }

        let hwnd = self.create_viewport(root);
        if hwnd.is_null() {
            return false;
        }

        self.add_label(
            hwnd,
            PAGE_TRIGGERS,
            "Configure how each wheel starts and how gestures pause.",
            LayoutRect {
                x: 28,
                y: 8,
                width: 850,
                height: 30,
            },
        );
        self.add_label(
            hwnd,
            PAGE_TRIGGERS,
            "Hold a trigger and drag; release on a slice to choose. Centre, Esc, or right-click cancels.",
            LayoutRect { x: 28, y: 43, width: 890, height: 38 },
        );
        self.add_label(
            hwnd,
            PAGE_TRIGGERS,
            "Wheel triggers",
            LayoutRect {
                x: 28,
                y: 91,
                width: 820,
                height: 26,
            },
        );
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_PING_TRIGGER,
            "&Ping trigger",
            LayoutRect {
                x: 28,
                y: 128,
                width: 300,
                height: 24,
            },
            LayoutRect {
                x: 340,
                y: 123,
                width: 310,
                height: 30,
            },
            trigger_choices(false),
            Choice::Trigger(self.settings.trigger),
        );
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_EMOTE_TRIGGER,
            "&Emote trigger (optional)",
            LayoutRect {
                x: 28,
                y: 174,
                width: 300,
                height: 24,
            },
            LayoutRect {
                x: 340,
                y: 169,
                width: 310,
                height: 30,
            },
            trigger_choices(true),
            self.settings
                .emote_trigger
                .map_or(Choice::TriggerOff, Choice::Trigger),
        );
        self.add_label(
            hwnd,
            PAGE_TRIGGERS,
            "Toggle shortcut",
            LayoutRect {
                x: 28,
                y: 224,
                width: 820,
                height: 26,
            },
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_CTRL,
            "&Ctrl",
            LayoutRect {
                x: 28,
                y: 265,
                width: 100,
                height: 28,
            },
            self.settings.toggle_hotkey.mods.0 & ToggleMods::CTRL.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_ALT,
            "&Alt",
            LayoutRect {
                x: 140,
                y: 265,
                width: 100,
                height: 28,
            },
            self.settings.toggle_hotkey.mods.0 & ToggleMods::ALT.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_SHIFT,
            "S&hift",
            LayoutRect {
                x: 252,
                y: 265,
                width: 110,
                height: 28,
            },
            self.settings.toggle_hotkey.mods.0 & ToggleMods::SHIFT.0 != 0,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_WIN,
            "&Win",
            LayoutRect {
                x: 374,
                y: 265,
                width: 100,
                height: 28,
            },
            self.settings.toggle_hotkey.mods.0 & ToggleMods::WIN.0 != 0,
        );
        self.add_combo(
            hwnd,
            PAGE_TRIGGERS,
            COMBO_TOGGLE_KEY,
            "Toggle &key",
            LayoutRect {
                x: 28,
                y: 311,
                width: 300,
                height: 24,
            },
            LayoutRect {
                x: 340,
                y: 306,
                width: 310,
                height: 30,
            },
            key_choices(),
            Choice::Key(self.settings.toggle_hotkey.vk),
        );
        self.add_edit(
            hwnd,
            PAGE_TRIGGERS,
            EDIT_THRESHOLD,
            "Drag threshold (2–40 physical pixels)",
            LayoutRect {
                x: 28,
                y: 356,
                width: 300,
                height: 24,
            },
            LayoutRect {
                x: 340,
                y: 351,
                width: 120,
                height: 30,
            },
            self.settings.drag_threshold_px.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_GAME,
            "Pause gestures on the display occupied by a fullscreen app",
            LayoutRect {
                x: 28,
                y: 404,
                width: 790,
                height: 30,
            },
            self.settings.game_check,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_PING_CLICK,
            "Play the selected ping on a trigger tap",
            LayoutRect {
                x: 28,
                y: 444,
                width: 790,
                height: 30,
            },
            self.settings.click_ping,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_EMOTE_CLICK,
            "Play the selected emote on a trigger tap",
            LayoutRect {
                x: 28,
                y: 484,
                width: 790,
                height: 30,
            },
            self.settings.emote_click,
        );
        self.add_check(
            hwnd,
            PAGE_TRIGGERS,
            CHECK_ENABLED_START,
            "Enable gestures when Pingloom starts",
            LayoutRect {
                x: 28,
                y: 524,
                width: 790,
                height: 30,
            },
            self.settings.enabled_on_start,
        );
        let trigger_help = self.add_label(
            hwnd,
            PAGE_TRIGGERS,
            "Short trigger taps and unrelated shortcuts pass through to other apps.",
            LayoutRect {
                x: 28,
                y: 572,
                width: 850,
                height: 30,
            },
        );
        if let Some(child) = self
            .children
            .iter_mut()
            .find(|child| child.hwnd == trigger_help)
        {
            child.position = ChildPosition::TriggerHelp;
        }

        self.add_label(
            hwnd,
            PAGE_PINGS,
            "Pings",
            LayoutRect {
                x: 28,
                y: 8,
                width: 850,
                height: 30,
            },
        );
        self.add_label(
            hwnd,
            PAGE_PINGS,
            "Eight distinct assignments, starting North and moving clockwise. Preview follows your unsaved choices.",
            LayoutRect { x: 28, y: 43, width: 890, height: 34 },
        );
        self.add_numeric(
            hwnd,
            PAGE_PINGS,
            EDIT_PING_SIZE,
            "Ping size (60–220 px)",
            self.settings.ping_size_px.to_string(),
        );
        self.add_numeric(
            hwnd,
            PAGE_PINGS,
            EDIT_DURATION,
            "Ping duration (1–8 sec)",
            self.settings.ping_duration_s.to_string(),
        );
        self.add_numeric(
            hwnd,
            PAGE_PINGS,
            EDIT_VOLUME,
            "Volume (0–100)",
            self.settings.volume.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_PINGS,
            CHECK_MUTE,
            "Mute all sounds",
            LayoutRect {
                x: 28,
                y: 158,
                width: 220,
                height: 28,
            },
            self.settings.muted,
        );
        self.add_check(
            hwnd,
            PAGE_PINGS,
            CHECK_TICK,
            "Play a sound when the wheel selection changes",
            LayoutRect {
                x: 266,
                y: 158,
                width: 480,
                height: 28,
            },
            self.settings.tick_sound,
        );
        self.add_combo(
            hwnd,
            PAGE_PINGS,
            COMBO_CLICK_PING,
            "Ping for trigger tap",
            LayoutRect {
                x: 330,
                y: 201,
                width: 200,
                height: 24,
            },
            LayoutRect {
                x: 532,
                y: 196,
                width: 330,
                height: 30,
            },
            ping_choices(),
            Choice::Ping(self.settings.click_ping_id),
        );
        self.add_preview(hwnd, PAGE_PINGS, PREVIEW_PING);
        for (slot, direction) in SLOTS.iter().enumerate() {
            self.add_slot_combo(
                hwnd,
                PAGE_PINGS,
                COMBO_PING_SLOT_BASE + slot as i32,
                slot,
                direction,
                ping_choices(),
                Choice::Ping(self.settings.wheel[slot]),
            );
        }

        self.add_label(
            hwnd,
            PAGE_EMOTES,
            "Emotes",
            LayoutRect {
                x: 28,
                y: 8,
                width: 850,
                height: 30,
            },
        );
        self.add_label(
            hwnd,
            PAGE_EMOTES,
            "Built-in animated emotes and sounds. The wheel preview follows your unsaved choices.",
            LayoutRect {
                x: 28,
                y: 43,
                width: 890,
                height: 34,
            },
        );
        self.add_numeric(
            hwnd,
            PAGE_EMOTES,
            EDIT_EMOTE_SIZE,
            "Emote size (80–300 px)",
            self.settings.emote_size_px.to_string(),
        );
        self.add_check(
            hwnd,
            PAGE_EMOTES,
            CHECK_EMOTE_SOUND,
            "Play the selected emote's sound",
            LayoutRect {
                x: 28,
                y: 158,
                width: 310,
                height: 28,
            },
            self.settings.emote_sound,
        );
        self.add_combo(
            hwnd,
            PAGE_EMOTES,
            COMBO_CLICK_EMOTE,
            "Emote for trigger tap",
            LayoutRect {
                x: 28,
                y: 201,
                width: 200,
                height: 24,
            },
            LayoutRect {
                x: 232,
                y: 196,
                width: 330,
                height: 30,
            },
            emote_choices(),
            Choice::Emote(self.settings.click_emote_id.clone()),
        );
        self.add_preview(hwnd, PAGE_EMOTES, PREVIEW_EMOTE);
        for (slot, direction) in SLOTS.iter().enumerate() {
            self.add_slot_combo(
                hwnd,
                PAGE_EMOTES,
                COMBO_EMOTE_SLOT_BASE + slot as i32,
                slot,
                direction,
                emote_choices(),
                Choice::Emote(self.settings.emote_wheel[slot].clone()),
            );
        }

        let rule = self.create_control(
            root,
            w!("STATIC"),
            "",
            WS_CHILD | WS_VISIBLE | SS_ETCHEDHORZ,
            0,
        );
        self.children.push(Child {
            page: None,
            hwnd: rule,
            position: ChildPosition::FooterRule,
        });
        let save = self.create_control(
            root,
            w!("BUTTON"),
            "&Save",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            BTN_SAVE,
        );
        let cancel = self.create_control(
            root,
            w!("BUTTON"),
            "&Cancel",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            BTN_CANCEL,
        );
        self.children.push(Child {
            page: None,
            hwnd: save,
            position: ChildPosition::Save,
        });
        self.children.push(Child {
            page: None,
            hwnd: cancel,
            position: ChildPosition::Cancel,
        });
        self.show_page(PAGE_TRIGGERS, false);
        self.refresh_duplicates();
        true
    }

    unsafe fn create_viewport(&mut self, parent: HWND) -> HWND {
        self.viewport = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            w!("pingloom-settings-viewport"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            parent,
            null_mut(),
            GetWindowLongPtrW(parent, GWLP_HINSTANCE) as HINSTANCE,
            null(),
        );
        self.viewport
    }

    unsafe fn create_control(
        &self,
        parent: HWND,
        class: windows_sys::core::PCWSTR,
        text: &str,
        style: u32,
        id: i32,
    ) -> HWND {
        let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            0,
            class,
            text.as_ptr(),
            style,
            0,
            0,
            0,
            0,
            parent,
            id as usize as _,
            null_mut(),
            null(),
        );
        if !hwnd.is_null() && self.font != 0 {
            SendMessageW(hwnd, WM_SETFONT, self.font as usize, 1);
        }
        hwnd
    }

    unsafe fn add_label(
        &mut self,
        parent: HWND,
        page: usize,
        text: &str,
        rect: LayoutRect,
    ) -> HWND {
        let hwnd = self.create_control(parent, w!("STATIC"), text, WS_CHILD | WS_VISIBLE, 0);
        self.children.push(Child {
            page: Some(page),
            hwnd,
            position: ChildPosition::Content(rect),
        });
        hwnd
    }

    unsafe fn add_numeric(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        value: String,
    ) {
        let label_hwnd = self.add_label(
            parent,
            page,
            label,
            LayoutRect {
                x: 24,
                y: 88,
                width: 240,
                height: 24,
            },
        );
        if let Some(child) = self
            .children
            .iter_mut()
            .find(|child| child.hwnd == label_hwnd)
        {
            child.position = ChildPosition::NumericLabel(id);
        }
        let input = self.create_control(
            parent,
            w!("EDIT"),
            &value,
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL as u32,
            id,
        );
        self.fields.push(Field {
            id,
            page,
            hwnd: input,
            kind: FieldKind::Edit,
            label_hwnd: Some(label_hwnd),
            label: label.replace('&', ""),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd: input,
            position: ChildPosition::NumericInput(id),
        });
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_edit(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        label_rect: LayoutRect,
        edit_rect: LayoutRect,
        value: String,
    ) {
        let label_hwnd = self.add_label(parent, page, label, label_rect);
        if id == EDIT_THRESHOLD {
            if let Some(child) = self
                .children
                .iter_mut()
                .find(|child| child.hwnd == label_hwnd)
            {
                child.position = ChildPosition::TriggerLabel(id);
            }
        }
        let input = self.create_control(
            parent,
            w!("EDIT"),
            &value,
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL as u32,
            id,
        );
        self.fields.push(Field {
            id,
            page,
            hwnd: input,
            kind: FieldKind::Edit,
            label_hwnd: Some(label_hwnd),
            label: label.to_owned(),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd: input,
            position: if id == EDIT_THRESHOLD {
                ChildPosition::TriggerInput(id)
            } else {
                ChildPosition::Content(edit_rect)
            },
        });
    }

    unsafe fn add_check(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        rect: LayoutRect,
        checked: bool,
    ) {
        let hwnd = self.create_control(
            parent,
            w!("BUTTON"),
            label,
            WS_TABSTOP
                | BS_AUTOCHECKBOX as u32
                | BS_MULTILINE as u32
                | BS_NOTIFY as u32
                | WS_CHILD
                | WS_VISIBLE,
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
        self.fields.push(Field {
            id,
            page,
            hwnd,
            kind: FieldKind::Check,
            label_hwnd: None,
            label: label.to_owned(),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd,
            position: ChildPosition::Check(id, rect),
        });
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_combo(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        label: &str,
        label_rect: LayoutRect,
        combo_rect: LayoutRect,
        choices: Vec<(String, Choice)>,
        selected: Choice,
    ) {
        let label_hwnd = self.add_label(parent, page, label, label_rect);
        let trigger_field = matches!(
            id,
            COMBO_PING_TRIGGER | COMBO_EMOTE_TRIGGER | COMBO_TOGGLE_KEY
        );
        if trigger_field {
            if let Some(child) = self
                .children
                .iter_mut()
                .find(|child| child.hwnd == label_hwnd)
            {
                child.position = ChildPosition::TriggerLabel(id);
            }
        }
        let hwnd = self.create_control(
            parent,
            w!("COMBOBOX"),
            "",
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
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
        SendMessageW(hwnd, CB_SETMINVISIBLE, 9, 0);
        self.fields.push(Field {
            id,
            page,
            hwnd,
            kind: FieldKind::Combo(values),
            label_hwnd: Some(label_hwnd),
            label: label.to_owned(),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd,
            position: if id == COMBO_CLICK_PING || id == COMBO_CLICK_EMOTE {
                ChildPosition::ClickCombo
            } else if trigger_field {
                ChildPosition::TriggerInput(id)
            } else {
                ChildPosition::Content(combo_rect)
            },
        });
        if id == COMBO_CLICK_PING || id == COMBO_CLICK_EMOTE {
            if let Some(child) = self
                .children
                .iter_mut()
                .find(|child| child.hwnd == label_hwnd)
            {
                child.position = ChildPosition::ClickLabel;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn add_slot_combo(
        &mut self,
        parent: HWND,
        page: usize,
        id: i32,
        slot: usize,
        direction: &'static str,
        choices: Vec<(String, Choice)>,
        selected: Choice,
    ) {
        let label_hwnd = self.add_label(
            parent,
            page,
            direction,
            LayoutRect {
                x: 440,
                y: 294,
                width: 90,
                height: 26,
            },
        );
        let hwnd = self.create_control(
            parent,
            w!("COMBOBOX"),
            "",
            WS_TABSTOP | WS_CHILD | WS_VISIBLE | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
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
        SendMessageW(hwnd, CB_SETMINVISIBLE, 9, 0);
        self.fields.push(Field {
            id,
            page,
            hwnd,
            kind: FieldKind::Combo(values),
            label_hwnd: Some(label_hwnd),
            label: direction.to_owned(),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd: label_hwnd,
            position: ChildPosition::SlotLabel(id),
        });
        self.children.push(Child {
            page: Some(page),
            hwnd,
            position: ChildPosition::SlotCombo(id),
        });
        let _ = slot;
    }

    unsafe fn add_preview(&mut self, parent: HWND, page: usize, id: i32) {
        let preview = self.create_control(
            parent,
            w!("STATIC"),
            "",
            WS_CHILD | WS_VISIBLE | SS_OWNERDRAW,
            id,
        );
        self.children.push(Child {
            page: Some(page),
            hwnd: preview,
            position: ChildPosition::Preview,
        });
    }

    unsafe fn first_field(&self, page: usize) -> HWND {
        self.fields
            .iter()
            .find(|field| field.page == page)
            .map_or(null_mut(), |field| field.hwnd)
    }

    unsafe fn show_page(&mut self, page: usize, focus: bool) {
        self.page = page;
        self.scroll_dip = 0;
        CheckRadioButton(
            self.hwnd,
            BTN_TRIGGER_PAGE,
            BTN_EMOTE_PAGE,
            match page {
                PAGE_TRIGGERS => BTN_TRIGGER_PAGE,
                PAGE_PINGS => BTN_PING_PAGE,
                _ => BTN_EMOTE_PAGE,
            },
        );
        self.layout_client();
        if focus {
            SetFocus(self.first_field(page));
        }
        self.invalidate_preview();
    }

    unsafe fn layout_client(&mut self) {
        if self.hwnd.is_null() {
            return;
        }
        let mut client: RECT = std::mem::zeroed();
        if GetClientRect(self.hwnd, &mut client) == 0 {
            return;
        }
        let width = ((client.right - client.left) * 96 / self.dpi.max(1) as i32).max(1);
        let height = ((client.bottom - client.top) * 96 / self.dpi.max(1) as i32).max(1);
        let narrow = width < 920;
        let stacked = width < 760;
        let content_height = match (self.page, narrow, width < 600) {
            (PAGE_TRIGGERS, _, true) => 632,
            (PAGE_TRIGGERS, _, false) => 622,
            (PAGE_PINGS, true, _) => 1140,
            (PAGE_EMOTES, true, _) => 1110,
            (PAGE_PINGS, false, _) => 660,
            _ => 640,
        };
        let viewport = (height - PAGE_TOP - FOOTER_HEIGHT - 12).max(1);
        self.scroll_dip = self.scroll_dip.clamp(0, (content_height - viewport).max(0));
        let scroll_info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS | SIF_DISABLENOSCROLL,
            nMin: 0,
            nMax: scale_dip(content_height.max(1), self.dpi) - 1,
            nPage: scale_dip(viewport, self.dpi).max(1) as u32,
            nPos: scale_dip(self.scroll_dip, self.dpi),
            nTrackPos: 0,
        };
        SetScrollInfo(self.hwnd, SB_VERT, &scroll_info, 1);

        if !self.viewport.is_null() {
            SetWindowPos(
                self.viewport,
                null_mut(),
                0,
                scale_dip(PAGE_TOP, self.dpi),
                scale_dip(width, self.dpi),
                scale_dip(viewport, self.dpi),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            ShowWindow(self.viewport, SW_SHOW);
        }

        let right = width - if content_height > viewport { 20 } else { 12 };
        for child in &self.children {
            let content = match child.position {
                ChildPosition::Content(mut rect) => {
                    rect.width = rect.width.min((width - rect.x - 24).max(100));
                    if rect.y == 43 && width < 600 {
                        rect.height = 42;
                    }
                    Some(rect)
                }
                ChildPosition::Check(id, mut rect) => {
                    if child.page == Some(PAGE_TRIGGERS) && width < 600 {
                        let row = match id {
                            CHECK_GAME => 0,
                            CHECK_PING_CLICK => 1,
                            CHECK_EMOTE_CLICK => 2,
                            CHECK_ENABLED_START => 3,
                            _ => -1,
                        };
                        if row >= 0 {
                            rect.x = 28;
                            rect.y = 404 + row * 44;
                            rect.height = 40;
                        }
                    } else if child.page == Some(PAGE_PINGS) && stacked {
                        match id {
                            CHECK_MUTE => rect.y = 158,
                            CHECK_TICK => {
                                rect.x = 28;
                                rect.y = 190;
                            }
                            _ => {}
                        }
                    }
                    rect.width = rect.width.min((width - rect.x - 28).max(100));
                    Some(rect)
                }
                ChildPosition::TriggerLabel(id) => {
                    let (label_y, _) = trigger_field_y(id);
                    Some(if width < 700 {
                        LayoutRect {
                            x: 28,
                            y: label_y,
                            width: 180,
                            height: 24,
                        }
                    } else {
                        LayoutRect {
                            x: 28,
                            y: label_y,
                            width: 300,
                            height: 24,
                        }
                    })
                }
                ChildPosition::TriggerInput(id) => {
                    let (_, input_y) = trigger_field_y(id);
                    Some(if width < 700 {
                        LayoutRect {
                            x: 220,
                            y: input_y,
                            width: (width - 248).max(120),
                            height: 30,
                        }
                    } else if id == EDIT_THRESHOLD {
                        LayoutRect {
                            x: 340,
                            y: input_y,
                            width: (width - 370).clamp(80, 150),
                            height: 30,
                        }
                    } else {
                        LayoutRect {
                            x: 340,
                            y: input_y,
                            width: (width - 370).max(180),
                            height: 30,
                        }
                    })
                }
                ChildPosition::TriggerHelp => Some(LayoutRect {
                    x: 28,
                    y: if width < 600 { 594 } else { 572 },
                    width: (width - 56).max(200),
                    height: 36,
                }),
                ChildPosition::NumericLabel(id) => {
                    let index = numeric_index(id, child.page.unwrap_or(self.page)) as i32;
                    if narrow {
                        let column = numeric_column(id, child.page.unwrap_or(self.page), width);
                        Some(LayoutRect {
                            x: column.0,
                            y: 88,
                            width: column.1,
                            height: 24,
                        })
                    } else {
                        let columns = (width - 48).max(600) / 3;
                        Some(LayoutRect {
                            x: 24 + index * columns,
                            y: 88,
                            width: (columns - 16).max(120),
                            height: 24,
                        })
                    }
                }
                ChildPosition::NumericInput(id) => {
                    let index = numeric_index(id, child.page.unwrap_or(self.page)) as i32;
                    if narrow {
                        let column = numeric_column(id, child.page.unwrap_or(self.page), width);
                        Some(LayoutRect {
                            x: column.0,
                            y: 115,
                            width: column.1.min(150),
                            height: 30,
                        })
                    } else {
                        let columns = (width - 48).max(600) / 3;
                        Some(LayoutRect {
                            x: 24 + index * columns,
                            y: 115,
                            width: (columns - 16).clamp(110, 180),
                            height: 30,
                        })
                    }
                }
                ChildPosition::ClickLabel | ChildPosition::ClickCombo => {
                    let emote_page = child.page == Some(PAGE_EMOTES);
                    let label = matches!(child.position, ChildPosition::ClickLabel);
                    Some(if emote_page {
                        if stacked {
                            if label {
                                LayoutRect {
                                    x: 28,
                                    y: 232,
                                    width: 154,
                                    height: 24,
                                }
                            } else {
                                LayoutRect {
                                    x: 190,
                                    y: 227,
                                    width: (width - 220).max(140),
                                    height: 30,
                                }
                            }
                        } else if label {
                            LayoutRect {
                                x: 28,
                                y: 201,
                                width: 200,
                                height: 24,
                            }
                        } else {
                            LayoutRect {
                                x: 232,
                                y: 196,
                                width: (width - 258).max(180),
                                height: 30,
                            }
                        }
                    } else if stacked {
                        if label {
                            LayoutRect {
                                x: 28,
                                y: 266,
                                width: 154,
                                height: 24,
                            }
                        } else {
                            LayoutRect {
                                x: 190,
                                y: 261,
                                width: (width - 220).max(140),
                                height: 30,
                            }
                        }
                    } else if narrow {
                        if label {
                            LayoutRect {
                                x: 28,
                                y: 242,
                                width: 154,
                                height: 24,
                            }
                        } else {
                            LayoutRect {
                                x: 190,
                                y: 237,
                                width: (width - 220).max(180),
                                height: 30,
                            }
                        }
                    } else if label {
                        LayoutRect {
                            x: 330,
                            y: 201,
                            width: 198,
                            height: 24,
                        }
                    } else {
                        LayoutRect {
                            x: 532,
                            y: 196,
                            width: (width - 558).max(180),
                            height: 30,
                        }
                    })
                }
                ChildPosition::Preview => Some(if narrow {
                    LayoutRect {
                        x: 24,
                        y: if stacked && child.page == Some(PAGE_PINGS) {
                            310
                        } else {
                            285
                        },
                        width: 320,
                        height: 320,
                    }
                } else {
                    LayoutRect {
                        x: 24,
                        y: 285,
                        width: 360,
                        height: 355,
                    }
                }),
                ChildPosition::SlotLabel(id) | ChildPosition::SlotCombo(id) => {
                    let slot = slot_from_id(id);
                    if narrow {
                        Some(if matches!(child.position, ChildPosition::SlotLabel(_)) {
                            LayoutRect {
                                x: 24,
                                y: if stacked { 650 } else { 630 } + slot as i32 * 54,
                                width: 158,
                                height: 28,
                            }
                        } else {
                            LayoutRect {
                                x: 190,
                                y: if stacked { 645 } else { 625 } + slot as i32 * 54,
                                width: (width - 220).max(180),
                                height: 30,
                            }
                        })
                    } else {
                        let col_width = ((width - 440 - 36) / 2).max(210);
                        let x = 414 + (slot / 4) as i32 * (col_width + 18);
                        let y = 294 + (slot % 4) as i32 * 78;
                        Some(if matches!(child.position, ChildPosition::SlotLabel(_)) {
                            LayoutRect {
                                x,
                                y,
                                width: 92,
                                height: 30,
                            }
                        } else {
                            LayoutRect {
                                x: x + 94,
                                y: y - 5,
                                width: (col_width - 94).max(116),
                                height: 30,
                            }
                        })
                    }
                }
                _ => None,
            };
            let position = if let Some(rect) = content {
                LayoutRect {
                    x: rect.x,
                    y: rect.y - self.scroll_dip,
                    width: rect.width,
                    height: rect.height,
                }
            } else {
                match child.position {
                    ChildPosition::Navigation(index) => {
                        let tab_width = ((width - 40) / 3).max(100);
                        LayoutRect {
                            x: 16 + index as i32 * (tab_width + 4),
                            y: 10,
                            width: tab_width,
                            height: 36,
                        }
                    }
                    ChildPosition::FooterRule => LayoutRect {
                        x: 0,
                        y: height - FOOTER_HEIGHT,
                        width: right + 12,
                        height: 2,
                    },
                    ChildPosition::Save => LayoutRect {
                        x: right - 218,
                        y: height - 47,
                        width: 100,
                        height: 34,
                    },
                    ChildPosition::Cancel => LayoutRect {
                        x: right - 106,
                        y: height - 47,
                        width: 100,
                        height: 34,
                    },
                    _ => unreachable!(),
                }
            };
            let visible = child.page.is_none_or(|page| page == self.page);
            SetWindowPos(
                child.hwnd,
                null_mut(),
                scale_dip(position.x, self.dpi),
                scale_dip(position.y, self.dpi),
                scale_dip(position.width, self.dpi).max(1),
                scale_dip(position.height, self.dpi).max(1),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            ShowWindow(child.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
        self.invalidate_preview();
    }

    unsafe fn scroll_by(&mut self, delta_dip: i32) {
        self.scroll_dip = self.scroll_dip.saturating_add(delta_dip);
        self.layout_client();
    }

    unsafe fn show_page_from_id(&mut self, id: i32) {
        let page = match id {
            BTN_TRIGGER_PAGE => PAGE_TRIGGERS,
            BTN_PING_PAGE => PAGE_PINGS,
            _ => PAGE_EMOTES,
        };
        self.show_page(page, true);
    }

    unsafe fn invalidate_preview(&self) {
        for child in &self.children {
            if matches!(child.position, ChildPosition::Preview) {
                InvalidateRect(child.hwnd, null(), 1);
            }
        }
    }

    unsafe fn current_choice(&self, field: &Field) -> Option<Choice> {
        match &field.kind {
            FieldKind::Combo(choices) => combo_value(field.hwnd, choices),
            _ => None,
        }
    }

    unsafe fn refresh_duplicates(&mut self) {
        let assignments: Vec<(i32, Choice)> = self
            .fields
            .iter()
            .filter(|field| is_slot_id(field.id))
            .filter_map(|field| self.current_choice(field).map(|choice| (field.id, choice)))
            .collect();
        for field in self.fields.iter().filter(|field| is_slot_id(field.id)) {
            let Some(choice) = self.current_choice(field) else {
                continue;
            };
            let slot = slot_from_id(field.id);
            let duplicate = assignments.iter().any(|(id, assigned)| {
                *id != field.id && *assigned == choice && slot_page(*id) == field.page
            });
            if let Some(label) = field.label_hwnd {
                let text = if duplicate {
                    format!("{} — duplicate", field.label)
                } else {
                    field.label.clone()
                };
                let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(label, text.as_ptr());
            }
            let _ = slot;
        }
        self.invalidate_preview();
    }

    unsafe fn draw_preview(&mut self, draw: &DRAWITEMSTRUCT) {
        let kind = match draw.CtlID as i32 {
            PREVIEW_PING => WheelKind::Ping,
            PREVIEW_EMOTE => WheelKind::Emote,
            _ => return,
        };
        let bounds = draw.rcItem;
        FillRect(draw.hDC, &bounds, GetSysColorBrush(COLOR_WINDOW));
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let center = POINT {
            x: bounds.left + width / 2,
            y: bounds.top + height / 2,
        };
        let radius = (width.min(height) / 2 - scale_dip(22, self.dpi)).max(scale_dip(50, self.dpi));
        let old_font = SelectObject(
            draw.hDC,
            if self.font != 0 {
                self.font as _
            } else {
                GetStockObject(DEFAULT_GUI_FONT)
            },
        );
        let pen = CreatePen(
            PS_SOLID,
            scale_dip(1, self.dpi).max(1),
            GetSysColor(COLOR_WINDOWTEXT),
        );
        let old_pen = SelectObject(draw.hDC, pen as _);
        let null_brush = GetStockObject(windows_sys::Win32::Graphics::Gdi::NULL_BRUSH);
        let old_brush = SelectObject(draw.hDC, null_brush);
        Ellipse(
            draw.hDC,
            center.x - radius,
            center.y - radius,
            center.x + radius,
            center.y + radius,
        );
        let inner = radius / 3;
        Ellipse(
            draw.hDC,
            center.x - inner,
            center.y - inner,
            center.x + inner,
            center.y + inner,
        );
        for slot in 0..8 {
            let angle = wheel_boundary_angle_degrees(slot).to_radians();
            let inner_point = POINT {
                x: center.x + (angle.cos() * inner as f64) as i32,
                y: center.y + (angle.sin() * inner as f64) as i32,
            };
            let outer_point = POINT {
                x: center.x + (angle.cos() * radius as f64) as i32,
                y: center.y + (angle.sin() * radius as f64) as i32,
            };
            MoveToEx(draw.hDC, inner_point.x, inner_point.y, null_mut());
            LineTo(draw.hDC, outer_point.x, outer_point.y);
        }
        let duplicate_ids = self.duplicate_slot_ids(kind);
        let assigned: Vec<(String, bool)> = (0..8)
            .map(|slot| {
                let id = slot_id(kind, slot);
                let value = self
                    .fields
                    .iter()
                    .find(|field| field.id == id)
                    .and_then(|field| self.current_choice(field));
                match (kind, value) {
                    (WheelKind::Ping, Some(Choice::Ping(ping))) => {
                        (ping.asset().name.to_owned(), duplicate_ids.contains(&id))
                    }
                    (WheelKind::Emote, Some(Choice::Emote(emote))) => {
                        (emote.replace('_', " "), duplicate_ids.contains(&id))
                    }
                    _ => ("Unassigned".to_owned(), duplicate_ids.contains(&id)),
                }
            })
            .collect();
        for (slot, (name, duplicate)) in assigned.iter().enumerate() {
            let angle = (slot as f64 * 45.0 - 90.0).to_radians();
            let icon_radius = f64::from(radius) * 0.62;
            let icon_center = POINT {
                x: center.x + (angle.cos() * icon_radius) as i32,
                y: center.y + (angle.sin() * icon_radius) as i32,
            };
            let direction_radius = radius - scale_dip(18, self.dpi);
            let label_center = POINT {
                x: center.x + (angle.cos() * direction_radius as f64) as i32,
                y: center.y + (angle.sin() * direction_radius as f64) as i32,
            };
            let icon_path = match kind {
                WheelKind::Ping => {
                    let ping = self
                        .fields
                        .iter()
                        .find(|field| field.id == COMBO_PING_SLOT_BASE + slot as i32)
                        .and_then(|field| self.current_choice(field));
                    match ping {
                        Some(Choice::Ping(ping)) => format!("textures/{}.png", ping.asset().icon),
                        _ => String::new(),
                    }
                }
                WheelKind::Emote => {
                    let emote = self
                        .fields
                        .iter()
                        .find(|field| field.id == COMBO_EMOTE_SLOT_BASE + slot as i32)
                        .and_then(|field| self.current_choice(field));
                    match emote {
                        Some(Choice::Emote(emote)) => format!("emotes/{emote}.png"),
                        _ => String::new(),
                    }
                }
            };
            if !icon_path.is_empty() {
                let icon = self.preview_asset(&icon_path);
                if let Some(icon) = icon {
                    draw_preview_image(
                        draw.hDC,
                        icon.as_ref(),
                        icon_center,
                        scale_dip(40, self.dpi),
                    );
                }
            }
            let mut label = format!(
                "{}\n{}{}",
                SLOTS[slot],
                name,
                if *duplicate { " *" } else { "" }
            );
            if label.len() > 36 {
                label.truncate(36);
            }
            let text: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
            let box_width = scale_dip(88, self.dpi);
            let box_height = scale_dip(42, self.dpi);
            let mut rect = RECT {
                left: label_center.x - box_width / 2,
                top: label_center.y - box_height / 2,
                right: label_center.x + box_width / 2,
                bottom: label_center.y + box_height / 2,
            };
            windows_sys::Win32::Graphics::Gdi::DrawTextW(
                draw.hDC,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
        let center_text: Vec<u16> = "Cancel".encode_utf16().chain(std::iter::once(0)).collect();
        let mut center_rect = RECT {
            left: center.x - inner + scale_dip(3, self.dpi),
            top: center.y - scale_dip(10, self.dpi),
            right: center.x + inner - scale_dip(3, self.dpi),
            bottom: center.y + scale_dip(10, self.dpi),
        };
        windows_sys::Win32::Graphics::Gdi::DrawTextW(
            draw.hDC,
            center_text.as_ptr(),
            -1,
            &mut center_rect,
            DT_CENTER | DT_VCENTER | DT_NOPREFIX,
        );
        SelectObject(draw.hDC, old_brush);
        SelectObject(draw.hDC, old_pen);
        SelectObject(draw.hDC, old_font);
        if !pen.is_null() {
            DeleteObject(pen as _);
        }
    }

    unsafe fn preview_asset(&mut self, path: &str) -> Option<Arc<image::RgbaImage>> {
        if let Some(image) = self.preview_cache.get(path) {
            return Some(Arc::clone(image));
        }
        let image = image::load_from_memory(ASSETS.get_file(path)?.contents())
            .ok()?
            .into_rgba8();
        let image = Arc::new(image);
        self.preview_cache
            .insert(path.to_owned(), Arc::clone(&image));
        Some(image)
    }

    fn duplicate_slot_ids(&self, kind: WheelKind) -> Vec<i32> {
        let base = match kind {
            WheelKind::Ping => COMBO_PING_SLOT_BASE,
            WheelKind::Emote => COMBO_EMOTE_SLOT_BASE,
        };
        let values: Vec<Option<Choice>> = (0..8)
            .map(|slot| {
                self.fields
                    .iter()
                    .find(|field| field.id == base + slot)
                    .and_then(|field| match &field.kind {
                        FieldKind::Combo(choices) => unsafe { combo_value(field.hwnd, choices) },
                        _ => None,
                    })
            })
            .collect();
        (0..8)
            .filter(|slot| {
                values[*slot].as_ref().is_some_and(|value| {
                    values.iter().enumerate().any(|(other, candidate)| {
                        other != *slot && candidate.as_ref() == Some(value)
                    })
                })
            })
            .map(|slot| base + slot as i32)
            .collect()
    }

    unsafe fn edit_value(hwnd: HWND) -> String {
        let length = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut text = vec![0u16; length + 1];
        let read = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&text[..read]).trim().to_owned()
    }

    unsafe fn save(&mut self) {
        if self.combo_is_accepting_item() {
            return;
        }
        let settings = match self.candidate_settings() {
            Ok(settings) => settings,
            Err(error) => {
                self.show_field_error(
                    error.page,
                    error.id,
                    &format!("{} is invalid. {}", error.label, error.recovery),
                );
                return;
            }
        };
        self.refresh_duplicates();
        if let Some(issue) = settings_issue(&settings) {
            self.settings_issue_error(issue);
            return;
        }
        if let Err(error) = settings.validate() {
            return message_box(
                self.hwnd,
                &format!("Settings were rejected: {error}. Review the values and try again."),
            );
        }
        if let Err(error) = settings.save_atomic(&self.path) {
            return message_box(self.hwnd, &format!("Could not save settings: {error}"));
        }
        let raw_settings = Box::into_raw(Box::new(settings));
        if !queue_main_message(APP_SETTINGS_MESSAGE, 0, raw_settings as LPARAM) {
            drop(Box::from_raw(raw_settings));
            return message_box(
                self.hwnd,
                "Settings were saved but could not be applied until restart.",
            );
        }
        if !self.destroy_window(true) {
            message_box(
                self.hwnd,
                "The settings were saved, but the settings window could not close.",
            );
        }
    }

    unsafe fn candidate_settings(&self) -> Result<Settings, FieldError> {
        let mut settings = self.settings.clone();
        for field in &self.fields {
            let error = |recovery| FieldError {
                page: field.page,
                id: field.id,
                label: field.label.clone(),
                recovery,
            };
            match (&field.kind, field.id) {
                (FieldKind::Edit, EDIT_PING_SIZE) => {
                    settings.ping_size_px = Self::edit_value(field.hwnd)
                        .parse()
                        .map_err(|_| error("Enter a whole number from 60 to 220."))?;
                }
                (FieldKind::Edit, EDIT_EMOTE_SIZE) => {
                    settings.emote_size_px = Self::edit_value(field.hwnd)
                        .parse()
                        .map_err(|_| error("Enter a whole number from 80 to 300."))?;
                }
                (FieldKind::Edit, EDIT_DURATION) => {
                    settings.ping_duration_s = Self::edit_value(field.hwnd)
                        .parse()
                        .map_err(|_| error("Enter a number from 1 to 8 seconds."))?;
                }
                (FieldKind::Edit, EDIT_VOLUME) => {
                    settings.volume = Self::edit_value(field.hwnd)
                        .parse()
                        .map_err(|_| error("Enter a whole number from 0 to 100."))?;
                }
                (FieldKind::Edit, EDIT_THRESHOLD) => {
                    settings.drag_threshold_px = Self::edit_value(field.hwnd)
                        .parse()
                        .map_err(|_| error("Enter a whole number from 2 to 40 pixels."))?;
                }
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
                        _ => return Err(error("Choose a ping trigger from the list.")),
                    }
                }
                (FieldKind::Combo(choices), COMBO_EMOTE_TRIGGER) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Trigger(value)) => settings.emote_trigger = Some(value),
                        Some(Choice::TriggerOff) => settings.emote_trigger = None,
                        _ => return Err(error("Choose a trigger or Off.")),
                    }
                }
                (FieldKind::Combo(choices), COMBO_TOGGLE_KEY) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Key(value)) => settings.toggle_hotkey.vk = value,
                        _ => return Err(error("Choose a toggle key from the list.")),
                    }
                }
                (FieldKind::Combo(choices), COMBO_CLICK_PING) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Ping(value)) => settings.click_ping_id = value,
                        _ => return Err(error("Choose a ping from the list.")),
                    }
                }
                (FieldKind::Combo(choices), COMBO_CLICK_EMOTE) => {
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Emote(value)) => settings.click_emote_id = value,
                        _ => return Err(error("Choose an emote from the list.")),
                    }
                }
                (FieldKind::Combo(choices), id)
                    if (COMBO_PING_SLOT_BASE..COMBO_PING_SLOT_BASE + 8).contains(&id) =>
                {
                    let slot = (id - COMBO_PING_SLOT_BASE) as usize;
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Ping(value)) => settings.wheel[slot] = value,
                        _ => return Err(error("Choose a ping for this wheel direction.")),
                    }
                }
                (FieldKind::Combo(choices), id)
                    if (COMBO_EMOTE_SLOT_BASE..COMBO_EMOTE_SLOT_BASE + 8).contains(&id) =>
                {
                    let slot = (id - COMBO_EMOTE_SLOT_BASE) as usize;
                    match combo_value(field.hwnd, choices) {
                        Some(Choice::Emote(value)) => settings.emote_wheel[slot] = value,
                        _ => return Err(error("Choose an emote for this wheel direction.")),
                    }
                }
                _ => {}
            }
        }
        Ok(settings)
    }

    unsafe fn settings_issue_error(&mut self, issue: SettingsIssue) {
        let (page, id, message) = match issue {
            SettingsIssue::TriggerConflict => (
                PAGE_TRIGGERS,
                COMBO_EMOTE_TRIGGER,
                "Ping and emote triggers must be different. Choose another emote trigger or Off.",
            ),
            SettingsIssue::ToggleModifiers => (
                PAGE_TRIGGERS,
                CHECK_CTRL,
                "The toggle shortcut needs Ctrl, Alt, or Win. Select at least one modifier.",
            ),
            SettingsIssue::ToggleKeyConflict => (
                PAGE_TRIGGERS,
                COMBO_TOGGLE_KEY,
                "The toggle key conflicts with a wheel trigger. Choose another key.",
            ),
            SettingsIssue::DragThreshold => (
                PAGE_TRIGGERS,
                EDIT_THRESHOLD,
                "Drag threshold must be between 2 and 40 physical pixels.",
            ),
            SettingsIssue::PingSize => (PAGE_PINGS, EDIT_PING_SIZE, "Ping size must be from 60 to 220 pixels."),
            SettingsIssue::PingDuration => (PAGE_PINGS, EDIT_DURATION, "Ping duration must be from 1 to 8 seconds."),
            SettingsIssue::Volume => (PAGE_PINGS, EDIT_VOLUME, "Volume must be from 0 to 100."),
            SettingsIssue::EmoteSize => (PAGE_EMOTES, EDIT_EMOTE_SIZE, "Emote size must be from 80 to 300 pixels."),
            SettingsIssue::DuplicatePingSlot(slot) => (
                PAGE_PINGS,
                COMBO_PING_SLOT_BASE + slot as i32,
                "Ping wheel assignments must be distinct. Choose a different ping for this direction.",
            ),
            SettingsIssue::DuplicateEmoteSlot(slot) => (
                PAGE_EMOTES,
                COMBO_EMOTE_SLOT_BASE + slot as i32,
                "Emote wheel assignments must be distinct. Choose a different emote for this direction.",
            ),
        };
        self.show_page(page, false);
        self.reveal_field(id);
        message_box(self.hwnd, message);
    }

    unsafe fn show_field_error(&mut self, page: usize, id: i32, message: &str) {
        self.show_page(page, false);
        self.reveal_field(id);
        if let Some(field) = self.fields.iter().find(|field| field.id == id) {
            if matches!(field.kind, FieldKind::Edit) {
                SendMessageW(field.hwnd, EM_SETSEL, 0, -1);
            }
        }
        message_box(self.hwnd, message);
    }

    unsafe fn reveal_field(&mut self, id: i32) {
        let Some(field) = self.fields.iter().find(|field| field.id == id) else {
            return;
        };
        let handle = field.hwnd;
        let edit = matches!(field.kind, FieldKind::Edit);
        self.ensure_field_visible(handle);
        SetFocus(handle);
        if edit {
            SendMessageW(handle, EM_SETSEL, 0, -1);
        }
    }

    unsafe fn reveal_focused_field(&mut self, id: i32, handle: HWND) {
        if self
            .fields
            .iter()
            .any(|field| field.id == id && field.hwnd == handle)
        {
            self.ensure_field_visible(handle);
        }
    }

    unsafe fn ensure_field_visible(&mut self, handle: HWND) {
        if self.viewport.is_null() {
            return;
        }
        let mut bounds: RECT = std::mem::zeroed();
        let mut viewport: RECT = std::mem::zeroed();
        if GetWindowRect(handle, &mut bounds) == 0
            || GetClientRect(self.viewport, &mut viewport) == 0
        {
            return;
        }
        MapWindowPoints(
            null_mut(),
            self.viewport,
            (&mut bounds as *mut RECT).cast(),
            2,
        );
        let dpi = i64::from(self.dpi.max(1));
        let top = (i64::from(bounds.top) * 96).div_euclid(dpi) as i32;
        let bottom_px = i64::from(bounds.bottom) * 96;
        let bottom = (-(-bottom_px).div_euclid(dpi)) as i32;
        let viewport_height =
            (i64::from(viewport.bottom - viewport.top) * 96).div_euclid(dpi) as i32;
        let delta = reveal_scroll_delta(top, bottom, viewport_height);
        if delta != 0 {
            self.scroll_dip = self.scroll_dip.saturating_add(delta);
            self.layout_client();
        }
    }

    unsafe fn combo_is_accepting_item(&self) -> bool {
        self.fields
            .iter()
            .filter(|field| matches!(field.kind, FieldKind::Combo(_)))
            .any(|field| SendMessageW(field.hwnd, CB_GETDROPPEDSTATE, 0, 0) != 0)
    }

    unsafe fn destroy_window(&mut self, notify: bool) -> bool {
        if self.hwnd.is_null() {
            return true;
        }
        let hwnd = self.hwnd;
        self.hwnd = null_mut();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        if DestroyWindow(hwnd) == 0 {
            self.hwnd = hwnd;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, self as *mut Self as isize);
            return false;
        }
        self.fields.clear();
        self.children.clear();
        self.preview_cache.clear();
        self.viewport = null_mut();
        self.delete_font();
        if notify && !queue_main_message(APP_SETTINGS_CLOSED, 0, 0) {
            super::output_debug_string("could not queue settings resume after closing the window");
            return false;
        }
        true
    }

    unsafe fn delete_font(&mut self) {
        if self.font != 0 {
            DeleteObject(self.font as _);
            self.font = 0;
        }
    }

    unsafe fn dpi_changed(&mut self, new_dpi: u32, suggested: &RECT) {
        self.dpi = new_dpi.max(96);
        let monitor_point = POINT {
            x: suggested.left + (suggested.right - suggested.left) / 2,
            y: suggested.top + (suggested.bottom - suggested.top) / 2,
        };
        let (work, _) = work_area_at(monitor_point, self.dpi);
        let proposed = UiRect {
            left: suggested.left,
            top: suggested.top,
            right: suggested.right,
            bottom: suggested.bottom,
        };
        let rect = constrain_rect_to_work_area(work, proposed);
        let new_font = create_ui_font(self.dpi);
        for child in &self.children {
            SendMessageW(child.hwnd, WM_SETFONT, new_font as usize, 1);
        }
        let old_font = self.font;
        self.font = new_font as isize;
        SetWindowPos(
            self.hwnd,
            null_mut(),
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        if old_font != 0 {
            DeleteObject(old_font as _);
        }
        self.layout_client();
    }

    unsafe fn minimum_track_size(
        &self,
        info: &mut windows_sys::Win32::UI::WindowsAndMessaging::MINMAXINFO,
    ) {
        let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return;
        }
        let mut monitor_info: windows_sys::Win32::Graphics::Gdi::MONITORINFO = std::mem::zeroed();
        monitor_info.cbSize = std::mem::size_of_val(&monitor_info) as u32;
        if GetMonitorInfoW(monitor, &mut monitor_info) == 0 {
            return;
        }
        let work_width = monitor_info.rcWork.right - monitor_info.rcWork.left;
        let work_height = monitor_info.rcWork.bottom - monitor_info.rcWork.top;
        info.ptMinTrackSize.x = scale_dip(720, self.dpi).min(work_width).max(1);
        info.ptMinTrackSize.y = scale_dip(420, self.dpi).min(work_height).max(1);
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        let Some(permit) = SETTINGS_CALLBACK_GATE.enter() else {
            return;
        };
        if !unsafe { self.destroy_window(false) } {
            super::output_debug_string("could not destroy settings window during shutdown");
        }
        unsafe { self.delete_font() };
        drop(permit);
    }
}

pub fn register(instance: HINSTANCE) -> Result<(), String> {
    let viewport_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(settings_viewport_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: unsafe { LoadCursorW(null_mut(), w!("IDC_ARROW")) },
        hbrBackground: (COLOR_WINDOW + 1) as _,
        lpszMenuName: null(),
        lpszClassName: w!("pingloom-settings-viewport"),
    };
    if unsafe { RegisterClassW(&viewport_class) } == 0 {
        return Err("could not register the settings content viewport class".into());
    }
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
        lpszClassName: w!("pingloom-settings"),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("could not register settings window class".into());
    }
    Ok(())
}

unsafe extern "system" fn settings_viewport_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if matches!(message, WM_COMMAND | WM_DRAWITEM | WM_MOUSEWHEEL) {
        let parent = GetParent(hwnd);
        if !parent.is_null() {
            return SendMessageW(parent, message, wparam, lparam);
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if message == WM_NCCREATE {
        let create =
            &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if message == WM_GETMINMAXINFO {
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
        if !pointer.is_null() {
            (&*pointer).minimum_track_size(
                &mut *(lparam as *mut windows_sys::Win32::UI::WindowsAndMessaging::MINMAXINFO),
            );
            return 0;
        }
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if message == WM_DRAWITEM {
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
        if !pointer.is_null() {
            let draw = &*(lparam as *const DRAWITEMSTRUCT);
            if matches!(draw.CtlID as i32, PREVIEW_PING | PREVIEW_EMOTE) {
                if let Some(permit) = SETTINGS_CALLBACK_GATE.enter() {
                    (&mut *pointer).draw_preview(draw);
                    drop(permit);
                }
                return 1;
            }
        }
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let id = (wparam & 0xffff) as i32;
    let notification = (wparam >> 16) as u32;
    let command = message == WM_COMMAND
        && ((notification == BN_CLICKED
            && matches!(
                id,
                BTN_TRIGGER_PAGE | BTN_PING_PAGE | BTN_EMOTE_PAGE | BTN_SAVE | BTN_CANCEL
            ))
            || (notification == 0 && matches!(id, BTN_SAVE | BTN_CANCEL)));
    let selection_change = message == WM_COMMAND && notification == CBN_SELCHANGE && is_slot_id(id);
    let focus_change = message == WM_COMMAND
        && matches!(notification, BN_SETFOCUS | CBN_SETFOCUS | EN_SETFOCUS)
        && is_settings_field_id(id);
    if !matches!(
        message,
        WM_CLOSE | WM_SIZE | WM_DPICHANGED | WM_VSCROLL | WM_MOUSEWHEEL
    ) && !command
        && !selection_change
        && !focus_change
    {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let Some(permit) = SETTINGS_CALLBACK_GATE.enter() else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWindow;
    if pointer.is_null() {
        drop(permit);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let window = &mut *pointer;
    let result = match message {
        WM_CLOSE => {
            if !window.destroy_window(true) {
                message_box(hwnd, "The settings window could not close.");
            }
            0
        }
        WM_SIZE => {
            window.layout_client();
            0
        }
        WM_DPICHANGED => {
            let suggested = &*(lparam as *const RECT);
            window.dpi_changed((wparam & 0xffff) as u32, suggested);
            0
        }
        WM_VSCROLL => {
            let mut scroll = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_ALL,
                nMin: 0,
                nMax: 0,
                nPage: 0,
                nPos: 0,
                nTrackPos: 0,
            };
            GetScrollInfo(hwnd, SB_VERT, &mut scroll);
            let height = client_height_dip(hwnd, window.dpi);
            let page_step = (height - PAGE_TOP - FOOTER_HEIGHT - 12).max(48);
            let next = match (wparam & 0xffff) as i32 {
                SB_LINEUP => scroll.nPos - scale_dip(42, window.dpi),
                SB_LINEDOWN => scroll.nPos + scale_dip(42, window.dpi),
                SB_PAGEUP => scroll.nPos - scale_dip(page_step, window.dpi),
                SB_PAGEDOWN => scroll.nPos + scale_dip(page_step, window.dpi),
                SB_THUMBPOSITION | SB_THUMBTRACK => scroll.nTrackPos,
                SB_BOTTOM => scroll.nMax,
                SB_ENDSCROLL => scroll.nPos,
                _ => scroll.nPos,
            };
            window.scroll_dip = (next * 96 / window.dpi.max(1) as i32).max(0);
            window.layout_client();
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16) as i32;
            if delta != 0 {
                window.scroll_by(-(delta / 120) * 48);
            }
            0
        }
        WM_COMMAND if command => match id {
            BTN_TRIGGER_PAGE | BTN_PING_PAGE | BTN_EMOTE_PAGE => {
                window.show_page_from_id(id);
                0
            }
            BTN_SAVE => {
                if window.combo_is_accepting_item() {
                    0
                } else {
                    window.save();
                    0
                }
            }
            BTN_CANCEL => {
                if !window.destroy_window(true) {
                    message_box(hwnd, "The settings window could not close.");
                }
                0
            }
            _ => 0,
        },
        WM_COMMAND if selection_change => {
            window.refresh_duplicates();
            0
        }
        WM_COMMAND if focus_change => {
            window.reveal_focused_field(id, lparam as HWND);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    };
    drop(permit);
    result
}

unsafe fn create_ui_font(dpi: u32) -> windows_sys::Win32::Graphics::Gdi::HFONT {
    let name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    CreateFontW(
        -scale_dip(12, dpi),
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        windows_sys::Win32::Graphics::Gdi::DEFAULT_CHARSET as u32,
        0,
        0,
        windows_sys::Win32::Graphics::Gdi::CLEARTYPE_QUALITY as u32,
        (windows_sys::Win32::Graphics::Gdi::DEFAULT_PITCH
            | windows_sys::Win32::Graphics::Gdi::FF_DONTCARE) as u32,
        name.as_ptr(),
    )
}

unsafe fn work_area_at_cursor() -> (UiRect, u32) {
    let mut point = POINT { x: 0, y: 0 };
    GetCursorPos(&mut point);
    work_area_at(point, 96)
}

unsafe fn work_area_at(point: POINT, fallback_dpi: u32) -> (UiRect, u32) {
    let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() {
        let width = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(0);
        let height = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(1);
        return (
            UiRect {
                left: 0,
                top: 0,
                right: width.max(1),
                bottom: height.max(1),
            },
            fallback_dpi,
        );
    }
    let mut info: windows_sys::Win32::Graphics::Gdi::MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of_val(&info) as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return (
            UiRect {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            },
            fallback_dpi,
        );
    }
    let mut x = fallback_dpi;
    let mut y = fallback_dpi;
    let dpi = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) >= 0 && x > 0 {
        x
    } else {
        fallback_dpi
    };
    (
        UiRect {
            left: info.rcWork.left,
            top: info.rcWork.top,
            right: info.rcWork.right,
            bottom: info.rcWork.bottom,
        },
        dpi,
    )
}

unsafe fn client_height_dip(hwnd: HWND, dpi: u32) -> i32 {
    let mut rect: RECT = std::mem::zeroed();
    if GetClientRect(hwnd, &mut rect) == 0 {
        500
    } else {
        (rect.bottom - rect.top) * 96 / dpi.max(1) as i32
    }
}

unsafe fn draw_preview_image(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    image: &image::RgbaImage,
    center: POINT,
    size: i32,
) {
    if image.width() == 0 || image.height() == 0 || size <= 0 {
        return;
    }
    let window_color = GetSysColor(COLOR_WINDOW) as u32;
    let background = [
        window_color as u8,
        (window_color >> 8) as u8,
        (window_color >> 16) as u8,
    ];
    let mut bgra = Vec::with_capacity(image.as_raw().len());
    for pixel in image.as_raw().as_chunks::<4>().0 {
        let rgb = composite_rgba_over_rgb(*pixel, background);
        bgra.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 0]);
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: image.width() as i32,
            biHeight: -(image.height() as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: bgra.len() as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [std::mem::zeroed()],
    };
    StretchDIBits(
        dc,
        center.x - size / 2,
        center.y - size / 2,
        size,
        size,
        0,
        0,
        image.width() as i32,
        image.height() as i32,
        bgra.as_ptr().cast(),
        &info,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
}

fn trigger_field_y(id: i32) -> (i32, i32) {
    match id {
        COMBO_PING_TRIGGER => (128, 123),
        COMBO_EMOTE_TRIGGER => (174, 169),
        COMBO_TOGGLE_KEY => (311, 306),
        EDIT_THRESHOLD => (356, 351),
        _ => (0, 0),
    }
}

fn numeric_index(id: i32, page: usize) -> usize {
    match (page, id) {
        (PAGE_PINGS, EDIT_PING_SIZE) => 0,
        (PAGE_PINGS, EDIT_DURATION) => 1,
        (PAGE_PINGS, EDIT_VOLUME) => 2,
        (PAGE_EMOTES, _) => 0,
        _ => 0,
    }
}

fn numeric_column(id: i32, page: usize, width: i32) -> (i32, i32) {
    let index = numeric_index(id, page) as i32;
    let column = (width - 48).max(450) / 3;
    (24 + index * column, (column - 16).max(100))
}

fn is_settings_field_id(id: i32) -> bool {
    matches!(id, EDIT_PING_SIZE..=COMBO_CLICK_EMOTE) || is_slot_id(id)
}

fn is_slot_id(id: i32) -> bool {
    (COMBO_PING_SLOT_BASE..COMBO_PING_SLOT_BASE + 8).contains(&id)
        || (COMBO_EMOTE_SLOT_BASE..COMBO_EMOTE_SLOT_BASE + 8).contains(&id)
}

fn slot_from_id(id: i32) -> usize {
    if id >= COMBO_EMOTE_SLOT_BASE {
        (id - COMBO_EMOTE_SLOT_BASE) as usize
    } else {
        (id - COMBO_PING_SLOT_BASE) as usize
    }
}

fn slot_page(id: i32) -> usize {
    if id >= COMBO_EMOTE_SLOT_BASE {
        PAGE_EMOTES
    } else {
        PAGE_PINGS
    }
}

fn slot_id(kind: WheelKind, slot: usize) -> i32 {
    match kind {
        WheelKind::Ping => COMBO_PING_SLOT_BASE + slot as i32,
        WheelKind::Emote => COMBO_EMOTE_SLOT_BASE + slot as i32,
    }
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

unsafe fn combo_value(hwnd: HWND, choices: &[Choice]) -> Option<Choice> {
    let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0);
    (index != CB_ERR as isize)
        .then(|| choices.get(index as usize).cloned())
        .flatten()
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
    windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
        hwnd,
        text.as_ptr(),
        w!("Pingloom settings"),
        MB_OK | MB_ICONWARNING,
    );
}
