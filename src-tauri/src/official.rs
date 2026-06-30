//! Official utilization source (Phase 4).
//!
//! Anthropic exposes a read-only, NON-quota-consuming endpoint that returns the
//! account's official 5h / 7d utilization:
//!   GET https://api.anthropic.com/api/oauth/usage
//! authenticated with the local Claude Code OAuth token. We poll it at a slow
//! cadence (the endpoint is known to 429 if hammered) and fall back to the local
//! estimate on any error. The token is read per-request and never persisted,
//! logged, or sent to the frontend.

use crate::creds;
use crate::types::{OfficialStatus, Settings};
use chrono::DateTime;
use serde::Deserialize;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA: &str = "oauth-2025-04-20";
const UA: &str = "claude-code/2.1.170";

const MIN_INTERVAL_MS: i64 = 180_000; // poll at most every 3 min
const BACKOFF_429_MS: i64 = 600_000; // back off 10 min on rate limit
const BACKOFF_ERR_MS: i64 = 120_000; // back off 2 min on other errors

#[derive(Default)]
pub struct OfficialCache {
    pub status: OfficialStatus,
    pub fetched_at_ms: i64,
    pub backoff_until_ms: i64,
}

impl Default for OfficialStatus {
    fn default() -> Self {
        OfficialStatus::unavailable("未启用官方源，显示本地估算值")
    }
}

#[derive(Deserialize)]
struct UsageResp {
    five_hour: Option<Window>,
    seven_day: Option<Window>,
    seven_day_opus: Option<Window>,
}

#[derive(Deserialize)]
struct Window {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

fn parse_window(w: &Option<Window>) -> (Option<f64>, Option<i64>) {
    match w {
        Some(w) => {
            let util = w.utilization.map(|u| u / 100.0); // % -> fraction
            let reset = w
                .resets_at
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|d| d.timestamp_millis());
            (util, reset)
        }
        None => (None, None),
    }
}

fn fetch(token: &str) -> Result<OfficialStatus, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", BETA)
        .header("User-Agent", UA)
        .header("Content-Type", "application/json")
        .send()
        .map_err(|e| e.to_string())?;

    let code = resp.status().as_u16();
    if code == 429 {
        return Err("rate_limited".to_string());
    }
    if code == 401 || code == 403 {
        return Err("unauthorized".to_string());
    }
    if !resp.status().is_success() {
        return Err(format!("http {code}"));
    }
    let body: UsageResp = resp.json().map_err(|e| e.to_string())?;
    let (u5, r5) = parse_window(&body.five_hour);
    let (u7, r7) = parse_window(&body.seven_day);
    let (uo, ro) = parse_window(&body.seven_day_opus);
    Ok(OfficialStatus {
        available: true,
        source: "official".to_string(),
        util5h: u5,
        util7d: u7,
        reset5h_ms: r5,
        reset7d_ms: r7,
        util_opus7d: uo,
        reset_opus7d_ms: ro,
        note: "官方利用率（来自 Claude 服务端）".to_string(),
    })
}

/// Refresh the cache if due. Network happens here only (background tick), never
/// in synchronous IPC commands. Keeps the last good status between refreshes.
pub fn maybe_refresh(cache: &mut OfficialCache, settings: &Settings, now_ms: i64) {
    if !settings.official_enabled {
        cache.status = OfficialStatus::unavailable("未启用官方源，显示本地估算值");
        return;
    }
    let due = now_ms >= cache.backoff_until_ms && now_ms - cache.fetched_at_ms >= MIN_INTERVAL_MS;
    if !due {
        return; // keep current cache.status
    }

    let Some(oauth) = creds::read_oauth() else {
        cache.status = OfficialStatus::unavailable("未找到本地 OAuth 凭证，显示本地估算值");
        cache.fetched_at_ms = now_ms;
        return;
    };

    // Skip the call when the token is expired: Claude Code refreshes it on use;
    // calling with a stale token just yields 401. (30s skew buffer.)
    if oauth.expires_at_ms != 0 && now_ms >= oauth.expires_at_ms - 30_000 {
        cache.status =
            OfficialStatus::unavailable("官方凭证已过期，请在 Claude Code 中活动以刷新后重试；暂用本地估算");
        cache.fetched_at_ms = now_ms;
        cache.backoff_until_ms = now_ms + BACKOFF_ERR_MS;
        return;
    }

    cache.fetched_at_ms = now_ms; // throttle even on failure
    match fetch(&oauth.access_token) {
        Ok(status) => {
            cache.status = status;
            cache.backoff_until_ms = 0;
        }
        Err(e) if e == "rate_limited" => {
            cache.backoff_until_ms = now_ms + BACKOFF_429_MS;
            cache.status = OfficialStatus::unavailable("官方接口限流(429)，暂用本地估算");
        }
        Err(e) if e == "unauthorized" => {
            cache.backoff_until_ms = now_ms + BACKOFF_ERR_MS;
            cache.status = OfficialStatus::unavailable("官方凭证无效或已过期，请在 Claude Code 中活动以刷新；暂用本地估算");
        }
        Err(e) => {
            cache.backoff_until_ms = now_ms + BACKOFF_ERR_MS;
            cache.status = OfficialStatus::unavailable(&format!("官方源读取失败（{e}），用本地估算"));
        }
    }
}

/// Force the next `maybe_refresh` to fetch immediately (manual refresh button).
pub fn invalidate(cache: &mut OfficialCache) {
    cache.fetched_at_ms = 0;
    cache.backoff_until_ms = 0;
}
