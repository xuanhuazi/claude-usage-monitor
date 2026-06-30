//! Turns stored events into a `Status`: the rolling 5-hour block, the 7-day
//! window, burn rate, and time-to-limit — all weighted & compared to limits.

use crate::store::Store;
use crate::types::{BurnRate, OfficialStatus, RawSums, Settings, Status, TierInfo, Weights, WindowStat};
use chrono::Utc;

const HOUR_MS: i64 = 3_600_000;
const FIVE_H_MS: i64 = 5 * HOUR_MS;
const WEEK_MS: i64 = 7 * 24 * HOUR_MS;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn floor_to_hour(ts_ms: i64) -> i64 {
    ts_ms - ts_ms.rem_euclid(HOUR_MS)
}

/// Find the most-recent 5h "block" (ccusage-style: anchored to the first
/// message of a contiguous run, floored to the hour, lasting 5 hours).
fn active_block(events: &[(i64, i64, i64, i64, i64)]) -> Option<(i64, RawSums)> {
    let mut block_start: Option<i64> = None;
    let mut prev_ts: Option<i64> = None;
    let mut sums = RawSums::default();

    for &(ts, i, o, cc, cr) in events {
        let new_block = match (block_start, prev_ts) {
            (None, _) => true,
            (Some(bs), Some(pt)) => (ts - bs >= FIVE_H_MS) || (ts - pt > FIVE_H_MS),
            (Some(bs), None) => ts - bs >= FIVE_H_MS,
        };
        if new_block {
            block_start = Some(floor_to_hour(ts));
            sums = RawSums::default();
        }
        sums.input += i;
        sums.output += o;
        sums.cc += cc;
        sums.cr += cr;
        sums.count += 1;
        prev_ts = Some(ts);
    }

    block_start.map(|bs| (bs, sums))
}

fn pick_limit(
    override_val: Option<i64>,
    base: i64,
    observed: i64,
    auto: bool,
) -> (i64, bool) {
    match override_val {
        Some(v) if v > 0 => (v, false),
        _ if auto => (base.max(observed).max(1), true),
        _ => (base.max(1), false),
    }
}

pub fn compute_status(
    store: &Store,
    tier: &TierInfo,
    settings: &Settings,
    official: &OfficialStatus,
) -> Status {
    let now = now_ms();
    let w_cc = settings.weight_cache_creation;
    let w_cr = settings.weight_cache_read;

    // ---- 5h rolling block ----
    let recent = store.recent_events(now - 6 * HOUR_MS);
    let (raw5, resets_at, remaining, cost5) = match active_block(&recent) {
        Some((bs, sums)) => {
            let reset = bs + FIVE_H_MS;
            if now >= reset {
                (RawSums::default(), None, None, 0.0)
            } else {
                let cost = store.window_cost(bs, now + 1);
                (sums, Some(reset), Some((reset - now) / 1000), cost)
            }
        }
        None => (RawSums::default(), None, None, 0.0),
    };
    let observed5h = store.observed_max(0, FIVE_H_MS, w_cc, w_cr);
    let (limit5h, auto5h) = pick_limit(
        settings.limit5h_override,
        tier.default_limit_5h,
        observed5h,
        settings.auto_calibrate,
    );
    let used5 = raw5.weighted(w_cc, w_cr);
    let mut window5h = WindowStat {
        used: used5,
        limit: limit5h,
        pct: used5 as f64 / limit5h as f64,
        raw: raw5.to_raw_tokens(),
        resets_at_ms: resets_at,
        remaining_seconds: remaining,
        observed_max: observed5h,
        auto_limit: auto5h,
        cost_usd: cost5,
    };

    // ---- 7d rolling window ----
    let raw7 = store.raw_sum(now - WEEK_MS, now + 1);
    let cost7 = store.window_cost(now - WEEK_MS, now + 1);
    let observed7 = store.observed_max(0, WEEK_MS, w_cc, w_cr);
    let (limit7d, auto7d) = pick_limit(
        settings.limit7d_override,
        tier.default_limit_7d,
        observed7,
        settings.auto_calibrate,
    );
    let used7 = raw7.weighted(w_cc, w_cr);
    let mut window7d = WindowStat {
        used: used7,
        limit: limit7d,
        pct: used7 as f64 / limit7d as f64,
        raw: raw7.to_raw_tokens(),
        resets_at_ms: None,
        remaining_seconds: None,
        observed_max: observed7,
        auto_limit: auto7d,
        cost_usd: cost7,
    };

    // ---- official utilization override (Phase 4) ----
    // When the official source is live, show the official percentage and reset
    // time, and derive a limit consistent with our token count (used / util).
    if official.available {
        apply_official(&mut window5h, official.util5h, official.reset5h_ms, used5, now);
        apply_official(&mut window7d, official.util7d, official.reset7d_ms, used7, now);
    }

    // ---- burn rate (last 60 min) ----
    let last_hour = store.raw_sum(now - HOUR_MS, now + 1);
    let per_min = last_hour.weighted(w_cc, w_cr) as f64 / 60.0;
    let remaining5 = window5h.limit - window5h.used;
    let seconds_to_limit = if per_min > 0.0 && remaining5 > 0 {
        Some(((remaining5 as f64 / per_min) * 60.0) as i64)
    } else {
        None
    };

    Status {
        generated_at_ms: now,
        tier: tier.key.clone(),
        plan_label: tier.label.clone(),
        subscription_type: tier.subscription_type.clone(),
        rate_limit_tier: tier.rate_limit_tier.clone(),
        window5h,
        window7d,
        burn: BurnRate {
            tokens_per_min: per_min,
            seconds_to_limit,
        },
        weights: Weights {
            cache_creation: w_cc,
            cache_read: w_cr,
        },
        total_events: store.total_events(),
        first_event_ms: store.first_ts(),
        official: official.clone(),
    }
}

