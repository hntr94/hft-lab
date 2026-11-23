//! PnL tracker: average-cost position accounting, fees and drawdown.
//!
//! All amounts are integers (`Cash` = 1e-7 USDT), so the numbers are exact.

use types::{Cash, Liquidity, Price, Qty, Side, notional};

/// Tracks the bot's position, cash flow, fees and drawdown from its fills.
#[derive(Debug, Clone, Default)]
pub struct PnlTracker {
    /// Signed position in lots (positive = long).
    position: i64,
    /// Signed cost of the open position: `sum(sign * notional)` of what is still open.
    cost_basis: i64,
    /// `-sum(sign * notional)` over all fills.
    cash_flow: i64,
    /// Fees paid (negative = rebate).
    fees: i64,
    /// Last mark price, if any was given.
    mark: Option<Price>,
    /// Price of the most recent fill, used as a mark until the first real one.
    last_fill_price: Option<Price>,
    /// Highest `total` seen so far (starts at 0, the total at the start).
    peak: i64,
    /// Lowest `total - peak` seen so far (always <= 0).
    max_drawdown: i64,
    fills: u64,
    buys: u64,
    sells: u64,
    maker_fills: u64,
    taker_fills: u64,
    volume: i64,
}

impl PnlTracker {
    /// A tracker with no fills and no mark.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one bot fill. `fee` is the exchange's fee (negative = rebate).
    pub fn on_fill(&mut self, side: Side, price: Price, qty: Qty, liquidity: Liquidity, fee: Cash) {
        let sign = side.sign();
        let value = notional(price, qty).0;
        let before = self.position;
        let delta = sign * qty.0;

        if before == 0 || before.signum() == sign {
            // Opening or increasing: the new fill adds to the cost.
            self.cost_basis += sign * value;
        } else {
            // Reducing, closing or flipping.
            let closed = qty.0.min(before.abs());
            // i128: cost_basis * closed can overflow i64 for large positions.
            let removed = (self.cost_basis as i128 * closed as i128 / before.abs() as i128) as i64;
            self.cost_basis -= removed;
            let reopened = qty.0 - closed;
            if reopened > 0 {
                // Flip: the old position is fully closed; the rest opens at the fill price.
                self.cost_basis = sign * notional(price, Qty(reopened)).0;
            }
        }
        self.position = before + delta;
        self.cash_flow -= sign * value;
        self.fees += fee.0;
        self.last_fill_price = Some(price);

        self.fills += 1;
        match side {
            Side::Buy => self.buys += 1,
            Side::Sell => self.sells += 1,
        }
        match liquidity {
            Liquidity::Maker => self.maker_fills += 1,
            Liquidity::Taker => self.taker_fills += 1,
        }
        self.volume += qty.0.abs();
        self.update_drawdown();
    }

    /// Set the mark price; `None` keeps the previous mark.
    pub fn mark(&mut self, mid: Option<Price>) {
        if mid.is_some() {
            self.mark = mid;
        }
        self.update_drawdown();
    }

    /// The price used to value the open position: the last mark, else the
    /// last fill price (a reasonable fallback before any mark), else 0.
    fn effective_mark(&self) -> i64 {
        self.mark.or(self.last_fill_price).map_or(0, |p| p.0)
    }

    fn position_value(&self) -> i64 {
        self.position * self.effective_mark()
    }

    fn update_drawdown(&mut self) {
        let total = self.total().0;
        self.peak = self.peak.max(total);
        self.max_drawdown = self.max_drawdown.min(total - self.peak);
    }

    /// Trading PnL before fees: cash flow plus the open position at the mark.
    pub fn gross(&self) -> Cash {
        Cash(self.cash_flow + self.position_value())
    }

    /// Fees paid (negative = rebates received).
    pub fn fees(&self) -> Cash {
        Cash(self.fees)
    }

    /// Realized PnL after fees.
    pub fn realized_net(&self) -> Cash {
        Cash(self.gross().0 - self.unrealized().0 - self.fees)
    }

    /// Open position at the mark minus its cost.
    pub fn unrealized(&self) -> Cash {
        Cash(self.position_value() - self.cost_basis)
    }

