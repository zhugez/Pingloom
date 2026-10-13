use super::runtime_mailbox::{
    wait_reply, wait_save_reply, wait_suspend_reply, OwnerError, OwnerMailbox, RuntimeStatus,
};
use crate::settings_lifecycle::Lifecycle;
use lolping_core::model::{EMOTE_ASSETS, PING_ASSETS};
use lolping_core::settings::Settings;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

#[derive(Default)]
pub(crate) struct DesktopSettingsState {
    pub mailbox: Mutex<Option<Arc<OwnerMailbox>>>,
    lifecycle: Mutex<Lifecycle>,
    pub save_in_flight: Mutex<bool>,
}

struct SaveGuard<'a>(&'a Mutex<bool>);
impl Drop for SaveGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.0.lock() {
            *pending = false;
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct Rejection {
    code: String,
    message: String,
    field: Option<String>,
    request_id: Option<String>,
}
impl From<OwnerError> for Rejection {
    fn from(e: OwnerError) -> Self {
        Self {
            code: e.code.into(),
            message: e.message,
            field: e.field,
            request_id: None,
        }
    }
}
#[derive(serde::Serialize)]
pub(crate) struct Entry {
    id: String,
    name: String,
    preview_url: String,
}
#[derive(serde::Serialize)]
pub(crate) struct Catalog {
    pings: Vec<Entry>,
    emotes: Vec<Entry>,
    trigger_keys: Vec<u8>,
}
#[derive(serde::Serialize)]
pub(crate) struct Boot {
    session_id: String,
    settings: Settings,
    status: String,
    catalog: Catalog,
    load_warning: Option<String>,
}
#[derive(serde::Serialize)]
pub(crate) struct Saved {
    request_id: String,
    settings: Settings,
    status: String,
}

fn trusted<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), Rejection> {
    let url = window.url().map_err(|e| Rejection {
        code: "unauthorized".into(),
        message: e.to_string(),
        field: None,
        request_id: None,
    })?;
    if window.label() != "settings"
        || url.scheme() != "http"
        || url.host_str() != Some("tauri.localhost")
        || url.port().is_some()
        || !matches!(url.path(), "/" | "/index.html")
    {
        return Err(Rejection {
            code: "unauthorized".into(),
            message: "Settings commands are available only to the bundled Settings window".into(),
            field: None,
            request_id: None,
        });
    }
    Ok(())
}
fn status(s: RuntimeStatus) -> String {
    if s.settings_suspended {
        "settings_suspended"
    } else if s.enabled {
        "active"
    } else {
        "paused"
    }
    .into()
}
fn mailbox(state: &DesktopSettingsState) -> Result<Arc<OwnerMailbox>, Rejection> {
    state
        .mailbox
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "owner state unavailable".into(),
            field: None,
            request_id: None,
        })?
        .clone()
        .ok_or_else(|| Rejection {
            code: "unavailable".into(),
            message: "native owner unavailable".into(),
            field: None,
            request_id: None,
        })
}
fn preview_data(path: &str) -> String {
    let bytes = super::assets::ASSETS
        .get_file(path)
        .map(|file| file.contents())
        .unwrap_or(&[]);
    format!(
        "data:image/png;base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    )
}

fn catalog() -> Catalog {
    let pings = PING_ASSETS
        .iter()
        .map(|p| Entry {
            id: serde_json::to_value(p.id).unwrap().as_str().unwrap().into(),
            name: p.name.into(),
            preview_url: preview_data(&format!("textures/{}.png", p.icon)),
        })
        .collect();
    let emotes = EMOTE_ASSETS
        .iter()
        .map(|id| Entry {
            id: (*id).into(),
            name: id.replace('_', " "),
            preview_url: preview_data(&format!("emotes/{id}.png")),
        })
        .collect();
    Catalog {
        pings,
        emotes,
        trigger_keys: (1u8..=254).filter(|vk| !matches!(*vk, 0x08 | 0x09 | 0x0d | 0x10..=0x12 | 0x1b | 0x20 | 0x25..=0x28 | 0x5b | 0x5c | 0xa0..=0xa5)).collect(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn get_settings_and_catalog<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, Arc<DesktopSettingsState>>,
) -> Result<Boot, Rejection> {
    trusted(&window)?;
    let generation = state
        .lifecycle
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: None,
        })?
        .active_generation()
        .ok_or_else(|| Rejection {
            code: "stale_session".into(),
            message: "Settings window is not active".into(),
            field: None,
            request_id: None,
        })?;
    let mb = mailbox(&state)?;
    let rx = mb.query().map_err(Rejection::from)?;
    let (settings, run_status) = tauri::async_runtime::spawn_blocking(move || wait_reply(rx))
        .await
        .map_err(|e| Rejection {
            code: "unavailable".into(),
            message: e.to_string(),
            field: None,
            request_id: None,
        })?
        .map_err(Rejection::from)?;
    let session = format!(
        "{}-{}",
        std::process::id(),
        SESSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    if !state
        .lifecycle
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: None,
        })?
        .bind_session(generation, session.clone(), std::time::Instant::now())
    {
        return Err(Rejection {
            code: "stale_session".into(),
            message: "Settings session is no longer active".into(),
            field: None,
            request_id: None,
        });
    }
    Ok(Boot {
        session_id: session,
        settings,
        status: status(run_status.clone()),
        catalog: catalog(),
        load_warning: run_status.load_warning,
    })
}
static SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn check_session(state: &DesktopSettingsState, session: &str) -> Result<(), Rejection> {
    if session.len() > 64 {
        return Err(Rejection {
            code: "invalid_payload".into(),
            message: "session_id exceeds 64 bytes".into(),
            field: None,
            request_id: None,
        });
    }
    if state
        .lifecycle
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: None,
        })?
        .accept_session(session)
        .is_none()
    {
        return Err(Rejection {
            code: "stale_session".into(),
            message: "Settings session is no longer active".into(),
            field: None,
            request_id: None,
        });
    }
    Ok(())
}
#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn settings_heartbeat<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, Arc<DesktopSettingsState>>,
    session_id: String,
) -> Result<(), Rejection> {
    trusted(&window)?;
    if session_id.len() > 64 {
        return Err(Rejection {
            code: "invalid_payload".into(),
            message: "session_id exceeds 64 bytes".into(),
            field: None,
            request_id: None,
        });
    }
    if !state
        .lifecycle
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: None,
        })?
        .heartbeat(&session_id, std::time::Instant::now())
    {
        return Err(Rejection {
            code: "stale_session".into(),
            message: "Settings session is no longer active".into(),
            field: None,
            request_id: None,
        });
    }
    Ok(())
}
#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn close_settings<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, Arc<DesktopSettingsState>>,
    session_id: String,
) -> Result<(), Rejection> {
    trusted(&window)?;
    check_session(&state, &session_id)?;
    let generation = state
        .lifecycle
        .lock()
        .map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: None,
        })?
        .accept_session(&session_id)
        .ok_or_else(|| Rejection {
            code: "stale_session".into(),
            message: "Settings session is no longer active".into(),
            field: None,
            request_id: None,
        })?;
    let app = window.app_handle().clone();
    let (tx, rx) = std::sync::mpsc::channel();
    close_generation(app, Arc::clone(&state), generation, false, Some(tx))
        .map_err(Rejection::from)?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv().unwrap_or_else(|_| {
            Err(OwnerError::unavailable(
                "Settings close completion was lost",
            ))
        })
    })
    .await
    .map_err(|e| Rejection {
        code: "unavailable".into(),
        message: e.to_string(),
        field: None,
        request_id: None,
    })?
    .map_err(Rejection::from)?;
    Ok(())
}
#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn save_settings<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, Arc<DesktopSettingsState>>,
    session_id: String,
    request_id: String,
    candidate: serde_json::Value,
) -> Result<Saved, Rejection> {
    trusted(&window)?;
    check_session(&state, &session_id)?;
    if request_id.is_empty() || request_id.len() > 64 {
        return Err(Rejection {
            code: "invalid_payload".into(),
            message: "Invalid request_id".into(),
            field: None,
            request_id: Some(request_id),
        });
    }
    let encoded = serde_json::to_vec(&candidate).unwrap_or_default();
    if encoded.len() > 65536 || !strict_tree(&candidate) || !strict_keys(&candidate) {
        return Err(Rejection {
            code: "invalid_payload".into(),
            message: "Candidate must be a complete bounded Settings object with no unknown fields"
                .into(),
            field: None,
            request_id: Some(request_id),
        });
    }
    let field = candidate_field(&candidate);
    let settings: Settings = serde_json::from_value(candidate).map_err(|_| Rejection {
        code: "validation".into(),
        message: "Candidate Settings contains an invalid enum, key, or asset".into(),
        field,
        request_id: Some(request_id.clone()),
    })?;
    {
        let mut pending = state.save_in_flight.lock().map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "save state unavailable".into(),
            field: None,
            request_id: Some(request_id.clone()),
        })?;
        if *pending {
            return Err(Rejection {
                code: "busy".into(),
                message: "A save is already in progress".into(),
                field: None,
                request_id: Some(request_id),
            });
        }
        *pending = true;
    }
    let _save_guard = SaveGuard(&state.save_in_flight);
    // Lock order is lifecycle then mailbox, as elsewhere; mailbox enqueue only PostMessages and
    // never calls back into lifecycle. Drop lifecycle immediately after synchronous admission.
    let mb = mailbox(&state)?;
    let saved_settings = settings.clone();
    let rx = {
        let lifecycle = state.lifecycle.lock().map_err(|_| Rejection {
            code: "unavailable".into(),
            message: "session unavailable".into(),
            field: None,
            request_id: Some(request_id.clone()),
        })?;
        if lifecycle.accept_session(&session_id).is_none() {
            return Err(Rejection {
                code: "stale_session".into(),
                message: "Settings session is no longer active".into(),
                field: None,
                request_id: Some(request_id),
            });
        }
        mb.save(settings).map_err(|e| {
            let mut rejection = Rejection::from(e);
            rejection.request_id = Some(request_id.clone());
            rejection
        })?
    };
    let result = tauri::async_runtime::spawn_blocking(move || wait_save_reply(rx))
        .await
        .map_err(|e| Rejection {
            code: "unavailable".into(),
            message: e.to_string(),
            field: None,
            request_id: None,
        });
    let run_status = result?.map_err(|e| {
        let mut r = Rejection::from(e);
        r.request_id = Some(request_id.clone());
        r
    })?;
    Ok(Saved {
        request_id,
        settings: saved_settings,
        status: status(run_status),
    })
}
fn valid_trigger(v: &serde_json::Value) -> bool {
    if v.as_str().is_some_and(|s| {
        [
            "alt",
            "ctrl",
            "shift",
            "win",
            "caps_lock",
            "mouse4",
            "mouse5",
        ]
        .contains(&s)
    }) {
        return true;
    }
    let Some(o) = v.as_object() else { return false };
    if o.len() != 1 || !o.contains_key("key") {
        return false;
    }
    o["key"].as_object().is_some_and(|key| {
        key.len() == 1 && key.get("vk").and_then(serde_json::Value::as_u64).is_some()
    })
}
fn strict_tree(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::String(s) => s.len() <= 64,
        serde_json::Value::Array(a) => a.iter().all(strict_tree),
        serde_json::Value::Object(o) => o.iter().all(|(k, v)| k.len() <= 64 && strict_tree(v)),
        _ => true,
    }
}
fn candidate_field(v: &serde_json::Value) -> Option<String> {
    if v.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Some("version".into());
    }
    for (field, min, max) in [
        ("drag_threshold_px", 2.0, 40.0),
        ("ping_size_px", 60.0, 220.0),
        ("ping_duration_s", 1.0, 8.0),
        ("volume", 0.0, 100.0),
        ("emote_size_px", 80.0, 300.0),
    ] {
        if v.get(field)
            .and_then(serde_json::Value::as_f64)
            .is_none_or(|n| n < min || n > max || (field != "ping_duration_s" && n.fract() != 0.0))
        {
            return Some(field.into());
        }
    }
    for (field, key) in [
        ("trigger", v.pointer("/trigger/key/vk")),
        ("emote_trigger", v.pointer("/emote_trigger/key/vk")),
    ] {
        if let Some(vk) = key.and_then(serde_json::Value::as_u64) {
            if vk > 254
                || matches!(vk,0x08|0x09|0x0d|0x10..=0x12|0x1b|0x20|0x25..=0x28|0x5b|0x5c|0xa0..=0xa5)
            {
                return Some(field.into());
            }
        }
    }
    if v.pointer("/toggle_hotkey/vk")
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|n| n == 0 || n > 254 || matches!(n,0x10..=0x12|0x5b|0x5c|0xa0..=0xa5))
    {
        return Some("toggle_hotkey.vk".into());
    }
    if v.pointer("/toggle_hotkey/mods")
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|n| n > 15)
    {
        return Some("toggle_hotkey.mods".into());
    }
    let ping_ids: Vec<String> = PING_ASSETS
        .iter()
        .map(|p| serde_json::to_value(p.id).unwrap().as_str().unwrap().into())
        .collect();
    for field in ["click_ping_id"] {
        if v.get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| !ping_ids.iter().any(|valid| valid == id))
        {
            return Some(field.into());
        }
    }
    if let Some(a) = v.get("wheel").and_then(serde_json::Value::as_array) {
        for (i, id) in a.iter().enumerate() {
            if id
                .as_str()
                .is_none_or(|s| !ping_ids.iter().any(|valid| valid == s))
            {
                return Some(format!("wheel.{i}"));
            }
        }
    }
    for field in ["click_emote_id"] {
        if v.get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| !EMOTE_ASSETS.contains(&id))
        {
            return Some(field.into());
        }
    }
    if let Some(a) = v.get("emote_wheel").and_then(serde_json::Value::as_array) {
        for (i, id) in a.iter().enumerate() {
            if id.as_str().is_none_or(|s| !EMOTE_ASSETS.contains(&s)) {
                return Some(format!("emote_wheel.{i}"));
            }
        }
    }
    None
}
fn strict_keys(v: &serde_json::Value) -> bool {
    const KEYS: &[&str] = &[
        "version",
        "enabled_on_start",
        "trigger",
        "emote_trigger",
        "toggle_hotkey",
        "drag_threshold_px",
        "click_ping",
        "click_ping_id",
        "game_check",
        "wheel",
        "ping_size_px",
        "ping_duration_s",
        "volume",
        "muted",
        "tick_sound",
        "emote_click",
        "click_emote_id",
        "emote_wheel",
        "emote_size_px",
        "emote_sound",
    ];
    let Some(o) = v.as_object() else { return false };
    o.len() == KEYS.len()
        && o.keys().all(|k| KEYS.contains(&k.as_str()))
        && v.get("wheel")
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.len() == 8)
        && v.get("emote_wheel")
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.len() == 8)
        && v.get("toggle_hotkey")
            .and_then(|x| x.as_object())
            .is_some_and(|o| o.len() == 2 && o.contains_key("mods") && o.contains_key("vk"))
        && ["trigger", "emote_trigger"].iter().all(|k| {
            v.get(*k)
                .is_some_and(|x| x.is_null() && *k == "emote_trigger" || valid_trigger(x))
        })
}

