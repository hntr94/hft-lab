use exchange::{SimConfig, Simulation, SyntheticConfig};
use types::*;

fn quiet(latency_ns: u64) -> SimConfig {
    SimConfig {
        synthetic: SyntheticConfig {
            enabled: false,
            ..Default::default()
        },
        latency_ns,
        ..Default::default()
    }
}

fn bot_limit(seq: u64, side: Side, price: i64, qty: i64) -> Command {
    Command::New(NewOrder {
        id: OrderId { trader: BOT, seq },
        side,
        qty: Qty(qty),
        kind: OrderType::Limit {
            price: Price(price),
        },
    })
}

fn collect(cfg: SimConfig) -> Vec<(Timestamp, BotEvent)> {
    let mut sim = Simulation::new(cfg);
    let mut out = Vec::new();
    while let Some(ev) = sim.next_bot_event() {
        out.push(ev);
    }
    out
}

#[test]
fn bot_order_arrives_after_latency() {
    let mut sim = Simulation::new(quiet(1_000_000));
    sim.send(bot_limit(1, Side::Buy, 100, 5));
    let (t, ev) = sim.next_bot_event().unwrap();
    assert_eq!(t, Timestamp(2_000_000));
    assert_eq!(
        ev,
        BotEvent::Exec(ExecEvent::Ack {
            id: OrderId {
                trader: BOT,
                seq: 1
            }
        })
    );
    let (t2, ev2) = sim.next_bot_event().unwrap();
    assert_eq!(t2, Timestamp(2_000_000));
    assert!(matches!(ev2, BotEvent::Market(MarketData::Snapshot(_))));
}

#[test]
fn cancel_loses_race_to_inflight_taker() {
    let mut sim = Simulation::new(quiet(1_000_000));
    let bid = OrderId {
        trader: BOT,
        seq: 1,
    };
    sim.send(bot_limit(1, Side::Buy, 100, 5));
    // Pump until the Ack (t = 2 ms).
    loop {
        let (t, ev) = sim.next_bot_event().unwrap();
        if matches!(ev, BotEvent::Exec(ExecEvent::Ack { .. })) {
            assert_eq!(t, Timestamp(2_000_000));
            break;
        }
    }
    sim.inject(
        Timestamp(2_500_000),
        Command::New(NewOrder {
            id: OrderId {
                trader: TAKER,
                seq: 1,
            },
            side: Side::Sell,
            qty: Qty(5),
            kind: OrderType::Market,
        }),
    );
    sim.send(Command::Cancel(bid));
    let mut execs = Vec::new();
    while let Some((t, ev)) = sim.next_bot_event() {
        if let BotEvent::Exec(e) = ev {
            execs.push((t, e));
        }
    }
    assert_eq!(execs.len(), 2, "{execs:?}");
    assert!(matches!(execs[0].1, ExecEvent::Fill { id, .. } if id == bid));
    assert_eq!(execs[0].0, Timestamp(3_500_000));
    assert_eq!(
        execs[1].1,
        ExecEvent::Reject {
            id: bid,
            reason: RejectReason::UnknownOrder
        }
    );
    assert_eq!(execs[1].0, Timestamp(4_000_000));
    assert!(
        !execs
            .iter()
            .any(|(_, e)| matches!(e, ExecEvent::Cancelled { .. }))
    );
}

#[test]
fn zero_latency_same_instant() {
    let mut sim = Simulation::new(quiet(0));
    sim.send(bot_limit(1, Side::Buy, 100, 5));
    let (t, ev) = sim.next_bot_event().unwrap();
    assert_eq!(t, sim.now());
    assert_eq!(t, Timestamp(0));
    assert!(matches!(ev, BotEvent::Exec(ExecEvent::Ack { .. })));
}

fn short(seed: u64) -> SimConfig {
    SimConfig {
        seed,
        duration_ns: 60_000_000_000,
        ..Default::default()
    }
}

#[test]
fn deterministic_trace() {
    let a = collect(short(42));
    let b = collect(short(42));
    assert!(!a.is_empty());
    assert_eq!(a, b);
}

#[test]
fn different_seed_different_trace() {
    assert_ne!(collect(short(42)), collect(short(43)));
}

#[test]
fn runs_until_duration() {
    let cfg = SimConfig {
        duration_ns: 10_000_000_000,
        ..Default::default()
    };
    let trace = collect(cfg);
    assert!(!trace.is_empty());
    assert!(trace.iter().all(|(t, _)| t.0 <= 10_000_000_000));
}

#[test]
fn synthetic_book_two_sided_after_start() {
    let mut sim = Simulation::new(SimConfig::default());
    loop {
        let (_, ev) = sim.next_bot_event().unwrap();
        if let BotEvent::Market(MarketData::Snapshot(s)) = ev {
            assert!(!s.bids.is_empty());
            assert!(!s.asks.is_empty());
            break;
        }
    }
    assert!(sim.exchange_mid().is_some());
}

#[test]
fn lp_refresh_crossing_bot_quote_delivers_bot_fill() {
    // Takers effectively off, so only an LP refresh can fill the bot.
    let cfg = SimConfig {
        duration_ns: 600_000_000_000,
        synthetic: SyntheticConfig {
            taker_rate_per_sec: 1e-9,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut sim = Simulation::new(cfg);
    // Learn the best ask, then rest a bid one tick under it.
    let ask = loop {
        let (_, ev) = sim.next_bot_event().unwrap();
        if let BotEvent::Market(MarketData::Snapshot(s)) = ev {
            break s.asks[0].price;
        }
    };
    sim.send(bot_limit(1, Side::Buy, ask.0 - 1, 1_000));
    let mut fill = false;
    while let Some((_, ev)) = sim.next_bot_event() {
        if matches!(ev, BotEvent::Exec(ExecEvent::Fill { .. })) {
            fill = true;
            break;
        }
    }
    assert!(
        fill,
        "bot never saw a Fill from an LP refresh crossing its bid"
    );
}
