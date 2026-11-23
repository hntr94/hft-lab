//! Strategy interface, a simple stand-in quoter, and the owner's exercise market maker.

use book::OrderBook;
use types::{OrderId, Price, Qty, Side, Timestamp};

pub mod mine;
pub mod standin;
pub mod tracker;

pub use mine::MarketMaker;
pub use standin::FixedSpreadQuoter;
pub use tracker::{OpenOrder, OrderTracker};

/// Something a strategy asks the bot to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a new limit order.
    Place { side: Side, price: Price, qty: Qty },
    /// Ask the exchange to cancel one of our orders.
    Cancel(OrderId),
}

/// Everything a strategy may look at when it is asked what to do.
pub struct StrategyContext<'a> {
    /// The current simulated time.
    pub now: Timestamp,
    /// The order book. It does not include our own orders.
    pub book: &'a dyn OrderBook,
    /// Our current position in lots (positive is long).
    pub position: Qty,
    /// Our orders that are live or in flight.
    pub open_orders: &'a [OpenOrder],
}

/// A trading strategy.
pub trait Strategy {
    /// Called after every bot event. Returns the actions to take now.
    fn on_update(&mut self, ctx: &StrategyContext) -> Vec<Action>;
}

/// Settings for a market-making strategy.
#[derive(Debug, Clone)]
pub struct MarketMakerConfig {
    /// Distance in ticks from the reservation price to each quote.
    pub half_spread_ticks: i64,
    /// Size of each quote, in lots.
    pub quote_qty: Qty,
    /// Ticks to shift the quotes for each `quote_qty` of position held.
    pub skew_ticks: i64,
}
