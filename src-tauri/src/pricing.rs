//! Equivalent pay-as-you-go API cost (public list prices). For subscription
//! users this is informational — "what this usage would cost on the API" — not
//! actual billing. Prices are USD per million tokens (late-2025/2026 list).

pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

fn price_per_million(family: &str) -> Price {
    match family {
        "opus" => Price { input: 15.0, output: 75.0, cache_write: 18.75, cache_read: 1.5 },
        "sonnet" => Price { input: 3.0, output: 15.0, cache_write: 3.75, cache_read: 0.3 },
        "haiku" => Price { input: 1.0, output: 5.0, cache_write: 1.25, cache_read: 0.1 },
        // Unknown/other models → Sonnet-tier as a rough mid estimate.
        _ => Price { input: 3.0, output: 15.0, cache_write: 3.75, cache_read: 0.3 },
    }
}

/// Cost in USD for the given family's token counts.
pub fn cost_usd(family: &str, input: i64, output: i64, cache_creation: i64, cache_read: i64) -> f64 {
    let p = price_per_million(family);
    (input as f64 * p.input
        + output as f64 * p.output
        + cache_creation as f64 * p.cache_write
        + cache_read as f64 * p.cache_read)
        / 1_000_000.0
}

/// Map a raw model string to a family key (matches store's FAM_CASE / breakdown).
pub fn family_of(model: &str) -> &'static str {
    let l = model.to_lowercase();
    if l.contains("opus") {
        "opus"
    } else if l.contains("sonnet") {
        "sonnet"
    } else if l.contains("haiku") {
        "haiku"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_mapping() {
        assert_eq!(family_of("claude-opus-4-8"), "opus");
        assert_eq!(family_of("claude-sonnet-4-6"), "sonnet");
        assert_eq!(family_of("claude-haiku-4-5-20251001"), "haiku");
        assert_eq!(family_of("claude-fable-5"), "other");
    }

    #[test]
    fn cost_math() {
        // 1M opus output = $75
        assert!((cost_usd("opus", 0, 1_000_000, 0, 0) - 75.0).abs() < 1e-6);
        // 1M sonnet input = $3
        assert!((cost_usd("sonnet", 1_000_000, 0, 0, 0) - 3.0).abs() < 1e-6);
        // 1M haiku cache_read = $0.10
        assert!((cost_usd("haiku", 0, 0, 0, 1_000_000) - 0.1).abs() < 1e-6);
    }
}
