use serde::{Deserialize, Serialize};
use std::fs;
use anyhow::Context;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum RunMode {
    Backtest,
    Paper,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExchangeConfig {
    pub ws_url: Option<String>,
    pub rest_url: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub run_mode: RunMode,
    pub data: DataConfig,
    pub logging: LoggingConfig,
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub exchange: ExchangeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    pub data_dir: String,
    pub symbol: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub price_precision: Option<u8>,
    pub size_precision: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub log_dir: String,
    pub log_level: String,
    pub log_to_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub glft: GlftConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlftConfig {
    pub gamma: f64,
    pub kappa: f64,
    pub sigma: f64,
    pub time_horizon_secs: u64,
    pub min_spread_bps: f64,
    pub max_inventory: f64,
    pub order_size: f64,
    pub quote_refresh_ms: u64,
    pub vol_window: u64,
    pub volatility_regime_multiplier: f64,
}

impl GlftConfig {
    /// Post-only orders are maker orders; maker fee is typically 0% on most exchanges.
    pub fn maker_fee_override(&self) -> f64 {
        0.0
    }
}

pub fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;
    let config: AppConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path))?;
    Ok(config)
}
