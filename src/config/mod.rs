use serde::Deserialize;
use std::fs;
use anyhow::Context;

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

/// Logging configuration — required, no defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub log_dir: String,
    pub log_level: String,
    pub log_to_file: bool,
}

/// Historical data source — required, no defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct DataConfig {
    pub data_dir: String,
    /// InstrumentId string, e.g. "BTC-USDT.BINANCE"
    pub symbol: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// Number of decimal places for prices
    pub price_precision: u8,
    /// Number of decimal places for sizes/quantities
    pub size_precision: u8,
}

/// Simulated venue (backtest only).
#[derive(Debug, Clone, Deserialize)]
pub struct VenueConfig {
    /// Venue name, e.g. "BINANCE"
    pub name: String,
    /// "Netting" | "Hedging"
    pub oms_type: String,
    /// "Cash" | "Margin"
    pub account_type: String,
    /// "L1_MBP" | "L2_MBP" | "L3_MBO"
    pub book_type: String,
    /// Starting balance, e.g. "100000 USDT"
    pub starting_balance: String,
}

/// Live exchange connection (paper / live).
#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeConfig {
    pub venue: String,
    pub ws_url: String,
    pub rest_url: String,
    pub api_key: String,
    pub api_secret: String,
}

// ---------------------------------------------------------------------------
// GLFT strategy parameters — no Default, all values must be in the config file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct GlftConfig {
    /// Risk-aversion coefficient γ
    pub gamma: f64,
    /// Order-book depth parameter κ (controls order arrival rate)
    pub kappa: f64,
    /// Initial volatility estimate σ (annualised); overridden by live estimate once ready
    pub sigma: f64,
    /// Optimisation time horizon T (seconds)
    pub time_horizon_secs: u64,
    /// Minimum half-spread floor in basis points
    pub min_spread_bps: f64,
    /// Maximum absolute inventory (base currency)
    pub max_inventory: f64,
    /// Quote size per order (base currency)
    pub order_size: f64,
    /// Minimum time between quote refreshes (milliseconds)
    pub quote_refresh_ms: u64,
    /// Rolling window size for volatility estimation
    pub vol_window: u64,
    /// Pause quoting when current_vol > base_vol × this multiplier
    pub volatility_regime_multiplier: f64,
}

// ---------------------------------------------------------------------------
// Per-runner config types
// ---------------------------------------------------------------------------

/// Config for `market-maker backtest`.
#[derive(Debug, Clone, Deserialize)]
pub struct BacktestConfig {
    pub data: DataConfig,
    pub venue: VenueConfig,
    pub logging: LoggingConfig,
    /// Strategy parameters live directly under [strategy]
    pub strategy: GlftConfig,
}

/// Config for `market-maker paper`.
#[derive(Debug, Clone, Deserialize)]
pub struct PaperConfig {
    pub exchange: ExchangeConfig,
    pub logging: LoggingConfig,
    pub strategy: GlftConfig,
}

/// Config for `market-maker live`.
#[derive(Debug, Clone, Deserialize)]
pub struct LiveConfig {
    pub exchange: ExchangeConfig,
    pub logging: LoggingConfig,
    pub strategy: GlftConfig,
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

pub fn load_backtest_config(path: &str) -> anyhow::Result<BacktestConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Cannot read backtest config: {}", path))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse backtest config: {}", path))
}

pub fn load_paper_config(path: &str) -> anyhow::Result<PaperConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Cannot read paper config: {}", path))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse paper config: {}", path))
}

pub fn load_live_config(path: &str) -> anyhow::Result<LiveConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Cannot read live config: {}", path))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse live config: {}", path))
}
