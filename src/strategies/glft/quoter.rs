use crate::strategies::glft::risk::RiskState;

pub struct GlftQuoter {
    pub gamma: f64,
    pub kappa: f64,
    pub min_spread_bps: f64,
}

#[derive(Debug, Clone)]
pub struct QuoteDecision {
    pub should_quote: bool,
    pub bid_price: Option<f64>,
    pub ask_price: Option<f64>,
    pub bid_size: f64,
    pub ask_size: f64,
    pub reservation_price: f64,
    pub half_spread: f64,
}

impl GlftQuoter {
    pub fn new(gamma: f64, kappa: f64, min_spread_bps: f64) -> Self {
        Self {
            gamma,
            kappa,
            min_spread_bps,
        }
    }

    pub fn compute(
        &self,
        mid_price: f64,
        inventory: f64,
        sigma: f64,
        elapsed_secs: f64,
        time_horizon_secs: f64,
        order_size: f64,
        risk_state: &RiskState,
    ) -> QuoteDecision {
        if !matches!(risk_state, RiskState::Normal | RiskState::InventoryBreached) {
            return QuoteDecision {
                should_quote: false,
                bid_price: None,
                ask_price: None,
                bid_size: order_size,
                ask_size: order_size,
                reservation_price: mid_price,
                half_spread: 0.0,
            };
        }

        // Normalized remaining time τ ∈ (0, 1]
        let remaining = (time_horizon_secs - elapsed_secs).max(1.0);
        let tau = (remaining / time_horizon_secs).clamp(0.001, 1.0);
        let gamma = self.gamma;
        let kappa = self.kappa;
        let sigma2 = sigma * sigma;

        // GLFT reservation price: r = s - q * γ * σ² * τ
        let reservation_price = mid_price - inventory * gamma * sigma2 * tau;

        // GLFT optimal half-spread: δ = (γ * σ² * τ) / 2 + (1/γ) * ln(1 + γ/κ)
        let mut half_spread = (gamma * sigma2 * tau) / 2.0
            + (1.0 / gamma) * (1.0 + gamma / kappa).ln();

        // Apply minimum spread floor
        let min_half = self.min_spread_bps * mid_price / 10_000.0 / 2.0;
        half_spread = half_spread.max(min_half);

        // Determine which sides to quote based on risk state
        let (bid, ask) = match risk_state {
            RiskState::InventoryBreached if inventory > 0.0 => {
                // Long beyond limit: only sell to reduce
                (None, Some(reservation_price + half_spread))
            }
            RiskState::InventoryBreached if inventory < 0.0 => {
                // Short beyond limit: only buy to reduce
                (Some(reservation_price - half_spread), None)
            }
            _ => (
                Some(reservation_price - half_spread),
                Some(reservation_price + half_spread),
            ),
        };

        QuoteDecision {
            should_quote: true,
            bid_price: bid,
            ask_price: ask,
            bid_size: order_size,
            ask_size: order_size,
            reservation_price,
            half_spread,
        }
    }
}
