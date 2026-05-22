use log::{info, warn};
use std::path::PathBuf;

use nautilus_backtest::{
    config::{BacktestEngineConfig, SimulatedVenueConfig},
    engine::BacktestEngine,
};
use nautilus_model::{
    enums::{AccountType, BookType, OmsType},
    identifiers::{InstrumentId, Venue},
    instruments::{InstrumentAny, currency_pair::CurrencyPair},
    types::{Currency, Money, Price, Quantity},
};
use nautilus_core::UnixNanos;

use crate::config::{BacktestConfig, VenueConfig};
use crate::data::{load_quotes, load_bars_as_quotes};
use crate::strategies::glft::GlftStrategy;

/// Build a minimal CurrencyPair stub for the given instrument_id.
/// In production you would load this from an exchange REST API or catalog.
fn build_instrument(
    instrument_id: InstrumentId,
    price_precision: u8,
    size_precision: u8,
) -> anyhow::Result<InstrumentAny> {
    // Parse base/quote from symbol like "BTC-USDT" (part before the dot)
    let symbol_str = instrument_id.symbol.inner().as_str();
    let (base_str, quote_str) = symbol_str
        .split_once('-')
        .unwrap_or((symbol_str, "USDT"));

    let base = Currency::from(base_str);
    let quote = Currency::from(quote_str);

    let price_increment = Price::new(10f64.powi(-(price_precision as i32)), price_precision);
    let size_increment = Quantity::new(10f64.powi(-(size_precision as i32)), size_precision);

    let pair = CurrencyPair::new_checked(
        instrument_id,
        instrument_id.symbol,
        base,
        quote,
        price_precision,
        size_precision,
        price_increment,
        size_increment,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        UnixNanos::default(),
        UnixNanos::default(),
    )?;
    Ok(InstrumentAny::CurrencyPair(pair))
}

/// Parse VenueConfig strings into nautilus enums.
fn parse_venue_config(vcfg: &VenueConfig) -> anyhow::Result<SimulatedVenueConfig> {
    let venue = Venue::from(vcfg.name.as_str());

    let oms_type = match vcfg.oms_type.as_str() {
        "Netting"  => OmsType::Netting,
        "Hedging"  => OmsType::Hedging,
        other      => anyhow::bail!("Unknown oms_type: {}", other),
    };
    let account_type = match vcfg.account_type.as_str() {
        "Cash"   => AccountType::Cash,
        "Margin" => AccountType::Margin,
        other    => anyhow::bail!("Unknown account_type: {}", other),
    };
    let book_type = match vcfg.book_type.as_str() {
        "L1_MBP" => BookType::L1_MBP,
        "L2_MBP" => BookType::L2_MBP,
        "L3_MBO" => BookType::L3_MBO,
        other    => anyhow::bail!("Unknown book_type: {}", other),
    };

    Ok(SimulatedVenueConfig::builder()
        .venue(venue)
        .oms_type(oms_type)
        .account_type(account_type)
        .book_type(book_type)
        .starting_balances(vec![Money::from(vcfg.starting_balance.as_str())])
        .build())
}

pub fn run_backtest(cfg: &BacktestConfig) -> anyhow::Result<()> {
    info!(
        "Initialising BacktestEngine | symbol={} venue={}",
        cfg.data.symbol, cfg.venue.name
    );

    let mut engine = BacktestEngine::new(BacktestEngineConfig::default())?;

    engine.add_venue(parse_venue_config(&cfg.venue)?)?;

    let instrument_id = InstrumentId::from(cfg.data.symbol.as_str());
    let price_precision = cfg.data.price_precision;
    let size_precision  = cfg.data.size_precision;

    let instrument = build_instrument(instrument_id, price_precision, size_precision)?;
    engine.add_instrument(&instrument)?;

    // --- Load data ---
    let data_dir = PathBuf::from(&cfg.data.data_dir);
    let stem = cfg.data.symbol
        .replace('.', "_")
        .replace('-', "_")
        .to_lowercase();

    let quotes_path = data_dir.join(format!("{}_quotes.csv", stem));
    let bars_path   = data_dir.join(format!("{}_bars.csv", stem));

    let data = if quotes_path.exists() {
        info!("Loading quotes from {}", quotes_path.display());
        let q = load_quotes(&quotes_path, instrument_id, price_precision, size_precision)?;
        info!("Loaded {} QuoteTicks", q.len());
        q
    } else if bars_path.exists() {
        info!("Loading bars from {} (synthetic quotes)", bars_path.display());
        let q = load_bars_as_quotes(&bars_path, instrument_id, price_precision, size_precision)?;
        info!("Loaded {} synthetic QuoteTicks", q.len());
        q
    } else {
        warn!(
            "No data found at {} or {}. Running empty backtest.",
            quotes_path.display(),
            bars_path.display()
        );
        vec![]
    };

    if !data.is_empty() {
        engine.add_data(data, None, true, true)?;
    }

    let strategy = GlftStrategy::new(instrument_id, cfg.strategy.clone());
    engine.add_strategy(strategy)?;

    info!("Starting backtest replay...");
    engine.run(None, None, None, false)?;

    info!("Backtest complete");
    Ok(())
}
