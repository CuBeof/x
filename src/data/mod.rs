use std::path::Path;
use crate::engine::event::{Bar, Quote, MarketEvent};
use chrono::{DateTime, Utc, TimeZone};
use serde::Deserialize;
use anyhow::Context;

// CSV record types for deserialization
#[derive(Debug, Deserialize)]
struct BarRecord {
    timestamp: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug, Deserialize)]
struct QuoteRecord {
    timestamp: String,
    bid_price: f64,
    ask_price: f64,
    bid_size: f64,
    ask_size: f64,
}

fn parse_timestamp(s: &str) -> anyhow::Result<DateTime<Utc>> {
    // Try unix ms first, then ISO8601
    if let Ok(ms) = s.parse::<i64>() {
        Ok(Utc
            .timestamp_millis_opt(ms)
            .single()
            .context("invalid timestamp millis")?)
    } else {
        Ok(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc))
    }
}

pub fn load_bars(path: &Path) -> anyhow::Result<Vec<Bar>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut bars = Vec::new();
    for result in reader.deserialize() {
        let record: BarRecord = result?;
        bars.push(Bar {
            open: record.open,
            high: record.high,
            low: record.low,
            close: record.close,
            volume: record.volume,
            timestamp: parse_timestamp(&record.timestamp)?,
        });
    }
    bars.sort_by_key(|b| b.timestamp);
    Ok(bars)
}

pub fn load_quotes(path: &Path) -> anyhow::Result<Vec<Quote>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut quotes = Vec::new();
    for result in reader.deserialize() {
        let record: QuoteRecord = result?;
        quotes.push(Quote {
            bid_price: record.bid_price,
            ask_price: record.ask_price,
            bid_size: record.bid_size,
            ask_size: record.ask_size,
            timestamp: parse_timestamp(&record.timestamp)?,
        });
    }
    quotes.sort_by_key(|q| q.timestamp);
    Ok(quotes)
}

/// Chronological event feed that supports len() before consuming as iterator
pub struct DataFeed {
    events: std::collections::VecDeque<MarketEvent>,
    total: usize,
}

impl DataFeed {
    pub fn from_quotes(quotes: Vec<Quote>) -> Self {
        let mut events: Vec<MarketEvent> =
            quotes.into_iter().map(MarketEvent::QuoteUpdate).collect();
        events.sort_by_key(|e| e.timestamp());
        let total = events.len();
        Self {
            events: events.into(),
            total,
        }
    }

    pub fn from_bars(bars: Vec<Bar>) -> Self {
        let mut events: Vec<MarketEvent> =
            bars.into_iter().map(MarketEvent::BarUpdate).collect();
        events.sort_by_key(|e| e.timestamp());
        let total = events.len();
        Self {
            events: events.into(),
            total,
        }
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

impl Iterator for DataFeed {
    type Item = MarketEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.events.pop_front()
    }
}
