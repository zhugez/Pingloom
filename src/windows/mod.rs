mod assets;
mod audio;
pub(crate) mod desktop_settings;
mod overlay;
pub(crate) mod runtime_mailbox;
#[allow(dead_code)]
mod settings_window;

use std::cell::RefCell;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use pingloom_core::gesture::{InputDecision, ReplayInput};
use pingloom_core::model::{Action, Button, InputEvent, Point, Trigger, WheelKind};
use pingloom_core::settings::Settings;
use pingloom_core::GestureEngine;

use super::logic::{insert_replay_batch, settings_issue, take_deferred, ReplayInsertReport};
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    EqualRect, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
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
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PeekMessageW, PostMessageW,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowsHookExW, TrackPopupMenu, TranslateMessage, UnhookWindowsHookEx, CREATESTRUCTW,
    GWLP_USERDATA, GWL_STYLE, HC_ACTION, HHOOK, HWND_MESSAGE, IDI_APPLICATION, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, LLMHF_INJECTED, MF_GRAYED, MF_STRING, MSLLHOOKSTRUCT, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_TIMER, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW, WS_CAPTION, WS_EX_TOOLWINDOW,
    WS_OVERLAPPED, XBUTTON1, XBUTTON2,
};

use overlay::{AnimatedFrame, Overlay};
use runtime_mailbox::{OwnerError, OwnerMailbox, OwnerRequest, RuntimeStatus, WM_OWNER_REQUEST};
use settings_window::SettingsWindow;
use std::sync::{mpsc, Arc};

const SELF_MARKER: usize = 0x4c50_494e;
const TIMER_FRAME: usize = 1;
const TIMER_WATCHDOG: usize = 2;
const WM_TRAY: u32 = WM_APP + 10;
const WM_INJECT: u32 = WM_APP + 11;
const WM_ACTION: u32 = WM_APP + 12;
const WM_FRAMES: u32 = WM_APP + 13;
const APP_SETTINGS_MESSAGE: u32 = WM_APP + 14;
const APP_SETTINGS_CLOSED: u32 = WM_APP + 15;
const WM_REPLAY_FAILURE: u32 = WM_APP + 16;
const WM_SETTINGS_FAILURE: u32 = WM_APP + 17;
const ID_SETTINGS: i32 = 1;
const ID_TOGGLE: i32 = 2;
const ID_QUIT: i32 = 3;

static mut APPLICATION: *mut App = null_mut();

#[derive(Clone, Copy)]
struct DeferredMainMessage {
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
}

thread_local! {
    static DEFERRED_MAIN_MESSAGES: RefCell<Vec<DeferredMainMessage>> = const { RefCell::new(Vec::new()) };
}

struct QueuedAction {
    action: Action,
    selection_origin: Option<Point>,
}

struct App {
    hwnd: HWND,
    instance: HINSTANCE,
    mouse_hook: HHOOK,
    key_hook: HHOOK,
    engine: GestureEngine,
    overlay: Overlay,
    audio: audio::Audio,
    settings_window: SettingsWindow,
    pending_inputs: Vec<ReplayInput>,
    native_keys_down: [bool; 256],
    tray_added: bool,
    shutting_down: bool,
    mailbox: Arc<OwnerMailbox>,
    settings_suspended: bool,
    load_warning: Option<String>,
    app_handle: tauri::AppHandle,
    desktop_state: Arc<desktop_settings::DesktopSettingsState>,
}

