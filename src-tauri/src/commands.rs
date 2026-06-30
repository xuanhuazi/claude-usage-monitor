//! Tauri IPC commands exposed to the React frontend.

use crate::aggregate::{compute_status, now_ms};
use crate::creds::current_tier;
use crate::pricing;
use crate::types::{Breakdown, HistPoint, LogEntry, RawSums, Settings, Status};
use crate::AppState;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

const DAY_MS: i64 = 24 * 3_600_000;

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Status {
    let tier = state.tier.lock().unwrap().clone();
    let settings = state.store.load_settings();
    let official = state.official.lock().unwrap().status.clone();
    compute_status(&state.store, &tier, &settings, &official)
}

#[tauri::command]
pub fn refresh_now(app: AppHandle) -> Status {
    crate::tick(&app)
}

/// Force an immediate official-usage fetch on the next tick (manual refresh).
#[tauri::command]
pub fn refresh_official(app: AppHandle) -> Status {
    {
        let state = app.state::<AppState>();
        let mut cache = state.official.lock().unwrap();
        crate::official::invalidate(&mut cache);
    }
    crate::tick(&app)
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.store.load_settings()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, settings: Settings) -> Status {
    let state = app.state::<AppState>();
    let _ = state.store.save_settings(&settings);

    // Re-derive tier (honoring a manual override) and apply autostart.
    let tier = current_tier(settings.tier_override.as_deref());
    *state.tier.lock().unwrap() = tier;
    apply_autostart(&app, settings.autostart);

    crate::tick(&app)
}

#[tauri::command]
pub fn get_history(state: State<AppState>, days: i64, bucket_minutes: i64) -> Vec<HistPoint> {
    let settings = state.store.load_settings();
    let from = now_ms() - days.max(1) * DAY_MS;
    let bucket = bucket_minutes.max(1) * 60_000;
    state.store.history(
        from,
        bucket,
        settings.weight_cache_creation,
        settings.weight_cache_read,
    )
}

#[tauri::command]
pub fn get_breakdown(state: State<AppState>, window: String) -> Breakdown {
    let settings = state.store.load_settings();
    let now = now_ms();
    let from = match window.as_str() {
        "5h" => now - 5 * 3_600_000,
        "7d" => now - 7 * DAY_MS,
        "30d" => now - 30 * DAY_MS,
        _ => 0,
    };
    state.store.breakdown(
        from,
        now + 1,
        settings.weight_cache_creation,
        settings.weight_cache_read,
    )
}

#[tauri::command]
pub fn get_log(state: State<AppState>, limit: i64) -> Vec<LogEntry> {
    let settings = state.store.load_settings();
    let rows = state.store.recent_event_rows(limit.clamp(1, 2000));
    rows.into_iter()
        .map(|(ts, model, project, i, o, cc, cr)| {
            let sums = RawSums { input: i, output: o, cc, cr, count: 1 };
            let fam = pricing::family_of(&model);
            LogEntry {
                ts_ms: ts,
                model,
                project,
                input: i,
                output: o,
                cache_creation: cc,
                cache_read: cr,
                weighted: sums.weighted(settings.weight_cache_creation, settings.weight_cache_read),
                cost_usd: pricing::cost_usd(fam, i, o, cc, cr),
            }
        })
        .collect()
}

fn apply_autostart(app: &AppHandle, enabled: bool) {
    let mgr = app.autolaunch();
    let _ = if enabled { mgr.enable() } else { mgr.disable() };
}
