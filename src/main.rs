use clap::{Parser, Subcommand};
use log::info;
use anyhow::Context;

mod config;
mod logging;
mod data;
mod runner;
mod strategies;

use config::{load_backtest_config, load_paper_config, load_live_config};

#[derive(Parser)]
#[command(
    name = "market-maker",
    about = "GLFT Market Maker Bot — powered by nautilus_trader",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a historical backtest
    Backtest {
        /// Path to backtest config file
        #[arg(short, long, default_value = "config/backtest.toml")]
        config: String,
    },
    /// Run in paper-trading mode (live market data, simulated orders)
    Paper {
        /// Path to paper config file
        #[arg(short, long, default_value = "config/paper.toml")]
        config: String,
    },
    /// Run in live-trading mode
    Live {
        /// Path to live config file
        #[arg(short, long, default_value = "config/live.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Backtest { config } => {
            let cfg = load_backtest_config(&config)
                .with_context(|| format!("Failed to load backtest config: {}", config))?;
            let _log = logging::init_logging(&cfg.logging)?;
            info!(
                "Backtest | symbol={} venue={} gamma={} kappa={} sigma={}",
                cfg.data.symbol,
                cfg.venue.name,
                cfg.strategy.gamma,
                cfg.strategy.kappa,
                cfg.strategy.sigma,
            );
            runner::backtest::run_backtest(&cfg)?;
        }

        Command::Paper { config } => {
            let cfg = load_paper_config(&config)
                .with_context(|| format!("Failed to load paper config: {}", config))?;
            let _log = logging::init_logging(&cfg.logging)?;
            info!(
                "Paper | venue={} gamma={} kappa={}",
                cfg.exchange.venue, cfg.strategy.gamma, cfg.strategy.kappa,
            );
            runner::paper::run_paper(&cfg)?;
        }

        Command::Live { config } => {
            let cfg = load_live_config(&config)
                .with_context(|| format!("Failed to load live config: {}", config))?;
            let _log = logging::init_logging(&cfg.logging)?;
            info!(
                "Live | venue={} gamma={} kappa={}",
                cfg.exchange.venue, cfg.strategy.gamma, cfg.strategy.kappa,
            );
            runner::live::run_live(&cfg)?;
        }
    }

    Ok(())
}
