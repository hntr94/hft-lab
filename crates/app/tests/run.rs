use app::{AppConfig, Impl, run, run_with};
use risk::{Order, RiskCheck, RiskContext, RiskViolation};
use types::Cash;

fn cfg() -> AppConfig {
    AppConfig {
        duration_secs: 600,
        ..AppConfig::default()
    }
}

#[test]
fn config_validation() {
    let bad: Vec<(&str, AppConfig)> = vec![
        (
            "duration_secs",
            AppConfig {
                duration_secs: 0,
                ..cfg()
            },
        ),
        (
            "quote_lots",
            AppConfig {
                quote_lots: 0,
                ..cfg()
            },
        ),
        (
            "quote_lots",
            AppConfig {
                quote_lots: -5,
                ..cfg()
            },
        ),
        (
            "half_spread_ticks",
            AppConfig {
                half_spread_ticks: -1,
                ..cfg()
            },
        ),
        (
            "max_open_orders",
            AppConfig {
                max_open_orders: 0,
                ..cfg()
            },
        ),
        (
            "max_position_lots",
            AppConfig {
                max_position_lots: 0,
                ..cfg()
            },
        ),
        (
            "max_order_lots",
            AppConfig {
                max_order_lots: 0,
                ..cfg()
            },
        ),
    ];
    for (field, c) in bad {
        let err = c.validate().expect_err(field);
        assert!(err.contains(field), "{err:?} should name {field}");
    }
    let ok = AppConfig {
        latency_us: 0,
        maker_fee_ppm: -100,
        ..cfg()
    };
    assert_eq!(ok.validate(), Ok(()));
    assert_eq!(AppConfig::default().validate(), Ok(()));
}

#[test]
fn deterministic_summary() {
    assert_eq!(run(&cfg()), run(&cfg()));
}

#[test]
fn defaults_trade_meaningfully() {
    let s = run(&cfg());
    assert!(s.fills >= 100, "fills = {}", s.fills);
    assert!(s.buys > 0 && s.sells > 0);
    assert_eq!(s.halted, None);
}

#[test]
fn latency_changes_outcome() {
    let slow = run(&cfg());
    let fast = run(&AppConfig {
        latency_us: 0,
        ..cfg()
    });
    assert_ne!(slow, fast);
}

#[test]
fn fees_change_pnl() {
    let with_fees = run(&cfg());
    let no_fees = run(&AppConfig {
        maker_fee_ppm: 0,
        taker_fee_ppm: 0,
        ..cfg()
    });
    assert_eq!(with_fees.fills, no_fees.fills);
    assert_eq!(with_fees.gross, no_fees.gross);
    assert_eq!(no_fees.fees_paid, Cash(0));
    assert_eq!(no_fees.total - with_fees.total, with_fees.fees_paid);
}

/// Allows everything until `on_pnl` has been called `limit` times, then halts.
struct HaltAfter {
    calls: u32,
    limit: u32,
}

impl RiskCheck for HaltAfter {
    fn check(&mut self, _order: &Order, _ctx: &RiskContext) -> Result<(), RiskViolation> {
        if self.is_halted() {
            Err(RiskViolation::Halted)
        } else {
            Ok(())
        }
    }
    fn on_pnl(&mut self, _total_pnl: Cash) {
        self.calls += 1;
    }
    fn is_halted(&self) -> bool {
        self.calls >= self.limit
    }
}

#[test]
fn halt_cancels_and_stops_quoting() {
    let c = cfg();
    let s = run_with(
        &c,
        Box::new(HaltAfter {
            calls: 0,
            limit: 1_000,
        }),
    );
    assert!(s.halted.is_some());
    assert!(s.fills > 0);
    // Quoting stopped early, so far fewer orders than a full run.
    assert!(s.orders_sent < run(&c).orders_sent);
}

#[test]
#[ignore = "enable after risk exercise"]
fn loss_limit_halts_bot() {
    let s = run(&AppConfig {
        max_loss_usdt: 0,
        risk: Impl::Mine,
        ..cfg()
    });
    assert!(s.halted.is_some());
}

#[test]
fn summary_display_lines() {
    let text = format!("{}", run(&cfg()));
    assert!(text.contains("venue:            sim (in-process) · BTC/USDT"));
    assert!(text.contains("latency:          1.000 ms each way"));
    assert!(text.contains("fees:             maker 0.020% / taker 0.050%"));
    assert!(text.contains("gross PnL:"));
    assert!(text.contains("fees paid:"));
    assert!(text.contains("total PnL:"));
    assert!(text.contains("halted:           no"));
}
