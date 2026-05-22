use std::collections::BTreeMap;
use ordered_float::OrderedFloat;
use crate::engine::event::{Quote, OrderSide, OrderFill};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct OrderBook {
    // bids: price descending (best bid = highest)
    bids: BTreeMap<std::cmp::Reverse<OrderedFloat<f64>>, f64>,
    // asks: price ascending (best ask = lowest)
    asks: BTreeMap<OrderedFloat<f64>, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_quote(&mut self, quote: &Quote) {
        self.bids.clear();
        self.asks.clear();
        self.bids.insert(
            std::cmp::Reverse(OrderedFloat(quote.bid_price)),
            quote.bid_size,
        );
        self.asks
            .insert(OrderedFloat(quote.ask_price), quote.ask_size);
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next().map(|r| r.0 .0)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|r| r.0)
    }

    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }

    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Post-only bid: fills only if price < best_ask (truly passive)
    pub fn try_fill_post_only_bid(
        &self,
        order_id: Uuid,
        price: f64,
        size: f64,
        fee_rate: f64,
    ) -> Option<OrderFill> {
        match self.best_ask() {
            Some(best_ask) if price < best_ask => Some(OrderFill {
                order_id,
                price,
                size,
                side: OrderSide::Buy,
                fee: price * size * fee_rate,
                timestamp: Utc::now(),
            }),
            None => None, // no book data yet
            _ => None,    // would cross => rejected as post-only
        }
    }

    /// Post-only ask: fills only if price > best_bid
    pub fn try_fill_post_only_ask(
        &self,
        order_id: Uuid,
        price: f64,
        size: f64,
        fee_rate: f64,
    ) -> Option<OrderFill> {
        match self.best_bid() {
            Some(best_bid) if price > best_bid => Some(OrderFill {
                order_id,
                price,
                size,
                side: OrderSide::Sell,
                fee: price * size * fee_rate,
                timestamp: Utc::now(),
            }),
            None => None,
            _ => None,
        }
    }
}
