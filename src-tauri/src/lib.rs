mod aggregate;
mod alerts;
mod commands;
mod creds;
mod ingest;
mod official;
mod pricing;
mod store;
mod tray;
mod types;

use aggregate::compute_status;
use std::sync::Mutex;
use store::Store;
use types::{Status, TierInfo};
use tauri::{Emitter, Manager, WindowEvent};

pub struct AppState {
    pub store: Store,
    pub tier: Mutex<TierInfo>,
    pub alerts: Mutex<alerts::AlertState>,
    pub last_status: Mutex<Option<Status>>,
    pub official: Mutex<official::OfficialCache>,
}

/// One monitoring cycle: ingest new log lines, recompute status, update the
/// tray, fire alerts, and push the status to the frontend.
pub fn tick(app: &tauri::AppHandle) -> Status {
    let state = app.state::<AppState>();
    ingest::scan(&state.store);

    let tier = state.tier.lock().unwrap().clone();
    let settings = state.store.load_settings();
    let official_status = {
        let mut cache = state.official.lock().unwrap();
        official::maybe_refresh(&mut cache, &settings, aggregate::now_ms());
        cache.status.clone()
    };
    let status = compute_status(&state.store, &tier, &settings, &official_status);

    *state.last_status.lock().unwrap() = Some(status.clone());
    tray::update_tray(app, &status);
    {
        let mut al = state.alerts.lock().unwrap();
        alerts::check(app, &mut al, &status, &settings);
    }
    let _ = app.emit("status-updated", &status);
    status
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the FIRST plugin. A second launch focuses the existing window
        // instead of starting a duplicate (which would double the tray + poller).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            let store = Store::open(&dir.join("usage.db")).expect("open store");
            let settings = store.load_settings();
            let tier = creds::current_tier(settings.tier_override.as_deref());

            app.manage(AppState {
                store,
                tier: Mutex::new(tier),
                alerts: Mutex::new(alerts::AlertState::default()),
                last_status: Mutex::new(None),
                official: Mutex::new(official::OfficialCache::default()),
            });

            tray::build_tray(&app.handle().clone())?;

            // The window is created hidden (visible:false). Show it on a normal
            // launch, but stay in the tray when autostarted with --minimized.
            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                show_main_window(app.handle());
            }

            // Background poller: initial backfill, then re-tick on an interval.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let _ = tick(&handle);
                loop {
                    let interval = {
                        let st = handle.state::<AppState>();
                        st.store.load_settings().poll_interval_secs.max(2)
                    };
                    std::thread::sleep(std::time::Duration::from_secs(interval));
                    let _ = tick(&handle);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Close button hides to tray instead of quitting.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::refresh_now,
            commands::get_settings,
            commands::set_settings,
            commands::get_history,
            commands::get_breakdown,
            commands::get_log,
            commands::refresh_official,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
