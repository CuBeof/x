//! Order-Flow Imbalance (OFI) indicator.
//!
//! Tracks changes in best-bid and best-ask volume between consecutive quotes.
//! Value in [-1, +1]. +1 = strong aggressive buying.

use std::collections::VecDeque;

pub struct OrderFlowImbalance {
    window: usize,
    prev_bid_sz: Option<f64>,
    prev_ask_sz: Option<f64>,
    prev_bid_px: Option<f64>,
    prev_ask_px: Option<f64>,
    ofi_buf: VecDeque<f64>,
    pub value: f64,
    pub initialized: bool,
}

impl OrderFlowImbalance {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            prev_bid_sz: None,
            prev_ask_sz: None,
            prev_bid_px: None,
            prev_ask_px: None,
            ofi_buf: VecDeque::with_capacity(window),
            value: 0.0,
            initialized: false,
        }
    }

    pub fn update(&mut self, bid_px: f64, ask_px: f64, bid_sz: f64, ask_sz: f64) {
        match (self.prev_bid_sz, self.prev_ask_sz, self.prev_bid_px, self.prev_ask_px) {
            (Some(pbsz), Some(pasz), Some(pbpx), Some(papx)) => {
                let delta_bid = if bid_px >= pbpx {
                    bid_sz - if bid_px == pbpx { pbsz } else { 0.0 }
                } else {
                    -pbsz
                };
                let delta_ask = if ask_px <= papx {
                    ask_sz - if ask_px == papx { pasz } else { 0.0 }
                } else {
                    -pasz
                };

                let ofi_raw = delta_bid - delta_ask;
                if self.ofi_buf.len() >= self.window {
                    self.ofi_buf.pop_front();
                }
                self.ofi_buf.push_back(ofi_raw);

                let total: f64 = self.ofi_buf.iter().map(|v| v.abs()).sum();
                self.value = if total > 0.0 {
                    self.ofi_buf.iter().sum::<f64>() / total
                } else {
                    0.0
                };

                if self.ofi_buf.len() >= self.window {
                    self.initialized = true;
                }
            }
            _ => {}
        }

        self.prev_bid_sz = Some(bid_sz);
        self.prev_ask_sz = Some(ask_sz);
        self.prev_bid_px = Some(bid_px);
        self.prev_ask_px = Some(ask_px);
    }

    pub fn reset(&mut self) {
        self.prev_bid_sz = None;
        self.prev_ask_sz = None;
        self.prev_bid_px = None;
        self.prev_ask_px = None;
        self.ofi_buf.clear();
        self.value = 0.0;
        self.initialized = false;
    }
}
