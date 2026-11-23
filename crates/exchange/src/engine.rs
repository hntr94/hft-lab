//! Price-time priority matching engine.
//!
//! Orders match at the resting order's price; within a price level the oldest
//! order goes first (FIFO). The engine knows nothing about time or latency: it
//! takes one command and returns everything that happened.
//!
//! Self-trade prevention ("expire taker"): if an incoming order would match a
//! resting order of the same trader, matching stops, the incoming order's
//! remaining quantity is cancelled (never rested) and the resting order is
//! left untouched.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use types::*;

/// An order waiting in the book.
#[derive(Debug, Clone)]
struct Resting {
    id: OrderId,
    leaves: Qty,
}

/// One side of the book: a FIFO queue of orders per price.
type BookSide = BTreeMap<Price, VecDeque<Resting>>;

/// Everything that happened while handling one command.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchOutput {
    /// Reports for the traders involved, in the order they happened.
    pub exec: Vec<(TraderId, ExecEvent)>,
    /// Public trades (only `MarketData::Trade`).
    pub trades: Vec<MarketData>,
    /// True when resting liquidity changed (rest, fill against resting, cancel).
    pub book_changed: bool,
}

/// A limit order book with price-time priority and maker/taker fees.
#[derive(Debug)]
pub struct MatchingEngine {
    fees: FeeSchedule,
    bids: BookSide,
    asks: BookSide,
    // Lookup only: never iterated, so output cannot depend on hash order.
    index: HashMap<OrderId, (Side, Price)>,
    seen: HashSet<OrderId>,
}

impl MatchingEngine {
    /// An empty book charging `fees`.
    pub fn new(fees: FeeSchedule) -> Self {
        MatchingEngine {
            fees,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            index: HashMap::new(),
            seen: HashSet::new(),
        }
    }

    /// Handle one command and report what happened.
    ///
    /// Self-trade prevention: when an incoming order would match a resting
    /// order of the same trader, matching stops and the incoming order's
    /// remaining quantity is reported `Cancelled` (after any fills already
    /// produced). It is not rested and the resting order is untouched.
    pub fn submit(&mut self, cmd: &Command) -> MatchOutput {
        match cmd {
            Command::New(order) => self.new_order(order),
            Command::Cancel(id) => self.cancel(*id),
        }
    }

    /// The highest resting buy price.
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    /// The lowest resting sell price.
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// The best `depth` levels on each side, with quantities summed per level.
    pub fn snapshot(&self, depth: usize) -> BookSnapshot {
        let level = |(price, queue): (&Price, &VecDeque<Resting>)| Level {
            price: *price,
            qty: Qty(queue.iter().map(|r| r.leaves.0).sum()),
        };
        BookSnapshot {
            bids: self.bids.iter().rev().take(depth).map(level).collect(),
            asks: self.asks.iter().take(depth).map(level).collect(),
        }
    }

    /// Ids of `trader`'s resting orders, sorted.
    pub fn resting_orders(&self, trader: TraderId) -> Vec<OrderId> {
        let mut ids: Vec<OrderId> = self
            .bids
            .values()
            .chain(self.asks.values())
            .flatten()
            .map(|r| r.id)
            .filter(|id| id.trader == trader)
            .collect();
        ids.sort();
        ids
    }

    fn reject(id: OrderId, reason: RejectReason) -> MatchOutput {
        MatchOutput {
            exec: vec![(id.trader, ExecEvent::Reject { id, reason })],
            ..MatchOutput::default()
        }
    }

