use anyhow::{Context, Result};
use nautilus_core::UnixNanos;
use nautilus_model::{
    data::TradeTick,
    enums::AggressorSide,
    identifiers::{InstrumentId, TradeId},
    types::{Price, Quantity},
};
use serde::Deserialize;
use std::path::Path;

use crate::orderbook::decimal_places;

pub struct ParseResult {
    pub ticks: Vec<TradeTick>,
    pub price_precision: u8,
    pub size_precision: u8,
}

#[derive(Deserialize)]
struct TradeRow {
    #[allow(dead_code)]
    instrument_name: Option<String>,
    trade_id: String,
    side: String,
    size: String,
    price: String,
    created_time: u64,
}

pub fn parse(
    path: &Path,
    instrument_id: &InstrumentId,
    price_precision: Option<u8>,
    size_precision: Option<u8>,
) -> Result<ParseResult> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("cannot open {}", path.display()))?;

    let mut rows: Vec<TradeRow> = Vec::new();
    for (i, result) in rdr.deserialize().enumerate() {
        let row: TradeRow =
            result.with_context(|| format!("line {}: CSV parse error", i + 2))?;
        rows.push(row);
    }

    let (price_prec, size_prec) =
        detect_precisions(&rows, price_precision, size_precision, 200);

    let mut ticks: Vec<TradeTick> = Vec::with_capacity(rows.len());

    for (i, row) in rows.into_iter().enumerate() {
        let price_f64: f64 = row
            .price
            .parse()
            .with_context(|| format!("row {}: invalid price {:?}", i + 2, row.price))?;
        let size_f64: f64 = row
            .size
            .parse()
            .with_context(|| format!("row {}: invalid size {:?}", i + 2, row.size))?;

        let aggressor_side = match row.side.to_lowercase().as_str() {
            "buy" => AggressorSide::Buyer,
            "sell" => AggressorSide::Seller,
            other => {
                anyhow::bail!("row {}: unknown trade side {:?}", i + 2, other)
            }
        };

        let ts = UnixNanos::from_millis(row.created_time);

        ticks.push(TradeTick::new(
            instrument_id.clone(),
            Price::new(price_f64, price_prec),
            Quantity::new(size_f64, size_prec),
            aggressor_side,
            TradeId::new(&row.trade_id),
            ts,
            ts,
        ));
    }

    Ok(ParseResult {
        ticks,
        price_precision: price_prec,
        size_precision: size_prec,
    })
}

fn detect_precisions(
    rows: &[TradeRow],
    override_price: Option<u8>,
    override_size: Option<u8>,
    sample: usize,
) -> (u8, u8) {
    if override_price.is_some() && override_size.is_some() {
        return (override_price.unwrap(), override_size.unwrap());
    }
    let mut price_prec: u8 = 0;
    let mut size_prec: u8 = 0;
    for row in rows.iter().take(sample) {
        price_prec = price_prec.max(decimal_places(&row.price));
        size_prec = size_prec.max(decimal_places(&row.size));
    }
    (
        override_price.unwrap_or(price_prec),
        override_size.unwrap_or(size_prec),
    )
}
