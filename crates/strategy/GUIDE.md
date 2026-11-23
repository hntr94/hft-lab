# Exercise: write your own market maker (`MarketMaker`)

You will fill in `on_update` in `crates/strategy/src/mine.rs`. Everything else already works.
Until you finish, the app uses `FixedSpreadQuoter`, a simple stand-in that quotes around the mid price
and ignores inventory.

## 1. What does a market maker do?

A market maker keeps a **buy order (bid)** and a **sell order (ask)** resting in the book at the same time, a little
below and a little above the fair price. When other traders hit those orders, the maker buys cheap and sells dear
and earns the gap.

Worked example. The book has best bid 9_990 and best ask 10_010, so the **mid** is 10_000. With a half spread of 5 ticks
the maker quotes:

- bid at 10_000 - 5 = **9_995**
- ask at 10_000 + 5 = **10_005**

If one trader sells to the bid and another buys from the ask, the maker bought at 9_995 and sold at 10_005. That is
10 ticks of profit per lot, minus fees. The risk is that only one side trades and the maker is left holding a
position while the price moves against it.

## 2. Inventory skew

Say the maker bought a lot and is now long. It would like to sell some, and it does not want to buy more.
So it **shifts both quotes down**: the ask gets closer to the market (easier to hit) and the bid gets further away
(harder to hit). If the maker is short, it shifts both quotes up.

The shifted centre is called the **reservation price**:

```
reservation = mid - skew_ticks * position / quote_qty
bid = reservation - half_spread_ticks
ask = reservation + half_spread_ticks
```

All of this is whole-number (`i64`) maths on the `.0` values, and `/` drops the fraction toward zero.
The tests use `half_spread_ticks: 5`, `quote_qty: Qty(100)`, `skew_ticks: 3` and mid 10_000.

| Position | skew = 3 * position / 100 | reservation | bid | ask |
| --- | --- | --- | --- | --- |
| 0 (flat) | 0 | 10_000 | 9_995 | 10_005 |
| 200 (long) | 6 | 9_994 | 9_989 | 9_999 |
| -200 (short) | -6 | 10_006 | 10_001 | 10_011 |

**Clamp.** A bid must never be at or above the best ask, and an ask never at or below the best bid, or the order would
trade immediately instead of resting. So after computing the quotes:

- if `bid >= best_ask`, set `bid = best_ask - 1`
- if `ask <= best_bid`, set `ask = best_bid + 1`

Example from the tests: book bid 9_999, ask 10_001, position -1000. Skew is 3 * -1000 / 100 = -30, so the
reservation is 10_030, the raw bid is 10_025 and the ask is 10_035. The bid would cross the best ask 10_001, so it is
clamped to 10_000. The ask 10_035 is fine.

## 3. Why cancel stale quotes?

Every time the book moves, the right quote price moves with it. An old order left at the old price is **stale**:
it may be too cheap to be worth filling, or so far from the market it never trades. A stale quote that does fill
can lose money straight away, because the market has already moved. So the maker cancels it and places a new one.

## 4. Why are cancels slow?

Every message takes time (latency) to reach the exchange. A cancel is only a request. In the meantime the order is
still live and can be filled, and the exchange may answer "too late, it already filled". That is why an order with
`cancel_pending` set must not be cancelled again, and must not count as a live quote: the cancel is already on its way.

## 5. The quoting rules

`on_update` gets a `StrategyContext` (`ctx.book`, `ctx.position`, `ctx.open_orders`) and returns a `Vec<Action>`.

1. If `ctx.book.mid()` is `None`, return an empty vec.
2. Compute `reservation`, `bid` and `ask` as in section 2.
3. Clamp as in section 2. Use `best_bid()` and `best_ask()`.
4. For each side, **Buy first, then Sell**:
   - A **live** order is one in `ctx.open_orders` on that side with `cancel_pending == false`.
     An order that has not been acknowledged yet (`acked == false`) still counts as live.
   - For every live order whose price is not the desired price, push `Action::Cancel(order.id)`.
   - If no live order sits at the desired price, push
     `Action::Place { side, price: desired, qty: quote_qty }`. Cancels come before the place for the same side.
5. Live orders already at the desired price need no action.

## 6. The Rust you need

- **`Vec<T>`**: a growable list. `let mut actions = Vec::new();` then `actions.push(x)`. Return it as the last expression.
- **Iterators**: `ctx.open_orders.iter()` walks the orders without changing them.
  - `.filter(|o| o.side == side && !o.cancel_pending)` keeps only the ones you want. The `|o| ...` part is a small
    unnamed function (a closure).
  - `.any(|o| o.price == desired)` answers "is there at least one?" with a `bool`.
  - Use them in a `for order in ... { }` loop, or call `.collect::<Vec<_>>()` to build a list.
- **`match` on enums**: `match side { Side::Buy => ..., Side::Sell => ... }`. You create actions with
  `Action::Place { side, price, qty }` and `Action::Cancel(id)`.
- **`Option`**: `ctx.book.mid()` returns `Option<Price>`. `let Some(mid) = ctx.book.mid() else { return Vec::new(); };`
  returns early when it is `None`.
- **Integer division**: for `i64`, `7 / 2` is `3` and `-7 / 2` is `-3` (it drops the fraction toward zero). Multiply before you divide:
  `skew_ticks * position / quote_qty`.
- **Prices**: `Price(10_000) - 5` gives `Price(9_995)`, and `Price - Price` gives an `i64`. `price.0` is the inner number.
  `OrderId` is `Copy`, so `order.id` can be used freely.

Open `crates/strategy/src/standin.rs` for a finished example. It does the same side-by-side work without skew or clamp.

## 7. Run the tests

From the repo root:

```
make exercise-strategy
```

The nine tests in `crates/strategy/tests/mine.rs` start out failing with `not yet implemented: exercise: ...`.
Work through them in order. When all nine pass, you are done. The normal `make check` skips them
(they are marked `#[ignore]`), so it stays green while you work.

## 8. Switch the app to your strategy

```
cargo run --release -p app -- --strategy mine
```

(The `--strategy` flag arrives with the CLI work in a later task. Leave it off to use the stand-in.)

## 9. Reading

- The Rust Book: [ch. 13.2 Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html).
- Optional: Cartea, Jaimungal and Penalva, *Algorithmic and High-Frequency Trading*, ch. 10 (market making with
  inventory risk).
