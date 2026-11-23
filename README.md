# hft-lab

A learning project: build the core machinery of a high-frequency trading (HFT) system in Rust, run it against a simulated exchange, and measure it.

**Goal:** understand how HFT systems work end to end, including market data, order books, strategy, risk, matching and latency, by building and measuring each piece.

**Non-goal:** competing with real HFT firms using real money. They rely on colocation, direct exchange feeds, microwave links and FPGAs, and win on microseconds. This project copies their architecture to learn from it, not their edge.

---

## 1. What HFT is

HFT firms trade very often, hold positions for seconds or less, and make a tiny profit per trade repeated millions of times. Speed matters because the first to react to new information gets the trade.

### Strategy families

| Strategy | Idea | Main risk |
|---|---|---|
| **Market making** | Quote a buy (bid) and a sell (ask) price at the same time, e.g. bid $100.00 / ask $100.02. Earn the spread when both sides fill. | *Inventory risk*: price moves against you before the other side fills. *Adverse selection*: faster traders pick off your stale quotes. |
| **Arbitrage** | Same asset trades at different prices on two venues. Buy cheap, sell expensive. | Someone faster gets there first; one leg fills, the other doesn't. |
| **Short-term signals** | Read the order book to predict the next tick, e.g. heavy buy-side imbalance suggests the price is about to rise. | Signal decays in milliseconds; noise. |

**Starting point for this project:** market making against a simulated exchange. It teaches order books, matching, spread, inventory risk, and why latency matters.

### Glossary

- **Order book**: the list of resting buy orders (bids) and sell orders (asks) at each price level.
- **Best bid / best ask (top of book)**: the highest bid and lowest ask. The gap between them is the **spread**.
- **Limit order**: buy/sell at a given price or better; may rest in the book.
- **Market order**: buy/sell immediately at the best available price.
- **Fill**: an order (partly) executed.
- **Matching engine**: the exchange component that pairs buy and sell orders, usually by price-time priority (best price first, then earliest order).
- **Tick**: the smallest allowed price increment.
- **Inventory / position**: how much of the asset you currently hold (positive = long, negative = short).
- **Latency**: time from an event (e.g. a market data update) to your reaction (e.g. an order on the wire). Measured in micro- or nanoseconds.
- **Tail latency**: worst-case latency (p99, p99.9). It matters more than the average, because the slow cases are when you lose money.

---

## 2. System overview

```
 ┌──────────────┐   market data   ┌────────────┐   book state   ┌──────────┐
 │  Simulated   │ ──────────────▶ │ Feed       │ ─────────────▶ │ Strategy │
 │  exchange    │                 │ handler +  │                │ (market  │
 │  (matching   │                 │ order book │                │  maker)  │
 │   engine)    │                 └────────────┘                └────┬─────┘
 │              │                                                    │ orders
 │              │   fills / acks   ┌────────────┐   checked orders   ▼
 │              │ ◀─────────────── │ Order      │ ◀──────────── ┌──────────┐
 │              │ ───────────────▶ │ gateway    │               │ Risk     │
 └──────────────┘                  └────────────┘               │ checks   │
                                                                └──────────┘
        everything timestamped ──▶ latency histograms + event log (replay)
```

Planned components:

1. **Order book**: maintains bids/asks per price level from market data updates.
2. **Matching engine**: the simulated exchange, using price-time priority.
3. **Feed handler**: decodes market data (binary format) and updates the book.
4. **Strategy**: a simple market maker that quotes around the mid price and skews quotes based on inventory.
5. **Risk layer**: pre-trade checks and a kill switch.
6. **Order gateway**: sends orders and receives acks and fills.
7. **Instrumentation**: timestamps at every stage, latency histograms, an event log for deterministic replay.
8. **Market data replay** (later): feed recorded real data, e.g. public crypto order book data.

---

## 3. Keeping latency low

The **critical path**, also called the **hot path**, runs from market data in, to decision, to order out. Everything on it must be fast and predictable.

| Where time goes | Technique | In this project? |
|---|---|---|
| Network | Colocation; kernel bypass (DPDK, Solarflare/OpenOnload) so packets skip the OS; UDP multicast feeds | No (needs special hardware/venues) |
| Data handling | Binary protocols (ITCH, SBE) instead of JSON; zero-copy parsing | Yes |
| Memory | Preallocate everything at startup; no allocation on the hot path; cache-friendly layouts (arrays over linked lists or pointer chasing) | Yes |
| Threads | Pin each thread to a dedicated CPU core; busy-spin instead of sleeping; lock-free single-producer/single-consumer ring buffers between threads ("LMAX Disruptor" pattern) | Yes |
| Code path | Short, branch-light hot path; no logging, disk I/O, locks or syscalls on it; push those to other threads | Yes |
| Hardware | FPGAs running strategy logic on the network card (<1 µs) | No |

