use glft_mm_rust::config::GlftMarketMakerConfig;
use glft_mm_rust::strategy::GlftMarketMaker;
use nautilus_backtest::{
    config::{
        BacktestDataConfig, BacktestEngineConfig, BacktestRunConfig, BacktestVenueConfig,
        NautilusDataType,
    },
    node::BacktestNode,
};
use nautilus_core::datetime::iso8601_to_unix_nanos;
use nautilus_data::engine::config::DataEngineConfig;
use nautilus_model::{
    enums::{AccountType, BookType, OmsType},
    identifiers::InstrumentId,
};

fn main() -> anyhow::Result<()> {
    let instrument_id = InstrumentId::from("ETHUSDT-PERP.SIM");
    let start = iso8601_to_unix_nanos("2026-05-22T00:00:00Z")?;
    let end = iso8601_to_unix_nanos("2026-05-22T16:00:00Z")?;

    let venue_config = BacktestVenueConfig::builder()
        .name("SIM".into())
        .oms_type(OmsType::Netting)
        .account_type(AccountType::Margin)
        .book_type(BookType::L2_MBP)
        .starting_balances(vec!["1_000 USDT".to_string()])
        .trade_execution(true)
        .queue_position(true)
        .build();

    let book_data_config = BacktestDataConfig::builder()
        .data_type(NautilusDataType::OrderBookDelta)
        .catalog_path("catalog".to_string())
        .instrument_id(instrument_id)
        .start_time(start)
        .end_time(end)
        .build();

    let trade_data_config = BacktestDataConfig::builder()
        .data_type(NautilusDataType::TradeTick)
        .catalog_path("catalog".to_string())
        .instrument_id(instrument_id)
        .start_time(start)
        .end_time(end)
        .build();

    let engine_config = BacktestEngineConfig::builder()
        .data_engine(
            DataEngineConfig::builder()
                .emit_quotes_from_book(true)
                .build(),
        )
        .build();

    let run_config = BacktestRunConfig::builder()
        .engine(engine_config)
        .venues(vec![venue_config])
        .data(vec![book_data_config, trade_data_config])
        .start(start)
        .end(end)
        .chunk_size(10_000)
        .build();

    let mut node = BacktestNode::new(vec![run_config])?;
    node.build()?;

    let run_id = node.configs()[0].id().to_string();
    let strategy_config = GlftMarketMakerConfig::new(instrument_id);
    let engine = node.get_engine_mut(&run_id).expect("engine not found");
    engine.add_strategy(GlftMarketMaker::new(strategy_config))?;

    let results = node.run()?;
    for result in &results {
        println!("Run ID:           {:?}", result.run_id);
        println!("Iterations:       {}", result.iterations);
        println!("Total orders:     {}", result.total_orders);
        println!("Total positions:  {}", result.total_positions);
        println!("Elapsed time:     {:.2}s", result.elapsed_time_secs);
        println!("PnL stats:        {:?}", result.stats_pnls);
    }

    Ok(())
}
