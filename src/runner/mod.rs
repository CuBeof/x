use crate::strategies::{Strategy, StrategySummary};

pub mod backtest;
pub mod paper;
pub mod live;

pub trait Runner {
    fn run(&mut self, strategy: Box<dyn Strategy>) -> anyhow::Result<StrategySummary>;
}
