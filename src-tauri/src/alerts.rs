//! Threshold alerts via desktop notifications. Each threshold fires once per
//! window; the 5h set resets when the rolling block resets, the 7d set resets
//! daily. On the very first check we prime (seed) without notifying to avoid a
//! burst when the app starts while already above a threshold.

use crate::types::{Settings, Status};
use std::collections::HashSet;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[derive(Default)]
pub struct AlertState {
    primed: bool,
    reset_key_5h: Option<i64>,
    fired_5h: HashSet<u32>,
    day_7d: i64,
    fired_7d: HashSet<u32>,
}

pub fn check(app: &AppHandle, state: &mut AlertState, status: &Status, settings: &Settings) {
    let first = !state.primed;
    state.primed = true;

    // 5h: reset fired thresholds when the block boundary changes.
    if state.reset_key_5h != status.window5h.resets_at_ms {
        state.reset_key_5h = status.window5h.resets_at_ms;
        state.fired_5h.clear();
    }
    let pct5 = (status.window5h.pct * 100.0) as u32;
    for &t in &settings.alert_thresholds {
        if pct5 >= t && !state.fired_5h.contains(&t) {
            state.fired_5h.insert(t);
            if !first {
                notify(app, "5 小时额度", t, status.window5h.pct);
            }
        }
    }

    // 7d: reset fired thresholds once per calendar day.
    let day = status.generated_at_ms / (24 * 3_600_000);
    if state.day_7d != day {
        state.day_7d = day;
        state.fired_7d.clear();
    }
    let pct7 = (status.window7d.pct * 100.0) as u32;
    for &t in &settings.alert_thresholds {
        if pct7 >= t && !state.fired_7d.contains(&t) {
            state.fired_7d.insert(t);
            if !first {
                notify(app, "7 天额度", t, status.window7d.pct);
            }
        }
    }
}

fn notify(app: &AppHandle, window: &str, threshold: u32, pct: f64) {
    let body = format!("{}已使用 {:.0}%（阈值 {}%）", window, pct * 100.0, threshold);
    let _ = app
        .notification()
        .builder()
        .title("Claude 额度提醒")
        .body(body)
        .show();
}
