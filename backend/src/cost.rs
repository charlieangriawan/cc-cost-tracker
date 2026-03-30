use crate::models::RateEntry;

pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

pub fn get_pricing(model: &str) -> Pricing {
    let n = normalize_model(model);
    if n.contains("opus") {
        Pricing { input: 15.00, output: 75.00, cache_write: 18.75, cache_read: 1.50 }
    } else if n.contains("haiku") {
        Pricing { input: 0.80, output: 4.00, cache_write: 1.00, cache_read: 0.08 }
    } else {
        // sonnet + unknown fallback
        Pricing { input: 3.00, output: 15.00, cache_write: 3.75, cache_read: 0.30 }
    }
}

/// Strip 8-digit date suffixes: claude-opus-4-6-20250514 → claude-opus-4-6
pub fn normalize_model(model: &str) -> String {
    let parts: Vec<&str> = model.split('-').collect();
    let end = if parts.last().map_or(false, |p| p.len() == 8 && p.chars().all(|c| c.is_ascii_digit())) {
        parts.len() - 1
    } else {
        parts.len()
    };
    parts[..end].join("-")
}

pub fn calculate_cost(
    input_tokens: u64,
    output_tokens: u64,
    cache_write_tokens: u64,
    cache_read_tokens: u64,
    model: &str,
) -> (f64, f64, f64, f64) {
    let p = get_pricing(model);
    (
        (input_tokens as f64 / 1_000_000.0) * p.input,
        (output_tokens as f64 / 1_000_000.0) * p.output,
        (cache_write_tokens as f64 / 1_000_000.0) * p.cache_write,
        (cache_read_tokens as f64 / 1_000_000.0) * p.cache_read,
    )
}

pub fn rate_card() -> Vec<RateEntry> {
    vec![
        RateEntry {
            model: "claude-opus-4".into(),
            input_per_mtok: 15.00,
            output_per_mtok: 75.00,
            cache_write_per_mtok: 18.75,
            cache_read_per_mtok: 1.50,
        },
        RateEntry {
            model: "claude-sonnet-4".into(),
            input_per_mtok: 3.00,
            output_per_mtok: 15.00,
            cache_write_per_mtok: 3.75,
            cache_read_per_mtok: 0.30,
        },
        RateEntry {
            model: "claude-haiku-4".into(),
            input_per_mtok: 0.80,
            output_per_mtok: 4.00,
            cache_write_per_mtok: 1.00,
            cache_read_per_mtok: 0.08,
        },
    ]
}
