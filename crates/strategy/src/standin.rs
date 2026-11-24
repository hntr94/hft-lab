//! Stand-in quoter used by the app until `MarketMaker` is finished.
//! It quotes a fixed distance around the mid price. It ignores inventory
//! skew and never clamps against the opposite side of the book.

use crate::{Action, MarketMakerConfig, OpenOrder, Strategy, StrategyContext};
use types::{Price, Side};

#[derive(Debug)]
pub struct FixedSpreadQuoter {
    cfg: MarketMakerConfig,
}

impl FixedSpreadQuoter {
    pub fn new(cfg: MarketMakerConfig) -> FixedSpreadQuoter {
        FixedSpreadQuoter { cfg }
    }

    /// Actions that bring one side's live orders to `desired`.
    fn side_actions(&self, side: Side, desired: Price, open_orders: &[OpenOrder]) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut at_target = false;
        for order in open_orders
            .iter()
            .filter(|o| o.side == side && !o.cancel_pending)
        {
            if order.price == desired {
                at_target = true;
            } else {
                actions.push(Action::Cancel(order.id));
            }
        }
        if !at_target {
            actions.push(Action::Place {
                side,
                price: desired,
                qty: self.cfg.quote_qty,
            });
        }
        actions
    }
}

impl Strategy for FixedSpreadQuoter {
    fn on_update(&mut self, ctx: &StrategyContext) -> Vec<Action> {
        let Some(mid) = ctx.book.mid() else {
            return Vec::new();
        };
        let mut actions =
            self.side_actions(Side::Buy, mid - self.cfg.half_spread_ticks, ctx.open_orders);
        actions.extend(self.side_actions(
            Side::Sell,
            mid + self.cfg.half_spread_ticks,
            ctx.open_orders,
        ));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use book::{OrderBook, TopOfBook};
    use types::{BookSnapshot, Level, Qty, Timestamp};

    fn lv(p: i64, q: i64) -> Level {
        Level {
            price: Price(p),
            qty: Qty(q),
        }
    }

    fn cfg() -> MarketMakerConfig {
        MarketMakerConfig {
            half_spread_ticks: 5,
            quote_qty: Qty(100),
            skew_ticks: 3,
        }
    }

    fn run(bids: Vec<Level>, asks: Vec<Level>) -> Vec<Action> {
        let mut book = TopOfBook::new();
        book.apply_snapshot(&BookSnapshot { bids, asks });
        let ctx = StrategyContext {
            now: Timestamp(0),
            book: &book,
            position: Qty(0),
            open_orders: &[],
        };
        FixedSpreadQuoter::new(cfg()).on_update(&ctx)
    }

    #[test]
    fn stand_in_quotes_around_mid() {
        let actions = run(vec![lv(9_990, 10)], vec![lv(10_010, 10)]);
        assert_eq!(
            actions,
            vec![
                Action::Place {
                    side: Side::Buy,
                    price: Price(9_995),
                    qty: Qty(100)
                },
                Action::Place {
                    side: Side::Sell,
                    price: Price(10_005),
                    qty: Qty(100)
                },
            ]
        );
    }

    #[test]
    fn stand_in_quotes_nothing_without_two_sided_book() {
        assert_eq!(run(vec![lv(9_990, 10)], vec![]), vec![]);
    }
}
