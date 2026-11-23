use std::collections::HashMap;

use exchange::MatchingEngine;
use proptest::prelude::*;
use types::*;

#[derive(Debug, Clone)]
enum Op {
    Limit(bool, i64, i64),
    Market(bool, i64),
    Cancel(usize),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (any::<bool>(), 90i64..110, 1i64..20).prop_map(|(b, p, q)| Op::Limit(b, p, q)),
        (any::<bool>(), 1i64..20).prop_map(|(b, q)| Op::Market(b, q)),
        (0usize..200).prop_map(Op::Cancel),
    ]
}

fn side(b: bool) -> Side {
    if b { Side::Buy } else { Side::Sell }
}

fn fees() -> FeeSchedule {
    FeeSchedule {
        maker_ppm: 200,
        taker_ppm: 500,
    }
}

proptest! {
    #[test]
    fn never_crossed(ops in prop::collection::vec(op_strategy(), 0..200)) {
        let mut eng = MatchingEngine::new(fees());
        let mut ids: Vec<OrderId> = Vec::new();
        for op in ops {
            let cmd = build(&op, &mut ids);
            eng.submit(&cmd);
            if let (Some(b), Some(a)) = (eng.best_bid(), eng.best_ask()) {
                prop_assert!(b < a, "crossed: bid {:?} ask {:?}", b, a);
            }
        }
    }

    #[test]
    fn quantity_conserved(ops in prop::collection::vec(op_strategy(), 0..200)) {
        let mut eng = MatchingEngine::new(fees());
        let mut ids: Vec<OrderId> = Vec::new();
        let mut accepted: HashMap<OrderId, i64> = HashMap::new();
        let mut done: HashMap<OrderId, i64> = HashMap::new();
        let mut submitted: HashMap<OrderId, i64> = HashMap::new();
        for op in ops {
            let cmd = build(&op, &mut ids);
            if let Command::New(o) = &cmd {
                submitted.insert(o.id, o.qty.0);
            }
            let out = eng.submit(&cmd);
            for (_, ev) in out.exec {
                match ev {
                    ExecEvent::Ack { id } => {
                        accepted.insert(id, submitted[&id]);
                    }
                    ExecEvent::Fill { id, qty, .. } => {
                        *done.entry(id).or_insert(0) += qty.0;
                    }
                    ExecEvent::Cancelled { id, leaves } => {
                        *done.entry(id).or_insert(0) += leaves.0;
                    }
                    ExecEvent::Reject { .. } => {}
                }
            }
        }
        let snap = eng.snapshot(usize::MAX);
        let resting_total: i64 = snap.bids.iter().chain(snap.asks.iter()).map(|l| l.qty.0).sum();
        let mut resting_sum = 0;
        for (id, qty) in &accepted {
            let resting = if eng.resting_orders(id.trader).contains(id) {
                // leaves of a resting order = accepted - fills so far
                qty - done.get(id).copied().unwrap_or(0)
            } else {
                0
            };
            resting_sum += resting;
            prop_assert_eq!(*qty, done.get(id).copied().unwrap_or(0) + resting, "order {:?}", id);
        }
        prop_assert_eq!(resting_sum, resting_total);
    }
}

fn build(op: &Op, ids: &mut Vec<OrderId>) -> Command {
    let id = OrderId {
        trader: TraderId(ids.len() as u32 % 3),
        seq: ids.len() as u64 + 1,
    };
    match op {
        Op::Cancel(i) if !ids.is_empty() => Command::Cancel(ids[i % ids.len()]),
        Op::Cancel(_) => Command::Cancel(id),
        Op::Limit(b, p, q) => {
            ids.push(id);
            Command::New(NewOrder {
                id,
                side: side(*b),
                qty: Qty(*q),
                kind: OrderType::Limit { price: Price(*p) },
            })
        }
        Op::Market(b, q) => {
            ids.push(id);
            Command::New(NewOrder {
                id,
                side: side(*b),
                qty: Qty(*q),
                kind: OrderType::Market,
            })
        }
    }
}
