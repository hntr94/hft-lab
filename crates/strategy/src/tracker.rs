//! Order tracker: allocates ids, follows acks/fills/cancels, holds the position.

use std::collections::BTreeMap;
use types::{
    BOT, BookSnapshot, Command, ExecEvent, Level, NewOrder, OrderId, OrderType, Price, Qty,
    RejectReason, Side,
};

/// One of the bot's orders that is still live (or in flight) on the exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrder {
    /// The order's id.
    pub id: OrderId,
    /// Buy or sell.
    pub side: Side,
    /// Limit price.
    pub price: Price,
    /// Quantity still unfilled.
    pub leaves: Qty,
    /// Whether the exchange has acknowledged the order.
    pub acked: bool,
    /// Whether a cancel has been sent and not yet answered.
    pub cancel_pending: bool,
}

/// Tracks the bot's own orders and position from the commands it sends and
/// the exchange events it receives.
#[derive(Debug)]
pub struct OrderTracker {
    next_seq: u64,
    orders: BTreeMap<OrderId, OpenOrder>,
    position: Qty,
}

impl Default for OrderTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderTracker {
    /// An empty tracker: no orders, flat position, ids start at seq 1.
    pub fn new() -> Self {
        Self {
            next_seq: 1,
            orders: BTreeMap::new(),
            position: Qty::ZERO,
        }
    }

    /// Allocate an id, record a new unacked limit order and return its command.
    pub fn place(&mut self, side: Side, price: Price, qty: Qty) -> Command {
        let id = OrderId {
            trader: BOT,
            seq: self.next_seq,
        };
        self.next_seq += 1;
        self.orders.insert(
            id,
            OpenOrder {
                id,
                side,
                price,
                leaves: qty,
                acked: false,
                cancel_pending: false,
            },
        );
        Command::New(NewOrder {
            id,
            side,
            qty,
            kind: OrderType::Limit { price },
        })
    }

    /// Start cancelling `id`. `None` if it is unknown or already being cancelled.
    pub fn cancel(&mut self, id: OrderId) -> Option<Command> {
        let order = self.orders.get_mut(&id)?;
        if order.cancel_pending {
            return None;
        }
        order.cancel_pending = true;
        Some(Command::Cancel(id))
    }

    /// Apply an event from the exchange. The exchange is authoritative:
    /// events about unknown ids never panic.
    pub fn on_exec(&mut self, ev: &ExecEvent) {
        match ev {
            ExecEvent::Ack { id } => {
                if let Some(order) = self.orders.get_mut(id) {
                    order.acked = true;
                }
            }
            ExecEvent::Fill {
                id,
                side,
                qty,
                leaves,
                ..
            } => {
                // The event itself is authoritative for the position, even if
                // the order is no longer tracked.
                self.position = Qty(self.position.0 + side.sign() * qty.0);
                if let Some(order) = self.orders.get_mut(id) {
                    order.leaves = *leaves;
                    if leaves.0 <= 0 {
                        self.orders.remove(id);
                    }
                }
            }
            ExecEvent::Cancelled { id, .. } => {
                self.orders.remove(id);
            }
            ExecEvent::Reject { reason, .. } if *reason == RejectReason::UnknownOrder => {}
            ExecEvent::Reject { id, .. } => {
                self.orders.remove(id);
            }
        }
    }

    /// All tracked orders, sorted by id.
    pub fn open_orders(&self) -> Vec<OpenOrder> {
        self.orders.values().cloned().collect()
    }

    /// Net position in lots (positive is long).
    pub fn position(&self) -> Qty {
        self.position
    }

    /// Sum of leaves on `side`, including cancel-pending orders.
    pub fn open_qty(&self, side: Side) -> Qty {
        Qty(self
            .orders
            .values()
            .filter(|o| o.side == side)
            .map(|o| o.leaves.0)
            .sum())
    }

    /// Remove the bot's own quotes from a book snapshot.
    ///
    /// This is how the bot avoids anchoring its mid on its own quotes.
    /// An order with a cancel in flight still rests on the exchange, so it is
    /// removed too; otherwise the bot would chase its own stale quote.
    ///
    /// The subtraction is exact under the sim's fixed one-way latency, because
    /// the execution reports for an order state always arrive before the book
    /// snapshot that reflects it. Under variable latency a snapshot can be
    /// older or newer than the tracker's view, so it is only approximate.
    pub fn exclude_own(&self, snap: &BookSnapshot) -> BookSnapshot {
        let strip = |levels: &[Level], side: Side| -> Vec<Level> {
            levels
                .iter()
                .filter_map(|level| {
                    let own: i64 = self
                        .orders
                        .values()
                        .filter(|o| o.acked && o.side == side && o.price == level.price)
                        .map(|o| o.leaves.0)
                        .sum();
                    let qty = Qty(level.qty.0 - own);
                    (qty.0 > 0).then_some(Level {
                        price: level.price,
                        qty,
                    })
                })
                .collect()
        };
        BookSnapshot {
            bids: strip(&snap.bids, Side::Buy),
            asks: strip(&snap.asks, Side::Sell),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Cash, Liquidity};

    fn fill(id: OrderId, side: Side, price: i64, qty: i64, leaves: i64) -> ExecEvent {
        ExecEvent::Fill {
            id,
            side,
            price: Price(price),
            qty: Qty(qty),
            liquidity: Liquidity::Maker,
            fee: Cash::ZERO,
            leaves: Qty(leaves),
        }
    }

    fn lv(price: i64, qty: i64) -> Level {
        Level {
            price: Price(price),
            qty: Qty(qty),
        }
    }

    #[test]
    fn place_allocates_sequential_ids() {
        let mut t = OrderTracker::new();
        let a = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        let b = t.place(Side::Sell, Price(101), Qty(5)).order_id();
        assert_eq!(
            a,
            OrderId {
                trader: BOT,
                seq: 1
            }
        );
        assert_eq!(
            b,
            OrderId {
                trader: BOT,
                seq: 2
            }
        );
        assert_eq!(t.open_orders().len(), 2);
        assert!(!t.open_orders()[0].acked);
    }

    #[test]
    fn ack_then_partial_then_full_fill() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        t.on_exec(&ExecEvent::Ack { id });
        assert!(t.open_orders()[0].acked);
        t.on_exec(&fill(id, Side::Buy, 100, 3, 2));
        assert_eq!(t.position(), Qty(3));
        assert_eq!(t.open_orders()[0].leaves, Qty(2));
        assert_eq!(t.open_qty(Side::Buy), Qty(2));
        t.on_exec(&fill(id, Side::Buy, 100, 2, 0));
        assert_eq!(t.position(), Qty(5));
        assert!(t.open_orders().is_empty());
    }

