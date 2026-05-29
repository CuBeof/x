use nautilus_model::identifiers::InstrumentId;
use nautilus_trading::strategy::config::StrategyConfig;

/// Pure GLFT model parameters.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub gamma: f64,
    /// Inventory-shift horizon scaling. shift = q_norm·γ·σ²·ξ (fraction).
    pub xi: f64,
    /// Fallback A until estimator warms up (per-second arrival intensity).
    pub a_init: f64,
    /// Fallback k until estimator warms up (per-fraction price decay).
    pub k_init: f64,
    /// Minimum trades before trusting estimator.
    pub estimator_min_trades: usize,
    /// Rolling trade window for A,k fit.
    pub estimator_window: usize,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            gamma: 0.1,
            xi: 1000.0,
            a_init: 1.0,
            k_init: 1000.0,
            estimator_min_trades: 500,
            estimator_window: 5000,
        }
    }
}

/// Quote ladder geometry and order parameters.
#[derive(Debug, Clone)]
pub struct QuoteConfig {
    pub levels: usize,
    pub level_spacing_bps: f64,
    pub notional_per_level: f64,
    pub max_notional_total: f64,
    pub min_spread_bps: f64,
    pub max_spread_bps: f64,
    pub tolerance_bps: f64,
    pub requote_interval_ms: u64,
}

impl Default for QuoteConfig {
    fn default() -> Self {
        Self {
            levels: 3,
            level_spacing_bps: 5.0,
            notional_per_level: 100.0,
            max_notional_total: 1000.0,
            min_spread_bps: 2.0,
            max_spread_bps: 200.0,
            tolerance_bps: 1.0,
            requote_interval_ms: 500,
        }
    }
}

/// Signal weights and feature toggles.
#[derive(Debug, Clone)]
pub struct SignalConfig {
    pub obi_weight: f64,
    pub ofi_weight: f64,
    pub use_obi: bool,
    pub use_ofi: bool,

    pub toxicity_spread_bps: f64,
    pub toxicity_size_floor: f64,
    pub use_toxicity: bool,

    pub markout_spread_bps: f64,
    pub use_markout: bool,

    pub vol_halflife_trades: usize,

    pub regime_low_bps: f64,
    pub regime_high_bps: f64,
    pub regime_low_spread_mult: f64,
    pub regime_low_size_mult: f64,
    pub regime_low_gamma_mult: f64,
    pub regime_med_spread_mult: f64,
    pub regime_med_size_mult: f64,
    pub regime_med_gamma_mult: f64,
    pub regime_high_spread_mult: f64,
    pub regime_high_size_mult: f64,
    pub regime_high_gamma_mult: f64,

    pub obi_levels: usize,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            obi_weight: 0.3,
            ofi_weight: 0.2,
            use_obi: true,
            use_ofi: true,
            toxicity_spread_bps: 10.0,
            toxicity_size_floor: 0.2,
            use_toxicity: true,
            markout_spread_bps: 5.0,
            use_markout: true,
            vol_halflife_trades: 500,
            regime_low_bps: 20.0,
            regime_high_bps: 60.0,
            regime_low_spread_mult: 0.5,
            regime_low_size_mult: 1.0,
            regime_low_gamma_mult: 0.8,
            regime_med_spread_mult: 1.0,
            regime_med_size_mult: 1.0,
            regime_med_gamma_mult: 1.0,
            regime_high_spread_mult: 1.5,
            regime_high_size_mult: 0.6,
            regime_high_gamma_mult: 1.3,
            obi_levels: 5,
        }
    }
}

/// Risk limits and guards.
#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_position_notional: f64,
    /// Inventory fraction at which to enter reducing-only mode.
    pub inventory_reduce_threshold: f64,
    /// Daily loss limit in quote ccy; negative trigger.
    pub daily_loss_limit: f64,
    pub max_orders_per_side: usize,
    pub max_book_spread_bps: f64,
    pub stale_data_ms: u64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_notional: 500.0,
            inventory_reduce_threshold: 0.8,
            daily_loss_limit: -200.0,
            max_orders_per_side: 5,
            max_book_spread_bps: 50.0,
            stale_data_ms: 5000,
        }
    }
}

/// Top-level strategy configuration.
#[derive(Debug, Clone)]
pub struct GlftMarketMakerConfig {
    /// nautilus StrategyConfig (strategy_id, order_id_tag, etc.)
    pub base: StrategyConfig,
    pub instrument_id: InstrumentId,
    pub model: ModelConfig,
    pub quote: QuoteConfig,
    pub signal: SignalConfig,
    pub risk: RiskConfig,
}

impl GlftMarketMakerConfig {
    pub fn new(instrument_id: InstrumentId) -> Self {
        Self {
            base: StrategyConfig::default(),
            instrument_id,
            model: ModelConfig::default(),
            quote: QuoteConfig::default(),
            signal: SignalConfig::default(),
            risk: RiskConfig::default(),
        }
    }
}
