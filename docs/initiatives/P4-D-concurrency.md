# D: Concurrency

Phase 4 (Performance) · Depends on: A · Size: L

## Vision

Real HFT systems split the hot path across dedicated threads: one reads the feed, one decides, one talks to the exchange. They are connected by lock-free queues, so no thread ever waits on a lock. This initiative builds that pipeline, including a hand-written lock-free ring buffer, and measures whether it actually helps. Sometimes it doesn't, and that is a lesson too.

**Success looks like:** the threaded pipeline produces exactly the same trading results as the single-threaded one on the same seed. You've written a lock-free single-producer/single-consumer queue that passes concurrency checkers. You can explain the latency and jitter differences with numbers.

**You'll learn:**
- threads in Rust
- `Send`/`Sync`
- `Arc`
- atomics and memory ordering (`Relaxed`, `Acquire`, `Release`)
- why locks hurt tail latency
- the LMAX Disruptor idea
- false sharing
- busy-spinning vs blocking
- how to test concurrent code (`loom`, Miri)

## Key concepts

- **SPSC ring buffer:** a fixed-size array with a head index (written by the producer) and a tail index (written by the consumer). Each side only writes its own index, so no locks are needed, only atomic loads and stores with the right ordering.
- **Memory ordering:** CPUs and compilers reorder memory operations. `Release` on a write plus `Acquire` on the matching read guarantees the consumer sees the data the producer wrote before it.
- **False sharing:** two threads writing different variables on the same 64-byte cache line slow each other down. The fix is padding (`#[repr(align(64))]`).
- **Busy-spin:** a thread loops checking the queue instead of sleeping. This gives lowest latency but burns a full core.
- **Determinism with threads:** thread scheduling is not deterministic, so the simulated-time mode stays single-threaded. The threaded pipeline runs in wall-clock mode, and is checked by comparing decisions on a recorded input.

## Design sketch

```
 exchange/sim ──▶ [feed thread] ──SPSC──▶ [strategy thread] ──SPSC──▶ [gateway thread] ──▶ exchange/sim
                   decode, book            decide, risk                 send, track acks
                        │                       │                            │
                        └───────────────────────┴────────SPSC──▶ [log/metrics thread] (off the hot path)
```

- The pipeline is built from the same components (book, strategy, risk), just placed on different threads. No component rewrites.
- Step 1 uses the `rtrb` crate for the queues. Step 2 swaps in the owner's own ring buffer behind the same interface.
- A wait strategy is configurable per thread: busy-spin, spin-then-yield, or block.
- Core pinning is attempted with `core_affinity`. On macOS this is only a hint (see README §6); real pinning comes in G.
- Equivalence check: feed the same recorded input (B's log) to the single-threaded and threaded pipelines, and compare their order decisions.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| D.1 | Guide: Rust threads, atomics, memory ordering, ring buffers, false sharing; reading list (incl. the LMAX paper and Mara Bos's *Rust Atomics and Locks*) | [Claude] | none |
| D.2 | Warm-up exercises: spawn threads, `Arc<Mutex>` counter, then an `AtomicU64` counter, then benchmark both | [You] | D.1 |
| D.3 | Queue interface (`push`/`pop`, fixed capacity) + `rtrb`-backed implementation | [Claude] | P1 |
| D.4 | Threaded pipeline (feed / strategy / gateway / log threads) on `rtrb` queues; configurable wait strategy | [Claude] | D.3, A |
| D.5 | Equivalence test: threaded and single-threaded produce identical decisions on a recorded input | [Claude] | D.4, B |
| D.6 | Scaffolding for the hand-written SPSC queue: tests (incl. `loom` model tests), guide | [Claude] | D.3 |
| D.7 | **Hand-written SPSC ring buffer** (atomics, cache-line padding, power-of-two capacity) | [You] | D.6, D.2 |
| D.8 | Run the queue tests under Miri and `loom`; fix what they find | [You] | D.7 |
| D.9 | Benchmark queue throughput and latency: own vs `rtrb` vs `std::sync::mpsc` | [You] | D.7 |
| D.10 | Measure the pipeline end to end: single-threaded vs threaded, wait strategies, pinning on/off, using A's histograms | [Together] | D.4, D.7 |
| D.11 | Write-up: when threading helped or hurt, and why | [You] | D.10 |

## Pitfalls

- Wrong memory ordering: works on your laptop, breaks elsewhere. Rely on `loom`/Miri, not on luck.
- Busy-spinning more threads than you have performance cores, especially on Apple Silicon's mixed cores.
- Expecting threads to always reduce latency. Handing data between cores costs time (~50–100 ns); a small single-threaded hot path can be faster.
- Breaking determinism. Keep the single-threaded sim mode as the reference.

## Done when

- [ ] The threaded pipeline passes the equivalence test against single-threaded.
- [ ] The owner's SPSC queue passes unit, `loom` and Miri tests, and replaces `rtrb` behind the interface.
- [ ] A latency and jitter comparison table is committed, with explanations.
