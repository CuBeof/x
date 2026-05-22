pub mod indicators;
pub mod inventory;
pub mod quoter;
pub mod risk;

use std::time::{Duration, Instant};
use uuid::Uuid;
use log::{info, debug, warn};

use crate::config::GlftConfig;
use crate::engine::event::{MarketEvent, OrderEvent, OrderSide};
use crate::engine::{Engine, OpenOrder};
use crate::strategies::{Strategy, StrategySummary};
use self::indicators::GlftIndicators;
use self::inventory::InventoryManager;
use self::quoter::GlftQuoter;
use self::risk::{RiskManager, RiskState};

pub struct GlftStrategy {
    config: GlftConfig,
    indicators: GlftIndicators,
    inventory: InventoryManager,
    quoter: GlftQuoter,
    risk: RiskManager,
    active_bid: Option<Uuid>,
    active_ask: Option<Uuid>,
    start_time: Option<Instant>,
    last_quote_time: Option<Instant>,
    last_mid: f64,
}

impl GlftStrategy {
    pub fn new(config: GlftConfig) -> Self {
        let indicators = GlftIndicators::new(config.vol_window as usize);
        let inventory = InventoryManager::new(config.max_inventory);
        let quoter = GlftQuoter::new(config.gamma, config.kappa, config.min_spread_bps);
        let risk = RiskManager::new(config.volatility_regime_multiplier, config.max_inventory);

        Self {
            config,
            indicators,
            inventory,
            quoter,
            risk,
            active_bid: None,
            active_ask: None,
            start_time: None,
            last_quote_time: None,
            last_mid: 0.0,
        }
    }

    fn refresh_due(&self) -> bool {
        match self.last_quote_time {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_millis(self.config.quote_refresh_ms),
        }
    }
}

impl Strategy for GlftStrategy {
    fn name(&self) -> &str {
        "GLFT"
    }

    fn on_start(&mut self) {
        self.start_time = Some(Instant::now());
        self.last_quote_time = None;
        info!(
            "GLFT strategy started gamma={} kappa={} sigma={}",
            self.config.gamma, self.config.kappa, self.config.sigma
        );
    }

    fn on_market_event(&mut self, event: &MarketEvent, engine: &mut Engine) {
        // Update indicators
        self.indicators.update(event);

        // Extract mid price from quote events
        let mid = match event {
            MarketEvent::QuoteUpdate(q) => {
                let m = q.mid_price();
                self.last_mid = m;
                Some(m)
            }
            MarketEvent::BarUpdate(b) => {
                self.last_mid = b.close;
                Some(b.close)
            }
            MarketEvent::TradeUpdate(t) => {
                self.last_mid = t.price;
                Some(t.price)
            }
        };

        let mid = match mid {
            Some(m) if m > 0.0 => m,
            _ => return,
        };

        // Assess risk
        let risk_state = self
            .risk
            .check_risk(&self.indicators, self.inventory.position);
        let risk_state = risk_state.clone();

        if !self.risk.is_quoting_allowed() && risk_state != RiskState::InventoryBreached {
            debug!("Quoting blocked by risk manager state={:?}", risk_state);
            return;
        }

        // Check if it is time to refresh quotes
        if !self.refresh_due() {
            return;
        }

        // Cancel all existing orders
        engine.cancel_all();
        self.active_bid = None;
        self.active_ask = None;

        // Compute sigma: use live estimate if ready, else fall back to config
        let sigma = if self.indicators.is_ready() {
            let v = self.indicators.volatility();
            if v > 0.0 { v } else { self.config.sigma }
        } else {
            self.config.sigma
        };

        let elapsed_secs = self
            .start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        let decision = self.quoter.compute(
            mid,
            self.inventory.position,
            sigma,
            elapsed_secs,
            self.config.time_horizon_secs as f64,
            self.config.order_size,
            &risk_state,
        );

        if !decision.should_quote {
            self.last_quote_time = Some(Instant::now());
            return;
        }

        // Submit bid
        if let Some(bid_price) = decision.bid_price {
            let oid = Uuid::new_v4();
            engine.submit_order(OpenOrder {
                order_id: oid,
                side: OrderSide::Buy,
                price: bid_price,
                size: decision.bid_size,
                is_post_only: true,
            });
            self.active_bid = Some(oid);
            debug!(
                "Submitted bid price={:.6} size={} reservation={:.6} half_spread={:.6}",
                bid_price, decision.bid_size, decision.reservation_price, decision.half_spread
            );
        }

        // Submit ask
        if let Some(ask_price) = decision.ask_price {
            let oid = Uuid::new_v4();
            engine.submit_order(OpenOrder {
                order_id: oid,
                side: OrderSide::Sell,
                price: ask_price,
                size: decision.ask_size,
                is_post_only: true,
            });
            self.active_ask = Some(oid);
            debug!(
                "Submitted ask price={:.6} size={}",
                ask_price, decision.ask_size
            );
        }

        self.last_quote_time = Some(Instant::now());
    }

