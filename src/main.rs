mod config;
mod strategies;

use anyhow::{bail, Context};
use clap::Parser;
use log::LevelFilter;
use nautilus_backtest::{
    config::{
        BacktestDataConfig, BacktestEngineConfig, BacktestRunConfig, BacktestVenueConfig,
        NautilusDataType,
    },
    node::BacktestNode,
};
use nautilus_common::logging::logger::LoggerConfig;
use nautilus_common::logging::writer::{FileRotateConfig, FileWriterConfig};
use nautilus_model::{
    enums::{AccountType, BookType, OmsType},
    identifiers::InstrumentId,
};
use std::path::PathBuf;
use std::str::FromStr;
use ustr::Ustr;

use config::{BacktestConfig, DataType};
use strategies::glft::GlftStrategy;

#[derive(Parser)]
#[command(about = "Nautilus Trader backtest runner")]
struct Args {
    /// Path to TOML config file
    #[arg(short, long, default_value = "configs/backtest.toml")]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cfg = BacktestConfig::from_file(&args.config)?;

    let instrument_id = InstrumentId::from_str(&cfg.instrument.id)
        .with_context(|| format!("invalid instrument id: {}", cfg.instrument.id))?;

    let oms_type = match cfg.venue.oms_type.as_str() {
        "Hedging" => OmsType::Hedging,
        "Netting" => OmsType::Netting,
        other => bail!("unsupported oms_type: {other}"),
    };
    let account_type = match cfg.venue.account_type.as_str() {
        "Margin" => AccountType::Margin,
        "Cash" => AccountType::Cash,
        other => bail!("unsupported account_type: {other}"),
    };
    let book_type = match cfg.venue.book_type.as_str() {
        "L1_MBP" => BookType::L1_MBP,
        "L2_MBP" => BookType::L2_MBP,
        "L3_MBO" => BookType::L3_MBO,
        other => bail!("unsupported book_type: {other}"),
    };

    let venue_config = BacktestVenueConfig::builder()
        .name(Ustr::from(&cfg.venue.name))
        .oms_type(oms_type)
        .account_type(account_type)
        .book_type(book_type)
        .starting_balances(cfg.venue.starting_balances.clone())
        .build();

    let data_configs: Vec<BacktestDataConfig> = cfg
        .data
        .types
        .iter()
        .map(|dt| {
            let data_type = match dt {
                DataType::QuoteTick => NautilusDataType::QuoteTick,
                DataType::OrderBookDelta => NautilusDataType::OrderBookDelta,
                DataType::TradeTick => NautilusDataType::TradeTick,
            };
            BacktestDataConfig::builder()
                .data_type(data_type)
                .catalog_path(cfg.catalog.path.clone())
                .instrument_id(instrument_id)
                .build()
        })
        .collect();

    let stdout_level = LevelFilter::from_str(&cfg.logging.stdout_level)
        .with_context(|| format!("invalid stdout_level: {}", cfg.logging.stdout_level))?;
    let fileout_level = LevelFilter::from_str(&cfg.logging.fileout_level)
        .with_context(|| format!("invalid fileout_level: {}", cfg.logging.fileout_level))?;

    let file_config = FileWriterConfig {
        directory: Some(cfg.logging.log_dir.clone()),
        file_name: Some(instrument_id.to_string().replace(".", "_")),
        file_rotate: Some(FileRotateConfig::from((
            cfg.logging.max_file_size_mb * 1_000_000,
            cfg.logging.max_backups,
        ))),
        ..Default::default()
    };

    let logging = LoggerConfig::builder()
        .stdout_level(stdout_level)
        .fileout_level(fileout_level)
        .file_config(file_config)
        .is_colored(true)
        .print_config(true)
        .build();

    let engine_config = BacktestEngineConfig::builder().logging(logging).build();

    let run_config = BacktestRunConfig::builder()
        .id(cfg.engine.run_id.clone())
        .venues(vec![venue_config])
        .data(data_configs)
        .engine(engine_config)
        .chunk_size(cfg.engine.chunk_size)
        .build();

    let mut node = BacktestNode::new(vec![run_config])?;
    node.build()?;

    let engine = node.get_engine_mut(&cfg.engine.run_id).unwrap();

    let strategy = GlftStrategy::new(instrument_id, None);
    engine.add_strategy(strategy)?;

    let results = node.run()?;

    println!("Backtest complete!");
    println!("Iterations: {}", results[0].iterations);
    println!("Total orders: {}", results[0].total_orders);

    Ok(())
}
