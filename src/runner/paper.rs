use log::{info, warn};

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
            "Paper trading not yet connected to live feed (symbol={}). \
             Implement a WebSocket connector to stream live quotes.",
            self.config.data.symbol
        );
        info!("Paper runner: returning empty summary");
        Ok(StrategySummary::default())
    }
}
