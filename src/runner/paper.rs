use tracing::{info, warn};

use crate::config::AppConfig;
use crate::runner::Runner;
use crate::strategies::{Strategy, StrategySummary};

pub struct PaperRunner {
    config: AppConfig,
}

impl PaperRunner {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

impl Runner for PaperRunner {
    fn run(&mut self, _strategy: Box<dyn Strategy>) -> anyhow::Result<StrategySummary> {
        warn!(
            symbol = %self.config.data.symbol,
            "Paper trading not yet connected to live feed. \
             Implement a WebSocket connector to stream live quotes."
        );
        info!("Paper runner: returning empty summary");
        Ok(StrategySummary::default())
    }
}