pub fn run(
    mailbox: Arc<OwnerMailbox>,
    ready: mpsc::Sender<Result<(), String>>,
    app_handle: tauri::AppHandle,
    desktop_state: Arc<desktop_settings::DesktopSettingsState>,
) -> Result<(), String> {
    let mut readiness_reported = false;
    let result = unsafe {
        run_inner(
            &mailbox,
            &ready,
            &mut readiness_reported,
            app_handle,
            desktop_state,
        )
    };
    mailbox.close_and_reject_pending();
    if let Err(error) = &result {
        if !readiness_reported {
            let _ = ready.send(Err(error.clone()));
        }
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

unsafe fn run_inner(
    mailbox: &Arc<OwnerMailbox>,
    ready: &mpsc::Sender<Result<(), String>>,
    readiness_reported: &mut bool,
    app_handle: tauri::AppHandle,
    desktop_state: Arc<desktop_settings::DesktopSettingsState>,
) -> Result<(), String> {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let instance = GetModuleHandleW(null());
    if instance.is_null() {
        return Err("could not get application module handle".into());
    }
    register_main(instance)?;
    overlay::register(instance)?;

    let settings_path = settings_path()?;
    let (settings, load_warning) = match Settings::load(&settings_path) {
        Ok(settings) => (settings, None),
        Err(error) => {
            let message = format!("Settings could not be loaded ({error}). The existing file was left unchanged; defaults are active until you save settings.");
            show_error(&message);
            (Settings::default(), Some(message))
        }
    };
    let engine = GestureEngine::new(settings).map_err(|error| error.to_string())?;
    let overlay = Overlay::new(instance)?;
    let mut app = Box::new(App {
        hwnd: null_mut(),
        instance,
        mouse_hook: null_mut(),
        key_hook: null_mut(),
        settings_window: SettingsWindow::new(settings_path),
        engine,
        overlay,
        audio: audio::Audio::new(),
        pending_inputs: Vec::new(),
        native_keys_down: [false; 256],
        tray_added: false,
        shutting_down: false,
        mailbox: Arc::clone(mailbox),
        settings_suspended: false,
        load_warning,
        app_handle,
        desktop_state,
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
        discard_deferred_main_messages();
        APPLICATION = null_mut();
        return Err(error);
    }
    SetTimer(hwnd, TIMER_FRAME, 33, None);
    SetTimer(hwnd, TIMER_WATCHDOG, 500, None);
    mailbox.publish_hwnd(hwnd);
    if let Ok(mut slot) = app.desktop_state.mailbox.lock() {
        *slot = Some(Arc::clone(mailbox));
    }
    let _ = ready.send(Ok(()));
    *readiness_reported = true;

    let mut message = zeroed();
    loop {
        let result = GetMessageW(&mut message, null_mut(), 0, 0);
        if result <= 0 {
            if result < 0 {
                app.shutdown();
                discard_deferred_main_messages();
                APPLICATION = null_mut();
                return Err("Windows message loop failed".into());
            }
            break;
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
        dispatch_deferred_main_messages();
    }
    dispatch_deferred_main_messages();
    app.shutdown();
    discard_deferred_main_messages();
    APPLICATION = null_mut();
    Ok(())
}

unsafe fn free_deferred_message(message: DeferredMainMessage) {
    match message.message {
        WM_ACTION => drop(Box::from_raw(message.lparam as *mut QueuedAction)),
        WM_FRAMES => drop(Box::from_raw(
            message.lparam as *mut (String, Vec<AnimatedFrame>),
        )),
        APP_SETTINGS_MESSAGE => drop(Box::from_raw(message.lparam as *mut Settings)),
        _ => {}
    }
}

pub(crate) fn queue_main_message(message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if !SettingsWindow::callback_busy() {
        return false;
    }
    DEFERRED_MAIN_MESSAGES.with(|queue| {
        queue.borrow_mut().push(DeferredMainMessage {
            message,
            wparam,
            lparam,
        })
    });
    true
}

unsafe fn dispatch_deferred_main_messages() {
    for _ in 0..8 {
        let messages = DEFERRED_MAIN_MESSAGES.with(|queue| take_deferred(&mut *queue.borrow_mut()));
        if messages.is_empty() {
            break;
        }
        for message in messages {
            match message.message {
                WM_OWNER_REQUEST => {
                    let app = APPLICATION;
                    if !app.is_null() { (&mut *app).process_owner_requests(); }
                }
                APP_SETTINGS_MESSAGE => apply_settings_message(message.lparam),
                WM_DISPLAYCHANGE => handle_display_change(),
                WM_REPLAY_FAILURE => show_error("Windows could not replay one or more inputs. Gestures have been paused; use the tray to resume."),
                WM_SETTINGS_FAILURE => show_error("Could not create the settings window."),
                WM_INJECT | WM_ACTION | WM_FRAMES | APP_SETTINGS_CLOSED => {
                    let app = APPLICATION;
                    if app.is_null() {
                        free_deferred_message(message);
                    } else {
                        let _ = (&mut *app).handle_message(
                            message.message,
                            message.wparam,
                            message.lparam,
                        );
                    }
                }
                _ => free_deferred_message(message),
            }
        }
    }
}

unsafe fn discard_deferred_main_messages() {
    let messages = DEFERRED_MAIN_MESSAGES.with(|queue| take_deferred(&mut *queue.borrow_mut()));
    if !messages.is_empty() {
        output_debug_string(&format!(
            "discarding {} deferred UI messages during shutdown",
            messages.len()
        ));
    }
    for message in messages {
        free_deferred_message(message);
    }
}

unsafe fn defer_during_settings_callback(message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if !SettingsWindow::callback_busy() {
        return false;
    }
    match message {
        WM_INJECT | WM_ACTION | WM_FRAMES | APP_SETTINGS_MESSAGE | APP_SETTINGS_CLOSED
        | WM_REPLAY_FAILURE | WM_SETTINGS_FAILURE | WM_DISPLAYCHANGE => {
            DEFERRED_MAIN_MESSAGES.with(|queue| {
                queue.borrow_mut().push(DeferredMainMessage {
                    message,
                    wparam,
                    lparam,
                })
            });
        }
        WM_TIMER | WM_TRAY | WM_OWNER_REQUEST => {}
        _ => return false,
    }
    true
}

fn safe_custom_trigger(trigger: Trigger) -> bool {
    let Trigger::Key { vk } = trigger else {
        return true;
    };
    (1..=254).contains(&vk)
        && !matches!(vk, 0x08 | 0x09 | 0x0d | 0x10..=0x12 | 0x1b | 0x20 | 0x25..=0x28 | 0x5b | 0x5c | 0xa0..=0xa5)
}

fn settings_path() -> Result<PathBuf, String> {
    let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    Ok(pingloom_core::settings::resolve_settings_path(roaming))
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
        self.update_tray_status();
        Ok(())
    }

    unsafe fn update_tray_status(&mut self) {
        if !self.tray_added {
            return;
        }
        let state = if self.settings_suspended {
            "Settings open — gestures paused"
        } else if self.engine.enabled() {
            "Gestures active"
        } else {
            "Gestures paused"
        };
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.hwnd;
        data.uID = 1;
        data.uFlags = NIF_TIP;
        let tip: Vec<u16> = format!("Pingloom — {state}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let length = tip.len().min(data.szTip.len());
        data.szTip[..length].copy_from_slice(&tip[..length]);
        Shell_NotifyIconW(NIM_MODIFY, &data);
    }

    unsafe fn set_gestures_enabled(&mut self, enabled: bool) {
        let decision = self.engine.set_enabled(enabled);
        self.apply_decision(decision, false, None);
        self.update_tray_status();
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
        self.shutting_down = true;
        let decision = self.engine.set_suspended(true);
        self.apply_decision(decision, false, None);
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
        if !self.pending_inputs.is_empty() && PostMessageW(self.hwnd, WM_INJECT, 0, 0) == 0 {
            self.drain_injections();
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
            Action::Toggled { .. } => self.update_tray_status(),
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
        desktop_settings::schedule_open(self.app_handle.clone(), Arc::clone(&self.desktop_state));
    }

    unsafe fn drain_injections(&mut self) {
        if self.pending_inputs.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_inputs);
        let report = insert_replay_batch(&pending, |replay| {
            let inputs = replay
                .iter()
                .copied()
                .map(|input| unsafe { native_input(input) })
                .collect::<Vec<_>>();
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            ) as usize
        });
        if !report.complete || !report.balance_complete {
            self.replay_failed(pending.len(), report);
        }
    }

    unsafe fn replay_failed(&mut self, requested: usize, report: ReplayInsertReport) {
        let message = format!(
            "SendInput accepted {} of {requested} replay events after {} attempts; {} of {} balancing releases were accepted after {} attempts.",
            report.inserted,
            report.attempts,
            report.balance_inserted,
            report.balance_attempted.len(),
            report.balance_attempts,
        );
        self.set_gestures_enabled(false);
        if self.shutting_down {
            output_debug_string(&message);
            return;
        }
        if PostMessageW(self.hwnd, WM_REPLAY_FAILURE, 0, 0) == 0 {
            output_debug_string(&message);
        }
    }

    fn owner_status(&self) -> RuntimeStatus {
        RuntimeStatus {
            enabled: self.engine.enabled(),
            settings_suspended: self.settings_suspended,
            load_warning: self.load_warning.clone(),
        }
    }

    unsafe fn process_owner_requests(&mut self) {
        for request in self.mailbox.take_all() {
            match request {
                OwnerRequest::Query(reply) => {
                    let _ = reply.send(Ok((self.settings().clone(), self.owner_status())));
                }
                OwnerRequest::Suspend(suspended, reply) => {
                    if self.settings_suspended != suspended {
                        let decision = self.engine.set_suspended(suspended);
                        self.apply_decision(decision, false, None);
                        if suspended {
                            self.drain_injections();
                        }
                        self.settings_suspended = suspended;
                        self.update_tray_status();
                    }
                    let _ = reply.send(Ok(self.owner_status()));
                }
                OwnerRequest::Save(settings, reply) => {
                    let settings = *settings;
                    let result = (|| {
                        if let Some(issue) = settings_issue(&settings) {
                            let field = match issue {
                                super::logic::SettingsIssue::TriggerConflict => "trigger".into(),
                                super::logic::SettingsIssue::ToggleModifiers => {
                                    "toggle_hotkey.mods".into()
                                }
                                super::logic::SettingsIssue::ToggleKeyConflict => {
                                    "toggle_hotkey.vk".into()
                                }
                                super::logic::SettingsIssue::DragThreshold => {
                                    "drag_threshold_px".into()
                                }
                                super::logic::SettingsIssue::PingSize => "ping_size_px".into(),
                                super::logic::SettingsIssue::PingDuration => {
                                    "ping_duration_s".into()
                                }
                                super::logic::SettingsIssue::Volume => "volume".into(),
                                super::logic::SettingsIssue::EmoteSize => "emote_size_px".into(),
                                super::logic::SettingsIssue::DuplicatePingSlot(slot) => {
                                    format!("wheel.{slot}")
                                }
                                super::logic::SettingsIssue::DuplicateEmoteSlot(slot) => {
                                    format!("emote_wheel.{slot}")
                                }
                            };
                            return Err(OwnerError {
                                code: "validation",
                                message: format!("Settings conflict: {issue:?}"),
                                field: Some(field),
                            });
                        }
                        settings.validate().map_err(|error| OwnerError {
                            code: "validation",
                            message: error.to_string(),
                            field: Some(
                                if settings.version != 1 {
                                    "version"
                                } else if matches!(settings.trigger, Trigger::Key { .. })
                                    && !safe_custom_trigger(settings.trigger)
                                {
                                    "trigger"
                                } else if settings.emote_trigger.is_some_and(|t| {
                                    matches!(t, Trigger::Key { .. }) && !safe_custom_trigger(t)
                                }) {
                                    "emote_trigger"
                                } else {
                                    "toggle_hotkey.vk"
                                }
                                .into(),
                            ),
                        })?;
                        settings
                            .save_atomic(settings_path().map_err(OwnerError::unavailable)?)
                            .map_err(|error| OwnerError {
                                code: "io",
                                message: error.to_string(),
                                field: None,
                            })?;
                        let decision =
                            self.engine
                                .set_settings(settings)
                                .map_err(|error| OwnerError {
                                    code: "validation",
                                    message: error.to_string(),
                                    field: None,
                                })?;
                        self.apply_decision(decision, false, None);
                        self.drain_injections();
                        self.load_warning = None;
                        let applied_settings = self.settings().clone();
                        self.overlay.tick(&applied_settings);
                        self.update_tray_status();
                        Ok(self.owner_status())
                    })();
                    let _ = reply.send(result);
                }
                OwnerRequest::Quit(reply) => {
                    let decision = self.engine.set_suspended(true);
                    self.apply_decision(decision, false, None);
                    self.drain_injections();
                    let _ = reply.send(Ok(()));
                    PostQuitMessage(0);
                }
            }
        }
    }

    unsafe fn handle_message(
        &mut self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
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
            // Dormant native Settings notifications are untagged; only desktop lifecycle ACKs may resume hooks.
            APP_SETTINGS_CLOSED => Some(0),
            WM_TIMER if wparam == TIMER_FRAME => {
                let settings = self.settings().clone();
                self.overlay.tick(&settings);
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
            _ => None,
        }
    }
}

unsafe fn native_input(replay: ReplayInput) -> INPUT {
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
            INPUT {
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
            }
        }
        ReplayInput::KeyDown(vk) | ReplayInput::KeyUp(vk) => {
            let down = matches!(replay, ReplayInput::KeyDown(_));
            let vk = u16::from(vk);
            let extended = matches!(
                vk,
                0xa3 | 0xa5 | 0x5b | 0x5c | 0x2d | 0x2e | 0x24 | 0x23 | 0x21 | 0x22 | 0x25..=0x28
            );
            let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
            if extended {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            INPUT {
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
            }
        }
    }
}

pub(super) fn output_debug_string(message: &str) {
    let text: Vec<u16> = format!("Pingloom input replay: {message}\0")
        .encode_utf16()
        .collect();
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringW(text.as_ptr());
    }
}

unsafe fn handle_tray_message(event: u32) {
    let app = APPLICATION;
    if app.is_null() {
        return;
    }
    if event == WM_LBUTTONUP {
        (&mut *app).show_settings();
        return;
    }
    if event != WM_RBUTTONUP && event != WM_CONTEXTMENU {
        return;
    }

    let hwnd = (*app).hwnd;
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    AppendMenuW(menu, MF_STRING, ID_SETTINGS as usize, w!("Settings"));
    let settings_open = (*app).settings_suspended;
    let status = if settings_open {
        w!("Gestures paused while Settings is open")
    } else if (*app).engine.enabled() {
        w!("Gestures active")
    } else {
        w!("Gestures paused")
    };
    AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, status);
    let toggle = if settings_open && (*app).engine.enabled() {
        w!("Disable gestures when Settings closes")
    } else if settings_open {
        w!("Enable gestures when Settings closes")
    } else if (*app).engine.enabled() {
        w!("Pause gestures")
    } else {
        w!("Resume gestures")
    };
    AppendMenuW(menu, MF_STRING, ID_TOGGLE as usize, toggle);
    AppendMenuW(menu, MF_STRING, ID_QUIT as usize, w!("Quit"));
    let mut point = POINT { x: 0, y: 0 };
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    let selected = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    );
    DestroyMenu(menu);
    match selected {
        ID_SETTINGS => (&mut *app).show_settings(),
        ID_TOGGLE => {
            let app = &mut *app;
            let enabled = !app.engine.enabled();
            app.set_gestures_enabled(enabled);
        }
        ID_QUIT => {
            let _ = (*app).mailbox.quit();
        }
        _ => {}
    }
}

