//! Reads `~/.claude/.credentials.json` to detect the subscription tier.
//! IMPORTANT: the OAuth `accessToken` is never read into our types, never
//! persisted, and never sent to the frontend. We only extract the plan fields.

use crate::types::TierInfo;
use std::path::PathBuf;

pub fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
}

pub fn projects_dir() -> PathBuf {
    claude_dir().join("projects")
}

/// OAuth material for the official-utilization feature.
/// SECURITY: `access_token` stays in memory for a single request and is never
/// persisted, logged, or sent to the frontend.
pub struct OAuth {
    pub access_token: String,
    pub expires_at_ms: i64,
}

/// Reads the OAuth access token + its expiry. We intentionally do NOT refresh
/// the token (Claude Code owns that; refreshing could rotate the refresh token
/// and break the user's login) — we only use it while still valid.
pub fn read_oauth() -> Option<OAuth> {
    let path = claude_dir().join(".credentials.json");
    let text = std::fs::read_to_string(&path).ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let o = &json["claudeAiOauth"];
    let token = o["accessToken"].as_str().filter(|s| !s.is_empty())?.to_string();
    let exp = o["expiresAt"].as_i64().unwrap_or(0);
    Some(OAuth {
        access_token: token,
        expires_at_ms: exp,
    })
}

/// Returns (subscriptionType, rateLimitTier) if available.
pub fn read_plan() -> (Option<String>, Option<String>) {
    let path = claude_dir().join(".credentials.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (None, None);
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    let oauth = &json["claudeAiOauth"];
    let sub = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let rlt = oauth
        .get("rateLimitTier")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (sub, rlt)
}

/// Map the raw plan fields to an internal tier with estimated limits.
/// The limit numbers are *rough community estimates* of weighted tokens
/// (input + output + cache_creation, cache_read excluded by default) usable in
/// a window; they exist only to give the gauges a scale and are normally
/// overridden by auto-calibration against observed usage.
/// (label, estimated 5h limit, estimated 7d limit) for a normalized tier key.
pub fn limits_for_key(key: &str) -> (&'static str, i64, i64) {
    match key {
        "max20x" => ("Claude Max 20x", 4_400_000, 32_000_000),
        "max5x" => ("Claude Max 5x", 1_100_000, 8_000_000),
        "pro" => ("Claude Pro", 220_000, 1_500_000),
        "free" => ("Claude Free", 60_000, 300_000),
        _ => ("未知套餐", 1_000_000, 7_000_000),
    }
}

pub fn detect_tier(sub: Option<String>, rlt: Option<String>) -> TierInfo {
    let rl = rlt.clone().unwrap_or_default().to_lowercase();
    let s = sub.clone().unwrap_or_default().to_lowercase();

    let key = if rl.contains("max_20x") {
        "max20x"
    } else if rl.contains("max_5x") {
        "max5x"
    } else if s == "max" {
        "max5x"
    } else if s == "pro" || rl.contains("pro") {
        "pro"
    } else if s == "free" || rl.contains("free") {
        "free"
    } else {
        "unknown"
    };

    let (label, l5h, l7d) = limits_for_key(key);
    TierInfo {
        key: key.to_string(),
        label: label.to_string(),
        subscription_type: sub,
        rate_limit_tier: rlt,
        default_limit_5h: l5h,
        default_limit_7d: l7d,
    }
}

/// Convenience: detect from disk, applying an optional manual tier override.
pub fn current_tier(override_key: Option<&str>) -> TierInfo {
    let (sub, rlt) = read_plan();
    let mut tier = detect_tier(sub, rlt);
    if let Some(k) = override_key {
        // Re-derive limits from the overridden key while keeping detected plan strings.
        let (label, l5h, l7d) = limits_for_key(k);
        tier.key = k.to_string();
        tier.label = format!("{}（手动）", label);
        tier.default_limit_5h = l5h;
        tier.default_limit_7d = l7d;
    }
    tier
}
