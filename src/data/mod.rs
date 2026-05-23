pub mod okx;

use std::path::Path;
use anyhow::Context;
use serde::Deserialize;
use nautilus_model::data::{Data, QuoteTick};
use nautilus_model::identifiers::InstrumentId;
use nautilus_model::types::{Price, Quantity};
use nautilus_core::UnixNanos;
use chrono::DateTime;

#[derive(Debug, Deserialize)]
struct QuoteRecord {
    timestamp: String,
    bid_price: f64,
    ask_price: f64,
    bid_size: f64,
    ask_size: f64,
}

#[derive(Debug, Deserialize)]
struct BarRecord {
    timestamp: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

fn parse_unix_nanos(s: &str) -> anyhow::Result<UnixNanos> {
    // Try unix ms (integer)
    if let Ok(ms) = s.parse::<i64>() {
        return Ok(UnixNanos::from(ms as u64 * 1_000_000));
    }
    // Try ISO8601
    let dt = DateTime::parse_from_rfc3339(s).context("invalid timestamp")?;
    let nanos = dt.timestamp_nanos_opt().unwrap_or(0) as u64;
    Ok(UnixNanos::from(nanos))
}

/// Load QuoteTick data from a CSV file.
/// Expected CSV columns: timestamp,bid_price,ask_price,bid_size,ask_size
pub fn load_quotes(
    path: &Path,
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> anyhow::Result<Vec<Data>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut quotes = Vec::new();

    for result in reader.deserialize::<QuoteRecord>() {
        let rec = result?;
        let ts = parse_unix_nanos(&rec.timestamp)?;

        let bid_price = Price::new(rec.bid_price, price_precision);
        let ask_price = Price::new(rec.ask_price, price_precision);
        let bid_size = Quantity::new(rec.bid_size, size_precision);
        let ask_size = Quantity::new(rec.ask_size, size_precision);

        let quote = QuoteTick::new(instrument_id, bid_price, ask_price, bid_size, ask_size, ts, ts);
        quotes.push(Data::Quote(quote));
    }

    // Sort chronologically
    quotes.sort_by_key(|d| {
        if let Data::Quote(q) = d {
            q.ts_event
        } else {
            UnixNanos::default()
        }
    });
    Ok(quotes)
}

/// Load bars as synthetic QuoteTick data (using high as ask, low as bid).
/// Fallback when only OHLCV data is available.
pub fn load_bars_as_quotes(
    path: &Path,
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> anyhow::Result<Vec<Data>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut quotes = Vec::new();

    for result in reader.deserialize::<BarRecord>() {
        let rec = result?;
        let ts = parse_unix_nanos(&rec.timestamp)?;

        // Use high as ask, low as bid (intrabar best approximation)
        let bid_price = Price::new(rec.low, price_precision);
        let ask_price = Price::new(rec.high, price_precision);
        let size = Quantity::new(rec.volume, size_precision);

        let quote = QuoteTick::new(instrument_id, bid_price, ask_price, size, size, ts, ts);
        quotes.push(Data::Quote(quote));
    }

    quotes.sort_by_key(|d| {
        if let Data::Quote(q) = d {
            q.ts_event
        } else {
            UnixNanos::default()
        }
    });
    Ok(quotes)
}
