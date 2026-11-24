//! Identifiers: who sent an order, and which order it is.

/// Identifies one participant in the simulated market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraderId(pub u32);

/// The trading bot (your code).
pub const BOT: TraderId = TraderId(0);
/// The synthetic participant that keeps resting orders in the book.
pub const LIQUIDITY: TraderId = TraderId(1);
/// The synthetic participant that sends market orders to hit resting ones.
pub const TAKER: TraderId = TraderId(2);

/// Identifies one order: the trader that owns it plus a per-trader counter.
///
/// Because the trader is part of the id, two traders can never clash even if
/// both count their orders from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderId {
    /// Who placed the order.
    pub trader: TraderId,
    /// That trader's own sequence number for the order.
    pub seq: u64,
}

/// Exchange fees in parts per million (ppm) of the traded value.
///
/// A negative number is a rebate: the exchange pays you.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSchedule {
    /// Fee charged when your resting order is hit (you provided liquidity).
    pub maker_ppm: i64,
    /// Fee charged when your order hits a resting one (you took liquidity).
    pub taker_ppm: i64,
}
