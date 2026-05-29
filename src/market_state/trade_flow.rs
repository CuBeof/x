//! Toxic-flow and adverse-selection indicators.

use std::collections::VecDeque;

/// VPIN-like toxicity score from trade-tick volume imbalance. Value in [0, 1].
pub struct ToxicityIndicator {
    bucket_size: f64,
    n_buckets: usize,
    spike_window: usize,
    spike_mult: f64,

    cur_buy_vol: f64,
    cur_sell_vol: f64,
    cur_total_vol: f64,

    bucket_toxicities: VecDeque<f64>,
    arrival_times_ns: VecDeque<u64>,

    pub value: f64,
    pub initialized: bool,
}

impl ToxicityIndicator {
    pub fn new(
        volume_bucket: f64,
        n_buckets: usize,
        spike_window: usize,
        spike_mult: f64,
    ) -> Self {
        Self {
            bucket_size: volume_bucket,
            n_buckets,
            spike_window,
            spike_mult,
            cur_buy_vol: 0.0,
            cur_sell_vol: 0.0,
            cur_total_vol: 0.0,
            bucket_toxicities: VecDeque::with_capacity(n_buckets),
            arrival_times_ns: VecDeque::with_capacity(spike_window * 2),
            value: 0.0,
            initialized: false,
        }
    }

    /// `is_buyer`: Some(true) = buyer aggressor, Some(false) = seller, None = unknown.
    pub fn update(&mut self, qty: f64, is_buyer: Option<bool>, ts_event_ns: u64) {
        self.arrival_times_ns.push_back(ts_event_ns);
        if self.arrival_times_ns.len() > self.spike_window * 2 {
            self.arrival_times_ns.pop_front();
        }

        match is_buyer {
            Some(true) => self.cur_buy_vol += qty,
            Some(false) => self.cur_sell_vol += qty,
            None => {
                self.cur_buy_vol += qty / 2.0;
                self.cur_sell_vol += qty / 2.0;
            }
        }
        self.cur_total_vol += qty;

        if self.cur_total_vol >= self.bucket_size {
            let tox =
                (self.cur_buy_vol - self.cur_sell_vol).abs() / self.cur_total_vol;
            if self.bucket_toxicities.len() >= self.n_buckets {
                self.bucket_toxicities.pop_front();
            }
            self.bucket_toxicities.push_back(tox);
            self.cur_buy_vol = 0.0;
            self.cur_sell_vol = 0.0;
            self.cur_total_vol = 0.0;
        }

        if self.bucket_toxicities.len() >= self.n_buckets {
            self.initialized = true;
        }

        self.update_value();
    }

    fn update_value(&mut self) {
        if self.bucket_toxicities.is_empty() {
            self.value = 0.0;
            return;
        }

        let vpin = self.bucket_toxicities.iter().sum::<f64>()
            / self.bucket_toxicities.len() as f64;

        let times: Vec<u64> = self.arrival_times_ns.iter().copied().collect();
        let n = times.len();
        let spike_mult = if n >= self.spike_window * 2 {
            let half = n / 2;
            let dt_recent = (times[n - 1] - times[n - 1 - half + 1]) as f64
                / (half - 1).max(1) as f64;
            let dt_baseline = (times[half - 1] - times[0]) as f64
                / (half - 1).max(1) as f64;
            if dt_baseline > 0.0 && dt_recent > 0.0 {
                let rate_ratio = dt_baseline / dt_recent;
                if rate_ratio > self.spike_mult {
                    (rate_ratio / self.spike_mult).min(2.0)
                } else {
                    1.0
                }
            } else {
                1.0
            }
        } else {
            1.0
        };

        self.value = (vpin * spike_mult).min(1.0);
    }

    pub fn reset(&mut self) {
        self.cur_buy_vol = 0.0;
        self.cur_sell_vol = 0.0;
        self.cur_total_vol = 0.0;
        self.bucket_toxicities.clear();
        self.arrival_times_ns.clear();
        self.value = 0.0;
        self.initialized = false;
    }
}

struct PendingFill {
    fill_price: f64,
    side: f64,           // +1 = buy, -1 = sell
    ticks_remaining: i32,
}

/// Tracks price move after our limit-order fills (markout PnL in bps).
pub struct MarkoutTracker {
    horizon: i32,
    window: usize,
    pending: Vec<PendingFill>,
    markouts: VecDeque<f64>,
    current_mid: f64,
}

impl MarkoutTracker {
    pub fn new(horizon: i32, window: usize) -> Self {
        Self {
            horizon,
            window,
            pending: Vec::new(),
            markouts: VecDeque::with_capacity(window),
            current_mid: 0.0,
        }
    }

    /// Rolling mean markout in bps (negative = adverse selection).
    pub fn value(&self) -> f64 {
        if self.markouts.is_empty() {
            0.0
        } else {
            self.markouts.iter().sum::<f64>() / self.markouts.len() as f64
        }
    }

    pub fn initialized(&self) -> bool {
        self.markouts.len() >= (self.window / 5).max(1)
    }

    pub fn record_fill(&mut self, fill_price: f64, side: f64) {
        self.pending.push(PendingFill {
            fill_price,
            side,
            ticks_remaining: self.horizon,
        });
    }

    pub fn update_price(&mut self, mid: f64) {
        if mid <= 0.0 {
            return;
        }
        self.current_mid = mid;
        let mut completed_indices = Vec::new();

        for (i, entry) in self.pending.iter_mut().enumerate() {
            entry.ticks_remaining -= 1;
            if entry.ticks_remaining <= 0 {
                if entry.fill_price > 0.0 {
                    let markout_bps =
                        entry.side * (mid - entry.fill_price) / entry.fill_price * 1e4;
                    if self.markouts.len() >= self.window {
                        self.markouts.pop_front();
                    }
                    self.markouts.push_back(markout_bps);
                }
                completed_indices.push(i);
            }
        }

        for i in completed_indices.into_iter().rev() {
            self.pending.remove(i);
        }
    }

    pub fn reset(&mut self) {
        self.pending.clear();
        self.markouts.clear();
        self.current_mid = 0.0;
    }
}