    /// Total PnL after fees.
    pub fn total(&self) -> Cash {
        Cash(self.gross().0 - self.fees)
    }

    /// Worst fall of `total` from its running peak (<= 0).
    pub fn max_drawdown(&self) -> Cash {
        Cash(self.max_drawdown)
    }

    /// Number of fills.
    pub fn fills(&self) -> u64 {
        self.fills
    }

    /// Number of buy fills.
    pub fn buys(&self) -> u64 {
        self.buys
    }

    /// Number of sell fills.
    pub fn sells(&self) -> u64 {
        self.sells
    }

    /// Number of maker fills.
    pub fn maker_fills(&self) -> u64 {
        self.maker_fills
    }

    /// Number of taker fills.
    pub fn taker_fills(&self) -> u64 {
        self.taker_fills
    }

    /// Total traded quantity (sum of |qty|).
    pub fn volume(&self) -> Qty {
        Qty(self.volume)
    }

    /// Signed position (positive = long).
    pub fn position(&self) -> Qty {
        Qty(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{fee, notional};

    fn fill(t: &mut PnlTracker, side: Side, price: i64, qty: i64, ppm: i64) {
        let n = notional(Price(price), Qty(qty));
        t.on_fill(side, Price(price), Qty(qty), Liquidity::Maker, fee(n, ppm));
    }

    fn round_trip(ppm: i64) -> PnlTracker {
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 100, ppm);
        fill(&mut t, Side::Sell, 10_050, 100, ppm);
        t.mark(Some(Price(10_050)));
        t
    }

    #[test]
    fn round_trip_with_fees() {
        let t = round_trip(200);
        assert_eq!(t.gross(), Cash(5_000));
        assert_eq!(t.fees(), Cash(401));
        assert_eq!(t.realized_net(), Cash(4_599));
        assert_eq!(t.unrealized(), Cash(0));
        assert_eq!(t.total(), Cash(4_599));
        assert_eq!((t.fills(), t.buys(), t.sells()), (2, 1, 1));
        assert_eq!((t.maker_fills(), t.taker_fills()), (2, 0));
        assert_eq!(t.volume(), Qty(200));
        assert_eq!(t.position(), Qty(0));
    }

    #[test]
    fn rebate_increases_pnl() {
        let t = round_trip(-100);
        assert_eq!(t.fees(), Cash(-200));
        assert_eq!(t.total(), Cash(5_200));
    }

    #[test]
    fn open_position_unrealized() {
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 100, 0);
        t.mark(Some(Price(10_020)));
        assert_eq!(t.unrealized(), Cash(2_000));
        assert_eq!(t.realized_net(), Cash(0));
        assert_eq!(t.total(), Cash(2_000));
    }

    #[test]
    fn position_flip_realizes_and_reopens() {
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 100, 0);
        fill(&mut t, Side::Sell, 10_100, 150, 0);
        t.mark(Some(Price(10_100)));
        assert_eq!(t.realized_net(), Cash(10_000));
        assert_eq!(t.position(), Qty(-50));
        assert_eq!(t.unrealized(), Cash(0));
        assert_eq!(t.cost_basis, -50 * 10_100);
    }

    #[test]
    fn unrealized_uses_last_known_mark() {
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 100, 0);
        t.mark(Some(Price(10_020)));
        let before = t.unrealized();
        t.mark(None);
        assert_eq!(t.unrealized(), before);
        assert_eq!(before, Cash(2_000));
    }

    #[test]
    fn open_position_without_mark_uses_last_fill_price() {
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 100, 0);
        assert_eq!(t.unrealized(), Cash(0));
        assert_eq!(t.total(), Cash(0));
    }

    #[test]
    fn drawdown_tracks_peak() {
        // 10 lots, so one tick of price is 10 cash units: +100, -50, +20.
        let mut t = PnlTracker::new();
        fill(&mut t, Side::Buy, 10_000, 10, 0);
        t.mark(Some(Price(10_010)));
        assert_eq!(t.total(), Cash(100));
        t.mark(Some(Price(9_995)));
        assert_eq!(t.total(), Cash(-50));
        t.mark(Some(Price(10_002)));
        assert_eq!(t.total(), Cash(20));
        assert_eq!(t.max_drawdown(), Cash(-150));
    }
}
