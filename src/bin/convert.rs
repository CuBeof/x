use anyhow::{Context, Result};
use clap::{Arg, Command};
use csv::ReaderBuilder;
use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{BookOrder, OrderBookDelta, TradeTick},
    enums::{AggressorSide, BookAction, OrderSide, RecordFlag},
    identifiers::{InstrumentId, Symbol, TradeId, Venue},
    instruments::{CryptoPerpetual, Instrument},
    types::{Currency, Money, Price, Quantity},
};
use nautilus_persistence::backend::catalog::ParquetDataCatalog;
use rust_decimal_macros::dec;
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

const DELTA_CHUNK: usize = 5_000_000;
const TRADE_CHUNK: usize = 5_000_000;
const MS_TO_NS: u64 = 1_000_000;

const F_LAST: u8 = RecordFlag::F_LAST as u8;
const F_SNAPSHOT: u8 = RecordFlag::F_SNAPSHOT as u8;

struct Args {
    book: Option<String>,

    trade: Option<String>,

    output: String,
}

impl Args {
    fn parse() -> Self {
        let matches = Command::new("okx_to_parquet")
            .about("Convert OKX historical order book and trade data to Nautilus Parquet catalog")
            .arg(
                Arg::new("book")
                    .long("order-book")
                    .visible_alias("book")
                    .help("Path to order book ndjson file")
                    .value_name("PATH"),
            )
            .arg(
                Arg::new("trade")
                    .long("trade")
                    .help("Path to trades CSV file")
                    .value_name("PATH"),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .help("Output directory for catalog")
                    .default_value(".")
                    .value_name("DIR"),
            )
            .get_matches();

        Self {
            book: matches.get_one::<String>("book").cloned(),
            trade: matches.get_one::<String>("trade").cloned(),
            output: matches
                .get_one::<String>("output")
                .cloned()
                .unwrap_or_else(|| ".".to_string()),
        }
    }
}

#[derive(Deserialize)]
struct OkxBookMsg {
    ts: String,
    action: String,
    bids: Option<Vec<[String; 3]>>,
    asks: Option<Vec<[String; 3]>>,
}

#[derive(Deserialize)]
struct TradeRecord {
    created_time: String,
    price: String,
    size: String,
    side: String,
    trade_id: String,
}

fn ethusdt_perp_sim() -> CryptoPerpetual {
    CryptoPerpetual::new(
        InstrumentId::new(Symbol::from("ETHUSDT-PERP"), Venue::from("SIM")),
        Symbol::from("ETHUSDT"),
        Currency::from("ETH"),
        Currency::from("USDT"),
        Currency::from("USDT"),
        false, // is_inverse
        2,     // price_precision
        2,     // size_precision
        Price::from("0.01"),
        Quantity::from("0.01"),
        None, // multiplier
        None, // lot_size
        Some(Quantity::from("10000.000")),
        Some(Quantity::from("0.01")),
        None, // max_notional
        Some(Money::new(10.00, Currency::from("USDT"))),
        Some(Price::from("152588.43")),
        Some(Price::from("29.91")),
        Some(dec!(1.00)),   // margin_init
        Some(dec!(0.35)),   // margin_maint
        Some(dec!(0.0000)), // maker_fee
        Some(dec!(0.0004)), // taker_fee
        None,               // info
        UnixNanos::from(1646199312128000000),
        UnixNanos::from(1646199342953849862),
    )
}

fn to_ns(ms: &str) -> Result<u64> {
    let ms_val: u64 = ms.parse().context("Failed to parse timestamp")?;
    Ok(ms_val * MS_TO_NS)
}

fn parse_ob_line(
    line: &str,
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
    last_ts: &mut u64,
) -> Result<Vec<OrderBookDelta>> {
    let msg: OkxBookMsg = serde_json::from_str(line).context("Failed to parse JSON")?;
    let ts_base = to_ns(&msg.ts)?;
    let bids = msg.bids.unwrap_or_default();
    let asks = msg.asks.unwrap_or_default();
    let mut deltas = Vec::new();

    let mut next_ts = || {
        let ts = ts_base.max(*last_ts + 1);
        *last_ts = ts;
        ts
    };

    if msg.action == "snapshot" {
        let levels: Vec<(OrderSide, &[String; 3])> = bids
            .iter()
            .map(|b| (OrderSide::Buy, b))
            .chain(asks.iter().map(|a| (OrderSide::Sell, a)))
            .collect();

        let no_levels = levels.is_empty();

        let ts = next_ts();
        deltas.push(OrderBookDelta::new(
            instrument_id,
            BookAction::Clear,
            BookOrder::default(),
            F_SNAPSHOT | if no_levels { F_LAST } else { 0 },
            0,
            UnixNanos::from(ts),
            UnixNanos::from(ts),
        ));

        for (i, (side, level)) in levels.iter().enumerate() {
            let is_last = i == levels.len() - 1;
            let price = Price::new(level[0].parse().unwrap_or(0.0), price_precision);
            let size = Quantity::new(level[1].parse().unwrap_or(0.0), size_precision);
            let order = BookOrder::new(*side, price, size, 0);

            let ts = next_ts();
            deltas.push(OrderBookDelta::new(
                instrument_id,
                BookAction::Add,
                order,
                F_SNAPSHOT | if is_last { F_LAST } else { 0 },
                0,
                UnixNanos::from(ts),
                UnixNanos::from(ts),
            ));
        }
    } else {
        let levels: Vec<(OrderSide, &[String; 3])> = bids
            .iter()
            .map(|b| (OrderSide::Buy, b))
            .chain(asks.iter().map(|a| (OrderSide::Sell, a)))
            .collect();

        for (i, (side, level)) in levels.iter().enumerate() {
            let is_last = i == levels.len() - 1;
            let price = Price::new(level[0].parse().unwrap_or(0.0), price_precision);
            let size_val: f64 = level[1].parse().unwrap_or(0.0);

            let (action, size) = if size_val == 0.0 {
                (BookAction::Delete, Quantity::new(0.0, size_precision))
            } else {
                (BookAction::Update, Quantity::new(size_val, size_precision))
            };

            let order = BookOrder::new(*side, price, size, 0);

            let ts = next_ts();
            deltas.push(OrderBookDelta::new(
                instrument_id,
                action,
                order,
                if is_last { F_LAST } else { 0 },
                0,
                UnixNanos::from(ts),
                UnixNanos::from(ts),
            ));
        }
    }

    Ok(deltas)
}