pub(crate) fn install<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: Arc<DesktopSettingsState>,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_settings_and_catalog,
            save_settings,
            close_settings,
            settings_heartbeat
        ])
}

pub(crate) fn spawn_watchdog(app: AppHandle, state: Arc<DesktopSettingsState>) {
    std::thread::Builder::new()
        .name("lolping-settings-watchdog".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            drive_resume(Arc::clone(&state));
            let generation = state
                .lifecycle
                .lock()
                .ok()
                .and_then(|l| l.active_generation());
            if let Some(generation) = generation {
                if let Err(error) =
                    close_generation(app.clone(), Arc::clone(&state), generation, true, None)
                {
                    eprintln!(
                        "Could not schedule Settings watchdog close: {}",
                        error.message
                    );
                }
            }
        })
        .ok();
}

fn drive_resume(state: Arc<DesktopSettingsState>) {
    let generation = match state
        .lifecycle
        .lock()
        .ok()
        .and_then(|mut l| l.claim_resume())
    {
        Some(g) => g,
        None => return,
    };
    let mb = mailbox(&state);
    let state_done = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let acknowledged = match mb {
            Ok(mb) => mb.suspend(false).and_then(wait_suspend_reply).is_ok(),
            Err(_) => false,
        };
        if let Ok(mut l) = state_done.lifecycle.lock() {
            l.finish_resume(generation, acknowledged);
        }
    });
}

