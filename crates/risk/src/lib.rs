//! Pre-trade risk interface, a simple stand-in, and the owner's exercise risk checker.

use types::{Cash, Price, Qty, Side};

pub mod mine;
pub mod standin;

pub use mine::MyRisk;
pub use standin::MaxPositionOnly;

/// The limits a risk checker enforces.
#[derive(Clone, Debug)]
pub struct RiskLimits {
    /// Largest position allowed, long or short, in lots.
    pub max_position: Qty,
    /// Largest size of a single order, in lots.
    pub max_order_qty: Qty,
    /// How far an order price may be from the mid, in ticks.
    pub price_band_ticks: i64,
    /// How many orders may be open at once.
    pub max_open_orders: usize,
    /// Halt when total PnL falls below minus this amount.
    pub max_loss: Cash,
}

/// What the risk checker may look at when judging an order.
#[derive(Clone, Debug)]
pub struct RiskContext {
    /// Our current position in lots (positive is long).
    pub position: Qty,
    /// Total size of our open buy orders.
    pub open_buy_qty: Qty,
    /// Total size of our open sell orders.
    pub open_sell_qty: Qty,
    /// How many orders we have open.
    pub open_orders: usize,
    /// The current mid price, if the book has both sides.
    pub mid: Option<Price>,
}

/// An order as the risk crate sees it. The app converts from the strategy's `Action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// A new limit order.
    Place { side: Side, price: Price, qty: Qty },
    /// A request to cancel an order.
    Cancel,
}

/// Why an order was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskViolation {
    /// Trading is halted after a loss.
    Halted,
    /// The order is bigger than `max_order_qty`.
    MaxOrderQty,
    /// Too many orders are already open.
    MaxOpenOrders,
    /// There is no mid price to compare against.
    NoReferencePrice,
    /// The price is too far from the mid.
    PriceBand,
    /// The order could take the position past `max_position`.
    MaxPosition,
}

/// A pre-trade risk checker.
pub trait RiskCheck {
    /// Judge one order. `Ok(())` means it may be sent.
    fn check(&mut self, order: &Order, ctx: &RiskContext) -> Result<(), RiskViolation>;
    /// Tell the checker the current total PnL (it may halt trading).
    fn on_pnl(&mut self, total_pnl: Cash);
    /// True once trading has been halted.
    fn is_halted(&self) -> bool;
}
