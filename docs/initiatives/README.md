# Initiatives

One document per phase or initiative. Each contains:

- **Vision**: why it exists, what success looks like, what you'll learn.
- **Key concepts**: background needed before starting.
- **Design sketch**: the shape of the solution. The builder refines details.
- **Task breakdown**: numbered tasks, each tagged with an owner and dependencies.
- **Pitfalls**: what usually goes wrong.
- **Done when**: acceptance criteria.

The overall plan, principles and decisions log live in [../VISION.md](../VISION.md).

## Owner tags

| Tag | Meaning |
|---|---|
| **[Claude]** | Claude implements it; the owner reviews, runs and asks questions. |
| **[You]** | The owner implements it. Claude first provides the interface, stubs (`todo!()`), a test suite as the spec, and a short guide. |
| **[Together]** | Pair on it: Claude drafts or explains, the owner decides and finishes. |

Tags are suggestions. Any task can change hands.

## Index

| Phase | Doc | Depends on |
|---|---|---|
| 0 | [P0 Setup](P0-setup.md) | none |
| 1 | [P1 Baseline bot](P1-baseline-bot.md) | P0 |
| 2 | [A Measurement](P2-A-measurement.md) | P1 |
| 3 | [B Resilience](P3-B-resilience.md) | A |
| 3 | [F Real market data replay](P3-F-market-data-replay.md) | B |
| 4 | [C Memory & data structures](P4-C-memory.md) | A |
| 4 | [D Concurrency](P4-D-concurrency.md) | A |
| 4 | [E Wire format](P4-E-wire-format.md) | A (B for sequence numbers) |
| 4 | [G Linux tuning](P4-G-linux-tuning.md) | D |
| 5 | [H1 Crypto testnet venue](P5-H1-crypto-testnet.md) | B |
| 5 | [H2 Stocks paper venue](P5-H2-stocks-paper.md) | H1 |

## Lifecycle of an initiative

1. **Before starting:** reread the doc and update it with anything learned since it was written. Write a detailed step-by-step implementation plan.
2. **During:** check off tasks; note decisions in the doc.
3. **After:** record results (benchmarks, lessons) in a "Results" section at the bottom of the doc.
