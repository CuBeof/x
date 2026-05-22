use std::time::{Duration, Instant};
use crate::strategies::glft::indicators::GlftIndicators;

#[derive(Debug, Clone, PartialEq)]
pub enum RiskState {
    Normal,
    HighVolatility,
    InventoryBreached,
    Paused,
}

pub struct RiskManager {
    pub state: RiskState,
    base_volatility: Option<f64>,
    vol_observations: u32,
    pub vol_multiplier: f64,
    pub max_inventory: f64,
    pub consecutive_rejections: u32,
    pause_until: Option<Instant>,
}

impl RiskManager {
    pub fn new(vol_multiplier: f64, max_inventory: f64) -> Self {
        Self {
            state: RiskState::Normal,
            base_volatility: None,
            vol_observations: 0,
            vol_multiplier,
            max_inventory,
            consecutive_rejections: 0,
            pause_until: None,
        }
    }

    pub fn check_risk(&mut self, indicators: &GlftIndicators, position: f64) -> &RiskState {
        // Expire any active pause
        if let Some(until) = self.pause_until {
            if Instant::now() >= until {
                self.pause_until = None;
                self.consecutive_rejections = 0;
                tracing::info!("Risk pause expired, resuming");
            } else {
                self.state = RiskState::Paused;
                return &self.state;
            }
        }

        let vol = indicators.volatility();

        // Calibrate base volatility from first 50 ready observations
        if self.base_volatility.is_none() && indicators.is_ready() {
            self.vol_observations += 1;
            if self.vol_observations >= 50 {
                self.base_volatility = Some(vol.max(0.001));
                tracing::info!(base_vol = vol, "Calibrated base volatility");
            }
        }

        // Too many consecutive rejections → pause
        if self.consecutive_rejections >= 5 {
            self.state = RiskState::Paused;
            self.pause_until = Some(Instant::now() + Duration::from_secs(10));
            tracing::warn!(
                rejections = self.consecutive_rejections,
                "Too many rejections, pausing 10s"
            );
            return &self.state;
        }

        // Inventory breach
        if position.abs() >= self.max_inventory {
            self.state = RiskState::InventoryBreached;
            tracing::warn!(position, max = self.max_inventory, "Inventory limit breached");
            return &self.state;
        }

        // Volatility regime change
        if let Some(base_vol) = self.base_volatility {
            if vol > base_vol * self.vol_multiplier {
                self.state = RiskState::HighVolatility;
                self.pause_until = Some(Instant::now() + Duration::from_secs(30));
                tracing::warn!(
                    current_vol = vol,
                    base_vol,
                    threshold = base_vol * self.vol_multiplier,
                    "High volatility, pausing 30s"
                );
                return &self.state;
            }
        }

        self.state = RiskState::Normal;
        &self.state
    }

    /// Returns true if we are allowed to place new quotes
    pub fn is_quoting_allowed(&self) -> bool {
        matches!(self.state, RiskState::Normal | RiskState::InventoryBreached)
    }

    /// Returns true if we should only place orders that reduce inventory
    pub fn is_reduce_only(&self) -> bool {
        self.state == RiskState::InventoryBreached
    }

    pub fn on_rejection(&mut self) {
        self.consecutive_rejections += 1;
        tracing::debug!(count = self.consecutive_rejections, "Order rejected");
    }

    pub fn on_fill(&mut self) {
        self.consecutive_rejections = 0;
    }
}
