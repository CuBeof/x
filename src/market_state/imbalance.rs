//! Order-book imbalance indicators.

/// L1 order-book imbalance from best bid/ask sizes. Value in [-1, +1].
pub struct L1Imbalance {
    pub value: f64,
    pub initialized: bool,
}

impl L1Imbalance {
    pub fn new() -> Self {
        Self { value: 0.0, initialized: false }
    }

    pub fn update(&mut self, bid_sz: f64, ask_sz: f64) {
        let total = bid_sz + ask_sz;
        self.value = if total > 0.0 { (bid_sz - ask_sz) / total } else { 0.0 };
        self.initialized = true;
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }
}

/// Weighted order-book imbalance across N depth levels. Value in [-1, +1].
pub struct MultiLevelImbalance {
    pub n_levels: usize,
    pub value: f64,
    pub initialized: bool,
}

impl MultiLevelImbalance {
    pub fn new(n_levels: usize) -> Self {
        Self { n_levels, value: 0.0, initialized: false }
    }

    /// Update from pre-collected size slices (one per level, sorted best-to-worst).
    pub fn update_from_levels(&mut self, bids: &[f64], asks: &[f64]) {
        if bids.is_empty() || asks.is_empty() {
            return;
        }
        let n = bids.len().min(asks.len()).min(self.n_levels);
        let mut total_bid = 0.0;
        let mut total_ask = 0.0;
        for lvl in 0..n {
            let weight = (self.n_levels - lvl) as f64;
            total_bid += bids[lvl] * weight;
            total_ask += asks[lvl] * weight;
        }
        let denom = total_bid + total_ask;
        self.value = if denom > 0.0 { (total_bid - total_ask) / denom } else { 0.0 };
        self.initialized = true;
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }
}
