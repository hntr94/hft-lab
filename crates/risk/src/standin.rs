//! A simple stand-in risk checker. It only enforces the position limit and never halts.

use crate::{Order, RiskCheck, RiskContext, RiskLimits, RiskViolation};
use types::{Cash, Side};

/// Applies only the max-position rule (rule 7). Cancels always pass.
#[derive(Debug)]
pub struct MaxPositionOnly {
    limits: RiskLimits,
}

impl MaxPositionOnly {
    pub fn new(limits: RiskLimits) -> MaxPositionOnly {
        MaxPositionOnly { limits }
    }
}

impl RiskCheck for MaxPositionOnly {
    fn check(&mut self, order: &Order, ctx: &RiskContext) -> Result<(), RiskViolation> {
        let (side, qty) = match order {
            Order::Cancel => return Ok(()),
            Order::Place { side, qty, .. } => (*side, *qty),
        };
        let max = self.limits.max_position.0;
        let too_big = match side {
            Side::Buy => ctx.position.0 + ctx.open_buy_qty.0 + qty.0 > max,
            Side::Sell => ctx.position.0 - ctx.open_sell_qty.0 - qty.0 < -max,
        };
        if too_big {
            Err(RiskViolation::MaxPosition)
        } else {
            Ok(())
        }
    }

    fn on_pnl(&mut self, _total_pnl: Cash) {}

    fn is_halted(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Cash, Price, Qty};

    fn limits() -> RiskLimits {
        RiskLimits {
            max_position: Qty(1_000),
            max_order_qty: Qty(500),
            price_band_ticks: 100,
            max_open_orders: 4,
            max_loss: Cash(1_000),
        }
    }

    fn ctx(position: i64, open_buy: i64) -> RiskContext {
        RiskContext {
            position: Qty(position),
            open_buy_qty: Qty(open_buy),
            open_sell_qty: Qty(0),
            open_orders: 0,
            mid: Some(Price(10_000)),
        }
    }

    fn buy(qty: i64) -> Order {
        Order::Place {
            side: Side::Buy,
            price: Price(10_000),
            qty: Qty(qty),
        }
    }

    #[test]
    fn standin_blocks_max_position() {
        let mut risk = MaxPositionOnly::new(limits());
        // 600 + 300 + 200 = 1_100 > 1_000
        assert_eq!(
            risk.check(&buy(200), &ctx(600, 300)),
            Err(RiskViolation::MaxPosition)
        );
        // 600 + 300 + 100 = 1_000, exactly at the limit
        assert_eq!(risk.check(&buy(100), &ctx(600, 300)), Ok(()));
    }

    #[test]
    fn standin_never_halts() {
        let mut risk = MaxPositionOnly::new(limits());
        risk.on_pnl(Cash(-1_000_000_000));
        assert!(!risk.is_halted());
        assert_eq!(risk.check(&buy(100), &ctx(0, 0)), Ok(()));
    }
}
