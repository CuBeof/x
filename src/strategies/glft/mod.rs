pub mod indicators;
pub mod inventory;
pub mod quoter;
pub mod risk;

use log::{debug, info, warn};
use std::fmt::Debug;
use std::time::{Duration, Instant};

use nautilus_common::actor::DataActor;
use nautilus_model::{
    data::QuoteTick,
    enums::{OrderSide, OrderStatus, TimeInForce},
    events::{OrderCanceled, OrderFilled, OrderRejected},
    identifiers::{ClientOrderId, InstrumentId, StrategyId},
    orders::Order,
    types::{Price, Quantity},
};
use nautilus_trading::{
    nautilus_strategy,
    strategy::{StrategyConfig, StrategyCore},
    Strategy,
};

use self::indicators::GlftIndicators;
use self::inventory::TradeTracker;
use self::quoter::GlftQuoter;
use self::risk::{RiskManager, RiskState};

#[derive(Debug, Clone)]
pub struct GlftConfig {
    /// Risk-aversion coefficient γ
    pub gamma: f64,
    /// Order-book depth parameter κ (controls order arrival rate)
    pub kappa: f64,
    /// Initial volatility estimate σ (annualized); overridden by live estimate once ready
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

impl Default for GlftConfig {
    fn default() -> Self {
        Self {
            gamma: 0.1,
            kappa: 1.5,
            sigma: 0.02,
            time_horizon_secs: 3600,
            min_spread_bps: 2.0,
            max_inventory: 1.0,
            order_size: 0.01,
            quote_refresh_ms: 1000,
            vol_window: 100,
            volatility_regime_multiplier: 3.0,
        }
    }
}

pub struct GlftStrategy {
    core: StrategyCore,
    instrument_id: InstrumentId,
    config: GlftConfig,
    indicators: GlftIndicators,
    tracker: TradeTracker,
    quoter: GlftQuoter,
    risk: RiskManager,
    active_bid: Option<ClientOrderId>,
    active_ask: Option<ClientOrderId>,
    start_time: Option<Instant>,
    last_quote_time: Option<Instant>,
    last_mid: f64,
}

impl Debug for GlftStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlftStrategy")
            .field("instrument_id", &self.instrument_id)
            .finish()
    }
}

nautilus_strategy!(GlftStrategy, {
    fn on_order_rejected(&mut self, event: OrderRejected) {
        self.risk.on_rejection();
        warn!("Order rejected reason={}", event.reason);
    }
});