**Why Rust:** no garbage collector (no surprise pauses), control over memory layout, zero-cost abstractions, and the compiler catches data races, which helps with lock-free code. See [section 6](#6-rust-for-low-latency) for details.

---

## 4. Resilience: surviving latency spikes and failures

Fast isn't enough. The system must notice when it's slow or wrong, and fail safe.

- **Timestamp everything**: on receive, after decode, at decision, on send. This lets you see where time goes and when you're lagging.
- **Staleness guards**: if market data is older than a threshold, stop quoting. Trading on an outdated view of the market is the classic way to lose money.
- **Pre-trade risk checks** (on the hot path, must be cheap): max position, max order size, max orders per second, price sanity band (reject prices far from the market, the "fat finger" check).
- **Kill switch**: cancel all orders and halt immediately on loss limit breach, disconnect, abnormal fill rate or check failure.
- **Cancel-on-disconnect**: the exchange pulls all your resting orders if your session drops, so you're never quoting blind.
- **Sequence-number gap detection**: every feed message carries a sequence number. A gap means a missed message, so your book is wrong. Stop trading, recover from a snapshot, then resume.
- **Deterministic replay**: log every input event. Replaying the log must reproduce the exact same decisions, which lets you debug a bad session precisely.
- **Hot standby / failover**: a second instance mirrors state and takes over if the primary dies.
- **Backpressure**: if a queue fills up, drop or coalesce stale market data rather than processing old updates late.

---

## 5. Measuring

- Report latency as **histograms** (p50 / p99 / p99.9 / max), not averages. HdrHistogram is the standard tool.
- Use a monotonic, high-resolution clock (TSC / `Instant`), not wall-clock time.
- Benchmark components in isolation (e.g. `criterion`) and end to end.
- Watch for **jitter** sources: page faults, CPU frequency scaling, context switches, cache misses, allocator calls.

---

## 6. Rust for low latency

### Toolchain setup

Install with [rustup](https://rustup.rs), the official installer and version manager:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"        # or restart the shell
rustc --version && cargo --version
```

This installs:
- `rustc`, the compiler
- `cargo`, the build tool and package manager
- `clippy`, a linter
- `rustfmt`, a formatter
- the docs, readable offline with `rustup doc`

Everyday commands:

| Command | What it does |
|---|---|
| `cargo build --release` | Optimized build; always use release mode for latency numbers |
| `cargo run --release` | Build and run |
| `cargo test` | Run tests |
| `cargo bench` | Run benchmarks (with `criterion`) |
| `cargo clippy` | Lint; catches common mistakes and slow patterns |
| `cargo fmt` | Format code |

### Rust features that matter here

| Concept | Why it matters for HFT |
|---|---|
| **Ownership and borrowing** | Memory is freed deterministically, with no garbage collector, so there are no pause spikes in tail latency. |
| **`Send` / `Sync`** | The compiler refuses to share data across threads unsafely, so data races are compile errors. |
| **Preallocation** (`Vec::with_capacity`, fixed-size arrays, slabs/arenas) | Lets you avoid heap allocation on the hot path. |
| **`#[repr(C)]`, `#[repr(align(64))]`** | Control struct layout; align to 64-byte cache lines to avoid *false sharing* between threads. |
| **Atomics** (`std::sync::atomic`, `Ordering::Acquire/Release`) | The building blocks of lock-free queues. |
| **`unsafe`** | Needed in a few low-level spots (custom ring buffers, zero-copy casts). Keep it small, isolated and tested. |
| **Zero-cost abstractions** (iterators, generics, traits) | High-level code compiles to the same machine code as hand-written loops. Prefer generics over `dyn Trait` on the hot path, since dynamic dispatch adds an indirect call. |
| **Integers for prices** | Represent prices as integer ticks (`i64`), never floats. Floats round, and equality comparisons break. |

Avoid on the hot path:
- `async`/`tokio`. It's fine for slow paths like config, admin and logging, but adds scheduling overhead and jitter.
- `Mutex`.
- `println!`.
- `String` formatting.
- `Box`/`Vec` growth.
- `HashMap` with the default hasher (SipHash is slow; use `FxHash`/`ahash` or arrays indexed by price level).

### Release profile

Add this to `Cargo.toml` for maximum performance:

```toml
[profile.release]
opt-level = 3
lto = "fat"          # whole-program optimization
codegen-units = 1    # better optimization, slower build
panic = "abort"      # no unwinding machinery
debug = true         # keep symbols for profiling
```

Optionally tune for the local CPU: `RUSTFLAGS="-C target-cpu=native" cargo build --release`.

### Useful crates

| Crate | Use |
|---|---|
| `rtrb` / `crossbeam` | Lock-free SPSC ring buffer / general concurrent queues and channels |
| `core_affinity` | Pin threads to CPU cores |
| `quanta` | Fast, high-resolution timestamps (TSC-based) |
| `hdrhistogram` | Latency histograms (p50/p99/p99.9) |
| `criterion` | Statistically sound micro-benchmarks |
| `zerocopy` / `bytemuck` | Safely reinterpret binary buffers as structs (zero-copy decoding) |
| `rustc-hash` | Fast non-cryptographic hasher (`FxHashMap`) |
| `proptest` | Property-based tests, e.g. "order book never crosses" |

### Profiling

- **macOS:** Instruments (`xcrun xctrace`) or [`samply`](https://github.com/mstange/samply) (`cargo install samply`).
- **Linux:** `perf` plus `cargo flamegraph`.

### macOS caveats

This project is developed on an Apple Silicon Mac, which limits some techniques:

- **No hard core pinning.** macOS only offers thread affinity *hints*, which Apple Silicon ignores. The OS decides where threads run. You can raise thread QoS to prefer performance cores, but true isolation (`isolcpus`, `taskset`, IRQ steering) needs Linux.
- **Mixed P-cores and E-cores.** Latency differs depending on which core a thread lands on, so expect more jitter.
- **Expect noisier numbers** than a tuned Linux server would give. The techniques still apply, and relative improvements are still measurable.

For serious latency work later, a cheap Linux box or VM with pinned cores is the next step.

---

## Development

**Prerequisites:** [rustup](https://rustup.rs). The pinned toolchain (stable, with clippy and rustfmt) installs automatically from `rust-toolchain.toml`.

**Commands** (run from the repo root):

| Target | What it does |
|---|---|
| `make check` | `cargo fmt --check`, clippy with `-D warnings`, then all tests |
| `make test` | `cargo test --workspace` |
| `make run` | `cargo run --release -p app --` (pass CLI flags after `--`) |
| `make exercise-book` / `exercise-strategy` / `exercise-risk` | Run that crate's `tests/mine.rs`, including `#[ignore]`d tests |

**Crate layout** (`crates/`):

- `types`: `Price`, `Qty`, `Side`, `OrderId`, `Timestamp`, instrument constants and shared events.
- `book`: the limit order book (owner-built).
- `exchange`: matching engine, synthetic market, latency and fees.
- `venue`: the `Venue` trait and `SimVenue`, the bot's only view of the exchange.
- `strategy`: the `Strategy` trait and the market maker (owner-built).
- `risk`: pre-trade checks (owner-built).
- `app`: binary with CLI, wiring and the run summary.

**Dependency graph** (crates depend only downward):

```
          app
   ┌───┬──┼────┬──────┐
strategy risk venue  exchange
   │    │    │  └──────┤
  book  │    │         │
   └────┴────┴──types──┘
```

## 7. Roadmap (draft)

> The authoritative plan (phases, initiatives, decisions) lives in [docs/VISION.md](docs/VISION.md). This section is a quick overview.

Every step ends with something runnable (`cargo run --release`), and the final goal is a full end-to-end demo.

1. Order book + matching engine with tests.
2. Market maker strategy against the simulator. **First end-to-end run:** start the simulator, run the bot, print profit/loss and fills.
3. Risk checks + kill switch.
4. Split into threads connected by lock-free queues; add latency instrumentation (histograms printed at exit).
5. Event log + deterministic replay.
6. Replay real recorded market data.
7. **Final demo:** a single command runs the simulated exchange, the market maker, risk and instrumentation together. It shows a live or summary dashboard with profit/loss, inventory, fill rate and latency percentiles.

## Further reading

- *Trading and Exchanges*, Larry Harris: how markets work.
- *Algorithmic and High-Frequency Trading*, Cartea, Jaimungal & Penalva: strategy maths, including market making.
- LMAX Disruptor paper: low-latency inter-thread messaging.
- Nasdaq TotalView-ITCH specification: a real binary market data protocol.
