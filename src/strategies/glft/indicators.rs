use std::collections::VecDeque;

pub struct VolatilityEstimator {
    window: usize,
    prices: VecDeque<f64>,
}

impl VolatilityEstimator {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            prices: VecDeque::new(),
        }
    }

    pub fn update(&mut self, price: f64) {
        self.prices.push_back(price);
        if self.prices.len() > self.window + 1 {
            self.prices.pop_front();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.prices.len() > 10
    }

    /// Returns annualized volatility (assumes per-quote updates).
    pub fn volatility(&self) -> f64 {
        if self.prices.len() < 2 {
            return 0.0;
        }
        let returns: Vec<f64> = self
            .prices
            .iter()
            .zip(self.prices.iter().skip(1))
            .map(|(a, b)| (b / a).ln())
            .collect();
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0).max(1.0);
        // Annualize: assume ~86400 quotes/day (1 per second)
        (variance * 86400.0 * 365.0).sqrt()
    }
}

pub struct SpreadAnalyzer {
    window: usize,
    spreads_bps: VecDeque<f64>,
}

impl SpreadAnalyzer {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            spreads_bps: VecDeque::new(),
        }
    }

    pub fn update(&mut self, bid: f64, ask: f64) {
        let mid = (bid + ask) / 2.0;
        if mid > 0.0 {
            let bps = (ask - bid) / mid * 10_000.0;
            self.spreads_bps.push_back(bps);
            if self.spreads_bps.len() > self.window {
                self.spreads_bps.pop_front();
            }
        }
    }

    pub fn avg_spread_bps(&self) -> f64 {
        if self.spreads_bps.is_empty() {
            return 0.0;
        }
        self.spreads_bps.iter().sum::<f64>() / self.spreads_bps.len() as f64
    }
}

pub struct OrderFlowImbalance {
    window: usize,
    buy_volume: VecDeque<f64>,
    sell_volume: VecDeque<f64>,
}

impl OrderFlowImbalance {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            buy_volume: VecDeque::new(),
            sell_volume: VecDeque::new(),
        }
    }

    pub fn update_trade(&mut self, size: f64, is_buy: bool) {
        let (b, s) = if is_buy { (size, 0.0) } else { (0.0, size) };
        self.buy_volume.push_back(b);
        self.sell_volume.push_back(s);
        if self.buy_volume.len() > self.window {
            self.buy_volume.pop_front();
            self.sell_volume.pop_front();
        }
    }

    /// Returns imbalance in [-1, 1]: +1 = all buys, -1 = all sells.
    pub fn imbalance(&self) -> f64 {
        let total_buy: f64 = self.buy_volume.iter().sum();
        let total_sell: f64 = self.sell_volume.iter().sum();
        let total = total_buy + total_sell;
        if total == 0.0 {
            0.0
        } else {
            (total_buy - total_sell) / total
        }
    }
}

pub struct GlftIndicators {
    pub vol: VolatilityEstimator,
    pub spread: SpreadAnalyzer,
    pub flow: OrderFlowImbalance,
}

impl GlftIndicators {
    pub fn new(vol_window: usize) -> Self {
        Self {
            vol: VolatilityEstimator::new(vol_window),
            spread: SpreadAnalyzer::new(vol_window),
            flow: OrderFlowImbalance::new(vol_window),
        }
    }

    pub fn update_quote(&mut self, bid: f64, ask: f64) {
        let mid = (bid + ask) / 2.0;
        self.vol.update(mid);
        self.spread.update(bid, ask);
    }

    pub fn volatility(&self) -> f64 {
        self.vol.volatility()
    }

    pub fn spread_bps(&self) -> f64 {
        self.spread.avg_spread_bps()
    }

    pub fn flow_imbalance(&self) -> f64 {
        self.flow.imbalance()
    }

    pub fn is_ready(&self) -> bool {
        self.vol.is_ready()
    }
}
