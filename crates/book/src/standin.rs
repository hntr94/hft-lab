//! Stand-in book used by the app until `MyBook` is finished.
//! It remembers only the best level on each side.

use crate::OrderBook;
use types::{BookSnapshot, Level, Price, Qty, Side};

#[derive(Debug, Default)]
pub struct TopOfBook {
    bid: Option<Level>,
    ask: Option<Level>,
}

impl TopOfBook {
    pub fn new() -> TopOfBook {
        TopOfBook::default()
    }
}

impl OrderBook for TopOfBook {
    fn apply_snapshot(&mut self, snap: &BookSnapshot) {
        // Scan instead of trusting the input order.
        self.bid = snap
            .bids
            .iter()
            .filter(|l| l.qty.0 > 0)
            .max_by_key(|l| l.price)
            .cloned();
        self.ask = snap
            .asks
            .iter()
            .filter(|l| l.qty.0 > 0)
            .min_by_key(|l| l.price)
            .cloned();
    }

    fn best_bid(&self) -> Option<Level> {
        self.bid.clone()
    }

    fn best_ask(&self) -> Option<Level> {
        self.ask.clone()
    }

    fn mid(&self) -> Option<Price> {
        let bid = self.bid.as_ref()?;
        let ask = self.ask.as_ref()?;
        Some(Price((bid.price.0 + ask.price.0).div_euclid(2)))
    }

    fn spread(&self) -> Option<i64> {
        let bid = self.bid.as_ref()?;
        let ask = self.ask.as_ref()?;
        Some(ask.price - bid.price)
    }

    fn depth_at(&self, side: Side, price: Price) -> Qty {
        let best = match side {
            Side::Buy => &self.bid,
            Side::Sell => &self.ask,
        };
        match best {
            Some(level) if level.price == price => level.qty,
            _ => Qty(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lv(p: i64, q: i64) -> Level {
        Level {
            price: Price(p),
            qty: Qty(q),
        }
    }

    #[test]
    fn standin_top_of_book() {
        let mut book = TopOfBook::new();
        book.apply_snapshot(&BookSnapshot {
            bids: vec![lv(100, 5), lv(99, 3)],
            asks: vec![lv(102, 4)],
        });
        assert_eq!(book.best_bid(), Some(lv(100, 5)));
        assert_eq!(book.best_ask(), Some(lv(102, 4)));
        assert_eq!(book.mid(), Some(Price(101)));
        assert_eq!(book.spread(), Some(2));
        assert_eq!(book.depth_at(Side::Buy, Price(100)), Qty(5));
        assert_eq!(book.depth_at(Side::Buy, Price(99)), Qty(0));
    }

    #[test]
    fn standin_empty() {
        let book = TopOfBook::new();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.mid(), None);
        assert_eq!(book.spread(), None);
    }
}
