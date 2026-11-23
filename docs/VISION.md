# hft-lab: Vision and Phases

Status: draft · 2026-11-06

## 1. Intent

A personal learning project. The owner is new to trading and wants to understand how high-frequency trading systems work by building one in Rust: market data, order books, matching, a strategy, risk controls, and the low-latency engineering around them. A secondary aim is to understand how a trading idea (market making) actually makes or loses money.

The project is too big for one sitting, so it is split into:
- **Phase 0, setup.**
- **Phase 1, a simple, correct baseline bot** that runs end to end.
- **Initiatives**: independent, self-contained projects that each add one layer (measurement, resilience, speed, real connectivity). Each gets its own design doc when it starts.

The README is the learning guide (concepts, glossary, Rust notes). This document is the plan.

## 2. Goals, non-goals, anti-goals

**Goals**
- Something runnable at every stage: `cargo run --release` always does something meaningful.
- Learn by building each component of an HFT stack, then making it faster and safer, with numbers to prove each improvement.
- Eventually connect to real markets through paper trading / testnet venues: crypto first, stocks later.

**Non-goals**
- Trading real money.
- Competing with real HFT firms. No colocation, kernel bypass or FPGAs.
- Production-grade operations: deployment, monitoring stack, multiple users.
- A GUI in phase 1.

**Anti-goals** (would make this a failure even if it "works")
- Code too clever to learn from. Optimizations must be explained and isolated, never sprinkled everywhere.
- Optimizing without measuring. Every speed claim needs a before/after number.
- One big tangled program where a later initiative forces a rewrite of everything.
- Anything that could place a real-money order by accident.

## 3. Guiding principles

1. **Baseline first, then swap.** Phase 1 uses the simplest correct implementation of every component. Initiatives replace one component at a time behind the same interface and run the same tests.
2. **Interfaces between components.** The strategy talks to a `Venue` trait, never to a specific exchange. The order book, risk checks and venue are likewise behind traits or clear module boundaries. This is what lets initiatives stay independent.
3. **Measure before optimizing.** Initiative A (measurement) comes before any speed work.
4. **Deterministic by default.** Simulation runs are seeded and reproducible: same seed, same output.
5. **Prices and quantities are integers** (ticks and lots), never floats.
6. **Safe by default.** Real venues only ever point at paper/testnet endpoints. A live endpoint would need deliberate, explicit configuration, which is out of scope.

## 4. Phase 0: Setup

**Outcome:** an empty but well-shaped Cargo workspace where `cargo build`, `cargo test`, `cargo clippy` and `cargo fmt --check` all pass.

