mod assets;
mod audio;
mod overlay;
mod settings_window;

use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use pingloom_core::gesture::{InputDecision, ReplayInput};
use pingloom_core::model::{Action, Button, InputEvent, Point, WheelKind};
use pingloom_core::settings::Settings;
use pingloom_core::GestureEngine;
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    EqualRect, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PeekMessageW, PostMessageW,
    PostQuitMessage, PostThreadMessageW, RegisterClassW, SetForegroundWindow, SetTimer,
    SetWindowLongPtrW, SetWindowsHookExW, TrackPopupMenu, TranslateMessage, UnhookWindowsHookEx,
    GWLP_USERDATA, GWL_STYLE, HC_ACTION, HHOOK, HWND_MESSAGE, IDI_APPLICATION, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, LLMHF_INJECTED, MF_STRING, MSLLHOOKSTRUCT, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_DISPLAYCHANGE,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_NCCREATE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW, WS_CAPTION, WS_EX_TOOLWINDOW, WS_OVERLAPPED, XBUTTON1,
    XBUTTON2,
};

use overlay::{AnimatedFrame, Overlay};
use settings_window::SettingsWindow;

const SELF_MARKER: usize = 0x4c50_494e;
const TIMER_FRAME: usize = 1;
const TIMER_WATCHDOG: usize = 2;
const WM_TRAY: u32 = WM_APP + 10;
const WM_INJECT: u32 = WM_APP + 11;
const WM_ACTION: u32 = WM_APP + 12;
const WM_FRAMES: u32 = WM_APP + 13;
const APP_SETTINGS_MESSAGE: u32 = WM_APP + 14;
const APP_SETTINGS_CLOSED: u32 = WM_APP + 15;
static mut CONTROL_FONT: isize = 0;
const ID_SETTINGS: i32 = 1;
const ID_TOGGLE: i32 = 2;
const ID_QUIT: i32 = 3;

static mut APPLICATION: *mut App = null_mut();

struct QueuedAction {
    action: Action,
    selection_origin: Option<Point>,
}

struct App {
    hwnd: HWND,
    instance: HINSTANCE,
    thread_id: u32,
    mouse_hook: HHOOK,
    key_hook: HHOOK,
    engine: GestureEngine,
    overlay: Overlay,
    audio: audio::Audio,
    settings_window: SettingsWindow,
    pending_inputs: Vec<ReplayInput>,
    native_keys_down: [bool; 256],
    tray_added: bool,
}

pub fn run() -> Result<(), String> {
    let result = unsafe { run_inner() };
    if let Err(error) = &result {
        show_error(error);
    }
    result
}

fn show_error(error: &str) {
    let message: Vec<u16> = error.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            null_mut(),
            message.as_ptr(),
            w!("Pingloom"),
            windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
}

