use anyhow::{Context, Result};
use arrow::record_batch::RecordBatch;
use nautilus_model::{
    data::{OrderBookDelta, TradeTick},
    identifiers::InstrumentId,
};
use nautilus_serialization::arrow::EncodeToRecordBatch;
use parquet::{
    arrow::ArrowWriter,
    basic::Compression,
    file::properties::WriterProperties,
};
use std::{collections::HashMap, fs, path::Path};

// ---------------------------------------------------------------------------
// Public write functions
// ---------------------------------------------------------------------------

pub fn write_order_book_deltas(
    catalog: &Path,
    instrument_id: &InstrumentId,
    price_precision: u8,
    size_precision: u8,
    deltas: Vec<OrderBookDelta>,
) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }

    let ts_start = deltas.first().unwrap().ts_init.as_u64();
    let ts_end = deltas.last().unwrap().ts_init.as_u64();

    let metadata = make_metadata(instrument_id, price_precision, size_precision);
    let batch = OrderBookDelta::encode_batch(&metadata, &deltas)
        .context("failed to encode OrderBookDelta to RecordBatch")?;

    let out_path = data_path(catalog, "order_book_deltas", instrument_id, ts_start, ts_end);
    fs::create_dir_all(out_path.parent().unwrap())?;
    write_batch(&batch, &out_path).context("failed to write order_book_deltas parquet")?;

    println!("  → {}", out_path.display());
    Ok(())
}

pub fn write_trade_ticks(
    catalog: &Path,
    instrument_id: &InstrumentId,
    price_precision: u8,
    size_precision: u8,
    ticks: Vec<TradeTick>,
) -> Result<()> {
    if ticks.is_empty() {
        return Ok(());
    }

    let ts_start = ticks.first().unwrap().ts_init.as_u64();
    let ts_end = ticks.last().unwrap().ts_init.as_u64();

    let metadata = make_metadata(instrument_id, price_precision, size_precision);
    let batch = TradeTick::encode_batch(&metadata, &ticks)
        .context("failed to encode TradeTick to RecordBatch")?;

    let out_path = data_path(catalog, "trade_ticks", instrument_id, ts_start, ts_end);
    fs::create_dir_all(out_path.parent().unwrap())?;
    write_batch(&batch, &out_path).context("failed to write trade_ticks parquet")?;

    println!("  → {}", out_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_metadata(
    instrument_id: &InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> HashMap<String, String> {
    HashMap::from([
        ("instrument_id".to_string(), instrument_id.to_string()),
        ("price_precision".to_string(), price_precision.to_string()),
        ("size_precision".to_string(), size_precision.to_string()),
    ])
}

fn data_path(
    catalog: &Path,
    data_type: &str,
    instrument_id: &InstrumentId,
    ts_start: u64,
    ts_end: u64,
) -> std::path::PathBuf {
    catalog
        .join("data")
        .join(data_type)
        .join(instrument_id.to_string())
        .join(format!("{ts_start}-{ts_end}.parquet"))
}

fn write_batch(batch: &RecordBatch, path: &Path) -> Result<()> {
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let file = fs::File::create(path)
        .with_context(|| format!("cannot create {}", path.display()))?;

    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))
        .context("failed to create ArrowWriter")?;

    writer.write(batch).context("failed to write RecordBatch")?;
    writer.close().context("failed to close ArrowWriter")?;

    Ok(())
}
