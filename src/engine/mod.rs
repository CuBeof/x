pub mod event;
pub mod order_book;

use std::collections::HashMap;
use uuid::Uuid;
use crate::engine::event::{MarketEvent, OrderEvent, OrderSide};
use crate::engine::order_book::OrderBook;

#[derive(Debug, Clone)]
pub struct OpenOrder {
    pub order_id: Uuid,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub is_post_only: bool,
}

pub struct Engine {
    pub order_book: OrderBook,
    pub open_orders: HashMap<Uuid, OpenOrder>,
    pub maker_fee: f64,
}

impl Engine {
    pub fn new(maker_fee: f64) -> Self {
        Self {
            order_book: OrderBook::new(),
            open_orders: HashMap::new(),
            maker_fee,
        }
    }

    pub fn submit_order(&mut self, order: OpenOrder) {
        self.open_orders.insert(order.order_id, order);
    }

    pub fn cancel_order(&mut self, order_id: &Uuid) -> bool {
        self.open_orders.remove(order_id).is_some()
    }

    pub fn cancel_all(&mut self) {
        self.open_orders.clear();
    }

    /// Process a market event, attempt to fill resting orders, return fill events
    pub fn process_market_event(&mut self, event: &MarketEvent) -> Vec<OrderEvent> {
        let mut events = Vec::new();

        if let MarketEvent::QuoteUpdate(quote) = event {
            self.order_book.update_from_quote(quote);

            let order_ids: Vec<Uuid> = self.open_orders.keys().cloned().collect();
            for oid in order_ids {
                let order = self.open_orders[&oid].clone();
                let fill = match order.side {
                    OrderSide::Buy => self.order_book.try_fill_post_only_bid(
                        oid,
                        order.price,
                        order.size,
                        self.maker_fee,
                    ),
                    OrderSide::Sell => self.order_book.try_fill_post_only_ask(
                        oid,
                        order.price,
                        order.size,
                        self.maker_fee,
                    ),
                };
                if let Some(fill) = fill {
                    self.open_orders.remove(&oid);
                    events.push(OrderEvent::Filled(fill));
                }
            }
        }

        events
    }
}
