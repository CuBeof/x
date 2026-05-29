/// Volatility-regime classification result.
#[derive(Debug, Clone)]
pub struct RegimeState {
    pub label: &'static str, // "low" | "med" | "high"
    pub spread_mult: f64,
    pub size_mult: f64,
    pub gamma_mult: f64,
}

/// All market-state features sampled at one requote cycle.
///
/// Unit convention:
/// - mid, microprice  : absolute price
/// - sigma            : log-return std (fraction, e.g. 0.003 = 30 bps)
/// - obi_l1, obi_multi: order-book imbalance in [-1, +1]
/// - ofi              : order-flow imbalance in [-1, +1]
/// - toxicity         : [0, 1]; 1 = highly toxic flow
/// - markout_bps      : fill markout in bps (negative = adverse selection)
/// - q_norm           : signed inventory / max_position, clamped to [-1, +1]
/// - a, k             : arrival intensity (per-s) and decay (per-fraction)
#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub timestamp_ns: u64,
    pub mid: f64,
    pub microprice: f64,

    pub sigma: f64,
    pub obi_l1: f64,
    pub obi_multi: f64,
    pub ofi: f64,
    pub toxicity: f64,
    pub markout_bps: f64,

    pub regime: RegimeState,

    pub q_norm: f64,
    pub a: f64,
    pub k: f64,
    pub estimator_ready: bool,
}
