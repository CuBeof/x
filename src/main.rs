mod strategies;

use anyhow::Context;
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

use strategies::glft::GlftStrategy;

fn main() -> anyhow::Result<()> {
    // 1. Create instrument and generate sample data
    let instrument_id = InstrumentId::from_str("ETHUSDT-PERP.SIM")?;

    let catalog_dir = PathBuf::from("data/catalog");
    let catalog_path = catalog_dir
        .to_str()
        .context("catalog path not valid UTF-8")?
        .to_string();

    let venue_config = BacktestVenueConfig::builder()
        .name(Ustr::from("SIM"))
        .oms_type(OmsType::Hedging)
        .account_type(AccountType::Margin)
        .book_type(BookType::L1_MBP)
        .starting_balances(vec!["1_000 USDT".to_string()])
        .build();

    let book_data = BacktestDataConfig::builder()
        .data_type(NautilusDataType::OrderBookDelta)
        .catalog_path(catalog_path.clone())
        .instrument_id(instrument_id)
        .build();

    let trade_data = BacktestDataConfig::builder()
        .data_type(NautilusDataType::TradeTick)
        .catalog_path(catalog_path)
        .instrument_id(instrument_id)
        .build();

    let file_config = FileWriterConfig {
        directory: Some("logs".to_string()),
        file_name: Some(instrument_id.to_string().replace(".", "_")),
        file_rotate: Some(FileRotateConfig::from((
            500_000_000u64, // max file size: 500 MB
            5u32,           // max backup count
        ))),
        ..Default::default()
    };

    let logging = LoggerConfig::builder()
        .stdout_level(LevelFilter::Info)
        .fileout_level(LevelFilter::Debug)
        .file_config(file_config)
        .is_colored(true)
        .print_config(true)
        .build();

    let engine_config = BacktestEngineConfig::builder().logging(logging).build();

    let run_config = BacktestRunConfig::builder()
        .id("okx-backtest".to_string())
        .venues(vec![venue_config])
        .data(vec![book_data, trade_data])
        .engine(engine_config)
        .chunk_size(10_000)
        .build();

    let mut node = BacktestNode::new(vec![run_config])?;
    node.build()?;

    let engine = node.get_engine_mut("okx-backtest").unwrap();

    let strategy = GlftStrategy::new(instrument_id.clone(), None);
    engine.add_strategy(strategy)?;
    // 7. Run backtest
    let results = node.run()?;

    println!("Backtest complete!");
    println!("Iterations: {}", results[0].iterations);
    println!("Total orders: {}", results[0].total_orders);

    Ok(())
}