- Cargo workspace at the repo root with one crate per component. Suggested layout (exact split is the builder's call):

  ```
  crates/
    types/      Price, Qty, Side, OrderId, events shared by everyone
    book/       order book
    exchange/   simulated matching engine + synthetic order flow
    venue/      Venue trait + SimVenue adapter
    strategy/   Strategy trait + market maker
    risk/       pre-trade checks
    app/        the binary: wires everything, prints the summary
  ```
- Release profile from README §6 (`lto`, `codegen-units = 1`, `panic = "abort"`, `debug = true`).
- `.gitignore` for `target/`.
- Rust edition 2024, stable toolchain (pinned with `rust-toolchain.toml`).
- Optional: a `Makefile`/`justfile` or a short "dev commands" section in the README. No CI service needed yet.

## 5. Phase 1: Baseline bot

**Outcome:** one command runs a simulated BTC/USDT market with a market-making bot trading in it, then prints a summary. Single-threaded, simple, correct and well-tested. Speed doesn't matter yet.

### 5.1 Components

**Simulated exchange (`exchange`)**
- One instrument: **BTC/USDT**, with tick size and lot size modeled on a real crypto exchange (e.g. 0.01 USDT tick, 0.00001 BTC lot; exact values are the builder's call).
- Order types: **limit** and **market**; **cancel**. Nothing else in phase 1.
- Matching by **price-time priority**; partial fills supported.
- Emits events: order acknowledged, rejected, filled (partial or full), cancelled, and market data updates (top of book at minimum; full depth if cheap).

**Synthetic market (also in `exchange`)**

A market maker needs other traders to trade with. Phase 1 generates them:
- A hidden **"fair price"** that moves as a random walk.
- **Background liquidity:** passive limit orders placed around the fair price so the book is never empty.
- **Takers:** random buy/sell market orders arriving at random intervals (e.g. Poisson). Sometimes they're informed: biased toward the direction the fair price is about to move. This creates the adverse selection a market maker must survive.
- All randomness is seeded.

**Clock**
- Phase 1 runs on **simulated time** (an event loop that jumps from event to event), not wall-clock time. Runs are fast and deterministic. A run is defined by a simulated duration, e.g. "1 simulated hour".

**Simulated latency** (configurable)
- Every message between the bot and the exchange takes simulated time to arrive, in both directions: orders and cancels going out; acks, fills and market data coming back.
- The event loop therefore holds an **in-flight message queue ordered by delivery time**. This must be built into the loop from the start, not added later.
- Configurable one-way delay; **default 1 ms each way**. Setting it to `0` gives instant delivery for comparison.
- Delay creates the effects the owner should be able to observe:
  - the bot gets picked off on stale quotes
  - a cancel loses the race against a fill
  - an order is acknowledged after the mid has already moved
- Optional (builder's call): random jitter on top of the fixed delay, seeded.

**Trading fees** (configurable)
- Every fill is charged a fee as a fraction of traded value: a **maker fee** for the resting order and a **taker fee** for the order that crossed the spread.
- **Defaults: maker 0.02%, taker 0.05%**, roughly a high-volume crypto exchange tier. Retail tiers are higher, around 0.1%.
- A **negative maker fee** is allowed and represents a rebate, which is how exchanges pay market makers for providing liquidity.
- Fees are computed in integer units (e.g. a fixed-point fraction of USDT), consistent with principle 5.

**Venue (`venue`)**
- `Venue` trait: submit order, cancel order, receive events. Shaped so that a websocket/REST testnet adapter can implement it later (initiative H1), which means it must allow for rejections and asynchronous acknowledgements.
- `SimVenue`: an in-process adapter over the simulated exchange.

**Order book (`book`)**
- The bot's own view of the market, rebuilt from the venue's market data. It is not shared with the exchange's internal book: the bot must only know what the market tells it.
- Baseline implementation: a `BTreeMap` per side is fine.

**Strategy (`strategy`)**
- `Strategy` trait: reacts to market data and own-order events and returns actions (place/cancel).
- **Simple market maker:**
  - quotes one bid and one ask around the mid price with a configurable spread
  - skews both quotes away from its inventory: when long, it lowers its prices to sell more and buy less
  - requotes when the mid moves or a fill happens

**Risk (`risk`)**
- Pre-trade checks before every order:
  - max absolute position
  - max order size
  - price sanity band around the mid
  - max open orders
- Breaching a hard limit (e.g. a loss limit) halts the bot. This is a minimal kill switch; the full version comes in initiative B.

**App (`app`)**
- Command-line options (or a small config file) for:
  - seed and simulated duration
  - spread, inventory skew and quote size
  - risk limits
  - one-way latency
  - maker and taker fees
- Runs the simulation and prints the summary.

### 5.2 Output: end-of-run summary

```
hft-lab run summary
  venue:            sim (in-process) · BTC/USDT
  seed:             42
  simulated time:   1h 00m
  latency:          1.000 ms each way
  fees:             maker 0.020% / taker 0.050%
  orders sent:      12,345   (rejected by risk: 3)
  fills:            1,234    (buy 610 / sell 624 · maker 1,200 / taker 34)
  volume:           1.234 BTC
  final inventory:  +0.012 BTC
  gross PnL:        +14.10 USDT (before fees)
  fees paid:        -2.32 USDT
  realized PnL:     +12.34 USDT (after fees)
  unrealized PnL:   -0.56 USDT (marked at final mid)
  total PnL:        +11.78 USDT
  max drawdown:     -4.20 USDT
  halted:           no
```

The venue, latency and fee lines are required, and PnL is shown both before and after fees. Other fields and formatting are the builder's call.

### 5.3 Testing

- Order book and matching engine unit tests: price-time priority, partial fills, cancels, the book never stays crossed.
- Property-based tests (`proptest`) for invariants on random order sequences.
- Determinism test: the same seed produces an identical summary.
- PnL accounting test on a hand-worked scenario, including maker/taker fees and a rebate case.
- Latency tests: with delay > 0, a cancel sent after a taker order is already in flight does not prevent the fill. With delay = 0, events arrive in the same simulated instant.

### 5.4 Done when

- `cargo run --release` with default options completes a simulated hour and prints the summary.
- All tests and `clippy` pass.
- Changing the spread or skew changes the PnL and inventory in ways the owner can reason about.
- Comparing latency 0 with 1 ms, and fees zero with defaults, shows visible PnL differences the owner can explain.

### 5.5 Who builds what

This is the owner's **first Rust project**. The work is split:

- **Claude builds the plumbing:**
  - workspace setup
  - shared types
  - the simulated exchange (matching engine, synthetic market, latency queue, fees)
  - `SimVenue`
  - the app wiring and summary
- **The owner builds the interesting parts:**
  - the **order book** (`book`)
  - the **market maker** (`strategy`)
  - the **risk checks** (`risk`)
- **For each owner-built part, Claude prepares the scaffolding first:**
  - the trait or struct signatures
  - stub bodies (`todo!()`)
  - a full test suite that acts as the spec ("make these tests pass")
  - a short guide covering what the component does, the Rust concepts it needs and pointers to reading
  - an optional reference solution kept out of the way, for comparison afterwards
- **Keep the Rust simple** in owner-facing code: concrete types before generics, owned data before lifetimes, no `unsafe`, and no macros beyond the standard ones.
- The owner can move items between the two lists at any time.
- **Stand-in implementations.** So that something runs end to end before the owner's parts exist, Claude provides minimal versions of book, strategy and risk. Examples: a top-of-book-only book, a fixed-spread quoter with no skew, risk that checks only max position. The owner then replaces each one with their own.

## 6. Phases 2–5: Initiatives

After phase 1, the work is organized into **four later phases**. Each phase groups initiatives that share a theme. Each **initiative** is a self-contained project:
- it ends with something runnable
- it must not force a rewrite of earlier work. It swaps implementations behind existing interfaces, or adds new ones.

**Every phase and initiative has its own extended vision and task breakdown in [`docs/initiatives/`](initiatives/README.md).** The sections below are summaries of those docs. Before an initiative starts, its doc is updated and turned into a detailed step-by-step plan.

### Phase map

```
Phase 0  Setup
   │
Phase 1  Baseline bot (sim exchange, latency, fees, market maker)
   │
Phase 2  Observability ............ A  Measurement
   │
Phase 3  Robustness ............... B  Resilience
   │                                F  Real market data replay
   │
Phase 4  Performance .............. C  Memory & data structures
   │      (any order)               D  Concurrency
   │                                E  Wire format
   │                                G  Linux tuning (optional)
   │
Phase 5  Real venues .............. H1 Crypto testnet venue
                                    H2 Stocks paper venue
```

| Phase | Initiative | Depends on | Rough size |
|---|---|---|---|
| 2 | A Measurement | Phase 1 | S–M |
| 3 | B Resilience | A | M–L |
| 3 | F Real market data replay | B | M |
| 4 | C Memory & data structures | A | M |
| 4 | D Concurrency | A | L |
| 4 | E Wire format | A | M |
| 4 | G Linux tuning | D | S–M (needs Linux) |
| 5 | H1 Crypto testnet venue | B | L |
| 5 | H2 Stocks paper venue | H1 | M |

Phases 4 and 5 can overlap: H1 only needs B, so connecting to a real testnet can happen before any performance work. Within phase 4, the order is free.

"Owner builds" in each breakdown below is a suggestion that keeps the owner on the parts that teach the most. It follows the §5.5 pattern: Claude provides the interface, stubs, tests and a guide.

---

### Phase 2: Observability

The goal is to make the system's speed and behavior visible, so every later change can be judged by numbers.

#### A. Measurement

- **Goal:** know where time goes and how it varies, before optimizing anything.
- **You'll learn:**
  - the difference between latency and throughput
  - percentiles and tail latency
  - monotonic clocks
  - how benchmarking can lie (warm-up, dead-code elimination, noise)
- **Scope:**
  - a **wall-clock run mode** alongside simulated time: the bot processes events as fast as it can, and real elapsed time is measured
  - **stage timestamps**: market data received → book updated → strategy decided → risk checked → order sent
  - **latency histograms** per stage and end to end (`hdrhistogram`), printed in the summary as p50, p99, p99.9 and max
  - **micro-benchmarks** (`criterion`) for book update, matching and strategy decision
  - a **baseline report** saved in `docs/benchmarks/`, so later initiatives compare against it
- **Out of scope:** making anything faster.
- **Owner builds:** the stage timestamping and histogram reporting.
- **Done when:** a wall-clock run prints per-stage latency percentiles; benchmarks run with `cargo bench`; the baseline numbers are committed.

---

### Phase 3: Robustness

The goal is to make the system behave safely when things go wrong, and to feed it real data.

#### B. Resilience

- **Goal:** the bot notices when its view of the market is stale or wrong, and fails safe.
- **You'll learn:**
  - failure modes of trading systems
  - sequence numbers
  - snapshot and incremental recovery
  - event sourcing
  - deterministic replay
- **Scope:**
  - **staleness guard**: stop quoting when market data is older than a threshold; resume when fresh
  - **full kill switch**: triggers on loss limit, drawdown, abnormal fill rate, or a manual signal; cancels everything and halts
  - **sequence numbers** on all market data, with **gap detection**: on a gap, stop, request a snapshot, rebuild the book, resume
  - **fault injection** in the simulator: dropped messages, delayed bursts, disconnects
  - an **append-only event log** of every input, plus a **replay mode** that reproduces a run's exact decisions
  - **backpressure**: when market data arrives faster than it can be processed, merge updates instead of processing a backlog
- **Out of scope:** failover to a second instance (possible later extension).
- **Owner builds:** the staleness guard, the kill switch triggers, and gap detection in the order book.
- **Done when:**
  - with fault injection on, the bot never trades on a known-bad book
  - replaying a logged run produces an identical summary
  - every kill switch trigger is covered by a test

#### F. Real market data replay

- **Goal:** test the bot against real market behavior instead of the synthetic market.
- **You'll learn:**
  - what real order book data looks like
  - websocket feeds
  - data recording and normalization
  - why backtests on recorded data still mislead (your orders don't affect the recorded market)
- **Scope:**
  - a **recorder** tool that connects to a public crypto websocket depth stream (e.g. Binance BTC/USDT), takes snapshots and updates, and saves them to disk in a compact format
  - a **replay source** that feeds recorded data into the simulator in place of the synthetic market, keeping the latency and fee models
  - a simple **fill model** for the bot's orders against recorded data (e.g. fill when the market trades through your price), with its limitations documented
- **Out of scope:** placing orders on a real venue (that's H1); historical data purchases.
- **Owner builds:** the recorder tool.
- **Done when:** an hour of real BTC/USDT data is recorded and the bot runs on it, printing the usual summary.

---

### Phase 4: Performance

The goal is to make the hot path fast and predictable. Every initiative here must show before/after numbers against A's baseline.

#### C. Memory & data structures

- **Goal:** remove allocation and pointer-chasing from the hot path.
- **You'll learn:**
  - the CPU cache hierarchy
  - heap vs stack
  - allocation cost
  - data-oriented design
  - why `BTreeMap` is slower than an array for a price ladder
- **Scope:**
  - an **array/ladder-based order book** (price level = array index around the mid), behind the same interface and tests as the phase 1 book
  - **preallocated pools** for orders and events; zero allocations per event on the hot path, verified by a counting allocator in tests
  - **cache-friendly layouts**: struct field ordering, `#[repr(align(64))]` where threads share data
- **Out of scope:** threads (that's D).
- **Owner builds:** the ladder-based order book.
- **Done when:** the new book passes all existing book tests; the hot path is allocation-free; benchmarks show the improvement against baseline.

#### D. Concurrency

- **Goal:** split the pipeline across threads without adding jitter.
- **You'll learn:**
  - threads in Rust
  - `Send`/`Sync`
  - atomics and memory ordering
  - lock-free single-producer/single-consumer queues
  - false sharing
  - busy-spinning vs blocking
- **Scope:**
  - split into **feed thread** (decode + book), **strategy thread** (decide + risk) and **gateway thread** (send/receive orders), connected by **SPSC ring buffers**
  - first with an existing crate (`rtrb`), then a **hand-written ring buffer** for learning, tested heavily and checked with `loom` or Miri
  - a **logging thread**, so the hot path never touches I/O
  - core pinning *attempted* on macOS (hint only); measured properly in G
- **Out of scope:** multiple instruments, multiple strategies.
- **Owner builds:** the hand-written SPSC ring buffer.
- **Done when:** the threaded pipeline produces the same results as single-threaded on the same seed; latency and jitter are measured and compared against the baseline.

#### E. Wire format

- **Goal:** replace in-memory Rust structs between exchange and bot with a real binary protocol, as real feeds use.
- **You'll learn:**
  - how ITCH/SBE-style protocols are laid out
  - endianness
  - fixed-size messages
  - zero-copy decoding
  - parsing safely in Rust
- **Scope:**
  - define a small **binary protocol** for market data and order messages: fixed-size, versioned, with sequence numbers (reusing B's)
  - the exchange **encodes**; the bot **decodes** with zero-copy (`zerocopy`/`bytemuck`)
  - optional: send it over a real local **UDP** socket instead of in memory, to see real OS networking cost
  - fuzz testing of the decoder (`cargo fuzz`)
- **Out of scope:** implementing a real exchange's protocol in full.
- **Owner builds:** the decoder.
- **Done when:** the bot runs over the binary protocol with identical results; the decoder survives fuzzing; decode cost is benchmarked.

#### G. Linux tuning (optional)

- **Goal:** see how much OS-level tuning matters, which macOS can't show.
- **You'll learn:**
  - CPU isolation
  - interrupts
  - CPU frequency scaling
  - how real HFT servers are configured
- **Scope:**
  - run on a Linux machine or VM with `isolcpus`, real core pinning, IRQ affinity, the CPU frequency governor set to `performance`
  - compare latency histograms with the macOS numbers
  - document the setup steps
- **Out of scope:** kernel bypass and special network cards.
- **Owner builds:** the machine setup (it's sysadmin work, which is a good learning experience).
- **Done when:** a comparison table of macOS vs tuned Linux latency is committed.

---

### Phase 5: Real venues

The goal is to connect the same bot to real markets, using paper money only.

#### H1. Crypto testnet venue

- **Goal:** run the market maker against a real exchange's testnet with fake money.
- **You'll learn:**
  - exchange APIs (REST + websocket)
  - API key authentication and request signing
  - rate limits
  - reconnects
  - order rejections
  - how real latency over the internet compares to the simulator
- **Scope:**
  - a `Venue` implementation for a crypto testnet (e.g. Binance Spot Testnet): market data stream, place/cancel orders, order and fill updates
  - **safety guard**: refuse to start unless the endpoint is a known testnet URL; API keys come from environment variables and are never committed
  - **reconnect and resync** using B's recovery logic
  - **rate limit handling**: stay under the exchange's limits; back off on errors
  - a summary that shows `venue: binance-testnet · BTC/USDT` and the real measured round-trip latency
- **Out of scope:** mainnet / real money; multiple exchanges at once.
- **Owner builds:** the request signing and rate limiter.
- **Done when:** the bot runs for an hour on the testnet, survives a forced disconnect, and prints the summary.

#### H2. Stocks paper venue

- **Goal:** run the same bot on US stocks with a paper trading account, and learn what's different from crypto.
- **You'll learn:**
  - market hours and auctions
  - stock tick-size rules
  - how a broker sits between you and the exchange
  - why the regulatory picture is different
- **Scope:**
  - a `Venue` implementation for a stock paper-trading API (e.g. Alpaca paper), for one liquid stock or ETF
  - **session handling**: only quote during market hours; flatten the position before the close
  - instrument rules: tick size, lot size, and short-selling constraints
  - the same safety guard as H1 (paper endpoint only)
- **Out of scope:** real-money trading; regulatory registration.
- **Owner builds:** the session-handling logic.
- **Done when:** the bot trades a full paper session and the summary shows `venue: alpaca-paper · <ticker>`.

## 7. Decisions log

**Decided by the owner**
- Language: Rust. Repo: `hft-lab` in Personal_Workspace.
- Structure: setup → simple bot → optimization layers as separate initiatives.
- Something must run end to end; phase 1 shows an end-of-run summary that includes the venue it is connected to.
- Real venues: crypto first, stocks later. Paper/testnet only.
- Mixed build: Claude does the plumbing; the owner writes some features. This is their first Rust project.
- Configurable simulated latency between bot and exchange.
- Configurable maker/taker trading fees.

**Claude's calls (open to change)**
- Owner builds book, strategy and risk; Claude builds the rest (§5.5).
- Latency default of 1 ms each way; fee defaults of maker 0.02% and taker 0.05%.
- Market making as the phase 1 strategy.
- BTC/USDT as the simulated instrument.
- Simulated time (not wall-clock) in phase 1; wall-clock arrives with initiative A.
- Synthetic market with random-walk fair price, background liquidity and partly informed takers.
- Crate layout in §4.
- Measurement (A) recommended as the first initiative.
- Grouping of initiatives into phases 2–5 (Observability, Robustness, Performance, Real venues), the rough sizes, and the "owner builds" suggestions for each initiative.
- `docs/VISION.md` + `docs/initiatives/` as the doc layout, with every phase and initiative documented up front (owner's request), named `P<phase>-<id>-<name>.md`.

## 8. Left to the builder

- Exact crate boundaries, type names and trait signatures.
- CLI library (or none) and config format.
- Synthetic market parameters (volatility, arrival rates, informed fraction), as long as defaults produce a bot that trades meaningfully.
- Exact tick and lot sizes.
- Summary formatting.
