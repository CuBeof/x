//! PureGlftStrategy — minimal market-maker exposing only the GLFT core.
//!
//! This strategy deliberately omits every production layer (OBI/OFI signals,
//! toxicity filter, regime detection, A/k estimator, risk manager) so you can
//! see exactly what the Guéant-Lehalle-Fernandez-Tapia model computes.
//!
//! ## GLFT model in one paragraph
//!
//! The model maximises expected utility of terminal wealth for a dealer who
//! faces Poisson order arrivals at rate λ(δ) = A·exp(-k·δ), where δ is the
//! distance of the quote from the mid-price.  The optimal solution yields:
//!
//!   reservation price  r = s - q·γ·σ²·ξ          (inventory-adjusted mid)
//!   half-spread        δ* = (1/γ)·ln(1+γ/k) + ½·c₂·σ   (symmetric distance)
//!
//!   bid price = r - δ*·mid
//!   ask price = r + δ*·mid
//!
//! Parameters
//!   γ  (gamma) — risk aversion: larger → tighter inventory control, wider spread
//!   σ  (sigma) — volatility: wider spread when market is moving fast
//!   A        — arrival intensity at zero distance (orders/second)
//!   k        — arrival decay: larger → orders only arrive very close to mid
//!   ξ  (xi)  — inventory horizon scale: larger → bigger reservation shift
//!
//! ## What changes with each parameter
//!
//! | Parameter | Increase → |
//! |-----------|-----------|
//! | gamma ↑   | wider spread, stronger inventory push |
//! | sigma ↑   | wider spread (volatility risk) |
//! | k ↑       | narrower spread (arrival more concentrated near mid) |
//! | A ↑       | narrower spread (more flow, can afford tighter quotes) |
//! | xi ↑      | larger reservation-price shift per unit of inventory |

use std::fmt::Debug;

use nautilus_common::actor::DataActor;
use nautilus_common::timer::TimeEvent;
use nautilus_model::{
    data::{QuoteTick, TradeTick},
    enums::{OrderSide, TimeInForce},
    identifiers::InstrumentId,
    instruments::{Instrument, InstrumentAny},
    types::{Price, Quantity},
};
use nautilus_trading::{nautilus_strategy, strategy::{Strategy, StrategyCore}};

use crate::model::{
    glft::{c1, c2_full, inventory_shift},
    parameters::GlftParameters,
};
use crate::quoting::rounding::{notional_to_base, round_price_to_tick_down, round_price_to_tick_up};
use nautilus_trading::strategy::config::StrategyConfig;

// ─── Config ──────────────────────────────────────────────────────────────── //

/// All the knobs for the pure GLFT strategy.
#[derive(Debug, Clone)]
pub struct PureGlftConfig {
    pub base: StrategyConfig,
    pub instrument_id: InstrumentId,

    // GLFT model parameters (fixed — no online estimation)
    pub gamma: f64,   // risk aversion
    pub sigma: f64,   // assumed volatility (fraction, e.g. 0.003 = 0.3%)
    pub a: f64,       // order-arrival intensity at δ=0 (orders/s)
    pub k: f64,       // arrival-rate decay per unit of fractional distance
    pub xi: f64,      // inventory-shift horizon scaling

    // Order sizing
    pub notional: f64,         // quote size in quote-currency per side
    pub max_position_notional: f64, // position cap for q_norm computation

    // Quote bounds
    pub min_spread_bps: f64,
    pub max_spread_bps: f64,

    // Requote frequency
    pub requote_interval_ms: u64,
}

impl PureGlftConfig {
    pub fn new(instrument_id: InstrumentId) -> Self {
        Self {
            base: StrategyConfig::default(),
            instrument_id,
            gamma: 0.1,
            sigma: 0.002,   // 0.2% per-interval vol — set to match your data
            a: 1.0,
            k: 1000.0,
            xi: 1000.0,
            notional: 100.0,
            max_position_notional: 500.0,
            min_spread_bps: 2.0,
            max_spread_bps: 200.0,
            requote_interval_ms: 500,
        }
    }
}

// ─── Strategy ────────────────────────────────────────────────────────────── //

pub struct PureGlftStrategy {
    pub(crate) core: StrategyCore,
    cfg: PureGlftConfig,
    instrument: Option<InstrumentAny>,
    last_mid: f64,  // tracked from quote ticks so we always have a price
}

const TIMER: &str = "pure_glft_requote";

impl PureGlftStrategy {
    pub fn new(cfg: PureGlftConfig) -> Self {
        Self {
            core: StrategyCore::new(cfg.base.clone()),
            cfg,
            instrument: None,
            last_mid: 0.0,
        }
    }

    // ── Core GLFT computation ─────────────────────────────────────────── //

    /// Derive bid/ask prices from the GLFT model.
    ///
    /// Returns `None` when not enough data is available.
    fn compute_quotes(&self, mid: f64, q_norm: f64) -> Option<(f64, f64)> {
        if mid <= 0.0 {
            return None;
        }
        let p = GlftParameters {
            gamma: self.cfg.gamma,
            sigma: self.cfg.sigma.max(1e-9),
            a: self.cfg.a,
            k: self.cfg.k,
            xi: self.cfg.xi,
        };

        // Step 1 — reservation price: shift mid toward reducing inventory.
        //   shift_frac = q_norm · γ · σ² · ξ
        //   r = mid · (1 − shift_frac)
        let shift = inventory_shift(&p, q_norm);      // fraction
        let reservation = mid * (1.0 - shift);

        // Step 2 — half-spread distance as a fraction of mid.
        //   c1 = (1/γ)·ln(1 + γ/k)   — "liquidity premium"
        //   c2 = √(γ / (2·A·k)) · (1 + γ/k)^(0.5 + k/(2γ))
        //   δ* = c1 + ½·c2·σ
        let spread_frac = c1(&p) + 0.5 * c2_full(&p) * p.sigma;

        // Step 3 — clamp to configured bounds.
        let min_frac = self.cfg.min_spread_bps * 1e-4;
        let max_frac = self.cfg.max_spread_bps * 1e-4;
        let delta = spread_frac.max(min_frac).min(max_frac);

        // Step 4 — symmetric quotes around reservation.
        let bid = reservation - delta * mid;
        let ask = reservation + delta * mid;

        log::debug!(
            "GLFT: mid={mid:.4} res={reservation:.4} shift_frac={shift:.6} \
             spread_frac={spread_frac:.6} bid={bid:.4} ask={ask:.4} q_norm={q_norm:.3}"
        );

        Some((bid, ask))
    }

