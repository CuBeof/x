use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::Context;
use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;

use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{
        Data, OrderBookDelta, TradeTick,
        order::BookOrder,
    },
    enums::{AggressorSide, BookAction, OrderSide, RecordFlag},
    identifiers::{InstrumentId, TradeId},
    types::{Price, Quantity},
};

// ─── NDJSON deserialization structs ─────────────────────────────────────────

#[derive(Deserialize)]
struct BookLine {
    action: String,
    data: Vec<BookData>,
}

#[derive(Deserialize)]
struct BookData {
    bids: Vec<Vec<Value>>,
    asks: Vec<Vec<Value>>,
    ts: String,
    #[serde(rename = "seqId")]
    seq_id: u64,
}

// ─── CSV deserialization struct ──────────────────────────────────────────────

#[derive(Deserialize)]
struct TradeRecord {
    instrument_name: String,
    trade_id: String,
    side: String,
    price: f64,
    size: f64,
    created_time: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_ts_ms(s: &str) -> anyhow::Result<UnixNanos> {
    if let Ok(ms) = s.parse::<u64>() {
        return Ok(UnixNanos::from(ms * 1_000_000));
    }
    let dt = DateTime::parse_from_rfc3339(s).context("invalid timestamp")?;
    let nanos = dt.timestamp_nanos_opt().unwrap_or(0) as u64;
    Ok(UnixNanos::from(nanos))
}

fn extract_f64(v: &Value) -> anyhow::Result<f64> {
    match v {
        Value::String(s) => s.parse::<f64>().context("expected numeric string"),
        Value::Number(n) => n.as_f64().ok_or_else(|| anyhow::anyhow!("non-f64 number")),
        _ => Err(anyhow::anyhow!("unexpected JSON type for price/size")),
    }
}

fn make_delta(
    instrument_id: InstrumentId,
    action: BookAction,
    side: OrderSide,
    price_f: f64,
    size_f: f64,
    price_prec: u8,
    size_prec: u8,
    flags: u8,
    seq: u64,
    ts: UnixNanos,
) -> OrderBookDelta {
    let price = Price::new(price_f, price_prec);
    let size = Quantity::new(size_f, size_prec);
    let order = BookOrder::new(side, price, size, 0);
    OrderBookDelta::new(instrument_id, action, order, flags, seq, ts, ts)
}

fn set_f_last(deltas: &mut Vec<Data>) {
    if let Some(Data::Delta(d)) = deltas.last_mut() {
        d.flags |= RecordFlag::F_LAST as u8;
    }
}

// ─── Snapshot processing ─────────────────────────────────────────────────────

fn process_snapshot(
    data: &BookData,
    instrument_id: InstrumentId,
    price_prec: u8,
    size_prec: u8,
) -> anyhow::Result<Vec<Data>> {
    let ts = parse_ts_ms(&data.ts)?;
    let seq = data.seq_id;
    let snap_flag = RecordFlag::F_SNAPSHOT as u8;

    let mut out: Vec<Data> = Vec::new();

    // Clear marks start of snapshot
    out.push(Data::Delta(OrderBookDelta::clear(instrument_id, seq, ts, ts)));

    for entry in &data.bids {
        if entry.len() < 2 {
            continue;
        }
        let price_f = extract_f64(&entry[0])?;
        let size_f = extract_f64(&entry[1])?;
        out.push(Data::Delta(make_delta(
            instrument_id, BookAction::Add, OrderSide::Buy,
            price_f, size_f, price_prec, size_prec, snap_flag, seq, ts,
        )));
    }

    for entry in &data.asks {
        if entry.len() < 2 {
            continue;
        }
        let price_f = extract_f64(&entry[0])?;
        let size_f = extract_f64(&entry[1])?;
        out.push(Data::Delta(make_delta(
            instrument_id, BookAction::Add, OrderSide::Sell,
            price_f, size_f, price_prec, size_prec, snap_flag, seq, ts,
        )));
    }

    set_f_last(&mut out);
    Ok(out)
}

// ─── Update processing ───────────────────────────────────────────────────────

fn process_update(
    data: &BookData,
    instrument_id: InstrumentId,
    price_prec: u8,
    size_prec: u8,
) -> anyhow::Result<Vec<Data>> {
    let ts = parse_ts_ms(&data.ts)?;
    let seq = data.seq_id;

    let mut out: Vec<Data> = Vec::new();

    for entry in &data.bids {
        if entry.len() < 2 {
            continue;
        }
        let price_f = extract_f64(&entry[0])?;
        let size_f = extract_f64(&entry[1])?;
        let (action, size_f) = if size_f > 0.0 {
            (BookAction::Update, size_f)
        } else {
            (BookAction::Delete, 0.0)
        };
        out.push(Data::Delta(make_delta(
            instrument_id, action, OrderSide::Buy,
            price_f, size_f, price_prec, size_prec, 0, seq, ts,
        )));
    }

    for entry in &data.asks {
        if entry.len() < 2 {
            continue;
        }
        let price_f = extract_f64(&entry[0])?;
        let size_f = extract_f64(&entry[1])?;
        let (action, size_f) = if size_f > 0.0 {
            (BookAction::Update, size_f)
        } else {
            (BookAction::Delete, 0.0)
        };
        out.push(Data::Delta(make_delta(
            instrument_id, action, OrderSide::Sell,
            price_f, size_f, price_prec, size_prec, 0, seq, ts,
        )));
    }

    set_f_last(&mut out);
    Ok(out)
}

// ─── Public loaders ───────────────────────────────────────────────────────────

/// Load L2 orderbook from OKX NDJSON format into `Vec<Data>` (OrderBookDelta).
///
/// Each line is a JSON object with `action` ("snapshot" | "update") and `data`
/// containing bids/asks as 3-element arrays `[price, size, count]`.
pub fn load_l2_orderbook(
    path: &Path,
    instrument_id: InstrumentId,
    price_prec: u8,
    size_prec: u8,
) -> anyhow::Result<Vec<Data>> {
    let file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut all: Vec<Data> = Vec::new();

    for (line_no, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let book_line: BookLine = serde_json::from_str(trimmed)
            .with_context(|| format!("{}:{} — invalid JSON", path.display(), line_no + 1))?;

        for book_data in &book_line.data {
            let batch = match book_line.action.as_str() {
                "snapshot" => process_snapshot(book_data, instrument_id, price_prec, size_prec)?,
                "update" => process_update(book_data, instrument_id, price_prec, size_prec)?,
                other => anyhow::bail!("Unknown book action: {}", other),
            };
            all.extend(batch);
        }
    }

    Ok(all)
}

/// Load trades from OKX CSV format into `Vec<Data>` (TradeTick).
///
/// Expected CSV header: instrument_name,trade_id,side,price,size,created_time
/// `side` is "buy" (buyer aggressor) or "sell" (seller aggressor).
pub fn load_trades(
    path: &Path,
    instrument_id: InstrumentId,
    price_prec: u8,
    size_prec: u8,
) -> anyhow::Result<Vec<Data>> {
    let mut reader = csv::Reader::from_path(path)
        .with_context(|| format!("Cannot open {}", path.display()))?;
    let mut all: Vec<Data> = Vec::new();

    for result in reader.deserialize::<TradeRecord>() {
        let rec = result.context("CSV parse error")?;

        let ts = parse_ts_ms(&rec.created_time)?;
        let price = Price::new(rec.price, price_prec);
        let size = Quantity::new(rec.size, size_prec);

        let aggressor = match rec.side.to_lowercase().as_str() {
            "buy" => AggressorSide::Buyer,
            "sell" => AggressorSide::Seller,
            _ => AggressorSide::NoAggressor,
        };

        // TradeId max 36 chars; OKX IDs are ≤ 20 chars but truncate just in case
        let id_str = if rec.trade_id.len() > 36 {
            rec.trade_id[..36].to_string()
        } else {
            rec.trade_id.clone()
        };
        let trade_id = TradeId::new(&id_str);

        let tick = TradeTick::new(instrument_id, price, size, aggressor, trade_id, ts, ts);
        all.push(Data::Trade(tick));
    }

    Ok(all)
}
