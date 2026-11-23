# H1: Crypto testnet venue

Phase 5 (Real venues) · Depends on: B · Size: L

## Vision

The bot leaves the simulator. A new `Venue` implementation connects to a real crypto exchange's **testnet**: real APIs, real order lifecycle, fake money. The strategy, book and risk code don't change. That's the payoff of the `Venue` interface from P1.

This is where everything that's easy to ignore in a simulator shows up:
- authentication
- rate limits
- rejected orders
- disconnects
- clocks that disagree
- latency of tens of milliseconds over the internet

**Success looks like:** the bot market-makes BTC/USDT on a testnet for an hour, survives a forced disconnect, respects rate limits, and prints the familiar summary with `venue: binance-testnet`. It is impossible to point it at a real-money endpoint by accident.

**You'll learn:**
- real exchange APIs (REST + websocket)
- API key authentication and HMAC request signing
- rate limiting (token buckets)
- order lifecycle on a real venue (new → partially filled → filled / cancelled / rejected / expired)
- reconnect and resync
- async Rust (`tokio`) at the edge of the system
- secrets handling
- real-world latency vs the simulator

## Key concepts

- **Testnet:** a copy of the exchange running with fake balances. It uses the same API shape as the real exchange, but liquidity and prices can be odd.
- **Request signing:** each private request carries a signature (an HMAC of the parameters with your secret key) and a timestamp. The exchange rejects stale or mis-signed requests.
- **Rate limits:** exchanges cap requests per time window (by weight, per IP or per account). Exceeding them can get you banned temporarily.
- **User data stream:** a websocket channel that pushes your own order updates and fills.
- **Client order IDs:** your own ID on each order, so you can match exchange updates to your intent even when replies are lost.
- **Async boundary:** network I/O uses `tokio`; the hot path stays synchronous. Messages cross the boundary over a channel.

## Design sketch

- New `venue-binance` crate (or similar) implementing `Venue`:
  - a websocket market data stream feeding the same book, with snapshot sync from B/F
  - REST endpoints for placing and cancelling orders, signed
  - a websocket user data stream for acks and fills
- **Safety guard:**
  - the endpoint must be on a hard-coded testnet allowlist, or startup aborts
  - keys come only from environment variables
  - the summary and logs never print secrets
- **Rate limiter:** a token bucket modeled on the exchange's published limits; it backs off on HTTP 429/418 responses.
- **Reconnect:** on drop, cancel-all as a precaution, resync the book, reconcile open orders through REST, then resume.
- **Clock sync:** measure the offset from the exchange's server time; use it for request timestamps.
- Summary shows the real measured round-trip latency (order sent → ack) using A's histograms.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| H1.1 | Guide: exchange API anatomy, signing, rate limits, async Rust basics; reading list | [Claude] | none |
| H1.2 | Create a testnet account and API keys; store them in a local `.env` that git ignores | [You] | none |
| H1.3 | Explore the testnet API by hand (`curl` / `websocat`): get a snapshot, place and cancel one order | [Together] | H1.2 |
| H1.4 | Safety guard: testnet allowlist, keys from env only, secrets redacted; tests prove a mainnet URL is refused | [Claude] | none |
| H1.5 | Scaffolding for signing + rate limiter: tests using the exchange's documented examples, guide | [Claude] | H1.1 |
| H1.6 | **Request signing (HMAC)** | [You] | H1.5 |
| H1.7 | **Token-bucket rate limiter** with backoff | [You] | H1.5 |
| H1.8 | Market data adapter: websocket depth + snapshot sync, reusing F's logic, feeding the book | [Claude] | F (or B) |
| H1.9 | Order adapter: place/cancel via signed REST; user data stream for acks/fills; client order ID mapping | [Claude] | H1.6, H1.7 |
| H1.10 | Reconnect + reconcile flow; forced-disconnect test | [Together] | H1.8, H1.9 |
| H1.11 | Clock offset measurement; real latency histograms in the summary | [Claude] | H1.9, A |
| H1.12 | Dry-run mode: connect, follow the market, log the orders it *would* send without sending them | [Claude] | H1.8 |
| H1.13 | First live testnet run: 10 minutes, tiny size, watched closely | [Together] | H1.10, H1.12 |
| H1.14 | One-hour testnet run; compare behavior with the simulator; write up the surprises | [You] | H1.13 |

## Pitfalls

- **Committing API keys.** Use `.env` in `.gitignore` from day one; run a secret scan before pushing.
- Testnet markets are thin and odd, with sudden jumps and empty books. Don't read strategy quality into testnet PnL.
- Forgetting that your simulated latency (1 ms) is far below real internet latency (20–200 ms). Expect much more adverse selection.
- Ignoring partial fills and order states the simulator never produced, e.g. "expired".
- Blocking the hot path on network I/O. Keep `tokio` at the edge.

## Done when

- [ ] Tests prove a non-testnet endpoint is refused at startup.
- [ ] One hour or more of live testnet market making completes, including surviving a forced disconnect.
- [ ] No rate-limit bans; rejections and partial fills are handled.
- [ ] The summary shows `venue: binance-testnet · BTC/USDT` plus real round-trip latency percentiles.
- [ ] The write-up compares the testnet run with the simulator.
