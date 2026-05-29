//! Quote size shaping: per-side, per-level sizing.

use crate::types::MarketSnapshot;

pub struct SizedLadder {
    pub bid_notionals: Vec<f64>,
    pub ask_notionals: Vec<f64>,
}

pub fn compute_sizes(
    snap: &MarketSnapshot,
    n_levels: usize,
    notional_per_level: f64,
    max_total_notional: f64,
    toxicity_size_floor: f64,
    use_toxicity: bool,
) -> SizedLadder {
    let regime_mult = snap.regime.size_mult;

    let tox_mult = if use_toxicity && snap.toxicity > 0.0 {
        1.0 - snap.toxicity * (1.0 - toxicity_size_floor)
    } else {
        1.0
    };

    let base = notional_per_level * regime_mult * tox_mult;
    let q = snap.q_norm;
    let bid_inv_mult = (1.0 - q.max(0.0)).max(0.0);
    let ask_inv_mult = (1.0 + q.min(0.0)).max(0.0);

    let mut bid_notionals: Vec<f64> = (0..n_levels)
        .map(|lvl| base * bid_inv_mult * (1.0 + lvl as f64 * 0.5))
        .collect();
    let mut ask_notionals: Vec<f64> = (0..n_levels)
        .map(|lvl| base * ask_inv_mult * (1.0 + lvl as f64 * 0.5))
        .collect();

    let bid_total: f64 = bid_notionals.iter().sum();
    if bid_total > max_total_notional {
        let factor = max_total_notional / bid_total;
        bid_notionals.iter_mut().for_each(|v| *v *= factor);
    }

    let ask_total: f64 = ask_notionals.iter().sum();
    if ask_total > max_total_notional {
        let factor = max_total_notional / ask_total;
        ask_notionals.iter_mut().for_each(|v| *v *= factor);
    }

    SizedLadder {
        bid_notionals,
        ask_notionals,
    }
}
