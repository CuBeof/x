//! Volatility regime classifier.

use crate::{config::SignalConfig, types::RegimeState};

pub fn classify_regime(sigma: f64, signal_cfg: &SignalConfig) -> RegimeState {
    let sigma_bps = sigma * 1e4;
    if sigma_bps < signal_cfg.regime_low_bps {
        RegimeState {
            label: "low",
            spread_mult: signal_cfg.regime_low_spread_mult,
            size_mult: signal_cfg.regime_low_size_mult,
            gamma_mult: signal_cfg.regime_low_gamma_mult,
        }
    } else if sigma_bps > signal_cfg.regime_high_bps {
        RegimeState {
            label: "high",
            spread_mult: signal_cfg.regime_high_spread_mult,
            size_mult: signal_cfg.regime_high_size_mult,
            gamma_mult: signal_cfg.regime_high_gamma_mult,
        }
    } else {
        RegimeState {
            label: "med",
            spread_mult: signal_cfg.regime_med_spread_mult,
            size_mult: signal_cfg.regime_med_size_mult,
            gamma_mult: signal_cfg.regime_med_gamma_mult,
        }
    }
}
