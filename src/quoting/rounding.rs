//! Price/size rounding helpers.

pub fn round_price_to_tick_down(price: f64, tick: f64) -> f64 {
    if tick <= 0.0 { return price; }
    (price / tick).floor() * tick
}

pub fn round_price_to_tick_up(price: f64, tick: f64) -> f64 {
    if tick <= 0.0 { return price; }
    (price / tick).ceil() * tick
}

pub fn round_size_to_lot(size: f64, lot: f64) -> f64 {
    if lot <= 0.0 { return size; }
    (size / lot).floor() * lot
}

pub fn notional_to_base(notional: f64, mid: f64, lot: f64) -> f64 {
    if mid <= 0.0 { return 0.0; }
    round_size_to_lot(notional / mid, lot)
}

pub fn abs_to_bps(abs_val: f64, mid: f64) -> f64 {
    if mid == 0.0 { 0.0 } else { abs_val / mid * 1e4 }
}
