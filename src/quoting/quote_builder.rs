//! Build the desired QuoteLadder from model outputs, signals, and regime.

use crate::{
    config::GlftMarketMakerConfig,
    model::{parameters::GlftParameters, sizing::compute_sizes, skew::compute_skew},
    quoting::{
        quote::{QuoteLevel, QuoteLadder},
        rounding::{notional_to_base, round_price_to_tick_down, round_price_to_tick_up},
    },
    types::MarketSnapshot,
};

pub fn build_ladder(
    snap: &MarketSnapshot,
    params: &GlftParameters,
    tick_size: f64,
    lot_size: f64,
    cfg: &GlftMarketMakerConfig,
) -> QuoteLadder {
    let sc = &cfg.signal;
    let qc = &cfg.quote;
    let mid = snap.mid;

    let skewed = compute_skew(
        params,
        snap,
        sc.obi_weight,
        sc.ofi_weight,
        sc.toxicity_spread_bps,
        sc.markout_spread_bps,
        sc.use_obi,
        sc.use_ofi,
        sc.use_toxicity,
        sc.use_markout,
    );

    let sized = compute_sizes(
        snap,
        qc.levels,
        qc.notional_per_level,
        qc.max_notional_total,
        sc.toxicity_size_floor,
        sc.use_toxicity,
    );

    let regime_mult = snap.regime.spread_mult;
    let spread_bps = qc.level_spacing_bps * 1e-4;
    let min_delta = qc.min_spread_bps * 1e-4;
    let max_delta = qc.max_spread_bps * 1e-4;

    let base_bid = clamp(skewed.delta_bid_frac * regime_mult, min_delta, max_delta);
    let base_ask = clamp(skewed.delta_ask_frac * regime_mult, min_delta, max_delta);

    let mut bid_levels: Vec<QuoteLevel> = Vec::with_capacity(qc.levels);
    let mut ask_levels: Vec<QuoteLevel> = Vec::with_capacity(qc.levels);

    for lvl in 0..qc.levels {
        let level_extra = lvl as f64 * spread_bps * regime_mult;

        let delta_bid = base_bid + level_extra;
        let bid_price_raw = skewed.reservation - delta_bid * mid;
        let bid_price = round_price_to_tick_down(bid_price_raw, tick_size);

        let delta_ask = base_ask + level_extra;
        let ask_price_raw = skewed.reservation + delta_ask * mid;
        let ask_price = round_price_to_tick_up(ask_price_raw, tick_size);

        let bid_notional = sized.bid_notionals[lvl];
        let ask_notional = sized.ask_notionals[lvl];

        if bid_price > 0.0 && bid_notional > 0.0 {
            let bid_base = notional_to_base(bid_notional, mid, lot_size);
            if bid_base > 0.0 {
                bid_levels.push(QuoteLevel {
                    price: bid_price,
                    notional: bid_notional,
                    base_size: bid_base,
                });
            }
        }

        if ask_price > 0.0 && ask_notional > 0.0 {
            let ask_base = notional_to_base(ask_notional, mid, lot_size);
            if ask_base > 0.0 {
                ask_levels.push(QuoteLevel {
                    price: ask_price,
                    notional: ask_notional,
                    base_size: ask_base,
                });
            }
        }
    }

    QuoteLadder {
        bids: bid_levels,
        asks: ask_levels,
        reservation: skewed.reservation,
        mid,
    }
}

fn clamp(value: f64, lo: f64, hi: f64) -> f64 {
    value.max(lo).min(hi)
}
