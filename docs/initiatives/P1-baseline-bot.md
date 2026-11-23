# P1: Baseline bot

Phase 1 · Depends on: P0 · Size: L

## Vision

The first end-to-end run. A simulated BTC/USDT exchange runs with a synthetic market of other traders. A simple market-making bot trades in it through a `Venue` interface, with realistic delays and fees, and prints a summary of how it did.

Everything is single-threaded and simple: correct first, fast later. This becomes the **reference implementation**. Every later initiative replaces a piece of it and must produce the same results.

**Success looks like:**
- `cargo run --release` simulates an hour of trading in seconds and prints PnL, fills, inventory, fees and venue.
- Turning the knobs (spread, skew, latency, fees) changes the results in ways you can explain.
- You wrote the order book, the market maker and the risk checks yourself.

**You'll learn:**
- *Trading:* how an order book and matching engine work; how a market maker earns the spread and loses through adverse selection and inventory risk; how fees and latency eat profit; PnL accounting.
- *Rust:* structs, enums, `match`, traits, `Vec`/`BTreeMap`/`VecDeque`, ownership in practice, unit tests, property tests.
- *Simulation:* discrete-event simulation, seeded randomness and determinism.

## Key concepts

- **Price-time priority:** best price matches first; at equal price, the earliest order matches first.
- **Maker vs taker:** the resting order is the maker; the incoming order that crosses the spread is the taker. They pay different fees.
- **Mid price:** (best bid + best ask) / 2. Market makers quote around it.
- **Inventory skew:** when long, lower both quotes so you're more likely to sell and less likely to buy, pushing inventory back toward zero.
- **Adverse selection:** you get filled most often right before the price moves against you, because informed traders hit you first.
- **Discrete-event simulation:** time doesn't tick. The loop takes the next scheduled event from a priority queue, advances the clock to it, and processes it.
- **Realized vs unrealized PnL:** realized comes from closed round trips; unrealized is open inventory valued at the current mid.

## Design sketch

```
                 ┌───────────────── event loop (sim clock) ─────────────────┐
                 │  priority queue of (deliver_at, seq, Event)              │
                 └───────▲─────────────────────────────────────┬────────────┘
                         │ schedule                            │ next event
   ┌─────────────────────┴───────┐                     ┌───────▼────────┐
   │ exchange                    │  market data, acks, │ SimVenue       │
   │  matching engine            │  fills (+latency)   │  → bot events  │
   │  synthetic market:          │ ───────────────────▶│                │
   │   fair price random walk    │                     └───────┬────────┘
   │   background liquidity      │                             ▼
   │   takers (some informed)    │                     book → strategy → risk
   │  fee model                  │  orders, cancels            │
   │                             │◀──── (+latency) ────────────┘
   └─────────────────────────────┘
```

- **Latency** is just scheduling: a message sent at `t` is delivered at `t + delay`.
- **Determinism:** one seeded RNG per source (fair price, liquidity, takers), and ties in the queue broken by a sequence number.
- **Stand-ins:** Claude provides a crude version of each owner-built component, so the loop runs end to end from early on:
  - `book`: top-of-book only
  - `strategy`: fixed spread, no skew
  - `risk`: max position only

## Task breakdown

### Stage 1: Exchange core [Claude]

| # | Task | Owner | Depends on |
|---|---|---|---|
| [x] 1.1 | Matching engine: limit, market and cancel orders; price-time priority; partial fills; emits ack/reject/fill/cancelled | [Claude] | P0 |
| [x] 1.2 | Matching engine tests, including property tests ("book never stays crossed", "quantity is conserved") | [Claude] | 1.1 |
| [x] 1.3 | Fee model: maker/taker as basis points, negative maker fee = rebate, integer math; per-fill fee on fill events | [Claude] | 1.1 |
| [x] 1.4 | Market data output: top of book plus depth snapshot after each change | [Claude] | 1.1 |

### Stage 2: Simulation engine [Claude]

| # | Task | Owner | Depends on |
|---|---|---|---|
| [x] 1.5 | Event loop with sim clock and priority queue (deliver time, then sequence number) | [Claude] | P0 |
| [x] 1.6 | Latency model: configurable one-way delay applied to every message in both directions; optional seeded jitter | [Claude] | 1.5 |
| [x] 1.7 | Synthetic market: fair price random walk, background liquidity around fair price, takers arriving at random intervals with an informed fraction | [Claude] | 1.1, 1.5 |
| [x] 1.8 | Determinism test: same seed gives an identical event trace | [Claude] | 1.7 |
| [x] 1.9 | Latency tests: an in-flight taker beats a later cancel; delay 0 delivers in the same instant | [Claude] | 1.6 |

