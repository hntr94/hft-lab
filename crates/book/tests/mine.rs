//! Spec for `MyBook`. Run with `make exercise-book` from the repo root.

use book::{MyBook, OrderBook};
use types::{BookSnapshot, Level, Price, Qty, Side};

fn lv(p: i64, q: i64) -> Level {
    Level {
        price: Price(p),
        qty: Qty(q),
    }
}

fn snap(bids: &[(i64, i64)], asks: &[(i64, i64)]) -> BookSnapshot {
    BookSnapshot {
        bids: bids.iter().map(|&(p, q)| lv(p, q)).collect(),
        asks: asks.iter().map(|&(p, q)| lv(p, q)).collect(),
    }
}

fn sample() -> BookSnapshot {
    snap(&[(100, 5), (99, 3)], &[(102, 4), (103, 1)])
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn empty_book_has_nothing() {
    let book = MyBook::new();
    assert_eq!(book.best_bid(), None);
    assert_eq!(book.best_ask(), None);
    assert_eq!(book.mid(), None);
    assert_eq!(book.spread(), None);
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn snapshot_sets_best_levels() {
    let mut book = MyBook::new();
    book.apply_snapshot(&sample());
    assert_eq!(book.best_bid(), Some(lv(100, 5)));
    assert_eq!(book.best_ask(), Some(lv(102, 4)));
    assert_eq!(book.mid(), Some(Price(101)));
    assert_eq!(book.spread(), Some(2));
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn depth_at_levels() {
    let mut book = MyBook::new();
    book.apply_snapshot(&sample());
    assert_eq!(book.depth_at(Side::Buy, Price(99)), Qty(3));
    assert_eq!(book.depth_at(Side::Sell, Price(103)), Qty(1));
    assert_eq!(book.depth_at(Side::Buy, Price(98)), Qty(0));
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn new_snapshot_replaces_old() {
    let mut book = MyBook::new();
    book.apply_snapshot(&sample());
    book.apply_snapshot(&snap(&[(98, 1)], &[(104, 2)]));
    assert_eq!(book.depth_at(Side::Buy, Price(100)), Qty(0));
    assert_eq!(book.best_bid(), Some(lv(98, 1)));
    assert_eq!(book.best_ask(), Some(lv(104, 2)));
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn one_sided_book_has_no_mid() {
    let mut book = MyBook::new();
    book.apply_snapshot(&snap(&[(100, 5)], &[]));
    assert_eq!(book.mid(), None);
    assert_eq!(book.spread(), None);
    assert!(book.best_bid().is_some());
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn mid_rounds_down() {
    let mut book = MyBook::new();
    book.apply_snapshot(&snap(&[(100, 1)], &[(101, 1)]));
    assert_eq!(book.mid(), Some(Price(100)));
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn unsorted_input_still_finds_best() {
    let mut book = MyBook::new();
    book.apply_snapshot(&snap(&[], &[(105, 1), (102, 4), (103, 1)]));
    assert_eq!(book.best_ask(), Some(lv(102, 4)));
}

#[test]
#[ignore = "exercise: run `make exercise-book`"]
fn zero_qty_levels_ignored() {
    let mut book = MyBook::new();
    book.apply_snapshot(&snap(&[(101, 0), (100, 5)], &[]));
    assert_eq!(book.best_bid(), Some(lv(100, 5)));
}
