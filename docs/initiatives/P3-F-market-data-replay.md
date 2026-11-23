# F: Real market data replay

Phase 3 (Robustness) · Depends on: B · Size: M

## Vision

The synthetic market is useful but made up. Here we record **real** BTC/USDT order book data from a public crypto exchange and run the bot against it. This is the first contact with real market behavior, with no account and no money needed.

**Success looks like:** a recorder captures an hour of real order book data to disk. The bot runs on the recording with the same latency and fee models and prints the usual summary. You understand why the result still flatters the strategy.

**You'll learn:**
- what real order book data looks like (bursts, quiet periods, spoofy liquidity)
- websocket market data feeds
- snapshot + diff-stream synchronization (B's recovery logic, now on real data)
- data normalization and storage
- the limits of backtesting

## Key concepts

- **Depth stream:** exchanges publish book changes (diffs) over websocket, and a REST endpoint gives a full snapshot. To build a correct book, you buffer diffs, fetch a snapshot, then apply only the diffs that come after it. This is the same idea as B's recovery.
- **Normalization:** convert the exchange's format (JSON strings for prices) into our integer `Price`/`Qty` types, once, at record time.
- **Market impact problem:** in a replay, your orders don't change what other traders did. A real market would have reacted to you. Replays are optimistic.
- **Fill model:** we have no real matching against our orders, so we approximate. A resting buy at price P fills when trades occur at or below P. Queue position is unknown, so assumptions are needed.

## Design sketch

- `recorder` binary (new crate):
  - connects to a public websocket depth stream and trade stream, e.g. Binance `btcusdt@depth@100ms` and `btcusdt@trade`
  - syncs with a REST snapshot
  - writes normalized events to a compact binary file, reusing B's event log format
- `ReplaySource` in the exchange crate: replaces the synthetic market. It replays recorded book and trade events on the sim clock, keeping the latency and fee models.
- **Fill model** for the bot's own orders against recorded trades, configurable between optimistic (front of queue) and pessimistic (back of queue).
- A small library of recordings in a git-ignored `data/` folder (they're large). Their metadata (date, duration, size) is committed.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| F.1 | Guide: real feed structure, snapshot+diff sync, backtest limitations; reading list | [Claude] | none |
| F.2 | Explore the exchange's public market data docs together; pick streams and endpoints | [Together] | F.1 |
| F.3 | Scaffolding for the recorder: interface, tests using canned JSON messages, guide (async Rust and websockets) | [Claude] | F.2 |
| F.4 | **Recorder**: websocket connection, snapshot sync, normalization, writing to disk, reconnect on drop | [You] | F.3, B |
| F.5 | Recording validation tool: checks sequence continuity and that the book is never crossed | [Claude] | F.4 |
| F.6 | `ReplaySource`: drive the sim clock from recorded events | [Claude] | F.4 |
| F.7 | Fill model for the bot's orders (optimistic / pessimistic) | [Together] | F.6 |
| F.8 | Summary shows `venue: replay(<file>) · BTC/USDT` and the fill model used | [Claude] | F.6 |
| F.9 | Record at least one hour; run the bot; compare with synthetic market results and with the two fill models | [You] | F.7 |
| F.10 | Write-up: how real data differs from the synthetic market; tune synthetic defaults to look more realistic | [You] | F.9 |

## Pitfalls

- Applying diffs without proper snapshot sync, which gives a silently wrong book.
- Storing raw JSON: huge files, and parsing happens again on every replay. Normalize once.
- Committing gigabytes of data to git.
- Treating backtest PnL as real. Always report it next to the fill model assumption.
- Exchange terms of use and rate limits. Use public endpoints politely.

## Done when

- [ ] One hour or more of real BTC/USDT data is recorded and passes validation.
- [ ] The bot runs on it with latency and fees and prints the summary.
- [ ] Results under both fill models are recorded and compared with the synthetic market.
