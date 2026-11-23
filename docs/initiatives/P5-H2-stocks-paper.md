# H2: Stocks paper venue

Phase 5 (Real venues) · Depends on: H1 · Size: M

## Vision

The same bot, a different world. Stocks trade only during market hours, open and close with auctions, follow stricter tick and short-selling rules, and reach the exchange through a broker. This initiative adds a stock paper-trading `Venue` and makes the bot session-aware. You'll see first-hand why crypto was the easier starting point.

**Success looks like:**
- The bot trades a full US market session on a paper account with one liquid ETF or stock (e.g. SPY).
- It stays flat outside market hours and closes its position before the bell.
- The summary shows `venue: alpaca-paper · SPY`.
- You can explain the key differences from crypto.

**You'll learn:**
- market sessions (pre-market, regular hours, after-hours)
- opening and closing auctions
- stock tick sizes and lot rules
- short selling and borrow
- how a broker sits between you and the exchange
- the regulatory landscape (Pattern Day Trader rule, Reg NMS, best execution), which paper trading avoids
- market data entitlements and delayed data

## Key concepts

- **Market hours:** US regular session is 9:30–16:00 ET. Outside it, liquidity is thin or absent. The bot must know the session calendar, including holidays and half-days.
- **Auctions:** the open and close are set by auctions, not continuous trading. Market makers usually avoid quoting into them at first.
- **Tick size:** generally $0.01 for stocks above $1. Fractional shares exist at some brokers but not on the exchange order book.
- **Short selling:** selling shares you don't own requires a borrow. Paper accounts may simplify this, but the bot should respect it.
- **Broker in the middle:** your orders go to the broker, which routes them to an exchange or wholesaler. There's an extra hop, plus broker-specific rules and rate limits.
- **Market data:** free tiers may be delayed or limited to one exchange (e.g. IEX only), not the full national picture.

## Design sketch

- A new `venue-alpaca` crate (or similar) implementing `Venue`, reusing H1's patterns: safety allowlist (paper endpoint only), env-var keys, rate limiter, reconnect and reconcile.
- A **session module**: a market calendar, the current session state, and rules such as no quoting in the first and last N minutes, and flatten by close.
- **Instrument rules** in `types`: tick size, lot size, shortable flag. These are loaded per instrument instead of hard-coded for BTC/USDT.
- Strategy tweak: market making with inventory limits that tighten toward the close.
- The summary adds session information: time quoted, flattened at close (yes/no).

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| H2.1 | Guide: market structure for stocks, sessions, auctions, brokers, regulation overview; reading list | [Claude] | none |
| H2.2 | Create a paper trading account and keys; add them to `.env` | [You] | none |
| H2.3 | Explore the paper API by hand: account, clock/calendar endpoint, quotes, one order | [Together] | H2.2 |
| H2.4 | Generalize instrument rules (tick, lot, shortable) so they're no longer BTC/USDT-specific | [Claude] | P1 |
| H2.5 | Venue adapter: market data, orders, order updates; safety guard (paper endpoint only) | [Claude] | H1, H2.4 |
| H2.6 | Scaffolding for session handling: tests with fixed clocks (pre-market, open, mid-day, near close, holiday), guide | [Claude] | H2.1 |
| H2.7 | **Session handling**: calendar, session state, quoting windows, flatten before close | [You] | H2.6 |
| H2.8 | Strategy adjustment: inventory limits tighten toward the close | [You] | H2.7 |
| H2.9 | Dry run during market hours (no orders sent) | [Together] | H2.5, H2.7 |
| H2.10 | Full-session paper run with tiny size | [You] | H2.9 |
| H2.11 | Write-up: crypto vs stocks, covering data, latency, rules, fills and PnL | [You] | H2.10 |

## Pitfalls

- Time zones: always compute sessions in exchange time (ET), not local time.
- Holidays and half-days; use the broker's calendar endpoint, not a hand-written list.
- Delayed or partial market data that makes the bot quote off stale prices. B's staleness guard matters here.
- Paper fills are simulated by the broker and often optimistic. Don't trust paper PnL.
- Accidentally pointing at a live endpoint. The safety guard is non-negotiable.

## Done when

- [ ] Tests prove a non-paper endpoint is refused.
- [ ] Session tests pass for all session states, including holidays.
- [ ] A full paper session completes; the position is flat at the close.
- [ ] The summary shows `venue: alpaca-paper · <ticker>` with session information.
- [ ] The crypto vs stocks write-up is committed.
