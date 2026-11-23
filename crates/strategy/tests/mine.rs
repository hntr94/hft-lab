//! Spec for `MarketMaker`. Run with `make exercise-strategy` from the repo root.

use book::{OrderBook, TopOfBook};
use strategy::{Action, MarketMaker, MarketMakerConfig, OpenOrder, Strategy, StrategyContext};
use types::{BOT, BookSnapshot, Level, OrderId, Price, Qty, Side, Timestamp};

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

/// A book with best bid 9_990 and best ask 10_010 (mid 10_000).
fn standard_book() -> TopOfBook {
    book_with(&[(9_990, 10)], &[(10_010, 10)])
}

fn book_with(bids: &[(i64, i64)], asks: &[(i64, i64)]) -> TopOfBook {
    let mut book = TopOfBook::new();
    book.apply_snapshot(&BookSnapshot {
        bids: bids.iter().map(|&(p, q)| lv(p, q)).collect(),
        asks: asks.iter().map(|&(p, q)| lv(p, q)).collect(),
    });
    book
}

fn order(seq: u64, side: Side, price: i64, acked: bool, cancel_pending: bool) -> OpenOrder {
    OpenOrder {
        id: OrderId { trader: BOT, seq },
        side,
        price: Price(price),
        leaves: Qty(100),
        acked,
        cancel_pending,
    }
}

fn place(side: Side, price: i64) -> Action {
    Action::Place {
        side,
        price: Price(price),
        qty: Qty(100),
    }
}

fn run(book: &TopOfBook, position: i64, open_orders: &[OpenOrder]) -> Vec<Action> {
    let ctx = StrategyContext {
        now: Timestamp(0),
        book,
        position: Qty(position),
        open_orders,
    };
    MarketMaker::new(cfg()).on_update(&ctx)
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn no_mid_no_actions() {
    let book = book_with(&[(9_990, 10)], &[]);
    assert_eq!(run(&book, 0, &[]), vec![]);
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn flat_quotes_symmetric() {
    assert_eq!(
        run(&standard_book(), 0, &[]),
        vec![place(Side::Buy, 9_995), place(Side::Sell, 10_005)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn long_inventory_skews_down() {
    // skew = 3 * 200 / 100 = 6, reservation = 10_000 - 6 = 9_994.
    assert_eq!(
        run(&standard_book(), 200, &[]),
        vec![place(Side::Buy, 9_989), place(Side::Sell, 9_999)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn short_inventory_skews_up() {
    // skew = 3 * -200 / 100 = -6, reservation = 10_000 + 6 = 10_006.
    assert_eq!(
        run(&standard_book(), -200, &[]),
        vec![place(Side::Buy, 10_001), place(Side::Sell, 10_011)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn orders_at_target_left_alone() {
    let open = [
        order(1, Side::Buy, 9_995, true, false),
        order(2, Side::Sell, 10_005, false, false),
    ];
    assert_eq!(run(&standard_book(), 0, &open), vec![]);
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn stale_quote_cancelled_and_replaced() {
    let stale = order(1, Side::Buy, 9_990, true, false);
    let good_ask = order(2, Side::Sell, 10_005, true, false);
    assert_eq!(
        run(&standard_book(), 0, &[stale.clone(), good_ask]),
        vec![Action::Cancel(stale.id), place(Side::Buy, 9_995)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn pending_cancel_not_cancelled_again() {
    let dying = order(1, Side::Buy, 9_990, true, true);
    let good_ask = order(2, Side::Sell, 10_005, true, false);
    assert_eq!(
        run(&standard_book(), 0, &[dying, good_ask]),
        vec![place(Side::Buy, 9_995)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn unacked_order_counts_as_live() {
    let open = [order(1, Side::Buy, 9_995, false, false)];
    assert_eq!(
        run(&standard_book(), 0, &open),
        vec![place(Side::Sell, 10_005)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn bid_clamped_below_best_ask() {
    // skew = 3 * -1000 / 100 = -30, reservation = 10_000 + 30 = 10_030.
    // The raw bid 10_025 would cross the best ask 10_001, so it becomes 10_000.
    let book = book_with(&[(9_999, 1)], &[(10_001, 1)]);
    assert_eq!(
        run(&book, -1_000, &[]),
        vec![place(Side::Buy, 10_000), place(Side::Sell, 10_035)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn ask_clamped_above_best_bid() {
    // skew = 3 * 1_000 / 100 = 30, reservation = 10_000 - 30 = 9_970.
    // bid = 9_970 - 5 = 9_965 (below the best ask 10_001, so unchanged).
    // The raw ask 9_975 would cross the best bid 9_999, so it becomes 9_999 + 1 = 10_000.
    let book = book_with(&[(9_999, 1)], &[(10_001, 1)]);
    assert_eq!(
        run(&book, 1_000, &[]),
        vec![place(Side::Buy, 9_965), place(Side::Sell, 10_000)]
    );
}

#[test]
#[ignore = "exercise: run `make exercise-strategy`"]
fn two_stale_bids_both_cancelled() {
    // Flat position: the desired bid is 9_995 and the desired ask is 10_005.
    // Both bids (9_990 and 9_980) are stale, so both are cancelled in
    // open_orders order, then the new bid is placed. The ask is already right.
    let stale_a = order(1, Side::Buy, 9_990, true, false);
    let stale_b = order(2, Side::Buy, 9_980, true, false);
    let good_ask = order(3, Side::Sell, 10_005, true, false);
    assert_eq!(
        run(
            &standard_book(),
            0,
            &[stale_a.clone(), stale_b.clone(), good_ask]
        ),
        vec![
            Action::Cancel(stale_a.id),
            Action::Cancel(stale_b.id),
            place(Side::Buy, 9_995)
        ]
    );
}
