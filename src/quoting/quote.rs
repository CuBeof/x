//! Value objects for desired quotes (no nautilus imports).

#[derive(Debug, Clone, Copy)]
pub struct QuoteLevel {
    pub price: f64,
    pub notional: f64,
    pub base_size: f64,
}

#[derive(Debug, Clone)]
pub struct QuoteLadder {
    pub bids: Vec<QuoteLevel>,
    pub asks: Vec<QuoteLevel>,
    pub reservation: f64,
    pub mid: f64,
}

impl QuoteLadder {
    pub fn empty(mid: f64) -> Self {
        Self { bids: vec![], asks: vec![], reservation: mid, mid }
    }

    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }
}