/// Override a window with official utilization when the official source is live.
fn apply_official(w: &mut WindowStat, util: Option<f64>, reset_ms: Option<i64>, used: i64, now: i64) {
    if let Some(u) = util {
        w.pct = u;
        if u > 0.0 && used > 0 {
            w.limit = ((used as f64) / u).round().max(1.0) as i64;
            w.auto_limit = false;
        }
        if let Some(r) = reset_ms {
            w.resets_at_ms = Some(r);
            w.remaining_seconds = Some((r - now) / 1000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawTokens;

    fn ev(ts: i64, i: i64, o: i64, cc: i64, cr: i64) -> (i64, i64, i64, i64, i64) {
        (ts, i, o, cc, cr)
    }

    #[test]
    fn floor_to_hour_aligns_to_utc_hour() {
        let t = 1_000_000_000_123; // arbitrary ms
        let f = floor_to_hour(t);
        assert_eq!(f % HOUR_MS, 0);
        assert!(f <= t && t - f < HOUR_MS);
    }

    #[test]
    fn active_block_accumulates_within_window() {
        let base = 10 * HOUR_MS; // hour-aligned
        let events = vec![
            ev(base, 1, 1, 0, 0),
            ev(base + HOUR_MS, 2, 2, 0, 0),
            ev(base + 2 * HOUR_MS, 3, 3, 0, 0),
        ];
        let (bs, sums) = active_block(&events).unwrap();
        assert_eq!(bs, base);
        assert_eq!(sums.input, 6);
        assert_eq!(sums.output, 6);
        assert_eq!(sums.count, 3);
    }

    #[test]
    fn active_block_resets_after_5h_span() {
        let base = 10 * HOUR_MS;
        let events = vec![ev(base, 1, 0, 0, 0), ev(base + 6 * HOUR_MS, 5, 0, 0, 0)];
        let (bs, sums) = active_block(&events).unwrap();
        assert_eq!(bs, base + 6 * HOUR_MS);
        assert_eq!(sums.input, 5);
        assert_eq!(sums.count, 1);
    }

    #[test]
    fn active_block_resets_on_long_gap() {
        let base = 10 * HOUR_MS;
        let events = vec![
            ev(base, 1, 0, 0, 0),
            ev(base + HOUR_MS, 1, 0, 0, 0),
            ev(base + 7 * HOUR_MS, 9, 0, 0, 0), // >5h gap from prev
        ];
        let (bs, sums) = active_block(&events).unwrap();
        assert_eq!(bs, base + 7 * HOUR_MS);
        assert_eq!(sums.input, 9);
    }

    #[test]
    fn active_block_empty_is_none() {
        assert!(active_block(&[]).is_none());
    }

    #[test]
    fn pick_limit_override_wins() {
        assert_eq!(pick_limit(Some(500), 100, 999, true), (500, false));
    }

    #[test]
    fn pick_limit_auto_uses_max_of_base_and_observed() {
        assert_eq!(pick_limit(None, 100, 300, true), (300, true));
        assert_eq!(pick_limit(None, 400, 300, true), (400, true));
    }

    #[test]
    fn pick_limit_fixed_when_not_auto() {
        assert_eq!(pick_limit(None, 100, 999, false), (100, false));
    }

    #[test]
    fn apply_official_sets_pct_limit_and_reset() {
        let mut w = WindowStat {
            used: 1000,
            limit: 5000,
            pct: 0.2,
            raw: RawTokens { input: 0, output: 0, cache_creation: 0, cache_read: 0 },
            resets_at_ms: None,
            remaining_seconds: None,
            observed_max: 0,
            auto_limit: true,
            cost_usd: 0.0,
        };
        let now = 1_000_000;
        apply_official(&mut w, Some(0.5), Some(now + 3_600_000), 1000, now);
        assert!((w.pct - 0.5).abs() < 1e-9);
        assert_eq!(w.limit, 2000); // 1000 / 0.5
        assert!(!w.auto_limit);
        assert_eq!(w.resets_at_ms, Some(now + 3_600_000));
        assert_eq!(w.remaining_seconds, Some(3600));
    }
}
