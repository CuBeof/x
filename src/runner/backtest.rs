use std::path::PathBuf;
use log::{info, warn};

use crate::config::AppConfig;
use crate::data::{DataFeed, load_quotes, load_bars};
use crate::engine::Engine;
use crate::runner::Runner;
use crate::strategies::{Strategy, StrategySummary};

pub struct BacktestRunner {
    config: AppConfig,
}

impl BacktestRunner {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

impl Runner for BacktestRunner {
    fn run(&mut self, mut strategy: Box<dyn Strategy>) -> anyhow::Result<StrategySummary> {
        info!("Starting backtest for symbol: {}", self.config.data.symbol);

        let data_dir = PathBuf::from(&self.config.data.data_dir);

        // Convert symbol "BTC-USDT.BINANCE" → "btc_usdt_binance"
        let symbol_file_stem = self
            .config
            .data
            .symbol
            .replace('.', "_")
            .replace('-', "_")
            .to_lowercase();

        let quotes_path = data_dir.join(format!("{}_quotes.csv", symbol_file_stem));
        let bars_path = data_dir.join(format!("{}_bars.csv", symbol_file_stem));

        let feed = if quotes_path.exists() {
            info!("Loading quotes from {}", quotes_path.display());
            let quotes = load_quotes(&quotes_path)?;
            info!("Loaded {} quotes", quotes.len());
            DataFeed::from_quotes(quotes)
        } else if bars_path.exists() {
            info!("Loading bars from {}", bars_path.display());
            let bars = load_bars(&bars_path)?;
            info!("Loaded {} bars", bars.len());
            DataFeed::from_bars(bars)
        } else {
            warn!(
                "No data files found. Expected: {} or {}. Running with empty feed.",
                quotes_path.display(),
                bars_path.display()
            );
            DataFeed::from_quotes(vec![])
        };

        let mut engine = Engine::new(self.config.strategy.glft.maker_fee_override());
        let total = feed.len();
        info!("Starting replay of {} market events", total);

        strategy.on_start();

        for (i, event) in feed.enumerate() {
            let order_events = engine.process_market_event(&event);
            strategy.on_market_event(&event, &mut engine);
            for oe in order_events {
                strategy.on_order_event(&oe);
            }
            if total > 0 && (i + 1) % 10_000 == 0 {
                info!(
                    "Progress: {}/{} events ({:.1}%)",
                    i + 1,
                    total,
                    (i + 1) as f64 / total as f64 * 100.0
                );
            }
        }

        strategy.on_stop();
        let summary = strategy.summary();

        info!("=== Backtest Complete ===");
        info!("Realized PnL:   {:.6}", summary.realized_pnl);
        info!("Unrealized PnL: {:.6}", summary.unrealized_pnl);
        info!("Total PnL:      {:.6}", summary.total_pnl);
        info!("Total Trades:   {}", summary.total_trades);
        info!("Inventory:      {:.6}", summary.inventory);
        info!("Sharpe Ratio:   {:.4}", summary.sharpe_ratio);
        info!("Max Drawdown:   {:.4}", summary.max_drawdown);

        Ok(summary)
    }
}