fn process_orderbook(
    ob_path: &Path,
    catalog: &ParquetDataCatalog,
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> Result<()> {
    let file = File::open(ob_path).context("Failed to open order book file")?;
    let reader = BufReader::new(file);

    let mut chunk = Vec::new();
    let mut total = 0;
    let mut last_ts = 0;

    for line in reader.lines() {
        let line = line.context("Failed to read line")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let deltas = parse_ob_line(
            line,
            instrument_id,
            price_precision,
            size_precision,
            &mut last_ts,
        )?;
        chunk.extend(deltas);

        if chunk.len() >= DELTA_CHUNK {
            catalog
                .write_to_parquet(chunk.clone(), None, None, Some(true))
                .context("Failed to write deltas to parquet")?;
            total += chunk.len();
            println!("  {} deltas written...", total);
            chunk.clear();
        }
    }

    if !chunk.is_empty() {
        let chunk_len = chunk.len();
        catalog
            .write_to_parquet(chunk, None, None, Some(true))
            .context("Failed to write deltas to parquet")?;
        total += chunk_len;
    }

    println!("  Total: {} order book deltas", total);
    Ok(())
}

fn process_trades(
    trade_path: &Path,
    catalog: &ParquetDataCatalog,
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> Result<()> {
    let file = File::open(trade_path).context("Failed to open trade file")?;
    let mut rdr = ReaderBuilder::new().from_reader(file);

    let mut chunk: Vec<TradeTick> = Vec::with_capacity(TRADE_CHUNK);
    let mut total = 0;
    let mut last_ts = 0;

    for result in rdr.deserialize() {
        let record: TradeRecord = result.context("Failed to parse CSV record")?;

        let ts_base = to_ns(&record.created_time)?;
        let ts = ts_base.max(last_ts + 1);
        last_ts = ts;

        let side = if record.side.to_lowercase() == "buy" {
            AggressorSide::Buyer
        } else {
            AggressorSide::Seller
        };

        let price = Price::new(record.price.parse().unwrap_or(0.0), price_precision);
        let size = Quantity::new(record.size.parse().unwrap_or(0.0), size_precision);
        let trade_id = TradeId::from(record.trade_id);

        chunk.push(TradeTick::new(
            instrument_id,
            price,
            size,
            side,
            trade_id,
            UnixNanos::from(ts),
            UnixNanos::from(ts),
        ));

        if chunk.len() >= TRADE_CHUNK {
            catalog
                .write_to_parquet(chunk.clone(), None, None, Some(true))
                .context("Failed to write ticks to parquet")?;
            total += chunk.len();
            println!("  {} ticks written...", total);
            chunk.clear();
        }
    }

    if !chunk.is_empty() {
        let chunk_len = chunk.len();
        catalog
            .write_to_parquet(chunk, None, None, Some(true))
            .context("Failed to write ticks to parquet")?;
        total += chunk_len;
    }

    println!("  Total: {} trade ticks", total);
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let output_path = Path::new(&args.output);
    std::fs::create_dir_all(output_path).context("Failed to create output directory")?;

    let catalog = ParquetDataCatalog::new(output_path, None, None, None, None);
    let instrument = ethusdt_perp_sim();

    if args.book.is_none() && args.trade.is_none() {
        println!("No input files provided. Use --order-book and/or --trade to specify input data.");
        return Ok(());
    }

    println!("Writing instrument definition...");
    catalog
        .write_instruments(vec![instrument.clone().into()])
        .context("Failed to write instrument")?;

    let price_precision = instrument.price_precision();
    let size_precision = instrument.size_precision();
    let instrument_id = instrument.id();

    if let Some(ref ob_path) = args.book {
        let path = Path::new(ob_path);
        if path.exists() {
            println!(
                "\nProcessing order book: {}",
                path.file_name().unwrap().to_string_lossy()
            );
            process_orderbook(
                path,
                &catalog,
                instrument_id,
                price_precision,
                size_precision,
            )?;
        } else {
            println!("Order book file not found: {}", ob_path);
        }
    }

    if let Some(ref trade_path) = args.trade {
        let path = Path::new(trade_path);
        if path.exists() {
            println!(
                "\nProcessing trades: {}",
                path.file_name().unwrap().to_string_lossy()
            );
            process_trades(
                path,
                &catalog,
                instrument_id,
                price_precision,
                size_precision,
            )?;
        } else {
            println!("Trade file not found: {}", trade_path);
        }
    }

    println!("\nDone!");
    Ok(())
}
