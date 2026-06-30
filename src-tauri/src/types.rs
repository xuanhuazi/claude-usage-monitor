//! Shared data types: raw JSONL parsing structs + domain/IPC types + settings.

use serde::{Deserialize, Serialize};

// ----------------------------------------------------------------------------
// Raw JSONL line parsing (only the fields we care about; everything optional).
// ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RawLine {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(rename = "requestId", default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub message: Option<RawMessage>,
}

#[derive(Debug, Deserialize)]
pub struct RawMessage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize)]
pub struct RawUsage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
}

/// A parsed, deduped usage event ready to store.
#[derive(Debug, Clone)]
pub struct Event {
    pub uuid: String,
    pub ts_ms: i64,
    pub model: String,
    pub project: String,
    pub session_id: String,
    pub input: i64,
    pub output: i64,
    pub cache_creation: i64,
    pub cache_read: i64,
}

// ----------------------------------------------------------------------------
// Aggregation helpers
// ----------------------------------------------------------------------------

/// Raw (un-weighted) token sums over a set of events.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawSums {
    pub input: i64,
    pub output: i64,
    pub cc: i64,
    pub cr: i64,
    pub count: i64,
}

impl RawSums {
    /// Apply the configurable weighting. input + output always count at 1x.
    pub fn weighted(&self, w_cc: f64, w_cr: f64) -> i64 {
        self.input
            + self.output
            + (self.cc as f64 * w_cc).round() as i64
            + (self.cr as f64 * w_cr).round() as i64
    }

    pub fn to_raw_tokens(&self) -> RawTokens {
        RawTokens {
            input: self.input,
            output: self.output,
            cache_creation: self.cc,
            cache_read: self.cr,
        }
    }

    pub fn add(&mut self, o: &RawSums) {
        self.input += o.input;
        self.output += o.output;
        self.cc += o.cc;
        self.cr += o.cr;
        self.count += o.count;
    }
}

// ----------------------------------------------------------------------------
// Domain / IPC types (serialized to the frontend as camelCase JSON)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTokens {
    pub input: i64,
    pub output: i64,
    pub cache_creation: i64,
    pub cache_read: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowStat {
    /// Weighted tokens used in the window.
    pub used: i64,
    pub limit: i64,
    /// 0.0 .. (can exceed 1.0 if over the estimated limit)
    pub pct: f64,
    pub raw: RawTokens,
    pub resets_at_ms: Option<i64>,
    pub remaining_seconds: Option<i64>,
    /// Highest weighted usage ever observed in a window of this size.
    pub observed_max: i64,
    /// True when `limit` came from auto-calibration rather than a fixed value.
    pub auto_limit: bool,
    /// Equivalent pay-as-you-go API cost (USD) for this window's tokens.
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BurnRate {
    pub tokens_per_min: f64,
    pub seconds_to_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Weights {
    pub cache_creation: f64,
    pub cache_read: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialStatus {
    pub available: bool,
    pub source: String,
    pub util5h: Option<f64>,
    pub util7d: Option<f64>,
    pub reset5h_ms: Option<i64>,
    pub reset7d_ms: Option<i64>,
    /// Max-plan separate weekly Opus limit (often the binding constraint for
    /// heavy Opus users). Only known via the official source.
    pub util_opus7d: Option<f64>,
    pub reset_opus7d_ms: Option<i64>,
    pub note: String,
}

impl OfficialStatus {
    pub fn unavailable(note: &str) -> Self {
        OfficialStatus {
            available: false,
            source: "local-estimate".to_string(),
            util5h: None,
            util7d: None,
            reset5h_ms: None,
            reset7d_ms: None,
            util_opus7d: None,
            reset_opus7d_ms: None,
            note: note.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub generated_at_ms: i64,
    pub tier: String,
    pub plan_label: String,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    pub window5h: WindowStat,
    pub window7d: WindowStat,
    pub burn: BurnRate,
    pub weights: Weights,
    pub total_events: i64,
    pub first_event_ms: Option<i64>,
    pub official: OfficialStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownItem {
    pub key: String,
    pub label: String,
    pub weighted: i64,
    pub raw: RawTokens,
    pub count: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub ts_ms: i64,
    pub model: String,
    pub project: String,
    pub input: i64,
    pub output: i64,
    pub cache_creation: i64,
    pub cache_read: i64,
    pub weighted: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    pub by_model: Vec<BreakdownItem>,
    pub by_project: Vec<BreakdownItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistPoint {
    pub bucket_ms: i64,
    pub weighted: i64,
    pub opus: i64,
    pub sonnet: i64,
    pub haiku: i64,
    pub other: i64,
}

// ----------------------------------------------------------------------------
// Tier info & settings
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TierInfo {
    pub key: String,
    pub label: String,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    pub default_limit_5h: i64,
    pub default_limit_7d: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub tier_override: Option<String>,
    pub limit5h_override: Option<i64>,
    pub limit7d_override: Option<i64>,
    pub auto_calibrate: bool,
    pub weight_cache_creation: f64,
    pub weight_cache_read: f64,
    pub alert_thresholds: Vec<u32>,
    pub autostart: bool,
    pub official_enabled: bool,
    pub theme: String,
    pub poll_interval_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            tier_override: None,
            limit5h_override: None,
            limit7d_override: None,
            auto_calibrate: true,
            weight_cache_creation: 1.0,
            weight_cache_read: 0.0,
            alert_thresholds: vec![75, 90, 100],
            autostart: false,
            official_enabled: false,
            theme: "system".to_string(),
            poll_interval_secs: 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_applies_cache_weights() {
        let s = RawSums { input: 100, output: 50, cc: 200, cr: 1000, count: 1 };
        // default: cache_creation x1, cache_read x0
        assert_eq!(s.weighted(1.0, 0.0), 350);
        // cache_read x0.1 -> +100
        assert_eq!(s.weighted(1.0, 0.1), 450);
        // cache_creation x0.25 -> 100 + 50 + 50
        assert_eq!(s.weighted(0.25, 0.0), 200);
    }

    #[test]
    fn settings_defaults_are_sane() {
        let s = Settings::default();
        assert!(s.auto_calibrate);
        assert_eq!(s.weight_cache_read, 0.0);
        assert_eq!(s.alert_thresholds, vec![75, 90, 100]);
        assert!(!s.official_enabled);
    }
}