    fn new_order(&mut self, order: &NewOrder) -> MatchOutput {
        let id = order.id;
        if order.qty <= Qty::ZERO {
            return Self::reject(id, RejectReason::InvalidQty);
        }
        if let OrderType::Limit { price } = order.kind
            && price.0 <= 0
        {
            return Self::reject(id, RejectReason::InvalidPrice);
        }
        if self.seen.contains(&id) {
            return Self::reject(id, RejectReason::DuplicateId);
        }
        self.seen.insert(id);

        let mut out = MatchOutput::default();
        out.exec.push((id.trader, ExecEvent::Ack { id }));

        let limit = match order.kind {
            OrderType::Limit { price } => Some(price),
            OrderType::Market => None,
        };
        let mut leaves = order.qty;
        let mut self_trade = false;
        while leaves > Qty::ZERO {
            let Some(price) = self.best_opposite(order.side) else {
                break;
            };
            let crosses = match (limit, order.side) {
                (None, _) => true,
                (Some(l), Side::Buy) => price <= l,
                (Some(l), Side::Sell) => price >= l,
            };
            if !crosses {
                break;
            }
            let front = self
                .book_side(order.side.opposite())
                .get(&price)
                .and_then(|q| q.front())
                .expect("best level is never empty");
            if front.id.trader == id.trader {
                self_trade = true;
                break;
            }
            leaves = self.match_at(order, price, leaves, &mut out);
        }

        if leaves > Qty::ZERO {
            match limit {
                Some(price) if !self_trade => {
                    self.book_side(order.side)
                        .entry(price)
                        .or_default()
                        .push_back(Resting { id, leaves });
                    self.index.insert(id, (order.side, price));
                    out.book_changed = true;
                }
                _ => out
                    .exec
                    .push((id.trader, ExecEvent::Cancelled { id, leaves })),
            }
        }
        out
    }

    /// Match the taker against the front order at `price`; returns the
    /// taker's remaining quantity.
    fn match_at(
        &mut self,
        taker: &NewOrder,
        price: Price,
        leaves: Qty,
        out: &mut MatchOutput,
    ) -> Qty {
        let fees = self.fees;
        let book = self.book_side(taker.side.opposite());
        let queue = book.get_mut(&price).expect("level exists");
        let maker = queue.front_mut().expect("level is never empty");
        let qty = Qty(leaves.0.min(maker.leaves.0));
        maker.leaves = maker.leaves - qty;
        let taker_leaves = leaves - qty;
        let value = notional(price, qty);
        let maker_id = maker.id;
        let maker_leaves = maker.leaves;
        if maker_leaves == Qty::ZERO {
            queue.pop_front();
            if queue.is_empty() {
                book.remove(&price);
            }
            self.index.remove(&maker_id);
        }
        out.exec.push((
            taker.id.trader,
            ExecEvent::Fill {
                id: taker.id,
                side: taker.side,
                price,
                qty,
                liquidity: Liquidity::Taker,
                fee: fee(value, fees.taker_ppm),
                leaves: taker_leaves,
            },
        ));
        out.exec.push((
            maker_id.trader,
            ExecEvent::Fill {
                id: maker_id,
                side: taker.side.opposite(),
                price,
                qty,
                liquidity: Liquidity::Maker,
                fee: fee(value, fees.maker_ppm),
                leaves: maker_leaves,
            },
        ));
        out.trades.push(MarketData::Trade {
            price,
            qty,
            aggressor: taker.side,
        });
        out.book_changed = true;
        taker_leaves
    }

    fn cancel(&mut self, id: OrderId) -> MatchOutput {
        let Some((side, price)) = self.index.remove(&id) else {
            return Self::reject(id, RejectReason::UnknownOrder);
        };
        let book = self.book_side(side);
        let queue = book.get_mut(&price).expect("indexed level exists");
        let pos = queue
            .iter()
            .position(|r| r.id == id)
            .expect("indexed order exists");
        let removed = queue.remove(pos).expect("position is valid");
        if queue.is_empty() {
            book.remove(&price);
        }
        MatchOutput {
            exec: vec![(
                id.trader,
                ExecEvent::Cancelled {
                    id,
                    leaves: removed.leaves,
                },
            )],
            trades: Vec::new(),
            book_changed: true,
        }
    }

    fn best_opposite(&self, side: Side) -> Option<Price> {
        match side {
            Side::Buy => self.best_ask(),
            Side::Sell => self.best_bid(),
        }
    }

