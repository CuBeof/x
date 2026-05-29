/// Immutable snapshot of GLFT model parameters for one requote cycle.
///
/// All spread/shift outputs from `glft.rs` are in FRACTIONS of price.
#[derive(Debug, Clone, Copy)]
pub struct GlftParameters {
    pub gamma: f64, // risk aversion (dimensionless, >0)
    pub sigma: f64, // log-return σ (fraction, e.g. 0.003)
    pub a: f64,     // order arrival intensity (per-second, >0)
    pub k: f64,     // arrival decay (per-fraction, >0); λ = A·exp(-k·δ_frac)
    pub xi: f64,    // inventory horizon scaling (dimensionless, >0)
}
