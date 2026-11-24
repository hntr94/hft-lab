//! Order book interface, a simple stand-in, and the owner's exercise book.

use types::{BookSnapshot, Level, Price, Qty, Side};

pub mod mine;
pub mod standin;

pub use mine::MyBook;
pub use standin::TopOfBook;

/// A view of one side-by-side order book, rebuilt from snapshots.
pub trait OrderBook {
    /// Replace the whole book with this snapshot.
    fn apply_snapshot(&mut self, snap: &BookSnapshot);
    /// Highest bid level, if any.
    fn best_bid(&self) -> Option<Level>;
    /// Lowest ask level, if any.
    fn best_ask(&self) -> Option<Level>;
    /// Floor of (best bid + best ask) / 2. `None` unless both sides exist.
    fn mid(&self) -> Option<Price>;
    /// Best ask minus best bid, in ticks. `None` unless both sides exist.
    fn spread(&self) -> Option<i64>;
    /// Quantity resting at exactly `price` on `side`; `Qty(0)` if none.
    fn depth_at(&self, side: Side, price: Price) -> Qty;
}
