//! Volatility estimator: EWMA of squared log returns from trade prices.
//! Updated manually in the strategy's on_trade handler.

use std::collections::VecDeque;

pub struct VolatilityEwma {
    _half_life: usize,
    warmup: usize,
    lam: f64,
    var: f64,
    prev_price: Option<f64>,
    init_buf: VecDeque<f64>,
    pub initialized: bool,
}

impl VolatilityEwma {
    pub fn new(half_life: usize, warmup: usize) -> Self {
        let lam = (-std::f64::consts::LN_2 / half_life as f64).exp();
        Self {
            _half_life: half_life,
            warmup,
            lam,
            var: 0.0,
            prev_price: None,
            init_buf: VecDeque::with_capacity(warmup),
            initialized: false,
        }
    }

    /// σ (log-return std, fraction).
    pub fn value(&self) -> f64 {
        if self.var > 0.0 { self.var.sqrt() } else { 0.0 }
    }

    /// Update with a new trade price.
    pub fn update(&mut self, price: f64) {
        if price <= 0.0 {
            return;
        }
        let prev = match self.prev_price {
            Some(p) if p > 0.0 => p,
            _ => {
                self.prev_price = Some(price);
                return;
            }
        };
        self.prev_price = Some(price);
        let log_ret = (price / prev).ln();

        if !self.initialized {
            self.init_buf.push_back(log_ret * log_ret);
            if self.init_buf.len() >= self.warmup {
                self.var = self.init_buf.iter().sum::<f64>() / self.init_buf.len() as f64;
                self.initialized = true;
            }
        } else {
            self.var = self.lam * self.var + (1.0 - self.lam) * log_ret * log_ret;
        }
    }

    pub fn reset(&mut self) {
        self.var = 0.0;
        self.prev_price = None;
        self.init_buf.clear();
        self.initialized = false;
    }
}
