//! Price skew: combine GLFT spread with inventory + signal adjustments.

use super::{
    glft::{half_spread, reservation_price},
    parameters::GlftParameters,
};
use crate::types::MarketSnapshot;

pub struct SkewedQuote {
    pub reservation: f64,
    pub delta_bid_frac: f64,
    pub delta_ask_frac: f64,
}

pub fn compute_skew(
    p: &GlftParameters,
    snap: &MarketSnapshot,
    obi_weight_bps: f64,
    ofi_weight_bps: f64,
    toxicity_spread_bps: f64,
    markout_spread_bps: f64,
    use_obi: bool,
    use_ofi: bool,
    use_toxicity: bool,
    use_markout: bool,
) -> SkewedQuote {
    let base_delta = half_spread(p);
    let r = reservation_price(snap.mid, p, snap.q_norm);

    let bps = 1e-4;
    let obi_skew = if use_obi { obi_weight_bps * snap.obi_l1 * bps } else { 0.0 };
    let ofi_skew = if use_ofi { ofi_weight_bps * snap.ofi * bps } else { 0.0 };

    let tox_extra = if use_toxicity { toxicity_spread_bps * snap.toxicity * bps } else { 0.0 };

    let mk_extra = if use_markout && snap.markout_bps < 0.0 {
        markout_spread_bps * snap.markout_bps.abs() / 10.0 * bps
    } else {
        0.0
    };

    let symmetric_extra = tox_extra + mk_extra;
    let signal = obi_skew + ofi_skew;

    let delta_ask = (base_delta + symmetric_extra + signal).max(0.0);
    let delta_bid = (base_delta + symmetric_extra - signal).max(0.0);

    SkewedQuote {
        reservation: r,
        delta_bid_frac: delta_bid,
        delta_ask_frac: delta_ask,
    }
}
