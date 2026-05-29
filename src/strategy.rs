//! GLFTMarketMaker — nautilus glue layer.
//!
//! Thin orchestrator: subscribes to data, registers timer, and coordinates
//! all modules. No quant math lives here.

use std::fmt::Debug;

use nautilus_common::{actor::DataActor, timer::TimeEvent};
use nautilus_model::{
    data::{OrderBookDeltas, QuoteTick, TradeTick},
    enums::{AggressorSide, BookType, OrderSide, TimeInForce},
    events::OrderFilled,
    identifiers::InstrumentId,
    instruments::{Instrument, InstrumentAny},
    types::{Price, Quantity},
};

use crate::{
    config::GlftMarketMakerConfig,
    market_state::{
        imbalance::{L1Imbalance, MultiLevelImbalance},
        microprice::Microprice,
        order_flow::OrderFlowImbalance,
        regime::classify_regime,
        trade_flow::{MarkoutTracker, ToxicityIndicator},
        volatility::VolatilityEwma,
    },
    model::{
        estimator::AkEstimator,
        parameters::GlftParameters,
    },
    quoting::quote_builder::build_ladder,
    risk::{guards::run_all_guards, risk_manager::RiskManager},
    types::MarketSnapshot,
};
use nautilus_trading::{nautilus_strategy, strategy::{Strategy, StrategyCore}};

const REQUOTE_TIMER: &str = "requote";

pub struct GlftMarketMaker {
    pub(crate) core: StrategyCore,

    cfg: GlftMarketMakerConfig,
    instrument_id: InstrumentId,

    // Cached on on_start
    instrument: Option<InstrumentAny>,

    // Market state indicators (updated in event handlers)
    microprice: Microprice,
    obi_l1: L1Imbalance,
    obi_multi: MultiLevelImbalance,
    ofi: OrderFlowImbalance,
    vol: VolatilityEwma,
    toxicity: ToxicityIndicator,
    markout: MarkoutTracker,

    // Model
    estimator: AkEstimator,

    // Risk
    risk: RiskManager,
}

impl GlftMarketMaker {
    pub fn new(cfg: GlftMarketMakerConfig) -> Self {
        let sc = &cfg.signal;
        let mc = &cfg.model;
        let instrument_id = cfg.instrument_id;

        Self {
            core: StrategyCore::new(cfg.base.clone()),
            instrument_id,
            instrument: None,

            microprice: Microprice::new(),
            obi_l1: L1Imbalance::new(),
            obi_multi: MultiLevelImbalance::new(sc.obi_levels),
            ofi: OrderFlowImbalance::new(200),
            vol: VolatilityEwma::new(sc.vol_halflife_trades, 50),
            toxicity: ToxicityIndicator::new(10.0, 50, 20, 3.0),
            markout: MarkoutTracker::new(20, 50),

            estimator: AkEstimator::new(10, 0.02, mc.estimator_window, mc.estimator_min_trades),
            risk: RiskManager::new(cfg.risk.clone()),

            cfg,
        }
    }

    // ------------------------------------------------------------------ //
    // Requote cycle                                                        //
    // ------------------------------------------------------------------ //

