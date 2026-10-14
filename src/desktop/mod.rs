//! Native Linux/X11 and macOS adapters. Windows keeps its existing owner and renderer.
mod input;
mod paint;
mod render;
use crate::assets;
#[path = "../windows/audio.rs"]
mod audio;
#[path = "../windows/desktop_settings.rs"]
pub(crate) mod desktop_settings;
#[path = "../windows/runtime_mailbox.rs"]
pub(crate) mod runtime_mailbox;

use pingloom_core::gesture::InputDecision;
use pingloom_core::{Action, GestureEngine, InputEvent, Point, Settings};
use runtime_mailbox::{OwnerError, OwnerMailbox, OwnerRequest, RuntimeStatus};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[derive(Clone, Copy)]
struct QueuedAction {
    action: Action,
    origin: Option<Point>,
    scale: f64,
}
struct OwnerState {
    engine: GestureEngine,
    suspended: bool,
    ready: bool,
    warning: Option<String>,
    origin: Option<Point>,
    scale: f64,
}
struct Shared {
    state: Mutex<OwnerState>,
    actions: mpsc::SyncSender<QueuedAction>,
    running: AtomicBool,
    overflow: AtomicBool,
    clock: Instant,
    monitors: Vec<(i32, i32, u32, u32, f64)>,
}
impl Shared {
    fn status(state: &OwnerState) -> RuntimeStatus {
        RuntimeStatus {
            enabled: state.ready && state.engine.enabled(),
            settings_suspended: state.suspended,
            load_warning: state.warning.clone(),
        }
    }
    fn decision(&self, state: &mut OwnerState, decision: InputDecision) -> bool {
        let replay = crate::logic::insert_replay_batch(&decision.replay, input::replay);
        if !replay.complete || !replay.balance_complete {
            state.warning = Some("Input replay failed; gestures paused. Close and restart Pingloom after checking input permissions.".into());
            let recovery = state.engine.set_enabled(false);
            let _ = crate::logic::insert_replay_batch(&recovery.replay, input::replay);
            state.origin = None;
            self.overflow.store(true, Ordering::Release);
            return false;
        }
        for action in decision.actions {
            let origin = match action {
                Action::WheelOpen { origin, .. } => {
                    state.origin = Some(origin);
                    Some(origin)
                }
                Action::Select { .. } | Action::Cancel => state.origin.take(),
                _ => state.origin,
            };
            if self
                .actions
                .try_send(QueuedAction {
                    action,
                    origin,
                    scale: state.scale,
                })
                .is_err()
            {
                self.overflow.store(true, Ordering::Release);
            }
        }
        decision.swallow
    }
    fn input(&self, event: InputEvent) -> bool {
        if !self.running.load(Ordering::Acquire) {
            return false;
        }
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if let InputEvent::PointerDown { dpi_scale, .. } = event {
            state.scale = dpi_scale.clamp(0.5, 4.0);
        }
        let decision = state.engine.on_event(event);
        self.decision(&mut state, decision)
    }
    fn ready(&self) {
        let mut state = self.state.lock().expect("native owner poisoned");
        state.ready = true;
        let enabled = state.engine.settings().enabled_on_start;
        let decision = state.engine.set_enabled(enabled);
        self.decision(&mut state, decision);
    }
    fn fail(&self, warning: String) {
        let mut state = self.state.lock().expect("native owner poisoned");
        state.ready = false;
        state.warning = Some(warning);
        let decision = state.engine.set_enabled(false);
        self.decision(&mut state, decision);
    }
    fn scale_at(&self, point: Point) -> f64 {
        if cfg!(target_os = "macos") {
            return 1.0;
        }
        self.monitors
            .iter()
            .find(|(x, y, w, h, _)| {
                point.x >= *x
                    && point.y >= *y
                    && point.x < *x + *w as i32
                    && point.y < *y + *h as i32
            })
            .map_or(1.0, |(_, _, _, _, scale)| *scale)
    }
    fn time_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mailbox = OwnerMailbox::new();
    let desktop = Arc::new(desktop_settings::DesktopSettingsState::default());
    *desktop.mailbox.lock().unwrap() = Some(Arc::clone(&mailbox));
    let stopping = Arc::new(AtomicBool::new(false));
    let stop_setup = Arc::clone(&stopping);
    let state_setup = Arc::clone(&desktop);
    let mailbox_setup = Arc::clone(&mailbox);
    let app = desktop_settings::install(tauri::Builder::default(), desktop)
        .setup(move |app| {
            let path = app.path().config_dir()?.join("pingloom").join("settings.json");
            let (settings, warning) = match Settings::load(&path) {
                Ok(settings) => (settings, None),
                Err(error) => (Settings::default(), Some(format!("Could not load settings; existing file was preserved: {error}"))),
            };
            let (tx, rx) = mpsc::sync_channel(96);
            let mut engine = GestureEngine::new(settings)?;
            engine.set_enabled(false);
            let shared = Arc::new(Shared { state: Mutex::new(OwnerState { engine, suspended: false, ready: false, warning, origin: None, scale: 1.0 }), actions: tx, running: AtomicBool::new(true), overflow: AtomicBool::new(false), clock: Instant::now(), monitors: app.available_monitors()?.into_iter().map(|m| (m.position().x, m.position().y, m.size().width, m.size().height, m.scale_factor())).collect() });
            let painter = paint::Painter::new(app.handle())?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let toggle = CheckMenuItem::with_id(app, "toggle", "Gestures enabled", true, false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &toggle, &quit])?;
            let handle = app.handle().clone();
            let event_shared = Arc::clone(&shared);
            let event_state = Arc::clone(&state_setup);
            let event_mailbox = Arc::clone(&mailbox_setup);
            TrayIconBuilder::with_id("pingloom")
                .icon(app.default_window_icon().ok_or("bundled tray icon missing")?.clone())
                .tooltip("Pingloom — desktop pings and emotes")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "settings" => desktop_settings::schedule_open(app.clone(), Arc::clone(&event_state)),
                    "toggle" => {
                        let mut state = event_shared.state.lock().unwrap();
                        let enabled = state.ready && !state.engine.enabled();
                        let decision = state.engine.set_enabled(enabled);
                        event_shared.decision(&mut state, decision);
                    }
                    "quit" => { let _ = event_mailbox.quit(); }
                    _ => {}
                }).build(app)?;
            mailbox_setup.publish_owner(1);
            let hook_shared = Arc::clone(&shared);
            std::thread::Builder::new().name("pingloom-input".into()).spawn(move || {
                if let Err(error) = input::run(Arc::clone(&hook_shared)) { hook_shared.fail(error); }
            })?;
            let mailbox_owner = Arc::clone(&mailbox_setup);
            let stop_owner = Arc::clone(&stop_setup);
            std::thread::Builder::new().name("pingloom-native-owner".into()).spawn(move || {
                let audio = audio::Audio::new();
                let mut renderer = render::Renderer::new();
                let mut last_enabled = false;
                while shared.running.load(Ordering::Acquire) {
                    for request in mailbox_owner.take_all() {
                        let mut state = shared.state.lock().unwrap();
                        match request {
                            OwnerRequest::Query(reply) => { let _ = reply.send(Ok((state.engine.settings().clone(), Shared::status(&state)))); }
                            OwnerRequest::Suspend(value, reply) => {
                                let decision = state.engine.set_suspended(value);
                                shared.decision(&mut state, decision);
                                state.suspended = value;
                                let _ = reply.send(Ok(Shared::status(&state)));
                            }
                            OwnerRequest::Save(settings, reply) => {
                                drop(state);
                                let persisted = input::validate(&settings).and_then(|()| settings.save_atomic(&path).map_err(|e| OwnerError::unavailable(e.to_string())));
                                let mut state = shared.state.lock().unwrap();
                                let result = persisted.and_then(|()| state.engine.set_settings(*settings).map_err(|e| OwnerError::unavailable(e.to_string())));
                                let response = match result { Ok(decision) => { shared.decision(&mut state, decision); Ok(Shared::status(&state)) }, Err(error) => Err(error) };
                                let _ = reply.send(response);
                            }
                            OwnerRequest::Quit(reply) => {
                                let decision = state.engine.set_suspended(true);
                                shared.decision(&mut state, decision);
                                shared.running.store(false, Ordering::Release);
                                stop_owner.store(true, Ordering::Release);
                                let _ = reply.send(Ok(()));
                            }
                        }
                    }
                    if shared.overflow.swap(false, Ordering::AcqRel) || painter.failed() {
                        shared.fail("Native renderer/input queue unavailable; gestures paused for input safety.".into());
                        while rx.try_recv().is_ok() {}
                        renderer.cancel();
                    }
                    let (settings, enabled) = {
                        let mut state = shared.state.lock().unwrap();
                        let decision = state.engine.tick(shared.time_ms());
                        shared.decision(&mut state, decision);
                        (state.engine.settings().clone(), state.ready && state.engine.enabled())
                    };
                    if enabled != last_enabled { let _ = toggle.set_checked(enabled); last_enabled = enabled; }
                    renderer.reduced = painter.reduced_motion();
                    while let Ok(action) = rx.try_recv() { renderer.action(action, &settings, &audio); }
                    painter.present(renderer.frame(&settings));
                    std::thread::sleep(Duration::from_millis(33));
                }
                mailbox_owner.close_and_reject_pending();
                painter.present(None);
                handle.exit(0);
            })?;
            desktop_settings::spawn_watchdog(app.handle().clone(), Arc::clone(&state_setup));
            if std::env::args().any(|arg| arg == "--settings") {
                desktop_settings::schedule_open(app.handle().clone(), Arc::clone(&state_setup));
            }
            Ok(())
        }).build(tauri::generate_context!())?;
    app.run(move |_app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !stopping.load(Ordering::Acquire) {
                api.prevent_exit();
                let _ = mailbox.quit();
            }
        }
    });
    Ok(())
}