fn close_generation<R: Runtime>(
    app: AppHandle<R>,
    state: Arc<DesktopSettingsState>,
    generation: u64,
    expired_only: bool,
    completion: Option<std::sync::mpsc::Sender<Result<(), OwnerError>>>,
) -> Result<(), OwnerError> {
    let state_main = Arc::clone(&state);
    let app_main = app.clone();
    app.run_on_main_thread(move || {
        let claimed = state_main
            .lifecycle
            .lock()
            .is_ok_and(|mut l| l.claim_close(generation, std::time::Instant::now(), expired_only));
        if !claimed {
            if let Some(tx) = completion {
                let _ = tx.send(Err(OwnerError {
                    code: "busy",
                    message: "Settings is already closing or no longer active".into(),
                    field: None,
                }));
            }
            return;
        }
        let destroyed = app_main
            .get_webview_window("settings")
            .is_none_or(|w| w.destroy().is_ok());
        if let Ok(mut l) = state_main.lifecycle.lock() {
            l.finish_close(generation, destroyed, std::time::Instant::now());
            if destroyed {
                let _ = l.destroyed(generation);
            }
        }
        if destroyed {
            drive_resume(state_main);
        }
        if let Some(tx) = completion {
            let result = if destroyed {
                Ok(())
            } else {
                Err(OwnerError::unavailable("Could not destroy Settings window"))
            };
            let _ = tx.send(result);
        }
    })
    .map_err(|error| OwnerError::unavailable(format!("could not schedule Settings close: {error}")))
}

