use clap::Parser;
use log::info;
use anyhow::Context;

mod config;
mod logging;
mod data;
mod engine;
mod runner;
mod strategies;

use config::{load_config, RunMode};
use strategies::glft::GlftStrategy;
use runner::Runner;

#[derive(Parser)]
#[command(name = "market-maker", about = "GLFT Market Maker Bot")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config/backtest.toml")]
    config: String,

    /// Override run mode (backtest, paper, live)
    #[arg(short, long)]
    mode: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut cfg = load_config(&cli.config).context("Failed to load config")?;

    if let Some(mode) = &cli.mode {
        cfg.run_mode = match mode.to_lowercase().as_str() {
            "backtest" => RunMode::Backtest,
            "paper" => RunMode::Paper,
            "live" => RunMode::Live,
            other => anyhow::bail!("Unknown mode: {}", other),
        };
    }

    let _guard = logging::init_logging(&cfg.logging)?;

    info!("Starting market-maker in {:?} mode", cfg.run_mode);
    info!("Strategy: GLFT | Symbol: {}", cfg.data.symbol);
    info!(
        "gamma={} kappa={} sigma={}",
        cfg.strategy.glft.gamma,
        cfg.strategy.glft.kappa,
        cfg.strategy.glft.sigma
    );

    let strategy = Box::new(GlftStrategy::new(cfg.strategy.glft.clone()));

    let summary = match cfg.run_mode {
        RunMode::Backtest => {
            let mut runner = runner::backtest::BacktestRunner::new(cfg);
            runner.run(strategy)?
        }
        RunMode::Paper => {
            let mut runner = runner::paper::PaperRunner::new(cfg);
            runner.run(strategy)?
        }
        RunMode::Live => {
            let mut runner = runner::live::LiveRunner::new(cfg);
            runner.run(strategy)?
        }
    };

    println!("\n=== Final Summary ===");
    println!("Realized PnL:   {:.6}", summary.realized_pnl);
    println!("Unrealized PnL: {:.6}", summary.unrealized_pnl);
    println!("Total Trades:   {}", summary.total_trades);
    println!("Inventory:      {:.6}", summary.inventory);
    println!("Sharpe Ratio:   {:.4}", summary.sharpe_ratio);
    println!("Max Drawdown:   {:.4}", summary.max_drawdown);

    Ok(())
}
