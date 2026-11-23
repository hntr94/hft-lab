//! The owner's exercise: fill in every `todo!` below. See GUIDE.md.

use crate::OrderBook;
use std::collections::BTreeMap;
use types::{BookSnapshot, Level, Price, Qty, Side};

// The fields are unused until you write the methods. Delete this line once they are used.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct MyBook {
    // Hint: keep one BTreeMap<Price, Qty> per side. A BTreeMap is always
    // sorted by key, so the best ask is the first key and the best bid is the last.
    bids: BTreeMap<Price, Qty>,
    asks: BTreeMap<Price, Qty>,
}

impl MyBook {
    pub fn new() -> MyBook {
        MyBook::default()
    }
}

impl OrderBook for MyBook {
    fn apply_snapshot(&mut self, _snap: &BookSnapshot) {
        todo!("exercise: clear both maps, then insert every level with qty > 0")
    }

    fn best_bid(&self) -> Option<Level> {
        todo!("exercise: return the highest bid level")
    }

    fn best_ask(&self) -> Option<Level> {
        todo!("exercise: return the lowest ask level")
    }

    fn mid(&self) -> Option<Price> {
        todo!("exercise: floor of (best bid + best ask) / 2, None unless both sides exist")
    }

    fn spread(&self) -> Option<i64> {
        todo!("exercise: best ask minus best bid, None unless both sides exist")
    }

    fn depth_at(&self, _side: Side, _price: Price) -> Qty {
        todo!("exercise: look the price up on the right side, Qty(0) if absent")
    }
}
