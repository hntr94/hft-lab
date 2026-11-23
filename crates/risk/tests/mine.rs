//! Spec for `MyRisk`. Run with `make exercise-risk` from the repo root.

use risk::{MyRisk, Order, RiskCheck, RiskContext, RiskLimits, RiskViolation};
use types::{Cash, Price, Qty, Side};

fn limits() -> RiskLimits {
    RiskLimits {
        max_position: Qty(1_000),
        max_order_qty: Qty(500),
        price_band_ticks: 100,
        max_open_orders: 4,
        max_loss: Cash(1_000),
    }
}

/// Flat, nothing open, mid 10_000.
fn base_ctx() -> RiskContext {
    RiskContext {
        position: Qty(0),
        open_buy_qty: Qty(0),
        open_sell_qty: Qty(0),
        open_orders: 0,
        mid: Some(Price(10_000)),
    }
}

fn place(side: Side, price: i64, qty: i64) -> Order {
    Order::Place {
        side,
        price: Price(price),
        qty: Qty(qty),
    }
}

fn risk() -> MyRisk {
    MyRisk::new(limits())
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn normal_order_passes() {
    let mut r = risk();
    assert_eq!(r.check(&place(Side::Buy, 10_000, 100), &base_ctx()), Ok(()));
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn order_too_large() {
    let mut r = risk();
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 501), &base_ctx()),
        Err(RiskViolation::MaxOrderQty)
    );
    assert_eq!(r.check(&place(Side::Buy, 10_000, 500), &base_ctx()), Ok(()));
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn too_many_open_orders() {
    let mut r = risk();
    let ctx = RiskContext {
        open_orders: 4,
        ..base_ctx()
    };
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 100), &ctx),
        Err(RiskViolation::MaxOpenOrders)
    );
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn no_mid_blocks_orders() {
    let mut r = risk();
    let ctx = RiskContext {
        mid: None,
        ..base_ctx()
    };
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 100), &ctx),
        Err(RiskViolation::NoReferencePrice)
    );
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn price_band() {
    let mut r = risk();
    assert_eq!(
        r.check(&place(Side::Buy, 10_101, 100), &base_ctx()),
        Err(RiskViolation::PriceBand)
    );
    assert_eq!(r.check(&place(Side::Buy, 10_100, 100), &base_ctx()), Ok(()));
    assert_eq!(r.check(&place(Side::Buy, 9_900, 100), &base_ctx()), Ok(()));
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn buy_position_limit_counts_open_orders() {
    let mut r = risk();
    let ctx = RiskContext {
        position: Qty(600),
        open_buy_qty: Qty(300),
        ..base_ctx()
    };
    // 600 + 300 + 200 = 1_100 > 1_000
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 200), &ctx),
        Err(RiskViolation::MaxPosition)
    );
    // 600 + 300 + 100 = 1_000, exactly at the limit
    assert_eq!(r.check(&place(Side::Buy, 10_000, 100), &ctx), Ok(()));
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn sell_position_limit() {
    let mut r = risk();
    let ctx = RiskContext {
        position: Qty(-600),
        open_sell_qty: Qty(300),
        ..base_ctx()
    };
    // -600 - 300 - 200 = -1_100 < -1_000
    assert_eq!(
        r.check(&place(Side::Sell, 10_000, 200), &ctx),
        Err(RiskViolation::MaxPosition)
    );
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn loss_limit_halts() {
    let mut r = risk();
    r.on_pnl(Cash(-1_000));
    assert!(!r.is_halted());
    r.on_pnl(Cash(-1_001));
    assert!(r.is_halted());
    r.on_pnl(Cash(0));
    assert!(r.is_halted());
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn halted_blocks_places_not_cancels() {
    let mut r = risk();
    r.on_pnl(Cash(-1_001));
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 100), &base_ctx()),
        Err(RiskViolation::Halted)
    );
    assert_eq!(r.check(&Order::Cancel, &base_ctx()), Ok(()));
}

#[test]
#[ignore = "exercise: run `make exercise-risk`"]
fn check_order_is_fixed() {
    // Rules 3 and 5 both fail here. The earlier rule (MaxOrderQty) wins.
    let mut r = risk();
    let ctx = RiskContext {
        mid: None,
        ..base_ctx()
    };
    assert_eq!(
        r.check(&place(Side::Buy, 10_000, 600), &ctx),
        Err(RiskViolation::MaxOrderQty)
    );
}
