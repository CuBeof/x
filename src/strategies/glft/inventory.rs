use chrono::{DateTime, Utc};
use crate::engine::event::OrderSide;

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub timestamp: DateTime<Utc>,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub fee: f64,
    pub realized_pnl: f64,
}

pub struct InventoryManager {
    pub position: f64,
    pub realized_pnl: f64,
    pub avg_entry_price: f64,
    pub max_inventory: f64,
    pub trades: Vec<TradeRecord>,
    equity_curve: Vec<f64>,
    peak_equity: f64,
    max_dd: f64,
}

impl InventoryManager {
    pub fn new(max_inventory: f64) -> Self {
        Self {
            position: 0.0,
            realized_pnl: 0.0,
            avg_entry_price: 0.0,
            max_inventory,
            trades: Vec::new(),
            equity_curve: vec![0.0],
            peak_equity: 0.0,
            max_dd: 0.0,
        }
    }

    pub fn on_fill(
        &mut self,
        price: f64,
        size: f64,
        side: &OrderSide,
        fee: f64,
        timestamp: DateTime<Utc>,
    ) -> f64 {
        let realized = match side {
            OrderSide::Buy => {
                let prev = self.position;
                if prev < 0.0 {
                    // closing short
                    let close_qty = size.min(-prev);
                    let pnl = close_qty * (self.avg_entry_price - price) - fee;
                    self.position += size;
                    if self.position > 0.0 {
                        self.avg_entry_price = price;
                    }
                    pnl
                } else {
                    let total = prev + size;
                    self.avg_entry_price = if total > 0.0 {
                        (prev * self.avg_entry_price + size * price) / total
                    } else {
                        price
                    };
                    self.position = total;
                    -fee
                }
            }
            OrderSide::Sell => {
                let prev = self.position;
                if prev > 0.0 {
                    // closing long
                    let close_qty = size.min(prev);
                    let pnl = close_qty * (price - self.avg_entry_price) - fee;
                    self.position -= size;
                    if self.position < 0.0 {
                        self.avg_entry_price = price;
                    }
                    pnl
                } else {
                    let total = prev - size;
                    self.avg_entry_price = if total < 0.0 {
                        ((-prev) * self.avg_entry_price + size * price) / (-total)
                    } else {
                        price
                    };
                    self.position = total;
                    -fee
                }
            }
        };

        self.realized_pnl += realized;

        let equity = self.realized_pnl;
        self.equity_curve.push(equity);
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }
        let dd = if self.peak_equity.abs() > 0.0 {
            (self.peak_equity - equity) / self.peak_equity.abs()
        } else {
            0.0
        };
        if dd > self.max_dd {
            self.max_dd = dd;
        }

        self.trades.push(TradeRecord {
            timestamp,
            side: side.clone(),
            price,
            size,
            fee,
            realized_pnl: realized,
        });

        realized
    }

    pub fn unrealized_pnl(&self, mid: f64) -> f64 {
        if self.position == 0.0 {
            return 0.0;
        }
        self.position * (mid - self.avg_entry_price)
    }

    pub fn total_pnl(&self, mid: f64) -> f64 {
        self.realized_pnl + self.unrealized_pnl(mid)
    }

    /// position / max_inventory ∈ [-1, 1]
    pub fn inventory_skew(&self) -> f64 {
        if self.max_inventory == 0.0 {
            return 0.0;
        }
        self.position / self.max_inventory
    }

    pub fn is_at_limit(&self) -> bool {
        self.position.abs() >= self.max_inventory
    }

    pub fn max_drawdown(&self) -> f64 {
        self.max_dd
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
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let std = variance.sqrt();
        if std == 0.0 {
            return 0.0;
        }
        // Annualize assuming each trade is ~1 minute apart
        mean / std * (n * 365.0 * 24.0 * 60.0 / n).sqrt()
    }
}