    // ── Requote cycle ─────────────────────────────────────────────────── //

    fn requote(&mut self) -> anyhow::Result<()> {
        // --- Current mid from order book (prefer) or last quote tick ---
        let mid = {
            let cache = self.cache();
            cache
                .order_book(&self.cfg.instrument_id)
                .and_then(|b| b.midpoint())
                .filter(|&m| m > 0.0)
                .unwrap_or(self.last_mid)
        };

        if mid <= 0.0 {
            return Ok(());
        }

        // --- Net inventory (normalised to [-1, 1]) ---
        let raw_pos: f64 = {
            let cache = self.cache();
            let mut signed = 0.0_f64;
            for p in cache.positions_open(None, Some(&self.cfg.instrument_id), None, None, None) {
                signed += p.signed_qty;
            }
            signed
        };
        let q_norm = (raw_pos * mid / self.cfg.max_position_notional)
            .max(-1.0)
            .min(1.0);

        // --- GLFT quotes ---
        let (bid_raw, ask_raw) = match self.compute_quotes(mid, q_norm) {
            Some(q) => q,
            None => return Ok(()),
        };

        // --- Instrument tick/lot info ---
        let (tick, lot, pp, sp) = match self.instrument.as_ref() {
            Some(i) => (
                i.price_increment().as_f64(),
                i.size_increment().as_f64(),
                i.price_precision(),
                i.size_precision(),
            ),
            None => return Ok(()),
        };

        let bid_price = round_price_to_tick_down(bid_raw, tick);
        let ask_price = round_price_to_tick_up(ask_raw, tick);
        let base_size = notional_to_base(self.cfg.notional, mid, lot);

        if bid_price <= 0.0 || ask_price <= 0.0 || base_size <= 0.0 {
            return Ok(());
        }

        // --- Cancel all live quotes then resubmit ---
        self.cancel_all_orders(self.cfg.instrument_id, None, None, None)?;

        let bid_order = self.core.order_factory().limit(
            self.cfg.instrument_id,
            OrderSide::Buy,
            Quantity::new(base_size, sp),
            Price::new(bid_price, pp),
            Some(TimeInForce::Gtc),
            None,
            Some(true), // post_only
            None, None, None, None, None, None, None, None, None,
        );
        self.submit_order(bid_order, None, None, None)?;

        let ask_order = self.core.order_factory().limit(
            self.cfg.instrument_id,
            OrderSide::Sell,
            Quantity::new(base_size, sp),
            Price::new(ask_price, pp),
            Some(TimeInForce::Gtc),
            None,
            Some(true), // post_only
            None, None, None, None, None, None, None, None, None,
        );
        self.submit_order(ask_order, None, None, None)?;

        Ok(())
    }
}

nautilus_strategy!(PureGlftStrategy);

impl Debug for PureGlftStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PureGlftStrategy")
            .field("instrument_id", &self.cfg.instrument_id)
            .finish()
    }
}

impl DataActor for PureGlftStrategy {
    fn on_start(&mut self) -> anyhow::Result<()> {
        let id = self.cfg.instrument_id;

        let instrument = {
            let cache = self.cache();
            match cache.instrument(&id) {
                Some(i) => i.clone(),
                None => {
                    log::error!("Instrument not found: {id}");
                    return Ok(());
                }
            }
        };
        self.instrument = Some(instrument);

        self.subscribe_quotes(id, None, None);
        self.subscribe_trades(id, None, None);

        let interval_ns = self.cfg.requote_interval_ms * 1_000_000;
        self.clock().set_timer_ns(
            TIMER, interval_ns, None, None, None, None, Some(false),
        )?;

        log::info!(
            "PureGlftStrategy started on {id} | γ={} σ={} A={} k={} ξ={} every {}ms",
            self.cfg.gamma, self.cfg.sigma, self.cfg.a, self.cfg.k,
            self.cfg.xi, self.cfg.requote_interval_ms
        );
        Ok(())
    }

    fn on_stop(&mut self) -> anyhow::Result<()> {
        self.clock().cancel_timer(TIMER);
        self.cancel_all_orders(self.cfg.instrument_id, None, None, None)?;
        log::info!("PureGlftStrategy stopped");
        Ok(())
    }

    fn on_reset(&mut self) -> anyhow::Result<()> {
        self.instrument = None;
        self.last_mid = 0.0;
        Ok(())
    }

    fn on_quote(&mut self, tick: &QuoteTick) -> anyhow::Result<()> {
        // Keep a fallback mid in case the order book isn't populated yet.
        self.last_mid = (tick.bid_price.as_f64() + tick.ask_price.as_f64()) / 2.0;
        Ok(())
    }

    fn on_trade(&mut self, _tick: &TradeTick) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_time_event(&mut self, event: &TimeEvent) -> anyhow::Result<()> {
        if event.name.as_str() == TIMER {
            if let Err(e) = self.requote() {
                log::error!("PureGlft requote error: {e}");
            }
        }
        Ok(())
    }
}