    fn requote(&mut self) -> anyhow::Result<()> {
        if self.risk.is_kill_switch_active() {
            return Ok(());
        }

        // --- Gather book state from cache (scoped borrow) ---
        let (mid, best_bid, best_ask) = {
            let cache = self.cache();
            let book = match cache.order_book(&self.instrument_id) {
                Some(b) => b,
                None => return Ok(()),
            };
            let mid = match book.midpoint() {
                Some(m) if m > 0.0 => m,
                _ => return Ok(()),
            };
            let best_bid = match book.best_bid_price() {
                Some(p) => p.as_f64(),
                None => return Ok(()),
            };
            let best_ask = match book.best_ask_price() {
                Some(p) => p.as_f64(),
                None => return Ok(()),
            };
            (mid, best_bid, best_ask)
        };

        // --- Get instrument info ---
        let (tick_size, lot_size, price_precision, size_precision) =
            match self.instrument.as_ref() {
                Some(inst) => (
                    inst.price_increment().as_f64(),
                    inst.size_increment().as_f64(),
                    inst.price_precision(),
                    inst.size_precision(),
                ),
                None => return Ok(()),
            };

        // --- Net position (base units) from open positions ---
        let raw_pos: f64 = {
            let cache = self.cache();
            let mut signed: f64 = 0.0;
            for p in cache.positions_open(None, Some(&self.instrument_id), None, None, None) {
                signed += p.signed_qty;
            }
            signed
        };

        let max_pos_notional = self.cfg.risk.max_position_notional;
        let net_pos_notional = raw_pos * mid;
        let q_norm = (net_pos_notional / max_pos_notional).max(-1.0).min(1.0);

        // --- Unrealised PnL from portfolio ---
        let upnl: f64 = {
            let portfolio = self.core.portfolio();
            portfolio
                .borrow_mut()
                .unrealized_pnl(&self.instrument_id)
                .map(|m| m.as_f64())
                .unwrap_or(0.0)
        };

        // --- Build market snapshot ---
        let now_ns = self.timestamp_ns().as_u64();
        let microprice = if self.microprice.initialized { self.microprice.value } else { mid };
        let sigma = if self.vol.initialized { self.vol.value() } else { 0.0 };
        let regime = classify_regime(sigma, &self.cfg.signal);

        let ak = self.estimator.estimate();
        let a = if ak.valid { ak.a } else { self.cfg.model.a_init };
        let k = if ak.valid { ak.k } else { self.cfg.model.k_init };

        let snap = MarketSnapshot {
            timestamp_ns: now_ns,
            mid: microprice,
            microprice,
            sigma,
            obi_l1: if self.obi_l1.initialized { self.obi_l1.value } else { 0.0 },
            obi_multi: if self.obi_multi.initialized { self.obi_multi.value } else { 0.0 },
            ofi: if self.ofi.initialized { self.ofi.value } else { 0.0 },
            toxicity: if self.toxicity.initialized { self.toxicity.value } else { 0.0 },
            markout_bps: self.markout.value(),
            regime,
            q_norm,
            a,
            k,
            estimator_ready: ak.valid,
        };

        // --- GLFT parameters ---
        let params = GlftParameters {
            gamma: self.cfg.model.gamma * snap.regime.gamma_mult,
            sigma: sigma.max(1e-6),
            a,
            k,
            xi: self.cfg.model.xi,
        };

        // --- Build ladder ---
        let ladder = build_ladder(&snap, &params, tick_size, lot_size, &self.cfg);

        // --- Apply risk filters ---
        let ladder = match self.risk.check(ladder, net_pos_notional, upnl) {
            Ok(l) => l,
            Err(e) => {
                log::error!("Kill switch: {e}");
                self.cancel_all_orders(self.instrument_id, None, None, None)?;
                return Ok(());
            }
        };

        // --- Pre-quote guards ---
        let (passed, reason) = run_all_guards(
            &snap,
            &ladder,
            best_bid,
            best_ask,
            mid,
            now_ns,
            &self.cfg.risk,
        );
        if !passed {
            log::debug!("Guard failed: {reason}");
            return Ok(());
        }

        // --- Cancel all live orders then resubmit ---
        self.cancel_all_orders(self.instrument_id, None, None, None)?;

        for level in &ladder.bids {
            let price = Price::new(level.price, price_precision);
            let qty = Quantity::new(level.base_size, size_precision);
            if qty.as_f64() <= 0.0 {
                continue;
            }
            let order = self.core.order_factory().limit(
                self.instrument_id,
                OrderSide::Buy,
                qty,
                price,
                Some(TimeInForce::Gtc),
                None,
                Some(true), // post_only
                None, None, None, None, None, None, None, None, None,
            );
            self.submit_order(order, None, None, None)?;
        }

        for level in &ladder.asks {
            let price = Price::new(level.price, price_precision);
            let qty = Quantity::new(level.base_size, size_precision);
            if qty.as_f64() <= 0.0 {
                continue;
            }
            let order = self.core.order_factory().limit(
                self.instrument_id,
                OrderSide::Sell,
                qty,
                price,
                Some(TimeInForce::Gtc),
                None,
                Some(true), // post_only
                None, None, None, None, None, None, None, None, None,
            );
            self.submit_order(order, None, None, None)?;
        }

        Ok(())
    }
}

nautilus_strategy!(GlftMarketMaker);

impl Debug for GlftMarketMaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlftMarketMaker")
            .field("instrument_id", &self.instrument_id)
            .finish()
    }
}

impl DataActor for GlftMarketMaker {
    fn on_start(&mut self) -> anyhow::Result<()> {
        let instrument_id = self.instrument_id;

        // Resolve instrument from cache
        let instrument = {
            let cache = self.cache();
            match cache.instrument(&instrument_id) {
                Some(i) => i.clone(),
                None => {
                    log::error!("Instrument not found: {instrument_id}");
                    return Ok(());
                }
            }
        };
        self.instrument = Some(instrument);

        // Subscribe to data streams
        self.subscribe_quotes(instrument_id, None, None);
        self.subscribe_trades(instrument_id, None, None);
        self.subscribe_book_deltas(instrument_id, BookType::L2_MBP, None, None, true, None);

        // Set recurring requote timer
        let interval_ns = self.cfg.quote.requote_interval_ms * 1_000_000;
        self.clock().set_timer_ns(
            REQUOTE_TIMER,
            interval_ns,
            None,
            None,
            None,
            None,
            Some(false),
        )?;

        log::info!(
            "GLFT MM started on {instrument_id}, requote every {}ms",
            self.cfg.quote.requote_interval_ms
        );
        Ok(())
    }

