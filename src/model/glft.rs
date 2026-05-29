//! GLFT closed-form market-making model (pure functions, no nautilus imports).
//!
//! Unit convention (all inputs and outputs in FRACTIONS of price):
//!   δ     — distance from mid as a fraction (e.g. 0.0005 = 5 bps = 0.05%)
//!   σ     — log-return volatility (fraction per √interval)
//!   shift — reservation price offset as a fraction
//!   spread— half-spread as a fraction

use super::parameters::GlftParameters;

/// λ(δ) = A · exp(-k · δ)
pub fn arrival_rate(delta_frac: f64, a: f64, k: f64) -> f64 {
    a * (-k * delta_frac).exp()
}

/// GLFT spread constant: (1 / γ) · ln(1 + γ / k).
pub fn c1(p: &GlftParameters) -> f64 {
    (1.0 / p.gamma) * (1.0 + p.gamma / p.k).ln()
}

/// Simplified GLFT volatility coefficient: sqrt(γ / (2·A·k)).
pub fn c2_simplified(p: &GlftParameters) -> f64 {
    (p.gamma / (2.0 * p.a * p.k)).sqrt()
}

/// Full GLFT volatility coefficient including the (1 + γ/k)^(0.5 + k/(2γ)) factor.
pub fn c2_full(p: &GlftParameters) -> f64 {
    let gamma_over_k = p.gamma / p.k;
    let exponent = 0.5 + p.k / (2.0 * p.gamma);
    (p.gamma / (2.0 * p.a * p.k)).sqrt() * (1.0 + gamma_over_k).powf(exponent)
}

/// Symmetric base half-spread δ* = c1 + 0.5·c2·σ  (fraction of price).
pub fn half_spread(p: &GlftParameters) -> f64 {
    c1(p) + 0.5 * c2_full(p) * p.sigma
}

/// Reservation price offset from mid: shift = q_norm · γ · σ² · ξ  (fraction).
pub fn inventory_shift(p: &GlftParameters, q_norm: f64) -> f64 {
    q_norm * p.gamma * p.sigma.powi(2) * p.xi
}

/// r = mid · (1 - shift_frac).  Returns absolute price.
pub fn reservation_price(mid: f64, p: &GlftParameters, q_norm: f64) -> f64 {
    let shift = inventory_shift(p, q_norm);
    mid * (1.0 - shift)
}
