# B: Resilience

Phase 3 (Robustness) · Depends on: A · Size: M–L

## Vision

Real trading systems spend as much engineering on *not losing money when things break* as on making money. In this initiative the bot learns to notice when its view of the market is stale, incomplete or wrong, and to fail safe. Every run also becomes perfectly reproducible from a log.

This is a prerequisite for touching any real venue (H1), where disconnects, gaps and rejections are routine.

**Success looks like:** the simulator injects faults (dropped messages, bursts, disconnects) and the bot never trades on a known-bad view of the market. A kill switch reliably flattens and halts. Any run can be replayed from its log to produce an identical summary.

**You'll learn:**
- failure modes of trading systems
- sequence numbers and gap recovery (snapshot plus incremental updates)
- event sourcing and deterministic replay
- backpressure
- designing for "safe by default"
- testing failure paths, not just happy paths

## Key concepts

- **Staleness:** market data older than a threshold can't be trusted. Pull quotes until fresh data arrives.
- **Sequence numbers:** every market data message is numbered. A jump from 101 to 103 means 102 was lost, so the book is wrong.
- **Snapshot recovery:** on a gap, discard the book, request a full snapshot, then apply only updates numbered after the snapshot.
- **Kill switch:** a single action that cancels all orders and halts. It must work even when other things are broken.
- **Event sourcing:** the log of inputs is the source of truth; state is whatever you get by replaying it.
- **Backpressure / conflation:** if updates arrive faster than they can be processed, merge them (keep only the latest book state) instead of falling ever further behind.

## Design sketch

- Add sequence numbers to market data in `types`, populated by the exchange.
- Book states: `Synced` → (gap detected) → `Recovering` → (snapshot applied) → `Synced`. The strategy only quotes while `Synced` and fresh.
- A **fault injector** in the simulator, configured per run: drop probability, burst delays, disconnects (which cancel all orders on the exchange side, i.e. cancel-on-disconnect).
- **Kill switch triggers:** loss limit, drawdown limit, abnormal fill rate, risk-check failure, repeated rejections, manual (a key press or signal).
- **Event log:** an append-only binary file of every input event, with its timestamp. **Replay mode** feeds the log instead of the live simulator and must produce the same decisions.
- **Conflation:** a per-stream "latest snapshot wins" buffer, used when the inbound queue exceeds a depth threshold.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| B.1 | Guide: failure modes, gap recovery, event sourcing; reading list | [Claude] | none |
| B.2 | Sequence numbers on market data; snapshot-request message in the protocol | [Claude] | P1 |
| B.3 | Fault injector in the simulator: drops, bursts, disconnects with cancel-on-disconnect | [Claude] | B.2 |
| B.4 | Scaffolding for gap detection + recovery in the book: tests + guide | [Claude] | B.2 |
| B.5 | **Gap detection + snapshot recovery** in the owner's book (state machine) | [You] | B.4 |
| B.6 | Scaffolding for staleness guard + kill switch: tests + guide | [Claude] | P1 |
| B.7 | **Staleness guard**: pull quotes when data is stale; resume when fresh | [You] | B.6 |
| B.8 | **Kill switch**: all triggers, cancel-all, halt, with the reason shown in the summary | [You] | B.6 |
| B.9 | Event log writer (append-only, binary, off the hot path) | [Claude] | P1 |
| B.10 | Replay mode: run from a log; replay-equals-original test | [Claude] | B.9 |
| B.11 | Conflation of market data under backpressure | [Together] | B.2 |
| B.12 | Chaos experiments: run with heavy fault injection; check the bot never trades while `Recovering` or stale; record the results | [You] | B.5, B.7, B.8 |
| B.13 | Summary additions: gaps detected, recoveries, stale periods, kill switch events | [Claude] | B.8 |

## Pitfalls

- A kill switch that depends on the component that failed. Keep it simple and independent.
- Replay mismatches caused by hidden non-determinism: wall-clock reads, unseeded RNGs, `HashMap` order.
- Logging on the hot path. Write to a buffer that a separate step flushes.
- Testing only that recovery *happens*, not that the bot *stopped trading* while recovering.

## Done when

- [ ] With fault injection enabled, tests prove that no order is sent while the book is `Recovering` or stale.
- [ ] Every kill switch trigger has a test, and the summary reports why the bot halted.
- [ ] Replaying a logged run produces a byte-identical summary.
- [ ] Results of the chaos experiments are recorded in this doc.