unsafe fn apply_settings_message(lparam: LPARAM) {
    let settings = *Box::from_raw(lparam as *mut Settings);
    let app = APPLICATION;
    if app.is_null() {
        return;
    }
    let error = {
        let app = &mut *app;
        match app.engine.set_settings(settings) {
            Ok(decision) => {
                app.apply_decision(decision, false, None);
                let settings = app.settings().clone();
                app.overlay.tick(&settings);
                None
            }
            Err(error) => Some(error.to_string()),
        }
    };
    if let Some(error) = error {
        show_error(&error);
    }
}

unsafe fn handle_display_change() {
    let app = APPLICATION;
    if app.is_null() {
        return;
    }
    let instance = (*app).instance;
    match Overlay::new(instance) {
        Ok(overlay) => (*app).overlay = overlay,
        Err(error) => show_error(&error),
    }
}

unsafe extern "system" fn main_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if message == WM_CLOSE || message == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }
    if SettingsWindow::callback_busy() {
        if defer_during_settings_callback(message, wparam, lparam) {
            return 0;
        }
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    match message {
        WM_TRAY => {
            handle_tray_message(lparam as u32);
            0
        }
        WM_OWNER_REQUEST => {
            let app = APPLICATION;
            if !app.is_null() {
                (&mut *app).process_owner_requests();
            }
            0
        }
        APP_SETTINGS_MESSAGE => {
            apply_settings_message(lparam);
            0
        }
        WM_DISPLAYCHANGE => {
            handle_display_change();
            0
        }
        WM_REPLAY_FAILURE => {
            show_error("Windows could not replay one or more inputs. Gestures have been paused; use the tray to resume.");
            0
        }
        WM_SETTINGS_FAILURE => {
            show_error("Could not create the settings window.");
            0
        }
        _ if matches!(
            message,
            WM_INJECT | WM_ACTION | WM_FRAMES | APP_SETTINGS_CLOSED | WM_OWNER_REQUEST
        ) || message == WM_TIMER && matches!(wparam, TIMER_FRAME | TIMER_WATCHDOG) =>
        {
            let app = APPLICATION;
            if app.is_null() {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let result = (&mut *app).handle_message(message, wparam, lparam);
            result.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if SettingsWindow::callback_busy() {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
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
    if SettingsWindow::callback_busy() {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }
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
