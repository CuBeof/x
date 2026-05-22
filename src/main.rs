use clap::Parser;
use log::info;
use anyhow::Context;

mod config;
mod logging;
mod data;
mod runner;
mod strategies;

use config::{load_config, RunMode};

#[derive(Parser)]
#[command(name = "market-maker", about = "GLFT Market Maker Bot (nautilus_trader)")]
struct Cli {
    #[arg(short, long, default_value = "config/backtest.toml")]
    config: String,
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

    let _log_handle = logging::init_logging(&cfg.logging)?;

    info!(
        "Starting market-maker in {:?} mode | symbol={}",
        cfg.run_mode, cfg.data.symbol
    );

    match cfg.run_mode {
        RunMode::Backtest => runner::backtest::run_backtest(&cfg)?,
        RunMode::Paper => runner::paper::run_paper(&cfg)?,
        RunMode::Live => runner::live::run_live(&cfg)?,
    }

    Ok(())
}
