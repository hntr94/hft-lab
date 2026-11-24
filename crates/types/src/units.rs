//! Integer units for prices, quantities, cash and time.
//!
//! # Why newtypes?
//! A `Price` and a `Qty` are both "just a number", and it would be easy to add
//! one to the other by mistake. A *newtype* wraps the number in a one-field
//! struct (`struct Price(pub i64)`), so the compiler treats them as different
//! types and rejects such mix-ups. The wrapper costs nothing at run time.
//!
//! # Why integers?
//! Floats cannot store most decimals exactly (0.1 + 0.2 != 0.3), so tiny
//! errors would creep into PnL and make runs hard to compare. Everything here
//! is a whole number of a small unit instead:
//!
//! * `Price` counts ticks of 0.01 USDT.
//! * `Qty` counts lots of 0.00001 BTC.
//! * `Cash` counts units of 1e-7 USDT, chosen so that
//!   `price_ticks * qty_lots` is exactly a `Cash` value (0.01 * 0.00001 = 1e-7).
//! * `Timestamp` counts nanoseconds since the simulation started.

use std::ops::{Add, Neg, Sub};

/// Size of one price step in USDT.
pub const TICK_USDT: f64 = 0.01;
/// Size of one quantity step in BTC.
pub const LOT_BTC: f64 = 0.00001;
/// How many `Cash` units make one USDT.
pub const CASH_UNITS_PER_USDT: i64 = 10_000_000;

/// A price, counted in ticks (1 tick = 0.01 USDT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(pub i64);

/// A quantity, counted in lots (1 lot = 0.00001 BTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Qty(pub i64);

/// An amount of money, counted in units of 1e-7 USDT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cash(pub i64);

/// A point in simulated time: nanoseconds since the simulation started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub u64);

impl Qty {
    /// No quantity at all.
    pub const ZERO: Qty = Qty(0);
}

impl Cash {
    /// No money.
    pub const ZERO: Cash = Cash(0);
}

/// Moving a price up by a number of ticks.
impl Add<i64> for Price {
    type Output = Price;
    fn add(self, ticks: i64) -> Price {
        Price(self.0 + ticks)
    }
}

/// Moving a price down by a number of ticks.
impl Sub<i64> for Price {
    type Output = Price;
    fn sub(self, ticks: i64) -> Price {
        Price(self.0 - ticks)
    }
}

/// The distance between two prices, in ticks (positive if `self` is higher).
impl Sub<Price> for Price {
    type Output = i64;
    fn sub(self, other: Price) -> i64 {
        self.0 - other.0
    }
}

impl Add for Qty {
    type Output = Qty;
    fn add(self, other: Qty) -> Qty {
        Qty(self.0 + other.0)
    }
}

impl Sub for Qty {
    type Output = Qty;
    fn sub(self, other: Qty) -> Qty {
        Qty(self.0 - other.0)
    }
}

impl Neg for Qty {
    type Output = Qty;
    fn neg(self) -> Qty {
        Qty(-self.0)
    }
}

impl Add for Cash {
    type Output = Cash;
    fn add(self, other: Cash) -> Cash {
        Cash(self.0 + other.0)
    }
}

impl Sub for Cash {
    type Output = Cash;
    fn sub(self, other: Cash) -> Cash {
        Cash(self.0 - other.0)
    }
}

impl Neg for Cash {
    type Output = Cash;
    fn neg(self) -> Cash {
        Cash(-self.0)
    }
}

impl Timestamp {
    /// This moment plus `ns` nanoseconds.
    pub fn plus_nanos(self, ns: u64) -> Timestamp {
        Timestamp(self.0 + ns)
    }
}

/// Which side of the book an order is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// Buying.
    Buy,
    /// Selling.
    Sell,
}

impl Side {
    /// The other side: the side you trade against.
    pub fn opposite(self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    /// +1 for a buy (position goes up), -1 for a sell (position goes down).
    pub fn sign(self) -> i64 {
        match self {
            Side::Buy => 1,
            Side::Sell => -1,
        }
    }
}

/// The value of a trade: `price * qty`, exactly, in `Cash` units.
///
/// Example: 100,000 USDT * 0.001 BTC = 100 USDT.
pub fn notional(price: Price, qty: Qty) -> Cash {
    Cash(price.0 * qty.0)
}

/// The fee on a trade worth `notional`, at `ppm` parts per million.
///
/// The division truncates toward zero, so a fee smaller than one `Cash` unit
/// is zero. A negative `ppm` gives a negative fee, which is a rebate.
pub fn fee(notional: Cash, ppm: i64) -> Cash {
    Cash(notional.0 * ppm / 1_000_000)
}

/// Format `negative`, `whole` and a fractional part as `[-]whole.frac`.
fn decimal(negative: bool, abs: u64, per_whole: u64, frac_digits: usize) -> String {
    let sign = if negative { "-" } else { "" };
    format!(
        "{sign}{}.{:0width$}",
        abs / per_whole,
        abs % per_whole,
        width = frac_digits
    )
}

/// A price as USDT with 2 decimals, e.g. `Price(10_000_012)` is `"100000.12"`.
pub fn fmt_price(price: Price) -> String {
    decimal(price.0 < 0, price.0.unsigned_abs(), 100, 2)
}

/// A quantity as BTC with 5 decimals, e.g. `Qty(1_234)` is `"0.01234"`.
pub fn fmt_qty(qty: Qty) -> String {
    decimal(qty.0 < 0, qty.0.unsigned_abs(), 100_000, 5)
}

/// Cash as USDT with 2 decimals and an explicit sign, e.g. `"+12.34"`.
///
/// Amounts smaller than one cent are truncated toward zero (the same rule as
/// [`fee`]), so `Cash(-4)` prints as `"+0.00"`: zero is always shown with `+`,
/// never as `-0.00`.
pub fn fmt_cash(cash: Cash) -> String {
    let cents = cash.0.unsigned_abs() / (CASH_UNITS_PER_USDT as u64 / 100);
    let sign = if cash.0 < 0 && cents > 0 { "-" } else { "+" };
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notional_is_exact_cash() {
        assert_eq!(notional(Price(10_000_000), Qty(100)), Cash(1_000_000_000));
    }
    #[test]
    fn maker_fee_default() {
        assert_eq!(fee(Cash(1_000_000_000), 200), Cash(200_000));
    }
    #[test]
    fn rebate_is_negative() {
        assert_eq!(fee(Cash(1_000_000_000), -100), Cash(-100_000));
    }
    #[test]
    fn fee_truncates_toward_zero() {
        assert_eq!(fee(Cash(4_999), 200), Cash(0));
    }
    #[test]
    fn display() {
        assert_eq!(fmt_price(Price(10_000_012)), "100000.12");
        assert_eq!(fmt_qty(Qty(1_234)), "0.01234");
        assert_eq!(fmt_cash(Cash(123_400_000)), "+12.34");
        assert_eq!(fmt_cash(Cash(-5_600_000)), "-0.56");
    }
    #[test]
    fn side_helpers() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.sign(), -1);
    }
    #[test]
    fn sub_cent_cash_truncates_to_plus_zero() {
        assert_eq!(fmt_cash(Cash::ZERO), "+0.00");
        assert_eq!(fmt_cash(Cash(-4)), "+0.00");
        assert_eq!(fmt_cash(Cash(99_999)), "+0.00");
    }
    #[test]
    fn price_arithmetic() {
        assert_eq!(Price(100) + 5, Price(105));
        assert_eq!(Price(100) - 5, Price(95));
        assert_eq!(Price(100) - Price(97), 3);
    }
}
