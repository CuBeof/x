//! Pre-quote sanity guards.

use crate::{
    config::RiskConfig,
    quoting::{quote::QuoteLadder, rounding::abs_to_bps},
    types::MarketSnapshot,
};

pub fn check_stale_data(
    snap: &MarketSnapshot,
    now_ns: u64,
    stale_ms: u64,
) -> (bool, &'static str) {
    let age_ms = now_ns.saturating_sub(snap.timestamp_ns) / 1_000_000;
    if age_ms > stale_ms {
        (false, "stale data")
    } else {
        (true, "")
    }
}

pub fn check_book_spread(
    bid: f64,
    ask: f64,
    mid: f64,
    max_spread_bps: f64,
) -> (bool, &'static str) {
    if bid <= 0.0 || ask <= 0.0 || mid <= 0.0 {
        return (false, "invalid bid/ask/mid");
    }
    if ask <= bid {
        return (false, "crossed book");
    }
    let spread_bps = abs_to_bps(ask - bid, mid);
    if spread_bps > max_spread_bps {
        (false, "book spread too wide")
    } else {
        (true, "")
    }
}

pub fn check_self_trade(ladder: &QuoteLadder) -> (bool, &'static str) {
    match (ladder.bids.first(), ladder.asks.first()) {
        (Some(b), Some(a)) if b.price >= a.price => (false, "self-trade risk"),
        _ => (true, ""),
    }
}

pub fn check_order_count(ladder: &QuoteLadder, max_per_side: usize) -> (bool, &'static str) {
    if ladder.bids.len() > max_per_side || ladder.asks.len() > max_per_side {
        (false, "too many orders per side")
    } else {
        (true, "")
    }
}

pub fn run_all_guards(
    snap: &MarketSnapshot,
    ladder: &QuoteLadder,
    bid: f64,
    ask: f64,
    mid: f64,
    now_ns: u64,
    cfg: &RiskConfig,
) -> (bool, &'static str) {
    let checks = [
        check_stale_data(snap, now_ns, cfg.stale_data_ms),
        check_book_spread(bid, ask, mid, cfg.max_book_spread_bps),
        check_self_trade(ladder),
        check_order_count(ladder, cfg.max_orders_per_side),
    ];
    for (passed, reason) in checks {
        if !passed {
            return (false, reason);
        }
    }
    (true, "")
}
