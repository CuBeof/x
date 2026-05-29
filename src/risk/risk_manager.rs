//! Inventory, drawdown, and position-limit risk management.

use crate::{config::RiskConfig, quoting::quote::{QuoteLadder, QuoteLevel}};

#[derive(Debug)]
pub struct KillSwitchError(pub String);

impl std::fmt::Display for KillSwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for KillSwitchError {}

pub struct RiskManager {
    cfg: RiskConfig,
    realised_pnl: f64,
    kill_switch: bool,
}

impl RiskManager {
    pub fn new(cfg: RiskConfig) -> Self {
        Self { cfg, realised_pnl: 0.0, kill_switch: false }
    }

    pub fn record_realised_pnl(&mut self, delta: f64) {
        self.realised_pnl += delta;
    }

    pub fn is_kill_switch_active(&self) -> bool {
        self.kill_switch
    }

    /// Apply risk filters; return filtered ladder or KillSwitchError.
    pub fn check(
        &mut self,
        ladder: QuoteLadder,
        net_position_notional: f64,
        unrealised_pnl: f64,
    ) -> Result<QuoteLadder, KillSwitchError> {
        if self.kill_switch {
            return Err(KillSwitchError("Kill switch active".into()));
        }

        let total_pnl = self.realised_pnl + unrealised_pnl;
        if total_pnl < self.cfg.daily_loss_limit {
            self.kill_switch = true;
            return Err(KillSwitchError(format!(
                "Daily loss limit breached: {total_pnl:.2} < {:.2}",
                self.cfg.daily_loss_limit
            )));
        }

        let max_notional = self.cfg.max_position_notional;
        let reduce_threshold = self.cfg.inventory_reduce_threshold * max_notional;
        let abs_pos = net_position_notional.abs();

        if abs_pos >= max_notional {
            return Ok(if net_position_notional > 0.0 {
                QuoteLadder { bids: vec![], asks: ladder.asks, ..ladder }
            } else {
                QuoteLadder { bids: ladder.bids, asks: vec![], ..ladder }
            });
        }

        if abs_pos >= reduce_threshold {
            let scale = (1.0
                - (abs_pos - reduce_threshold) / (max_notional - reduce_threshold))
                .max(0.1);
            return Ok(if net_position_notional > 0.0 {
                QuoteLadder {
                    bids: scale_levels(ladder.bids, scale),
                    asks: ladder.asks,
                    ..ladder
                }
            } else {
                QuoteLadder {
                    bids: ladder.bids,
                    asks: scale_levels(ladder.asks, scale),
                    ..ladder
                }
            });
        }

        Ok(ladder)
    }

    pub fn reset(&mut self) {
        self.realised_pnl = 0.0;
        self.kill_switch = false;
    }
}

fn scale_levels(levels: Vec<QuoteLevel>, scale: f64) -> Vec<QuoteLevel> {
    levels
        .into_iter()
        .map(|l| QuoteLevel {
            price: l.price,
            notional: l.notional * scale,
            base_size: l.base_size * scale,
        })
        .collect()
}