    #[test]
    fn cancel_sets_pending_once() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        assert_eq!(t.cancel(id), Some(Command::Cancel(id)));
        assert!(t.open_orders()[0].cancel_pending);
        assert_eq!(t.cancel(id), None);
        assert_eq!(
            t.cancel(OrderId {
                trader: BOT,
                seq: 99
            }),
            None
        );
    }

    #[test]
    fn unknown_order_reject_after_fill_is_noop() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Sell, Price(100), Qty(4)).order_id();
        t.on_exec(&ExecEvent::Ack { id });
        t.cancel(id);
        t.on_exec(&fill(id, Side::Sell, 100, 4, 0));
        t.on_exec(&ExecEvent::Reject {
            id,
            reason: RejectReason::UnknownOrder,
        });
        assert!(t.open_orders().is_empty());
        assert_eq!(t.position(), Qty(-4));
    }

    #[test]
    fn invalid_reject_removes_order() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(0)).order_id();
        t.on_exec(&ExecEvent::Reject {
            id,
            reason: RejectReason::InvalidQty,
        });
        assert!(t.open_orders().is_empty());
        assert_eq!(t.position(), Qty::ZERO);
    }

    #[test]
    fn cancelled_removes_and_unknown_events_are_noops() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        t.on_exec(&ExecEvent::Cancelled { id, leaves: Qty(5) });
        assert!(t.open_orders().is_empty());
        t.on_exec(&ExecEvent::Ack { id });
        t.on_exec(&ExecEvent::Cancelled { id, leaves: Qty(5) });
        assert!(t.open_orders().is_empty());
    }

    #[test]
    fn fill_for_unknown_id_still_moves_position() {
        let mut t = OrderTracker::new();
        let id = OrderId {
            trader: BOT,
            seq: 7,
        };
        t.on_exec(&fill(id, Side::Buy, 100, 3, 0));
        assert_eq!(t.position(), Qty(3));
    }

    #[test]
    fn exclude_own_removes_own_qty() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        let snap = BookSnapshot {
            bids: vec![lv(100, 8), lv(99, 2)],
            asks: vec![lv(101, 4)],
        };
        // Unacked: not subtracted.
        assert_eq!(t.exclude_own(&snap), snap);
        t.on_exec(&ExecEvent::Ack { id });
        let out = t.exclude_own(&snap);
        assert_eq!(out.bids, vec![lv(100, 3), lv(99, 2)]);
        assert_eq!(out.asks, snap.asks);
        // Cancel-pending: still resting on the exchange until the cancel
        // arrives, so it is still subtracted.
        t.cancel(id);
        assert_eq!(t.exclude_own(&snap).bids, vec![lv(100, 3), lv(99, 2)]);

        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(8)).order_id();
        t.on_exec(&ExecEvent::Ack { id });
        assert_eq!(t.exclude_own(&snap).bids, vec![lv(99, 2)]);
    }

    #[test]
    fn exclude_own_removes_own_ask() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Sell, Price(101), Qty(3)).order_id();
        t.on_exec(&ExecEvent::Ack { id });
        let snap = BookSnapshot {
            bids: vec![lv(100, 8)],
            asks: vec![lv(101, 4), lv(102, 2)],
        };
        // 4 - 3 = 1 left at 101; bids are untouched.
        let out = t.exclude_own(&snap);
        assert_eq!(out.asks, vec![lv(101, 1), lv(102, 2)]);
        assert_eq!(out.bids, snap.bids);
    }

    #[test]
    fn exclude_own_after_partial_fill_uses_leaves() {
        let mut t = OrderTracker::new();
        let id = t.place(Side::Buy, Price(100), Qty(5)).order_id();
        t.on_exec(&ExecEvent::Ack { id });
        t.on_exec(&fill(id, Side::Buy, 100, 2, 3));
        let snap = BookSnapshot {
            bids: vec![lv(100, 8)],
            asks: vec![],
        };
        // Only the 3 still resting are ours: 8 - 3 = 5.
        assert_eq!(t.exclude_own(&snap).bids, vec![lv(100, 5)]);
    }

    #[test]
    fn default_matches_new() {
        let t = OrderTracker::default();
        assert_eq!(t.position(), Qty::ZERO);
        assert!(t.open_orders().is_empty());
    }
}