    fn on_order_event(&mut self, event: &OrderEvent) {
        match event {
            OrderEvent::Filled(fill) => {
                let realized = self.inventory.on_fill(
                    fill.price,
                    fill.size,
                    &fill.side,
                    fill.fee,
                    fill.timestamp,
                );
                self.risk.on_fill();

                // Clear tracking
                if Some(fill.order_id) == self.active_bid {
                    self.active_bid = None;
                }
                if Some(fill.order_id) == self.active_ask {
                    self.active_ask = None;
                }

                info!(
                    "Order filled side={:?} price={:.6} size={} fee={:.6} \
                     realized_pnl={:.6} total_realized={:.6} position={:.6}",
                    fill.side, fill.price, fill.size, fill.fee,
                    realized, self.inventory.realized_pnl, self.inventory.position
                );
            }
            OrderEvent::Rejected { order_id, reason } => {
                self.risk.on_rejection();
                warn!("Order rejected order_id={} reason={}", order_id, reason);

                if Some(*order_id) == self.active_bid {
                    self.active_bid = None;
                }
                if Some(*order_id) == self.active_ask {
                    self.active_ask = None;
                }
            }
            OrderEvent::Cancelled(order_id) => {
                debug!("Order cancelled order_id={}", order_id);
                if Some(*order_id) == self.active_bid {
                    self.active_bid = None;
                }
                if Some(*order_id) == self.active_ask {
                    self.active_ask = None;
                }
            }
            OrderEvent::Accepted(order_id) => {
                debug!("Order accepted order_id={}", order_id);
            }
        }
    }

    fn on_stop(&mut self) {
        let mid = self.last_mid;
        let summary = self.summary_with_mid(mid);
        info!(
            "GLFT strategy stopped realized_pnl={:.6} unrealized_pnl={:.6} \
             total_trades={} inventory={:.6} sharpe={:.4} max_dd={:.4}",
            summary.realized_pnl, summary.unrealized_pnl,
            summary.total_trades, summary.inventory,
            summary.sharpe_ratio, summary.max_drawdown
        );
    }

    fn summary(&self) -> StrategySummary {
        self.summary_with_mid(self.last_mid)
    }
}

impl GlftStrategy {
    fn summary_with_mid(&self, mid: f64) -> StrategySummary {
        let unrealized = if mid > 0.0 {
            self.inventory.unrealized_pnl(mid)
        } else {
            0.0
        };
        StrategySummary {
            strategy_name: "GLFT".to_string(),
            realized_pnl: self.inventory.realized_pnl,
            unrealized_pnl: unrealized,
            total_pnl: self.inventory.realized_pnl + unrealized,
            total_trades: self.inventory.trades.len() as u64,
            inventory: self.inventory.position,
            sharpe_ratio: self.inventory.sharpe_ratio(),
            max_drawdown: self.inventory.max_drawdown(),
        }
    }
}
