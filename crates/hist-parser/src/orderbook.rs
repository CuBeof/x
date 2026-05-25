use anyhow::{Context, Result};
use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{BookOrder, OrderBookDelta},
    enums::{BookAction, OrderSide, RecordFlag},
    identifiers::InstrumentId,
    types::{Price, Quantity},
};
use serde::Deserialize;
use std::path::Path;

pub struct ParseResult {
    pub deltas: Vec<OrderBookDelta>,
    pub price_precision: u8,
    pub size_precision: u8,
}

/// One line in the NDJSON orderbook file.
#[derive(Deserialize)]
struct OBLine {
    // The spec wrote "actio" but the actual field name is typically "action".
    // Accept both via serde alias.
    #[serde(alias = "actio", alias = "action")]
    action: String,
    #[serde(default)]
    asks: Vec<serde_json::Value>,
    #[serde(default)]
    bids: Vec<serde_json::Value>,
    ts: u64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn parse(
    path: &Path,
    instrument_id: &InstrumentId,
    price_precision: Option<u8>,
    size_precision: Option<u8>,
) -> Result<ParseResult> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;

    let (price_prec, size_prec) =
        detect_precisions(&content, price_precision, size_precision, 200);

    let mut deltas: Vec<OrderBookDelta> = Vec::new();
    let mut sequence: u64 = 0;

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ob: OBLine = serde_json::from_str(line)
            .with_context(|| format!("line {}: invalid JSON", line_no + 1))?;

        let ts = UnixNanos::from_millis(ob.ts);
        let is_snapshot = ob.action == "snapshot";

        let batch_start = deltas.len();

        if is_snapshot {
            // OrderBookDelta::clear sets flags = F_SNAPSHOT automatically
            deltas.push(OrderBookDelta::clear(
                instrument_id.clone(),
                sequence,
                ts,
                ts,
            ));
            sequence += 1;
        }

        for entry in &ob.bids {
            let (price, qty) = parse_level(entry)
                .with_context(|| format!("line {}: bad bid entry", line_no + 1))?;
            let action = level_action(is_snapshot, qty);
            deltas.push(OrderBookDelta::new(
                instrument_id.clone(),
                action,
                BookOrder::new(
                    OrderSide::Buy,
                    Price::new(price, price_prec),
                    Quantity::new(qty.max(0.0), size_prec),
                    0,
                ),
                0,
                sequence,
                ts,
                ts,
            ));
            sequence += 1;
        }

        for entry in &ob.asks {
            let (price, qty) = parse_level(entry)
                .with_context(|| format!("line {}: bad ask entry", line_no + 1))?;
            let action = level_action(is_snapshot, qty);
            deltas.push(OrderBookDelta::new(
                instrument_id.clone(),
                action,
                BookOrder::new(
                    OrderSide::Sell,
                    Price::new(price, price_prec),
                    Quantity::new(qty.max(0.0), size_prec),
                    0,
                ),
                0,
                sequence,
                ts,
                ts,
            ));
            sequence += 1;
        }

        // Mark the last delta in this message with F_LAST
        if let Some(d) = deltas[batch_start..].last_mut() {
            d.flags |= RecordFlag::F_LAST as u8;
        }
    }

    Ok(ParseResult {
        deltas,
        price_precision: price_prec,
        size_precision: size_prec,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn level_action(is_snapshot: bool, qty: f64) -> BookAction {
    if is_snapshot || qty > 0.0 {
        BookAction::Add
    } else {
        BookAction::Delete
    }
}

/// Parse a level from `["price", "qty", "count"]` or `[price, qty, count]`.
fn parse_level(entry: &serde_json::Value) -> Result<(f64, f64)> {
    let arr = entry
        .as_array()
        .with_context(|| "level entry is not an array")?;
    if arr.len() < 2 {
        anyhow::bail!("level entry has fewer than 2 elements");
    }
    let price = json_to_f64(&arr[0]).context("invalid price")?;
    let qty = json_to_f64(&arr[1]).context("invalid qty")?;
    Ok((price, qty))
}

fn json_to_f64(v: &serde_json::Value) -> Result<f64> {
    if let Some(s) = v.as_str() {
        s.parse::<f64>().context("not a float string")
    } else if let Some(n) = v.as_f64() {
        Ok(n)
    } else {
        anyhow::bail!("cannot convert {:?} to f64", v)
    }
}

// ---------------------------------------------------------------------------
// Precision detection
// ---------------------------------------------------------------------------

pub fn detect_precisions(
    content: &str,
    override_price: Option<u8>,
    override_size: Option<u8>,
    sample_lines: usize,
) -> (u8, u8) {
    if override_price.is_some() && override_size.is_some() {
        return (override_price.unwrap(), override_size.unwrap());
    }

    let mut price_prec: u8 = 0;
    let mut size_prec: u8 = 0;
    let mut seen = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(ob) = serde_json::from_str::<OBLine>(line) {
            for entry in ob.asks.iter().chain(ob.bids.iter()) {
                if let Some(arr) = entry.as_array() {
                    if arr.len() >= 2 {
                        if let Some(s) = arr[0].as_str() {
                            price_prec = price_prec.max(decimal_places(s));
                        }
                        if let Some(s) = arr[1].as_str() {
                            size_prec = size_prec.max(decimal_places(s));
                        }
                    }
                }
            }
        }
        seen += 1;
        if seen >= sample_lines {
            break;
        }
    }

    (
        override_price.unwrap_or(price_prec),
        override_size.unwrap_or(size_prec),
    )
}

pub fn decimal_places(s: &str) -> u8 {
    match s.find('.') {
        Some(pos) => (s.len() - pos - 1) as u8,
        None => 0,
    }
}
