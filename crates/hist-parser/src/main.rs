mod orderbook;
mod trades;
mod writer;

use anyhow::{Context, Result};
use clap::Parser;
use nautilus_model::identifiers::InstrumentId;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "hist-parser",
    about = "Parse historical OKX L2 orderbook (NDJSON) and trades (CSV) into Nautilus parquet catalog"
)]
struct Args {
    /// Instrument ID, e.g. "ETHUSDT-PERP.OKX"
    #[arg(short, long)]
    instrument: String,

    /// Path to orderbook NDJSON file
    #[arg(long)]
    orderbook: Option<PathBuf>,

    /// Path to trades CSV file
    #[arg(long)]
    trades: Option<PathBuf>,

    /// Output catalog root path (default: data/catalog)
    #[arg(short, long, default_value = "data/catalog")]
    output: PathBuf,

    /// Price decimal precision (auto-detected from first 200 lines if omitted)
    #[arg(long)]
    price_precision: Option<u8>,

    /// Size decimal precision (auto-detected from first 200 lines if omitted)
    #[arg(long)]
    size_precision: Option<u8>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let instrument_id = InstrumentId::from_str(&args.instrument)
        .with_context(|| format!("invalid instrument id: {}", args.instrument))?;

    if args.orderbook.is_none() && args.trades.is_none() {
        anyhow::bail!("at least one of --orderbook or --trades must be provided");
    }

    // --- orderbook ----------------------------------------------------------
    if let Some(ob_path) = &args.orderbook {
        println!("Parsing orderbook: {}", ob_path.display());
        let result = orderbook::parse(
            ob_path,
            &instrument_id,
            args.price_precision,
            args.size_precision,
        )
        .with_context(|| format!("failed to parse orderbook: {}", ob_path.display()))?;

        println!(
            "  parsed {} deltas  (price_prec={}, size_prec={})",
            result.deltas.len(),
            result.price_precision,
            result.size_precision,
        );

        writer::write_order_book_deltas(
            &args.output,
            &instrument_id,
            result.price_precision,
            result.size_precision,
            result.deltas,
        )
        .context("failed to write orderbook parquet")?;

        println!("  written to {}", args.output.display());
    }

    // --- trades -------------------------------------------------------------
    if let Some(tr_path) = &args.trades {
        println!("Parsing trades: {}", tr_path.display());
        let result = trades::parse(
            tr_path,
            &instrument_id,
            args.price_precision,
            args.size_precision,
        )
        .with_context(|| format!("failed to parse trades: {}", tr_path.display()))?;

        println!(
            "  parsed {} trade ticks  (price_prec={}, size_prec={})",
            result.ticks.len(),
            result.price_precision,
            result.size_precision,
        );

        writer::write_trade_ticks(
            &args.output,
            &instrument_id,
            result.price_precision,
            result.size_precision,
            result.ticks,
        )
        .context("failed to write trades parquet")?;

        println!("  written to {}", args.output.display());
    }

    println!("Done.");
    Ok(())
}
