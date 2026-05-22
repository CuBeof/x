use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub timestamp: DateTime<Utc>,
}

impl Quote {
    pub fn mid_price(&self) -> f64 {
        (self.bid_price + self.ask_price) / 2.0
    }

    pub fn spread(&self) -> f64 {
        self.ask_price - self.bid_price
    }

    pub fn spread_bps(&self) -> f64 {
        self.spread() / self.mid_price() * 10_000.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub price: f64,
    pub size: f64,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum MarketEvent {
    QuoteUpdate(Quote),
    TradeUpdate(Trade),
    BarUpdate(Bar),
}

impl MarketEvent {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            MarketEvent::QuoteUpdate(q) => q.timestamp,
            MarketEvent::TradeUpdate(t) => t.timestamp,
            MarketEvent::BarUpdate(b) => b.timestamp,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderFill {
    pub order_id: Uuid,
    pub price: f64,
    pub size: f64,
    pub side: OrderSide,
    pub fee: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum OrderEvent {
    Accepted(Uuid),
    Filled(OrderFill),
    Cancelled(Uuid),
    Rejected { order_id: Uuid, reason: String },
}