unsafe fn run_inner() -> Result<(), String> {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let instance = GetModuleHandleW(null());
    if instance.is_null() {
        return Err("could not get application module handle".into());
    }
    CONTROL_FONT = windows_sys::Win32::Graphics::Gdi::GetStockObject(
        windows_sys::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
    ) as isize;
    register_main(instance)?;
    overlay::register(instance)?;
    settings_window::register(instance)?;

    let settings_path = settings_path()?;
    let settings = match Settings::load(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            let message = format!("Settings could not be loaded ({error}). The existing file was left unchanged; defaults are active until you save settings.");
            show_error(&message);
            Settings::default()
        }
    };
    let engine = GestureEngine::new(settings).map_err(|error| error.to_string())?;
    let overlay = Overlay::new(instance)?;
    let mut app = Box::new(App {
        hwnd: null_mut(),
        instance,
        thread_id: GetCurrentThreadId(),
        mouse_hook: null_mut(),
        key_hook: null_mut(),
        settings_window: SettingsWindow::new(settings_path),
        engine,
        overlay,
        audio: audio::Audio::new(),
        pending_inputs: Vec::new(),
        native_keys_down: [false; 256],
        tray_added: false,
    });
    let app_ptr = (&mut *app) as *mut App;
    APPLICATION = app_ptr;
    PeekMessageW(&mut zeroed(), null_mut(), WM_APP, WM_APP, 0); // ensure the hook thread has a message queue
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        w!("pingloom-main"),
        w!("Pingloom"),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        null_mut(),
        instance,
        app_ptr.cast(),
    );
    if hwnd.is_null() {
        APPLICATION = null_mut();
        return Err("could not create the native application message window".into());
    }
    app.hwnd = hwnd;
    if let Err(error) = app.install_tray().and_then(|()| app.install_hooks()) {
        app.shutdown();
        APPLICATION = null_mut();
        return Err(error);
    }
    SetTimer(hwnd, TIMER_FRAME, 33, None);
    SetTimer(hwnd, TIMER_WATCHDOG, 500, None);

    let mut message = zeroed();
    loop {
        let result = GetMessageW(&mut message, null_mut(), 0, 0);
        if result <= 0 {
            if result < 0 {
                app.shutdown();
                APPLICATION = null_mut();
                return Err("Windows message loop failed".into());
            }
            break;
        }
        if message.hwnd.is_null() && message.message == WM_INJECT {
            app.drain_injections();
            continue;
        }
        if app.settings_window.hwnd().is_null()
            || windows_sys::Win32::UI::WindowsAndMessaging::IsDialogMessageW(
                app.settings_window.hwnd(),
                &message,
            ) == 0
        {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    app.shutdown();
    APPLICATION = null_mut();
    Ok(())
}

fn settings_path() -> Result<PathBuf, String> {
    let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    Ok(PathBuf::from(roaming).join("Pingloom").join("settings.json"))
}

unsafe fn register_main(instance: HINSTANCE) -> Result<(), String> {
    let class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(main_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: w!("pingloom-main"),
    };
    if RegisterClassW(&class) == 0 {
        return Err("could not register application message window".into());
    }
    Ok(())
}

impl App {
    fn settings(&self) -> &Settings {
        self.engine.settings()
    }

    unsafe fn install_tray(&mut self) -> Result<(), String> {
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
        let tip: Vec<u16> = "Pingloom — desktop pings and emotes"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
            return Err("could not add the notification-area icon".into());
        }
        self.tray_added = true;
        Ok(())
    }

    unsafe fn install_hooks(&mut self) -> Result<(), String> {
        self.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), self.instance, 0);
        if self.mouse_hook.is_null() {
            return Err(format!(
                "could not install the low-level mouse hook (Windows error {})",
                windows_sys::Win32::Foundation::GetLastError()
            ));
        }
        self.key_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), self.instance, 0);
        if self.key_hook.is_null() {
            UnhookWindowsHookEx(self.mouse_hook);
            self.mouse_hook = null_mut();
            return Err(format!(
                "could not install the low-level keyboard hook (Windows error {})",
                windows_sys::Win32::Foundation::GetLastError()
            ));
        }
        Ok(())
    }

    unsafe fn shutdown(&mut self) {
        self.overlay.wheel_close();
        self.drain_injections();
        if !self.mouse_hook.is_null() {
            UnhookWindowsHookEx(self.mouse_hook);
            self.mouse_hook = null_mut();
        }
        if !self.key_hook.is_null() {
            UnhookWindowsHookEx(self.key_hook);
            self.key_hook = null_mut();
        }
        if self.tray_added {
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.hwnd;
            data.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &data);
            self.tray_added = false;
        }
    }

    unsafe fn apply_decision(
        &mut self,
        decision: InputDecision,
        is_injected: bool,
        replay_original: Option<ReplayInput>,
    ) -> bool {
        for action in decision.actions {
            if self.update_wheel_state(action) {
                continue;
            }
            let selection_origin = match action {
                Action::Select { wheel, .. } => self
                    .overlay
                    .wheel_origin()
                    .filter(|(active_wheel, _)| *active_wheel == wheel)
                    .map(|(_, origin)| origin),
                _ => None,
            };
            let raw = Box::into_raw(Box::new(QueuedAction {
                action,
                selection_origin,
            }));
            if PostMessageW(self.hwnd, WM_ACTION, 0, raw as LPARAM) == 0 {
                drop(Box::from_raw(raw));
            }
        }
        let swallow = decision.swallow;
        let ordered_input = replay_original.is_some();
        let has_prior_injects = ordered_input && !is_injected && !self.pending_inputs.is_empty();
        self.pending_inputs.extend(decision.replay);
        if !swallow && has_prior_injects {
            if let Some(replay) = replay_original {
                self.pending_inputs.push(replay);
            }
        }
        if !self.pending_inputs.is_empty() {
            PostThreadMessageW(self.thread_id, WM_INJECT, 0, 0);
        }
        swallow || (!is_injected && has_prior_injects)
    }

    unsafe fn on_hook_event(
        &mut self,
        event: InputEvent,
        injected: bool,
        replay: ReplayInput,
    ) -> bool {
        if !injected {
            match event {
                InputEvent::KeyDown { vk, .. } => self.native_keys_down[usize::from(vk)] = true,
                InputEvent::KeyUp { vk, .. } => self.native_keys_down[usize::from(vk)] = false,
                _ => {}
            }
        }
        let decision = self.engine.on_event(event);
        self.apply_decision(decision, injected, Some(replay))
    }

    unsafe fn resync_native_keys(&mut self) {
        let settings = self.settings().clone();
        for vk in 0..=u8::MAX {
            if !super::logic::swallowed_trigger_key(&settings, vk)
                && GetAsyncKeyState(i32::from(vk)) >= 0
            {
                self.native_keys_down[usize::from(vk)] = false;
            }
        }
    }

    unsafe fn update_wheel_state(&mut self, action: Action) -> bool {
        match action {
            Action::WheelOpen { wheel, origin } => {
                self.overlay.wheel_open(wheel, origin, dpi_scale_at(origin));
            }
            Action::WheelMove { point, .. } => {
                let settings = self.settings().clone();
                if self.overlay.wheel_move(point) && settings.tick_sound {
                    let volume = if settings.muted {
                        0.0
                    } else {
                        f32::from(settings.volume) / 200.0
                    };
                    self.audio.play("sounds/button.wav", volume);
                }
            }
            Action::Cancel => self.overlay.wheel_close(),
            _ => return false,
        }
        true
    }

    unsafe fn handle_action(&mut self, queued: QueuedAction) {
        let QueuedAction {
            action,
            selection_origin,
        } = queued;
        let settings = self.settings().clone();
        let volume = if settings.muted {
            0.0
        } else {
            f32::from(settings.volume) / 100.0
        };
        match action {
            Action::WheelOpen { .. } | Action::WheelMove { .. } => {}
            Action::Select { wheel, slot, .. } => {
                if let Some(point) = selection_origin {
                    self.overlay.wheel_close_if(wheel, point);
                    match wheel {
                        WheelKind::Ping => {
                            if let Some(id) = settings.wheel.get(slot).copied() {
                                self.overlay.spawn_ping(id, point);
                                self.audio
                                    .play(format!("sounds/{}.wav", id.asset().sound), volume);
                            }
                        }
                        WheelKind::Emote => {
                            if let Some(slug) = settings.emote_wheel.get(slot) {
                                self.spawn_emote(slug, point, settings.emote_size_px);
                                if settings.emote_sound {
                                    self.audio.play(format!("emotes/{slug}.ogg"), volume);
                                }
                            }
                        }
                    }
                }
            }
            Action::Click { wheel, point } => match wheel {
                WheelKind::Ping => {
                    let id = settings.click_ping_id;
                    self.overlay.spawn_ping(id, point);
                    self.audio
                        .play(format!("sounds/{}.wav", id.asset().sound), volume);
                }
                WheelKind::Emote => {
                    let slug = settings.click_emote_id.clone();
                    self.spawn_emote(&slug, point, settings.emote_size_px);
                    if settings.emote_sound {
                        self.audio.play(format!("emotes/{slug}.ogg"), volume);
                    }
                }
            },
            Action::Cancel => self.overlay.wheel_close(),
            Action::Toggled { .. } => {}
        }
        self.overlay.tick(&settings);
    }

    unsafe fn spawn_emote(&mut self, slug: &str, point: Point, size: u16) {
        if let Some(slug) = self.overlay.spawn_emote(slug, point, size) {
            let hwnd = self.hwnd as usize;
            let slug = slug.clone();
            std::thread::Builder::new()
                .name("pingloom-emote-decode".into())
                .spawn(move || {
                    let hwnd = hwnd as HWND;
                    let Some(frames) = overlay::decode_emote(&slug) else {
                        return;
                    };
                    let raw = Box::into_raw(Box::new((slug, frames)));
                    if PostMessageW(hwnd, WM_FRAMES, 0, raw as LPARAM) == 0 {
                        drop(Box::from_raw(raw));
                    }
                })
                .ok();
        }
    }

    unsafe fn show_settings(&mut self) {
        let newly_opened = self.settings_window.hwnd().is_null();
        if newly_opened {
            let decision = self.engine.set_suspended(true);
            self.apply_decision(decision, false, None);
        }
        let settings = self.settings().clone();
        if !self
            .settings_window
            .open(self.hwnd, &settings, self.instance)
        {
            if newly_opened {
                let decision = self.engine.set_suspended(false);
                self.apply_decision(decision, false, None);
            }
            show_error("Could not create the settings window.");
        }
    }

    unsafe fn show_tray_menu(&mut self) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, ID_SETTINGS as usize, w!("Settings"));
        let toggle = if self.engine.enabled() {
            w!("Pause gestures")
        } else {
            w!("Resume gestures")
        };
        AppendMenuW(menu, MF_STRING, ID_TOGGLE as usize, toggle);
        AppendMenuW(menu, MF_STRING, ID_QUIT as usize, w!("Quit"));
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        SetForegroundWindow(self.hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            self.hwnd,
            null(),
        );
        DestroyMenu(menu);
        match selected {
            ID_SETTINGS => self.show_settings(),
            ID_TOGGLE => {
                let decision = self.engine.set_enabled(!self.engine.enabled());
                self.apply_decision(decision, false, None);
            }
            ID_QUIT => PostQuitMessage(0),
            _ => {}
        }
    }

    unsafe fn drain_injections(&mut self) {
        if self.pending_inputs.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_inputs);
        let mut inputs = Vec::with_capacity(pending.len());
        for replay in pending {
            match replay {
                ReplayInput::ButtonDown(button) | ReplayInput::ButtonUp(button) => {
                    let down = matches!(replay, ReplayInput::ButtonDown(_));
                    let (flags, mouse_data) = match button {
                        Button::Left => (
                            if down {
                                MOUSEEVENTF_LEFTDOWN
                            } else {
                                MOUSEEVENTF_LEFTUP
                            },
                            0,
                        ),
                        Button::Right => (
                            if down {
                                MOUSEEVENTF_RIGHTDOWN
                            } else {
                                MOUSEEVENTF_RIGHTUP
                            },
                            0,
                        ),
                        Button::Middle => (
                            if down {
                                MOUSEEVENTF_MIDDLEDOWN
                            } else {
                                MOUSEEVENTF_MIDDLEUP
                            },
                            0,
                        ),
                        Button::X1 => (
                            if down {
                                MOUSEEVENTF_XDOWN
                            } else {
                                MOUSEEVENTF_XUP
                            },
                            XBUTTON1,
                        ),
                        Button::X2 => (
                            if down {
                                MOUSEEVENTF_XDOWN
                            } else {
                                MOUSEEVENTF_XUP
                            },
                            XBUTTON2,
                        ),
                    };
                    inputs.push(INPUT {
                        r#type: INPUT_MOUSE,
                        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: 0,
                                dy: 0,
                                mouseData: u32::from(mouse_data),
                                dwFlags: flags,
                                time: 0,
                                dwExtraInfo: SELF_MARKER,
                            },
                        },
                    });
                }
                ReplayInput::KeyDown(vk) | ReplayInput::KeyUp(vk) => {
                    let down = matches!(replay, ReplayInput::KeyDown(_));
                    let vk = u16::from(vk);
                    let extended = matches!(
                        vk,
                        0xa3 | 0xa5 | 0x5b | 0x5c | 0x2d | 0x2e | 0x24 | 0x23 | 0x21 | 0x22 | 0x25
                            ..=0x28
                    );
                    let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
                    if extended {
                        flags |= KEYEVENTF_EXTENDEDKEY;
                    }
                    inputs.push(INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: vk,
                                wScan: MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) as u16,
                                dwFlags: flags,
                                time: 0,
                                dwExtraInfo: SELF_MARKER,
                            },
                        },
                    });
                }
            }
        }
        if !inputs.is_empty() {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            );
        }
    }

    unsafe fn handle_message(
        &mut self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
            WM_TRAY => {
                let event = lparam as u32;
                if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    self.show_tray_menu();
                } else if event == WM_LBUTTONUP {
                    self.show_settings();
                }
                Some(0)
            }
            WM_INJECT => {
                self.drain_injections();
                Some(0)
            }
            WM_ACTION => {
                let action = Box::from_raw(lparam as *mut QueuedAction);
                self.handle_action(*action);
                Some(0)
            }
            WM_FRAMES => {
                let (slug, frames) = *Box::from_raw(lparam as *mut (String, Vec<AnimatedFrame>));
                self.overlay.set_emote_frames(&slug, frames);
                Some(0)
            }
            APP_SETTINGS_CLOSED => {
                let decision = self.engine.set_suspended(false);
                self.apply_decision(decision, false, None);
                Some(0)
            }
            APP_SETTINGS_MESSAGE => {
                let settings = Box::from_raw(lparam as *mut Settings);
                match self.engine.set_settings(*settings) {
                    Ok(decision) => {
                        self.apply_decision(decision, false, None);
                        let settings = self.settings().clone();
                        self.overlay.tick(&settings);
                    }
                    Err(error) => show_error(&error.to_string()),
                }
                Some(0)
            }
            WM_TIMER if wparam == TIMER_FRAME => {
                let settings = self.settings().clone();
                self.overlay.tick(&settings);
                Some(0)
            }
            WM_DISPLAYCHANGE => {
                match Overlay::new(self.instance) {
                    Ok(overlay) => self.overlay = overlay,
                    Err(error) => show_error(&error),
                }
                Some(0)
            }
            WM_TIMER if wparam == TIMER_WATCHDOG => {
                self.engine
                    .resync_key_states(|vk| GetAsyncKeyState(i32::from(vk)) < 0);
                self.resync_native_keys();
                let decision = self.engine.tick(GetTickCount64());
                self.apply_decision(decision, false, None);
                Some(0)
            }
            WM_CLOSE | WM_DESTROY => {
                PostQuitMessage(0);
                Some(0)
            }
            WM_NCCREATE | WM_APP => None,
            _ => None,
        }
    }
}

