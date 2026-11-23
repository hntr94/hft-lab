# P0: Setup

Phase 0 · Depends on: none · Size: S

## Vision

A clean, well-shaped Rust workspace that every later phase builds on. Nothing trades yet, but the skeleton is in place:
- crates exist
- shared types are defined
- quality checks run with one command

Getting this right early prevents the "one big tangled program" anti-goal.

**Success looks like:** you can clone the repo, run one command, and see build, tests, lint and format checks all pass. You understand what each crate is for and how they depend on each other.

**You'll learn:**
- Cargo workspaces
- crates and modules
- `Cargo.toml`
- the dev and release profiles
- `clippy` and `rustfmt`
- how Rust organizes code into crates (`lib.rs`) and modules (`mod`)

## Key concepts

- **Workspace:** several crates sharing one `Cargo.lock` and `target/`. The root `Cargo.toml` lists members.
- **Library vs binary crate:** libraries (`lib.rs`) hold logic; one binary crate (`app`, `main.rs`) wires everything together.
- **Dependency direction:** crates may only depend "downward" (e.g. `strategy` → `types`, never `types` → `strategy`). This keeps components swappable.
- **`rust-toolchain.toml`:** pins the Rust version so builds are reproducible.

## Design sketch

```
hft-lab/
├── Cargo.toml              workspace root + shared [profile.release]
├── rust-toolchain.toml     stable, with clippy + rustfmt
├── .gitignore              /target
├── justfile (or Makefile)  check, test, run, bench shortcuts
└── crates/
    ├── types/      Price, Qty, Side, OrderId, Timestamp, events
    ├── book/       order book (owner-built in P1)
    ├── exchange/   matching engine + synthetic market + latency + fees
    ├── venue/      Venue trait + SimVenue
    ├── strategy/   Strategy trait + market maker (owner-built in P1)
    ├── risk/       pre-trade checks (owner-built in P1)
    └── app/        binary: CLI, wiring, summary
```

Dependency graph:

```
          app
   ┌───┬──┼────┬──────┐
strategy risk venue  exchange
   │    │    │  └──────┤
  book  │    │         │
   └────┴────┴──types──┘
```

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| 0.1 | Create workspace root `Cargo.toml` with members and the release profile from README §6 | [Claude] | none |
| 0.2 | Add `rust-toolchain.toml` (stable + clippy + rustfmt) and `.gitignore` | [Claude] | none |
| 0.3 | Create the 7 crates (`cargo new --lib` ×6, `--bin` ×1) with empty `lib.rs`/`main.rs` and correct inter-crate dependencies | [Together] | 0.1 |
| 0.4 | Define core types in `types`: `Price` and `Qty` as integer newtypes, `Side`, `OrderId`, `Timestamp` (nanoseconds, `u64`), `Instrument` (tick/lot size) | [Claude] | 0.3 |
| 0.5 | Define shared event and command enums in `types`: `NewOrder`, `Cancel`, `Ack`, `Reject`, `Fill`, `Cancelled`, `MarketData` | [Claude] | 0.4 |
| 0.6 | Add a `justfile` (or Makefile) with `check` (fmt + clippy + test), `run`, `bench` | [Claude] | 0.3 |
| 0.7 | Add a "Development" section to the README: prerequisites, commands, layout | [Claude] | 0.6 |
| 0.8 | Walkthrough session: owner reads every file, explains back the dependency graph and the newtype pattern | [You] | 0.5 |
| 0.9 | First commit on `main` | [Together] | 0.8 |

### Task notes

- **0.4:** Newtypes (`struct Price(i64)`) make it impossible to accidentally add a price to a quantity. This is a key Rust idiom and a good first lesson.
- **0.5:** Keep events plain data (no behavior), so they can later be logged (B) and encoded (E) without changes.

## Pitfalls

- Over-designing the types now. Add fields when a phase needs them, not before.
- Circular crate dependencies; Cargo rejects them. If two crates need the same thing, it belongs in `types`.
- Forgetting `--release` when timing anything later.

## Done when

- [ ] `just check` (or equivalent) passes: format, clippy with no warnings, tests.
- [ ] `cargo run -p app` prints a placeholder line.
- [ ] The owner can explain what each crate is for and why `types` sits at the bottom.
- [ ] Committed.