pub(crate) fn schedule_open(app: AppHandle, state: Arc<DesktopSettingsState>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }
    let generation = match state
        .lifecycle
        .lock()
        .ok()
        .and_then(|mut l| l.reserve_open())
    {
        Some(g) => g,
        None => return,
    };
    let mb = match mailbox(&state) {
        Ok(mb) => mb,
        Err(_) => {
            if let Ok(mut l) = state.lifecycle.lock() {
                l.opening_failed_before_enqueue(generation);
            }
            return;
        }
    };
    let rx = match mb.suspend(true) {
        Ok(rx) => rx,
        Err(_) => {
            if let Ok(mut l) = state.lifecycle.lock() {
                l.opening_failed_before_enqueue(generation);
            }
            return;
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        if wait_suspend_reply(rx).is_err() {
            if let Ok(mut l) = state.lifecycle.lock() {
                l.need_resume(generation);
            }
            drive_resume(state);
            return;
        }
        let app_main = app.clone();
        let state_main = Arc::clone(&state);
        if app
            .run_on_main_thread(move || {
                let still_opening = state_main
                    .lifecycle
                    .lock()
                    .is_ok_and(|l| l.is_opening(generation));
                if !still_opening {
                    return;
                }
                let effects = tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::Mica)
                    .build();
                let built = tauri::WebviewWindowBuilder::new(
                    &app_main,
                    "settings",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("Pingloom Settings")
                .inner_size(1000.0, 760.0)
                .min_inner_size(760.0, 600.0)
                .resizable(true)
                .decorations(false)
                .transparent(true)
                .effects(effects)
                .on_navigation(|url| {
                    url.scheme() == "http"
                        && url.host_str() == Some("tauri.localhost")
                        && url.port().is_none()
                })
                .build();
                if let Ok(built_window) = built {
                    let callback_state = Arc::clone(&state_main);
                    let callback_app = app_main.clone();
                    built_window.on_window_event(move |event| match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            if let Err(error) = close_generation(
                                callback_app.clone(),
                                Arc::clone(&callback_state),
                                generation,
                                false,
                                None,
                            ) {
                                eprintln!("Could not schedule Settings close: {}", error.message);
                            }
                        }
                        tauri::WindowEvent::Destroyed => {
                            let recovered = callback_state
                                .lifecycle
                                .lock()
                                .is_ok_and(|mut l| l.destroyed(generation));
                            if recovered {
                                drive_resume(Arc::clone(&callback_state));
                            }
                        }
                        _ => {}
                    });
                    let exists = app_main.get_webview_window("settings").is_some();
                    let opened = exists
                        && state_main
                            .lifecycle
                            .lock()
                            .is_ok_and(|mut l| l.opened(generation, std::time::Instant::now()));
                    if !opened {
                        if let Some(stray) = app_main.get_webview_window("settings") {
                            let _ = stray.destroy();
                        }
                        if let Ok(mut l) = state_main.lifecycle.lock() {
                            l.need_resume(generation);
                        }
                        drive_resume(state_main);
                    }
                } else {
                    if let Ok(mut l) = state_main.lifecycle.lock() {
                        l.need_resume(generation);
                    }
                    drive_resume(state_main);
                }
            })
            .is_err()
        {
            if let Ok(mut l) = state.lifecycle.lock() {
                l.need_resume(generation);
            }
            drive_resume(state);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_requires_complete_known_fields_and_exact_wheels() {
        let valid = serde_json::to_value(Settings::default()).unwrap();
        assert!(strict_keys(&valid));
        assert!(strict_tree(&valid));
        let mut unknown = valid.clone();
        unknown["extra"] = serde_json::json!(true);
        assert!(!strict_keys(&unknown));
        let mut short = valid.clone();
        short["wheel"] = serde_json::json!(["danger"]);
        assert!(!strict_keys(&short));
        let mut nested = valid;
        nested["trigger"] = serde_json::json!({"key":{"vk":81,"extra":1}});
        assert!(!strict_keys(&nested));
    }
    #[test]
    fn stale_sessions_rejected_without_an_active_session() {
        let state = DesktopSettingsState::default();
        assert_eq!(
            check_session(&state, "old").unwrap_err().code,
            "stale_session"
        );
    }
}