    fn on_stop(&mut self) -> anyhow::Result<()> {
        self.clock().cancel_timer(REQUOTE_TIMER);
        self.cancel_all_orders(self.instrument_id, None, None, None)?;
        log::info!("GLFT MM stopped; all orders cancelled");
        Ok(())
    }

    fn on_reset(&mut self) -> anyhow::Result<()> {
        self.microprice.reset();
        self.obi_l1.reset();
        self.obi_multi.reset();
        self.ofi.reset();
        self.vol.reset();
        self.toxicity.reset();
        self.markout.reset();
        self.estimator.reset();
        self.risk.reset();
        self.instrument = None;
        Ok(())
    }

    // ------------------------------------------------------------------ //
    // Market data handlers                                                 //
    // ------------------------------------------------------------------ //

    fn on_quote(&mut self, tick: &QuoteTick) -> anyhow::Result<()> {
        let bid = tick.bid_price.as_f64();
        let ask = tick.ask_price.as_f64();
        let bid_sz = tick.bid_size.as_f64();
        let ask_sz = tick.ask_size.as_f64();

        self.microprice.update(bid, ask, bid_sz, ask_sz);
        self.obi_l1.update(bid_sz, ask_sz);
        self.ofi.update(bid, ask, bid_sz, ask_sz);

        let mid = (bid + ask) / 2.0;
        self.markout.update_price(mid);

        Ok(())
    }

    fn on_trade(&mut self, tick: &TradeTick) -> anyhow::Result<()> {
        let price = tick.price.as_f64();
        let qty = tick.size.as_f64();
        let ts_ns = tick.ts_event.as_u64();

        self.vol.update(price);

        let is_buyer = match tick.aggressor_side {
            AggressorSide::Buyer => Some(true),
            AggressorSide::Seller => Some(false),
            _ => None,
        };
        self.toxicity.update(qty, is_buyer, ts_ns);

        // Feed estimator with trade distance from current order book mid
        let mid_opt: Option<f64> = {
            let cache = self.cache();
            cache
                .order_book(&self.instrument_id)
                .and_then(|b| b.midpoint())
                .filter(|&m| m > 0.0)
        };
        if let Some(mid) = mid_opt {
            let dist = (price - mid).abs() / mid;
            self.estimator.record_trade(dist);
        }

        Ok(())
    }

    fn on_book_deltas(&mut self, _deltas: &OrderBookDeltas) -> anyhow::Result<()> {
        // The managed order book in cache has already been updated.
        // Collect level sizes then update the multi-level OBI indicator.
        let (bids, asks) = {
            let cache = self.cache();
            let book = match cache.order_book(&self.instrument_id) {
                Some(b) => b,
                None => return Ok(()),
            };
            let n = self.obi_multi.n_levels;
            let bids: Vec<f64> = book.bids(Some(n)).map(|l| l.size()).collect();
            let asks: Vec<f64> = book.asks(Some(n)).map(|l| l.size()).collect();
            (bids, asks)
        };
        self.obi_multi.update_from_levels(&bids, &asks);
        Ok(())
    }

    fn on_order_filled(&mut self, event: &OrderFilled) -> anyhow::Result<()> {
        // Record fill in estimator and markout tracker
        let mid_opt: Option<f64> = {
            let cache = self.cache();
            cache
                .order_book(&self.instrument_id)
                .and_then(|b| b.midpoint())
                .filter(|&m| m > 0.0)
        };
        if let Some(mid) = mid_opt {
            let fill_price = event.last_px.as_f64();
            let dist = (fill_price - mid).abs() / mid;
            self.estimator.record_fill(dist);

            let side = if event.order_side == OrderSide::Buy { 1.0 } else { -1.0 };
            self.markout.record_fill(fill_price, side);
        }

        // Commission is negative PnL (approximate)
        let fee_pnl = event
            .commission
            .as_ref()
            .map(|m| -m.as_f64())
            .unwrap_or(0.0);
        self.risk.record_realised_pnl(fee_pnl);

        Ok(())
    }

    fn on_time_event(&mut self, event: &TimeEvent) -> anyhow::Result<()> {
        if event.name.as_str() == REQUOTE_TIMER {
            if let Err(e) = self.requote() {
                log::error!("Requote error: {e}");
            }
        }
        Ok(())
    }
}
