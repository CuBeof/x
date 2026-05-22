pub mod glft;

use crate::engine::event::OrderEvent;
use crate::engine::Engine;

#[derive(Debug, Clone, Default)]
pub struct StrategySummary {
    pub strategy_name: String,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_pnl: f64,
    pub total_trades: u64,
    pub inventory: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}

pub trait Strategy: Send {
    fn name(&self) -> &str;
    fn on_start(&mut self);
    fn on_market_event(&mut self, event: &crate::engine::event::MarketEvent, engine: &mut Engine);
    fn on_order_event(&mut self, event: &OrderEvent);
    fn on_stop(&mut self);
    fn summary(&self) -> StrategySummary;
}
