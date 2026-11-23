# A: Measurement

Phase 2 (Observability) · Depends on: P1 · Size: S–M

## Vision

Before making anything fast, know how fast it is, where the time goes, and how much it varies. This initiative turns latency from a guess into numbers, and produces a **baseline** that every phase 4 initiative must beat.

**Success looks like:** a run prints a per-stage latency table (p50, p99, p99.9, max). Micro-benchmarks exist for the hot functions. A committed baseline report shows "this is how slow the simple version is".

**You'll learn:**
- latency vs throughput
- why averages hide problems and tail percentiles matter
- monotonic vs wall clocks and the cost of reading a clock
- how benchmarks lie (warm-up, dead-code elimination, CPU frequency changes, noise) and how to make them honest
- profiling with flamegraphs

## Key concepts

- **Wall-clock mode:** in P1, time is simulated, so code speed is invisible. Here we add a mode that processes events as fast as possible and measures real elapsed time per event. Simulated latency still drives the *trading* logic; real time measures the *code*.
- **Stages of the hot path:** event received → book updated → strategy decided → risk checked → order emitted. Each gets a timestamp.
- **Percentiles:** p99 = 99% of events were faster than this. In trading, the slow 1% is often when the market moves, and when it matters most.
- **HdrHistogram:** records latencies with fixed memory and precise percentiles; recording is cheap enough for the hot path.
- **Coordinated omission:** measuring only when the system is ready to measure hides queueing delays. Be aware of it; HdrHistogram has tools for it.

## Design sketch

- A `Clock` abstraction: the simulated clock (P1) and a real clock (`quanta` or `std::time::Instant`) behind one interface, so the code under test doesn't change.
- A `Probe` (or similar) that records stage timestamps for each event into per-stage histograms. It must be cheap: no allocation, no I/O.
- Histograms are printed at the end of the run in the summary, and optionally exported as CSV/JSON for comparison.
- `criterion` benchmarks in `crates/*/benches/` for: book update, matching one order, strategy decision, risk check.
- `docs/benchmarks/baseline.md` records the machine, the commit, and the numbers.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| A.1 | Guide: latency concepts, percentiles, benchmarking pitfalls; reading list | [Claude] | none |
| A.2 | `Clock` trait with sim and real implementations; wall-clock run mode flag | [Claude] | P1 |
| A.3 | Measure clock overhead: how many ns does reading the clock cost? | [You] | A.2 |
| A.4 | Scaffolding for stage instrumentation: interface, tests, guide | [Claude] | A.2 |
| A.5 | **Stage timestamps + per-stage `hdrhistogram`s** on the hot path | [You] | A.4 |
| A.6 | **Latency table in the summary** (p50/p90/p99/p99.9/max per stage + end to end) | [You] | A.5 |
| A.7 | `criterion` benchmarks for book, matching, strategy and risk | [Together] | P1 |
| A.8 | Profiling how-to: `samply` (macOS) / `cargo flamegraph` (Linux); first flamegraph of a run | [Together] | A.2 |
| A.9 | Benchmark hygiene checklist (release build, warm-up, quiet machine, plugged in, repeated runs) | [Claude] | A.7 |
| A.10 | **Baseline report**: machine specs, commit hash, histograms, benchmark results, and flamegraph observations, in `docs/benchmarks/baseline.md` | [You] | A.6, A.7, A.8 |
| A.11 | Optional: export histograms as JSON plus a script to compare two runs | [Claude] | A.6 |

## Pitfalls

- Timing in debug builds; numbers are meaningless.
- Measurement code that allocates or prints on the hot path, which distorts what it measures.
- Comparing runs from different machines, or from a laptop on battery power.
- Reporting averages only.
- Benchmarks that the compiler optimizes away. Use `std::hint::black_box`.

## Done when

- [ ] A wall-clock run prints a per-stage latency percentile table.
- [ ] `cargo bench` runs micro-benchmarks for the four hot components.
- [ ] Simulation results (PnL, fills) are unchanged by instrumentation.
- [ ] `docs/benchmarks/baseline.md` is committed, with the owner's notes on where time goes.