### Stage 3: Bot interfaces + stand-ins [Claude]

| # | Task | Owner | Depends on |
|---|---|---|---|
| [x] 1.10 | `Venue` trait (submit, cancel, poll events) and `SimVenue` | [Claude] | 1.5 |
| [x] 1.11 | `OrderBook`, `Strategy` and `RiskCheck` interfaces, with stand-in implementations | [Claude] | P0 |
| [x] 1.12 | App wiring: CLI options (seed, duration, spread, skew, size, limits, latency, fees), PnL tracker, summary printout | [Claude] | 1.10, 1.11 |
| [x] 1.13 | **First end-to-end run** with stand-ins | [Claude] | 1.12 |

### Stage 4: Owner-built components [You]

For each component, Claude first writes: the interface (already done in 1.11), stub bodies, a test suite as the spec, and a guide covering concepts, the Rust needed and reading links.

| # | Task | Owner | Depends on |
|---|---|---|---|
| [x] 1.14 | Scaffolding for book: tests + guide | [Claude] | 1.11 |
| [ ] 1.15 | **Order book**: apply snapshots/updates, best bid/ask, mid, spread, depth at level, using `BTreeMap` per side | [You] | 1.14 |
| [x] 1.16 | Scaffolding for strategy: tests + guide | [Claude] | 1.11 |
| [ ] 1.17 | **Market maker**: quote around mid at a configurable spread; inventory skew; requote on mid change or fill; cancel stale quotes | [You] | 1.16, 1.15 |
| [x] 1.18 | Scaffolding for risk: tests + guide | [Claude] | 1.11 |
| [ ] 1.19 | **Risk checks**: max position, max order size, price band around mid, max open orders; loss limit halts the bot | [You] | 1.18 |
| [ ] 1.20 | Swap stand-ins for owner implementations; full run | [Together] | 1.15, 1.17, 1.19 |

### Stage 5: Understanding the results [Together]

| # | Task | Owner | Depends on |
|---|---|---|---|
| [x] 1.21 | PnL accounting test on a hand-worked scenario, including fees and a rebate | [Together] | 1.12 |
| [ ] 1.22 | Experiments: sweep spread, skew, latency (0 vs 1 ms vs 10 ms) and fees (zero vs default vs retail), then tabulate the results | [You] | 1.20 |
| [ ] 1.23 | Write up findings: what makes the bot profitable or not, and why. Add a "Results" section to this doc | [You] | 1.22 |

## Pitfalls

- **Floats for prices.** Use integer ticks everywhere; convert only for display.
- **The bot peeking at the exchange's internal state.** It must only see what the `Venue` delivers.
- **Non-determinism** from `HashMap` iteration order or unseeded RNGs.
- **Default parameters where the bot never trades** (spread too wide) **or always loses** (informed fraction too high). Tune the defaults so results are interesting.
- **Forgetting that cancels are delayed too.** That's the point of the latency model.

## Done when

- [x] `cargo run --release` completes one simulated hour and prints the summary, including venue, latency and fee lines, and PnL before and after fees.
- [ ] Book, strategy and risk are the owner's implementations, and all their tests pass.
- [ ] Determinism, latency and PnL accounting tests pass; clippy is clean.
- [ ] The experiment table exists, and the owner can explain each trend in it.

## Results

First end-to-end run with the stand-in book, strategy and risk (`make run`, seed 42, default options, 1 simulated hour):

```
hft-lab run summary
  venue:            sim (in-process) · BTC/USDT
  seed:             42
  simulated time:   1h 00m
  latency:          1.000 ms each way
  fees:             maker 0.020% / taker 0.050%
  orders sent:      66,976   (rejected by risk: 16,327)
  fills:            17,360   (buy 8,704 / sell 8,656 · maker 17,360 / taker 0)
  volume:           12.92163 BTC
  final inventory:  +0.00945 BTC
  gross PnL:        +316.79 USDT (before fees)
  fees paid:        -258.32 USDT
  realized PnL:     +58.33 USDT (after fees)
  unrealized PnL:   +0.13 USDT (marked at final mid)
  total PnL:        +58.47 USDT
  max drawdown:     -0.33 USDT
  halted:           no
```

The experiment table and the write-up (tasks 1.22 and 1.23) are still to do.