impl GlftStrategy {
    pub fn new(instrument_id: InstrumentId, config: Option<GlftConfig>) -> Self {
        let config = config.unwrap_or_default();
        let strategy_config = StrategyConfig {
            strategy_id: Some(StrategyId::new("GLFT-001")),
            order_id_tag: Some("001".to_string()),
            ..Default::default()
        };
        let indicators = GlftIndicators::new(config.vol_window as usize);
        let tracker = TradeTracker::new();
        let quoter = GlftQuoter::new(config.gamma, config.kappa, config.min_spread_bps);
        let risk = RiskManager::new(config.volatility_regime_multiplier, config.max_inventory);

        Self {
            core: StrategyCore::new(strategy_config),
            instrument_id,
            config,
            indicators,
            tracker,
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

    fn elapsed_secs(&self) -> f64 {
        self.start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    fn net_position(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.core
            .portfolio()
            .borrow()
            .net_position(&self.instrument_id)
            .to_f64()
            .unwrap_or(0.0)
    }
}

impl DataActor for GlftStrategy {
    fn on_start(&mut self) -> anyhow::Result<()> {
        self.start_time = Some(Instant::now());
        self.subscribe_quotes(self.instrument_id, None, None);
        info!(
            "GLFT strategy started instrument={} gamma={} kappa={} sigma={}",
            self.instrument_id, self.config.gamma, self.config.kappa, self.config.sigma
        );
        Ok(())
    }

    fn on_quote(&mut self, quote: &QuoteTick) -> anyhow::Result<()> {
        let bid = quote.bid_price.as_f64();
        let ask = quote.ask_price.as_f64();
        let mid = (bid + ask) / 2.0;
        self.last_mid = mid;

        // Update indicators
        self.indicators.update_quote(bid, ask);

        // Get portfolio position
        let position = self.net_position();

        // Risk check
        let risk_state = self.risk.check_risk(&self.indicators, position).clone();

        if !matches!(risk_state, RiskState::Normal | RiskState::InventoryBreached) {
            debug!("Quoting blocked state={:?}", risk_state);
            return Ok(());
        }

        if !self.refresh_due() {
            return Ok(());
        }

        // Cancel existing orders
        self.cancel_all_orders(self.instrument_id, None, None, None)?;
        self.active_bid = None;
        self.active_ask = None;

        // Compute sigma
        let sigma = if self.indicators.is_ready() {
            let v = self.indicators.volatility();
            if v > 0.0 {
                v
            } else {
                self.config.sigma
            }
        } else {
            self.config.sigma
        };

        let decision = self.quoter.compute(
            mid,
            position,
            sigma,
            self.elapsed_secs(),
            self.config.time_horizon_secs as f64,
            self.config.order_size,
            &risk_state,
        );

        if !decision.should_quote {
            self.last_quote_time = Some(Instant::now());
            return Ok(());
        }

        // Submit bid (post-only)
        if let Some(bid_price) = decision.bid_price {
            let price = Price::new(bid_price, 8);
            let qty = Quantity::new(decision.bid_size, 8);
            let order = self.core.order_factory().limit(
                self.instrument_id,
                OrderSide::Buy,
                qty,
                price,
                Some(TimeInForce::Gtc),
                None,       // expire_time
                Some(true), // post_only
                None,       // reduce_only
                None,       // quote_quantity
                None,       // display_qty
                None,       // emulation_trigger
                None,       // trigger_instrument_id
                None,       // exec_algorithm_id
                None,       // exec_algorithm_params
                None,       // tags
                None,       // client_order_id
            );
            let coid = order.client_order_id();
            self.submit_order(order, None, None, None)?;
            self.active_bid = Some(coid);
            debug!(
                "Submitted bid price={:.6} size={} reservation={:.6} half_spread={:.6}",
                bid_price, decision.bid_size, decision.reservation_price, decision.half_spread
            );
        }

        // Submit ask (post-only)
        if let Some(ask_price) = decision.ask_price {
            let price = Price::new(ask_price, 8);
            let qty = Quantity::new(decision.ask_size, 8);
            let order = self.core.order_factory().limit(
                self.instrument_id,
                OrderSide::Sell,
                qty,
                price,
                Some(TimeInForce::Gtc),
                None,       // expire_time
                Some(true), // post_only
                None,       // reduce_only
                None,       // quote_quantity
                None,       // display_qty
                None,       // emulation_trigger
                None,       // trigger_instrument_id
                None,       // exec_algorithm_id
                None,       // exec_algorithm_params
                None,       // tags
                None,       // client_order_id
            );
            let coid = order.client_order_id();
            self.submit_order(order, None, None, None)?;
            self.active_ask = Some(coid);
            debug!(
                "Submitted ask price={:.6} size={}",
                ask_price, decision.ask_size
            );
        }

        self.last_quote_time = Some(Instant::now());
        Ok(())
    }

    fn on_order_filled(&mut self, event: &OrderFilled) -> anyhow::Result<()> {
        // An order can be partially filled multiple times with the same client_order_id.
        // Check the order status from cache to distinguish partial vs full fill.
        let is_fully_filled = self
            .cache()
            .order(&event.client_order_id)
            .map(|o| o.status() == OrderStatus::Filled)
            .unwrap_or(true); // assume complete if order not in cache

        let commission = event.commission.map(|m| m.as_f64()).unwrap_or(0.0);
        self.tracker.record_fill(-commission);

        if is_fully_filled {
            // Order fully filled — clear active tracking and reset rejection counter
            self.risk.on_fill();
            if Some(event.client_order_id) == self.active_bid {
                self.active_bid = None;
            }
            if Some(event.client_order_id) == self.active_ask {
                self.active_ask = None;
            }
            info!(
                "Order fully filled side={:?} price={} qty={} commission={}",
                event.order_side, event.last_px, event.last_qty, commission
            );
        } else {
            info!(
                "Order partially filled side={:?} price={} qty={} commission={}",
                event.order_side, event.last_px, event.last_qty, commission
            );
        }
        Ok(())
    }

    fn on_order_canceled(&mut self, event: &OrderCanceled) -> anyhow::Result<()> {
        // Triggered when cancel_all_orders() confirms cancellation of a resting order.
        // A partially filled order that is then cancelled will arrive here for the
        // remaining (unfilled) quantity — safe to clear active tracking at this point.
        if Some(event.client_order_id) == self.active_bid {
            self.active_bid = None;
        }
        if Some(event.client_order_id) == self.active_ask {
            self.active_ask = None;
        }
        debug!("Order canceled order_id={}", event.client_order_id);
        Ok(())
    }

    fn on_stop(&mut self) -> anyhow::Result<()> {
        info!(
            "GLFT strategy stopped trades={} sharpe={:.4} max_dd={:.4}",
            self.tracker.fill_count(),
            self.tracker.sharpe_ratio(),
            self.tracker.max_drawdown()
        );
        Ok(())
    }
}
