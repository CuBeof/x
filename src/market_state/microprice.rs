//! Microprice indicator — size-weighted mid from best bid/ask.

pub struct Microprice {
    pub value: f64,
    pub initialized: bool,
}

impl Microprice {
    pub fn new() -> Self {
        Self { value: 0.0, initialized: false }
    }

    pub fn update(&mut self, bid: f64, ask: f64, bid_sz: f64, ask_sz: f64) {
        let total = bid_sz + ask_sz;
        self.value = if total > 0.0 {
            (ask_sz * bid + bid_sz * ask) / total
        } else {
            (bid + ask) / 2.0
        };
        self.initialized = true;
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }
}