    fn book_side(&mut self, side: Side) -> &mut BookSide {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TraderId = TraderId(7);
    const M: TraderId = TraderId(8);

    fn fees() -> FeeSchedule {
        FeeSchedule {
            maker_ppm: 200,
            taker_ppm: 500,
        }
    }
    fn oid(trader: TraderId, seq: u64) -> OrderId {
        OrderId { trader, seq }
    }
    fn lim(trader: TraderId, seq: u64, side: Side, price: i64, qty: i64) -> Command {
        Command::New(NewOrder {
            id: oid(trader, seq),
            side,
            qty: Qty(qty),
            kind: OrderType::Limit {
                price: Price(price),
            },
        })
    }
    fn mkt(trader: TraderId, seq: u64, side: Side, qty: i64) -> Command {
        Command::New(NewOrder {
            id: oid(trader, seq),
            side,
            qty: Qty(qty),
            kind: OrderType::Market,
        })
    }
    fn fills(out: &MatchOutput) -> Vec<(TraderId, ExecEvent)> {
        out.exec
            .iter()
            .filter(|(_, e)| matches!(e, ExecEvent::Fill { .. }))
            .cloned()
            .collect()
    }

    #[test]
    fn limit_rests_and_acks() {
        let mut e = MatchingEngine::new(fees());
        let out = e.submit(&lim(T, 1, Side::Buy, 100, 5));
        assert_eq!(out.exec, vec![(T, ExecEvent::Ack { id: oid(T, 1) })]);
        assert_eq!(e.best_bid(), Some(Price(100)));
        assert!(out.book_changed);
        assert!(out.trades.is_empty());
    }

    #[test]
    fn price_time_priority() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 101, 3)); // A
        e.submit(&lim(M, 2, Side::Sell, 101, 3)); // B
        e.submit(&lim(M, 3, Side::Sell, 100, 3)); // C
        let out = e.submit(&mkt(T, 1, Side::Buy, 7));
        let makers: Vec<(OrderId, i64, i64)> = out
            .exec
            .iter()
            .filter_map(|(_, ev)| match ev {
                ExecEvent::Fill {
                    id,
                    price,
                    qty,
                    liquidity: Liquidity::Maker,
                    ..
                } => Some((*id, price.0, qty.0)),
                _ => None,
            })
            .collect();
        assert_eq!(
            makers,
            vec![
                (oid(M, 3), 100, 3),
                (oid(M, 1), 101, 3),
                (oid(M, 2), 101, 1)
            ]
        );
        assert_eq!(e.resting_orders(M), vec![oid(M, 2)]);
        assert_eq!(
            e.snapshot(10).asks,
            vec![Level {
                price: Price(101),
                qty: Qty(2)
            }]
        );
        // taker fill precedes maker fill, ack first
        assert!(matches!(out.exec[0].1, ExecEvent::Ack { .. }));
        assert!(matches!(
            out.exec[1].1,
            ExecEvent::Fill {
                liquidity: Liquidity::Taker,
                ..
            }
        ));
        assert!(matches!(
            out.exec[2].1,
            ExecEvent::Fill {
                liquidity: Liquidity::Maker,
                ..
            }
        ));
    }

    #[test]
    fn fees_per_side() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 10_000_000, 100));
        let out = e.submit(&mkt(T, 1, Side::Buy, 100));
        let f = fills(&out);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].0, T);
        assert_eq!(
            f[0].1,
            ExecEvent::Fill {
                id: oid(T, 1),
                side: Side::Buy,
                price: Price(10_000_000),
                qty: Qty(100),
                liquidity: Liquidity::Taker,
                fee: Cash(500_000),
                leaves: Qty(0),
            }
        );
        assert_eq!(f[1].0, M);
        assert_eq!(
            f[1].1,
            ExecEvent::Fill {
                id: oid(M, 1),
                side: Side::Sell,
                price: Price(10_000_000),
                qty: Qty(100),
                liquidity: Liquidity::Maker,
                fee: Cash(200_000),
                leaves: Qty(0),
            }
        );
        assert_eq!(
            out.trades,
            vec![MarketData::Trade {
                price: Price(10_000_000),
                qty: Qty(100),
                aggressor: Side::Buy
            }]
        );
    }

    #[test]
    fn negative_maker_fee_is_rebate() {
        let mut e = MatchingEngine::new(FeeSchedule {
            maker_ppm: -100,
            taker_ppm: 500,
        });
        e.submit(&lim(M, 1, Side::Sell, 10_000_000, 100));
        let out = e.submit(&mkt(T, 1, Side::Buy, 100));
        match &fills(&out)[1].1 {
            ExecEvent::Fill { fee, liquidity, .. } => {
                assert_eq!(*fee, Cash(-100_000));
                assert_eq!(*liquidity, Liquidity::Maker);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn crossing_limit_partially_fills_then_rests() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 101, 2));
        let out = e.submit(&lim(T, 1, Side::Buy, 102, 5));
        assert!(matches!(out.exec[0].1, ExecEvent::Ack { .. }));
        match &fills(&out)[0].1 {
            ExecEvent::Fill {
                price, qty, leaves, ..
            } => {
                assert_eq!((*price, *qty, *leaves), (Price(101), Qty(2), Qty(3)));
            }
            _ => unreachable!(),
        }
        assert_eq!(e.best_bid(), Some(Price(102)));
        assert_eq!(
            e.snapshot(10).bids,
            vec![Level {
                price: Price(102),
                qty: Qty(3)
            }]
        );
        assert_eq!(e.best_ask(), None);
        assert!(out.book_changed);
    }

    #[test]
    fn self_trade_prevented_limit() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(T, 1, Side::Sell, 101, 5));
        let out = e.submit(&lim(T, 2, Side::Buy, 102, 3));
        assert_eq!(
            out.exec,
            vec![
                (T, ExecEvent::Ack { id: oid(T, 2) }),
                (
                    T,
                    ExecEvent::Cancelled {
                        id: oid(T, 2),
                        leaves: Qty(3)
                    }
                ),
            ]
        );
        assert!(fills(&out).is_empty());
        assert!(out.trades.is_empty());
        assert_eq!(e.best_bid(), None);
        assert_eq!(
            e.snapshot(5).asks,
            vec![Level {
                price: Price(101),
                qty: Qty(5)
            }]
        );
        assert_eq!(e.resting_orders(T), vec![oid(T, 1)]);
    }

    #[test]
    fn self_trade_after_partial_fill() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 100, 2));
        e.submit(&lim(T, 1, Side::Sell, 101, 5));
        let out = e.submit(&mkt(T, 2, Side::Buy, 6));
        // 2 fill against M at 100, then the next level is T's own: expire the
        // remaining 6 - 2 = 4.
        let f = fills(&out);
        assert_eq!(f.len(), 2);
        assert!(matches!(
            f[0],
            (
                T,
                ExecEvent::Fill {
                    price: Price(100),
                    qty: Qty(2),
                    liquidity: Liquidity::Taker,
                    ..
                }
            )
        ));
        assert_eq!(
            out.exec.last().unwrap(),
            &(
                T,
                ExecEvent::Cancelled {
                    id: oid(T, 2),
                    leaves: Qty(4)
                }
            )
        );
        assert_eq!(e.resting_orders(T), vec![oid(T, 1)]);
        assert_eq!(e.best_ask(), Some(Price(101)));
    }

    #[test]
    fn market_order_remainder_cancelled() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 101, 3));
        e.submit(&lim(M, 2, Side::Sell, 102, 1));
        let out = e.submit(&mkt(T, 1, Side::Buy, 10));
        let total: i64 = out
            .exec
            .iter()
            .filter_map(|(t, ev)| match ev {
                ExecEvent::Fill { qty, .. } if *t == T => Some(qty.0),
                _ => None,
            })
            .sum();
        assert_eq!(total, 4);
        assert_eq!(
            out.exec.last().unwrap(),
            &(
                T,
                ExecEvent::Cancelled {
                    id: oid(T, 1),
                    leaves: Qty(6)
                }
            )
        );
        assert_eq!(e.best_bid(), None);
        assert!(e.resting_orders(T).is_empty());
    }

    #[test]
    fn market_order_on_empty_book_is_acked_and_cancelled() {
        let mut e = MatchingEngine::new(fees());
        let out = e.submit(&mkt(T, 1, Side::Sell, 4));
        assert_eq!(
            out.exec,
            vec![
                (T, ExecEvent::Ack { id: oid(T, 1) }),
                (
                    T,
                    ExecEvent::Cancelled {
                        id: oid(T, 1),
                        leaves: Qty(4)
                    }
                ),
            ]
        );
        assert!(!out.book_changed);
    }

    #[test]
    fn market_order_sweeps_side() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 101, 3));
        e.submit(&lim(M, 2, Side::Sell, 102, 3));
        e.submit(&mkt(T, 1, Side::Buy, 6));
        assert_eq!(e.best_ask(), None);
        assert!(e.snapshot(10).asks.is_empty());
    }

    #[test]
    fn cancel_resting() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(T, 1, Side::Buy, 100, 5));
        let out = e.submit(&Command::Cancel(oid(T, 1)));
        assert_eq!(
            out.exec,
            vec![(
                T,
                ExecEvent::Cancelled {
                    id: oid(T, 1),
                    leaves: Qty(5)
                }
            )]
        );
        assert!(out.book_changed);
        assert_eq!(e.best_bid(), None);
    }

    #[test]
    fn cancel_after_fill_is_rejected_unknown() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(M, 1, Side::Sell, 101, 3));
        e.submit(&mkt(T, 1, Side::Buy, 3));
        let out = e.submit(&Command::Cancel(oid(M, 1)));
        assert_eq!(
            out.exec,
            vec![(
                M,
                ExecEvent::Reject {
                    id: oid(M, 1),
                    reason: RejectReason::UnknownOrder
                }
            )]
        );
        assert!(!out.book_changed);
        // twice-cancelled and never-seen ids too
        e.submit(&lim(T, 2, Side::Buy, 100, 1));
        e.submit(&Command::Cancel(oid(T, 2)));
        let again = e.submit(&Command::Cancel(oid(T, 2)));
        assert!(matches!(
            again.exec[0].1,
            ExecEvent::Reject {
                reason: RejectReason::UnknownOrder,
                ..
            }
        ));
        let never = e.submit(&Command::Cancel(oid(T, 99)));
        assert_eq!(never.exec[0].0, T);
    }

    #[test]
    fn validation_rejects() {
        let mut e = MatchingEngine::new(fees());
        let out = e.submit(&lim(T, 1, Side::Buy, 100, 0));
        assert_eq!(
            out.exec,
            vec![(
                T,
                ExecEvent::Reject {
                    id: oid(T, 1),
                    reason: RejectReason::InvalidQty
                }
            )]
        );
        assert!(!out.book_changed);
        let out = e.submit(&lim(T, 2, Side::Buy, 0, 5));
        assert!(matches!(
            out.exec[0].1,
            ExecEvent::Reject {
                reason: RejectReason::InvalidPrice,
                ..
            }
        ));
        // rejected ids are not recorded as seen
        let out = e.submit(&lim(T, 1, Side::Buy, 100, 5));
        assert!(matches!(out.exec[0].1, ExecEvent::Ack { .. }));
        // reuse of accepted id
        let out = e.submit(&lim(T, 1, Side::Buy, 100, 5));
        assert!(matches!(
            out.exec[0].1,
            ExecEvent::Reject {
                reason: RejectReason::DuplicateId,
                ..
            }
        ));
        // reuse after fill/cancel still duplicate
        e.submit(&Command::Cancel(oid(T, 1)));
        let out = e.submit(&lim(T, 1, Side::Buy, 100, 5));
        assert!(matches!(
            out.exec[0].1,
            ExecEvent::Reject {
                reason: RejectReason::DuplicateId,
                ..
            }
        ));
    }

    #[test]
    fn snapshot_aggregates_levels_and_orders() {
        let mut e = MatchingEngine::new(fees());
        e.submit(&lim(T, 1, Side::Buy, 100, 2));
        e.submit(&lim(T, 2, Side::Buy, 100, 3));
        e.submit(&lim(T, 3, Side::Buy, 99, 4));
        e.submit(&lim(T, 4, Side::Buy, 98, 1));
        let s = e.snapshot(10);
        assert_eq!(
            s.bids,
            vec![
                Level {
                    price: Price(100),
                    qty: Qty(5)
                },
                Level {
                    price: Price(99),
                    qty: Qty(4)
                },
                Level {
                    price: Price(98),
                    qty: Qty(1)
                },
            ]
        );
        assert_eq!(e.snapshot(2).bids.len(), 2);
        assert_eq!(
            e.resting_orders(T),
            vec![oid(T, 1), oid(T, 2), oid(T, 3), oid(T, 4)]
        );
    }
}