unsafe extern "system" fn main_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let app = APPLICATION;
    if !app.is_null() {
        let app = &mut *app;
        if app.hwnd.is_null() && message == WM_NCCREATE {
            app.hwnd = hwnd;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as *mut App as isize);
        }
        if let Some(result) = app.handle_message(message, wparam, lparam) {
            return result;
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let app = APPLICATION;
    if app.is_null() {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let data = &*(lparam as *const MSLLHOOKSTRUCT);
    let injected = data.flags & LLMHF_INJECTED != 0 && data.dwExtraInfo == SELF_MARKER;
    if injected {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let point = Point::new(data.pt.x, data.pt.y);
    let time = GetTickCount64();
    let button = match wparam as u32 {
        WM_LBUTTONDOWN | WM_LBUTTONUP => Some(Button::Left),
        WM_RBUTTONDOWN | WM_RBUTTONUP => Some(Button::Right),
        WM_MBUTTONDOWN | WM_MBUTTONUP => Some(Button::Middle),
        WM_XBUTTONDOWN | WM_XBUTTONUP if (data.mouseData >> 16) as u16 == XBUTTON1 => {
            Some(Button::X1)
        }
        WM_XBUTTONDOWN | WM_XBUTTONUP if (data.mouseData >> 16) as u16 == XBUTTON2 => {
            Some(Button::X2)
        }
        _ => None,
    };
    let is_down = matches!(
        wparam as u32,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
    );
    let is_up = matches!(
        wparam as u32,
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP
    );
    let replay = button.and_then(|button| match (is_down, is_up) {
        (true, _) => Some(ReplayInput::ButtonDown(button)),
        (_, true) => Some(ReplayInput::ButtonUp(button)),
        _ => None,
    });
    let event = if wparam as u32 == WM_MOUSEMOVE {
        Some(InputEvent::PointerMove {
            point,
            time_ms: time,
            injected,
        })
    } else if let Some(button) = button {
        if is_down {
            let app = &mut *app;
            app.engine
                .resync_key_states(|vk| GetAsyncKeyState(i32::from(vk)) < 0);
            app.resync_native_keys();
            let candidate = app.engine.enabled()
                && app.settings_window.hwnd().is_null()
                && super::logic::gesture_candidate(app.settings(), button, |vk| {
                    app.native_keys_down[usize::from(vk)]
                });
            let fullscreen_app = candidate && app.settings().game_check && fullscreen_app_at(point);
            Some(InputEvent::PointerDown {
                button,
                point,
                time_ms: time,
                injected,
                fullscreen_app,
                dpi_scale: if candidate { dpi_scale_at(point) } else { 1.0 },
            })
        } else if is_up {
            Some(InputEvent::PointerUp {
                button,
                point,
                time_ms: time,
                injected,
            })
        } else {
            None
        }
    } else {
        None
    };
    let Some(event) = event else {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    };
    let app = &mut *app;
    let swallowed = if wparam as u32 == WM_MOUSEMOVE {
        let decision = app.engine.on_event(event);
        app.apply_decision(decision, injected, None)
    } else {
        app.on_hook_event(
            event,
            injected,
            replay.unwrap_or(ReplayInput::ButtonDown(Button::Left)),
        )
    };
    if swallowed {
        1
    } else {
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }
}

unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let app = APPLICATION;
    if app.is_null() {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let data = &*(lparam as *const KBDLLHOOKSTRUCT);
    let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
    let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
    if !down && !up || data.vkCode > u8::MAX as u32 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let injected = data.flags & LLKHF_INJECTED != 0 && data.dwExtraInfo == SELF_MARKER;
    if injected {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
    let vk = data.vkCode as u8;
    let event = if down {
        InputEvent::KeyDown { vk, injected }
    } else {
        InputEvent::KeyUp { vk, injected }
    };
    let replay = if down {
        ReplayInput::KeyDown(vk)
    } else {
        ReplayInput::KeyUp(vk)
    };
    if (&mut *app).on_hook_event(event, injected, replay) {
        1
    } else {
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }
}

fn dpi_scale_at(point: Point) -> f64 {
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: point.x,
                y: point.y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let (mut x, mut y) = (96, 96);
        if monitor.is_null()
            || GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) < 0
            || x == 0
        {
            1.0
        } else {
            f64::from(x) / 96.0
        }
    }
}

unsafe fn fullscreen_app_at(point: Point) -> bool {
    let foreground = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
    if foreground.is_null() {
        return false;
    }
    let mut name = [0u16; 64];
    let len = windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(
        foreground,
        name.as_mut_ptr(),
        name.len() as i32,
    );
    let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
    if ["Progman", "WorkerW", "Shell_TrayWnd"]
        .iter()
        .any(|shell| class.eq_ignore_ascii_case(shell))
    {
        return false;
    }
    let monitor = MonitorFromPoint(
        POINT {
            x: point.x,
            y: point.y,
        },
        MONITOR_DEFAULTTONEAREST,
    );
    if monitor.is_null() || MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) != monitor {
        return false;
    }
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    let mut window = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info) == 0
        || windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(foreground, &mut window) == 0
    {
        return false;
    }
    let screen = info.rcMonitor;
    let covers = window.left <= screen.left
        && window.top <= screen.top
        && window.right >= screen.right
        && window.bottom >= screen.bottom;
    if !covers {
        return false;
    }
    if EqualRect(&info.rcWork, &info.rcMonitor) != 0 {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(foreground, GWL_STYLE) as u32
            & WS_CAPTION
            == 0
    } else {
        true
    }
}
