# Exercise: write your own risk checker (`MyRisk`)

You will fill in `check`, `on_pnl` and `is_halted` in `crates/risk/src/mine.rs`. Everything else already works.
Until you finish, the app uses `MaxPositionOnly`, a stand-in that enforces only the position limit and never halts.

## 1. Why pre-trade risk exists

A trading bot sends orders far faster than a person can look at them, so a bug can do a lot of damage in a few seconds.
Pre-trade risk is a gatekeeper that every order must pass **before** it goes to the exchange. Two classic disasters:

- **Fat finger.** A typo sends an order 100 times too large, or at a price far from the market. One bad order can
  cost more than a month of profit.
- **Runaway bot.** A logic error makes the bot place order after order, or keep buying while it loses money.
  Nobody notices until the account is empty.

The checker does not try to be clever. It applies a few plain limits and says yes or no.

## 2. The types

- `RiskLimits`: the limits (`max_position`, `max_order_qty`, `price_band_ticks`, `max_open_orders`, `max_loss`).
  `MyRisk::new(limits)` already stores them in `self.limits`.
- `RiskContext`: what the bot looks at right now: `position`, `open_buy_qty`, `open_sell_qty`, `open_orders`
  and `mid` (an `Option<Price>`, `None` when the book is empty or one-sided).
- `Order`: either `Order::Place { side, price, qty }` or `Order::Cancel`.
- `RiskViolation`: the reason for a "no": `Halted`, `MaxOrderQty`, `MaxOpenOrders`, `NoReferencePrice`, `PriceBand`,
  `MaxPosition`.

`check` returns `Result<(), RiskViolation>`: `Ok(())` means "go ahead", `Err(reason)` means "blocked".

## 3. The rules, in order

The rules are checked **in this order, and the first one that fails wins**. You return that error straight away and
do not look at the later rules. The numbers below are the ones the tests use: `max_position` 1_000,
`max_order_qty` 500, `price_band_ticks` 100, `max_open_orders` 4, `max_loss` 1_000, and a base context with
position 0, nothing open, mid 10_000.

1. **Cancel is always OK.** If the order is `Order::Cancel`, return `Ok(())`. Even when halted (see section 4).
2. **Halted.** If trading is halted, return `Err(Halted)`.
3. **Order size.** If `qty > max_order_qty`, return `Err(MaxOrderQty)`. Qty 501 fails, qty 500 passes (the limit
   itself is allowed).
4. **Open orders.** If `open_orders >= max_open_orders`, return `Err(MaxOpenOrders)`. With 4 already open, a fifth
   is refused. Note `>=`, not `>`.
5. **Reference price.** If `mid` is `None`, return `Err(NoReferencePrice)`. Without a mid you cannot judge the
   price, so you refuse.
6. **Price band.** If `|price - mid| > price_band_ticks`, return `Err(PriceBand)`. With mid 10_000 and band 100:
   10_101 fails (distance 101), 10_100 passes (distance 100), 9_900 passes (distance 100).
7. **Position limit.** Count what the position **would become if every open order and this one filled**:
   - A **buy** fails if `position + open_buy_qty + qty > max_position`.
     Position 600, open buys 300, buy 200: 600 + 300 + 200 = 1_100 > 1_000, so `Err(MaxPosition)`.
     Buy 100 instead: 600 + 300 + 100 = 1_000, which is not above the limit, so `Ok`.
   - A **sell** fails if `position - open_sell_qty - qty < -max_position`.
     Position -600, open sells 300, sell 200: -600 - 300 - 200 = -1_100 < -1_000, so `Err(MaxPosition)`.

If all seven pass, return `Ok(())`.

Why order matters: the test `check_order_is_fixed` sends qty 600 with no mid. Two rules fail (3 and 5), but the
answer must be `MaxOrderQty` because rule 3 comes first.

## 4. The loss limit and the halt

`on_pnl(total_pnl)` is called with the bot's total profit and loss. Halt when `total_pnl < -max_loss`,
strictly less. With `max_loss` 1_000: `Cash(-1_000)` is **not** a halt, `Cash(-1_001)` is.

The halt is **sticky**: once set, a later `on_pnl(Cash(0))` does not undo it. A bot that lost money and then
"recovered" a little is not safe to turn back on by itself. A human decides that. Keep a `halted: bool` in the
struct (already there), set it to `true` when the limit is crossed, and never set it back to `false`.
`is_halted` just returns it.

**Why cancels are always allowed.** Halting means "stop taking on new risk". Cancelling reduces risk, because
it removes an order that could still fill. If a halt also blocked cancels, the orders already resting in the book
would keep trading while you were unable to pull them. So a halted bot refuses places but still lets cancels
through (test `halted_blocks_places_not_cancels`).

## 5. The Rust you need

- **`Result`, `Ok` and `Err`**: a function that can say "no" returns `Result<(), RiskViolation>`. Write `Ok(())`
  for success (the `()` means "nothing to report") and `Err(RiskViolation::PriceBand)` for a refusal.
- **Early return**: `return Err(RiskViolation::Halted);` leaves the function immediately. Use
  `if condition { return Err(...); }` for each rule, one after another, in the order above. This is how
  "the first failure wins" falls out naturally. (The `?` operator is also about `Result`, but you do not need it
  here: you are creating errors, not passing them on.)
- **`match` on enums**:
  ```rust
  let (side, price, qty) = match order {
      Order::Cancel => return Ok(()),
      Order::Place { side, price, qty } => (*side, *price, *qty),
  };
  ```
  `order` is a reference (`&Order`), so the fields come out as references too. The `*` copies the value out.
- **`Option`**: `ctx.mid` is `Option<Price>`. `let Some(mid) = ctx.mid else { return Err(...); };` returns early
  when it is `None`.
- **`abs()`**: `(price - mid).abs()`. `Price - Price` gives an `i64` (the distance in ticks), and `i64` has `.abs()`.
- **The inner number**: `qty.0`, `ctx.position.0`, `self.limits.max_position.0` are plain `i64`s you can compare with
  `>` and `<`. `ctx.open_orders` and `max_open_orders` are plain `usize`s.
- **Mutating self**: `check` and `on_pnl` take `&mut self`, so `self.halted = true;` works.

Open `crates/risk/src/standin.rs` for a finished example of rule 7.

## 6. Run the tests

From the repo root:

```
make exercise-risk
```

The ten tests in `crates/risk/tests/mine.rs` start out failing with `not yet implemented: exercise: ...`.
Work through them in order. When all ten pass, you are done. The normal `make check` skips them
(they are marked `#[ignore]`), so it stays green while you work.

## 7. Switch the app to your risk checker

```
cargo run --release -p app -- --risk mine
```

(Leave the flag off to use the stand-in. `make run ARGS="--risk mine"` does the same thing. The other two components keep their stand-ins.)

## 8. Reading

- The Rust Book: [ch. 9.2 Recoverable errors with `Result`](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html).
- Optional: Aldridge, *High-Frequency Trading*, the chapter on risk management and pre-trade controls.
