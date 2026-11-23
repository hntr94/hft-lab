# C: Memory & data structures

Phase 4 (Performance) · Depends on: A · Size: M

## Vision

The P1 baseline is simple and correct but wasteful: `BTreeMap` nodes scattered across the heap, allocations per event, structs laid out without thought. This initiative reshapes data for the CPU. That means contiguous memory, no allocation on the hot path, and layouts that fit in cache. Every improvement is proved with A's benchmarks.

**Success looks like:** a new order book passes every test the old one passed, and is measurably faster. The hot path allocates zero bytes per event, proven by a test. The baseline comparison shows the gains.

**You'll learn:**
- how the CPU cache hierarchy works (L1/L2/L3, cache lines, prefetching)
- why pointer-chasing is slow
- heap vs stack
- the cost of allocation
- data-oriented design
- struct layout and padding
- custom allocators for testing

## Key concepts

- **Cache line:** the CPU loads memory in 64-byte chunks. Data used together should live together.
- **Pointer chasing:** tree and linked-list nodes live at random addresses, so each step is a likely cache miss (~100 ns from RAM vs ~1 ns from L1).
- **Price ladder:** prices cluster near the mid, so the book can be a fixed array indexed by `(price - base) / tick`, re-centered occasionally. This gives O(1) access and contiguous memory.
- **Object pool / slab:** allocate all order slots at startup and reuse them. There is no `malloc`/`free` during trading.
- **Counting allocator:** a test-only global allocator that counts allocations, to prove "zero allocations per event".

## Design sketch

- `LadderBook` implements the same `OrderBook` interface as the P1 `BTreeMap` book. A cargo feature or CLI flag chooses between them.
- The ladder is a fixed-size array of levels around a moving center, with an overflow strategy for prices outside the window (re-center, or a fallback map).
- Pools for orders and events in the exchange and bot; IDs index into pools instead of `Box`es.
- A hot-path audit using A's flamegraphs: find and remove `String`, `format!`, `Vec` growth, `clone()` of large data, and `HashMap` with the default hasher.
- A test with a counting allocator asserts zero allocations across N steady-state events.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| C.1 | Guide: caches, memory layout, data-oriented design; reading list | [Claude] | none |
| C.2 | Experiment: benchmark summing a `Vec` vs a linked list vs a `BTreeMap`, to see cache effects firsthand | [You] | C.1, A |
| C.3 | Hot-path audit with the flamegraph: list every allocation and hot spot | [Together] | A |
| C.4 | Counting allocator + "zero allocations in steady state" test (expected to fail at first) | [Claude] | C.3 |
| C.5 | Scaffolding for `LadderBook`: reuse the book tests; add re-centering tests; guide | [Claude] | P1 |
| C.6 | **`LadderBook`** implementation | [You] | C.5 |
| C.7 | Benchmark `LadderBook` vs `BTreeMap` book; record the results | [You] | C.6 |
| C.8 | Order/event pools in the exchange and bot; remove the remaining hot-path allocations | [Claude] | C.4 |
| C.9 | Struct layout review: field ordering, sizes (`std::mem::size_of`), padding | [Together] | C.8 |
| C.10 | Re-run A's full measurement; update `docs/benchmarks/` with a before/after comparison | [You] | C.7, C.8 |

## Pitfalls

- Optimizing without measuring, or measuring in debug builds.
- A ladder that's too small, so it's constantly re-centering, or too big, so it doesn't fit in cache. Tune it with benchmarks.
- Breaking determinism or correctness for speed. The book tests and summary must match exactly.
- Micro-benchmark wins that don't show up end to end. Report both.

## Done when

- [ ] `LadderBook` passes all existing book tests and is selectable by flag.
- [ ] The zero-allocation test passes for the steady-state hot path.
- [ ] Simulation summaries are identical between the old and new implementations on the same seed.
- [ ] The before/after report is committed, with the owner's explanation of the gains.
