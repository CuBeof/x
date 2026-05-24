use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct BacktestConfig {
    pub catalog: CatalogConfig,
    pub instrument: InstrumentConfig,
    pub data: DataConfig,
    pub venue: VenueConfig,
    pub engine: EngineConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct CatalogConfig {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentConfig {
    pub id: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    QuoteTick,
    OrderBookDelta,
    TradeTick,
}

#[derive(Debug, Deserialize)]
pub struct DataConfig {
    pub types: Vec<DataType>,
}

#[derive(Debug, Deserialize)]
pub struct VenueConfig {
    pub name: String,
    pub oms_type: String,
    pub account_type: String,
    pub book_type: String,
    pub starting_balances: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct EngineConfig {
    pub run_id: String,
    pub chunk_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub stdout_level: String,
    pub fileout_level: String,
    pub log_dir: String,
    pub max_file_size_mb: u64,
    pub max_backups: u32,
}

impl BacktestConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read config file: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("invalid TOML in config file: {}", path.display()))
    }
}
