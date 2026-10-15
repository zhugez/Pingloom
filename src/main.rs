#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "windows/logic.rs"]
pub mod logic;

#[cfg(feature = "desktop")]
mod animation;
#[cfg(feature = "desktop")]
#[path = "windows/assets.rs"]
mod assets;
#[cfg(feature = "desktop")]
mod custom_emotes;
#[cfg(feature = "desktop")]
mod raster;
#[cfg(any(feature = "desktop", test))]
mod single_instance;
#[cfg(feature = "desktop")]
mod wheel;

#[cfg(any(feature = "desktop", test))]
#[path = "windows/settings_lifecycle.rs"]
mod settings_lifecycle;

#[cfg(all(windows, feature = "desktop"))]
mod windows;

#[cfg(all(not(windows), feature = "desktop"))]
mod desktop;

#[cfg(all(windows, feature = "desktop"))]
fn main() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    };
    use std::time::Duration;
    use tauri::Manager;
    use windows::desktop_settings::{install, DesktopSettingsState};
    use windows::runtime_mailbox::OwnerMailbox;

    let mailbox = OwnerMailbox::new();
    let desktop_state = Arc::new(DesktopSettingsState::default());
    let owner_thread = Arc::new(Mutex::new(None));
    let owner_done = Arc::new(AtomicBool::new(false));
    let mailbox_setup = Arc::clone(&mailbox);
    let thread_setup = Arc::clone(&owner_thread);
    let state_setup = Arc::clone(&desktop_state);
    let done_setup = Arc::clone(&owner_done);
    let result = install(tauri::Builder::default(), Arc::clone(&desktop_state))
        .setup(move |app| {
            let roaming = std::env::var_os("APPDATA").ok_or("APPDATA unavailable")?;
            let path = pingloom_core::settings::resolve_settings_path(roaming);
            custom_emotes::initialize(&path);
            app.manage(single_instance::acquire(
                path.parent().ok_or("settings directory unavailable")?,
            )?);
            let (ready_tx, ready_rx) = mpsc::channel();
            let (finished_tx, finished_rx) = mpsc::channel();
            let owner_mailbox = Arc::clone(&mailbox_setup);
            let app_handle = app.handle().clone();
            let owner_state = Arc::clone(&state_setup);
            let join = std::thread::Builder::new()
                .name("pingloom-win32-owner".into())
                .spawn(move || {
                    let result =
                        windows::run(owner_mailbox, ready_tx, app_handle.clone(), owner_state);
                    let _ = finished_tx.send(result.clone());
                    app_handle.exit(if result.is_ok() { 0 } else { 1 });
                })?;
            *thread_setup
                .lock()
                .map_err(|_| "owner thread lock poisoned")? = Some(join);
            match ready_rx.recv_timeout(Duration::from_secs(30)) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error.into()),
                Err(error) => return Err(format!("native owner startup failed: {error}").into()),
            }
            windows::desktop_settings::spawn_watchdog(
                app.handle().clone(),
                Arc::clone(&state_setup),
            );
            // A completed owner signals Tauri to exit even when shutdown originated in its tray.
            let app_handle = app.handle().clone();
            let owner_done = Arc::clone(&done_setup);
            std::thread::Builder::new()
                .name("pingloom-owner-watch".into())
                .spawn(move || {
                    let _ = finished_rx.recv();
                    owner_done.store(true, Ordering::Release);
                    app_handle.exit(0);
                })?;
            Ok(())
        })
        .build(tauri::generate_context!());
    match result {
        Ok(app) => {
            app.run(move |_handle, event| {
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    if owner_done.load(Ordering::Acquire) {
                        if let Ok(mut slot) = owner_thread.lock() {
                            if let Some(join) = slot.take() {
                                let _ = join.join();
                            }
                        }
                    } else {
                        // Losing the last Settings window must not terminate the tray application.
                        api.prevent_exit();
                    }
                }
            });
        }
        Err(error) => eprintln!("Pingloom could not start: {error}"),
    }
}

#[cfg(all(not(windows), feature = "desktop"))]
fn main() {
    if let Err(error) = desktop::run() {
        eprintln!("Pingloom could not start: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(feature = "desktop"))]
fn main() {
    eprintln!("Build Pingloom with its default desktop feature to run the application.");
    std::process::exit(1);
}
