/// Tracks trade history for custom Sharpe ratio and max drawdown calculations.
/// Net position and PnL are tracked by the nautilus Portfolio.
#[derive(Debug, Default)]
pub struct TradeTracker {
    equity_curve: Vec<f64>,
    peak_equity: f64,
    max_drawdown: f64,
}

impl TradeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_fill(&mut self, realized_pnl: f64) {
        let equity = self.equity_curve.last().copied().unwrap_or(0.0) + realized_pnl;
        self.equity_curve.push(equity);
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }
        let dd = if self.peak_equity > 0.0 {
            (self.peak_equity - equity) / self.peak_equity
        } else {
            0.0
        };
        if dd > self.max_drawdown {
            self.max_drawdown = dd;
        }
    }

    pub fn sharpe_ratio(&self) -> f64 {
        if self.equity_curve.len() < 10 {
            return 0.0;
        }
        let returns: Vec<f64> = self
            .equity_curve
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect();
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let std = {
            let var = returns
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / (n - 1.0).max(1.0);
            var.sqrt()
        };
        if std == 0.0 {
            0.0
        } else {
            mean / std * (n * 365.0).sqrt()
        }
    }

    pub fn max_drawdown(&self) -> f64 {
        self.max_drawdown
    }

    pub fn fill_count(&self) -> usize {
        self.equity_curve.len()
    }
}
