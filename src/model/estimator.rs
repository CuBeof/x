//! Online estimation of GLFT arrival parameters A and k.
//!
//! Fits λ(δ) = A · exp(-k · δ) by log-linear regression:
//!   ln(fill_rate_i) = ln(A) - k · δ_i
//!
//! Bucketed by distance-from-mid in fractional units.

use std::collections::VecDeque;

struct Bucket {
    lo: f64,
    hi: f64,
    n_arrivals: u64,
    n_fills: u64,
}

impl Bucket {
    fn fill_rate(&self) -> Option<f64> {
        if self.n_arrivals == 0 {
            None
        } else {
            Some(self.n_fills as f64 / self.n_arrivals as f64)
        }
    }

    fn midpoint(&self) -> f64 {
        (self.lo + self.hi) / 2.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AkEstimate {
    pub a: f64,
    pub k: f64,
    pub n_samples: usize,
    pub valid: bool,
}

pub struct AkEstimator {
    n_bins: usize,
    max_dist: f64,
    window: usize,
    min_trades: usize,
    bin_width: f64,
    buckets: Vec<Bucket>,
    history: VecDeque<(f64, bool)>,
    total_trades: usize,
}

impl AkEstimator {
    pub fn new(n_bins: usize, max_distance_frac: f64, window: usize, min_trades: usize) -> Self {
        let bin_width = max_distance_frac / n_bins as f64;
        let buckets = (0..n_bins)
            .map(|i| Bucket {
                lo: i as f64 * bin_width,
                hi: (i + 1) as f64 * bin_width,
                n_arrivals: 0,
                n_fills: 0,
            })
            .collect();
        Self {
            n_bins,
            max_dist: max_distance_frac,
            window,
            min_trades,
            bin_width,
            buckets,
            history: VecDeque::with_capacity(window),
            total_trades: 0,
        }
    }

    fn bucket_idx(&self, distance_frac: f64) -> Option<usize> {
        if distance_frac < 0.0 || distance_frac >= self.max_dist {
            return None;
        }
        Some(((distance_frac / self.bin_width) as usize).min(self.n_bins - 1))
    }

    pub fn record_trade(&mut self, distance_frac: f64) {
        let d = distance_frac.abs();
        if let Some(idx) = self.bucket_idx(d) {
            if self.history.len() >= self.window {
                if let Some((old_d, is_fill)) = self.history.pop_front() {
                    if let Some(old_idx) = self.bucket_idx(old_d) {
                        self.buckets[old_idx].n_arrivals =
                            self.buckets[old_idx].n_arrivals.saturating_sub(1);
                        if is_fill {
                            self.buckets[old_idx].n_fills =
                                self.buckets[old_idx].n_fills.saturating_sub(1);
                        }
                    }
                }
            }
            self.history.push_back((d, false));
            self.buckets[idx].n_arrivals += 1;
            self.total_trades += 1;
        }
    }

    pub fn record_fill(&mut self, distance_frac: f64) {
        let d = distance_frac.abs();
        if let Some(idx) = self.bucket_idx(d) {
            self.history.push_back((d, true));
            self.buckets[idx].n_fills += 1;
        }
    }

    pub fn estimate(&self) -> AkEstimate {
        if self.total_trades < self.min_trades {
            return AkEstimate {
                a: 0.0,
                k: 0.0,
                n_samples: self.total_trades,
                valid: false,
            };
        }

        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        for b in &self.buckets {
            if let Some(rate) = b.fill_rate() {
                if rate > 0.0 {
                    xs.push(b.midpoint());
                    ys.push(rate.ln());
                }
            }
        }

        if xs.len() < 3 {
            return AkEstimate {
                a: 0.0,
                k: 0.0,
                n_samples: self.total_trades,
                valid: false,
            };
        }

        let n = xs.len() as f64;
        let sx: f64 = xs.iter().sum();
        let sy: f64 = ys.iter().sum();
        let sxx: f64 = xs.iter().map(|x| x * x).sum();
        let sxy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
        let denom = n * sxx - sx * sx;

        if denom.abs() < 1e-12 {
            return AkEstimate {
                a: 0.0,
                k: 0.0,
                n_samples: self.total_trades,
                valid: false,
            };
        }

        let k_hat = -(n * sxy - sx * sy) / denom;
        let ln_a_hat = (sy - (-k_hat) * sx) / n;
        let a_hat = ln_a_hat.exp();

        if k_hat <= 0.0 || a_hat <= 0.0 {
            return AkEstimate {
                a: 0.0,
                k: 0.0,
                n_samples: self.total_trades,
                valid: false,
            };
        }

        AkEstimate {
            a: a_hat,
            k: k_hat,
            n_samples: self.total_trades,
            valid: true,
        }
    }

    pub fn reset(&mut self) {
        for b in &mut self.buckets {
            b.n_arrivals = 0;
            b.n_fills = 0;
        }
        self.history.clear();
        self.total_trades = 0;
    }
}
