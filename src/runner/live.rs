use crate::config::AppConfig;
use crate::runner::Runner;
use crate::strategies::{Strategy, StrategySummary};

pub struct LiveRunner {
    config: AppConfig,
}

impl LiveRunner {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

impl Runner for LiveRunner {
    fn run(&mut self, _strategy: Box<dyn Strategy>) -> anyhow::Result<StrategySummary> {
        // SAFETY: Live trading requires a fully tested exchange connector,
        // proper risk controls, and regulatory compliance before deployment.
        // Never run live trading with placeholder credentials or untested logic.
        anyhow::bail!(
            "Live trading requires exchange connector implementation. \
             Implement a connector for {} before enabling live mode.",
            self.config.data.symbol
        )
    }
}
