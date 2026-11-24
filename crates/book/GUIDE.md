# Exercise: build your own order book (`MyBook`)

You will fill in `crates/book/src/mine.rs`. Everything else already works.
Until you finish, the app uses `TopOfBook`, a simple stand-in.

## 1. What is an order book?

An exchange keeps two lists of resting orders for an instrument:

- **bids**: people who want to buy, with the price they will pay.
- **asks**: people who want to sell, with the price they want.

Orders at the same price are added up into one **level**: a price and a total quantity.
Bids are ranked from the highest price down, asks from the lowest price up.
The **best bid** is the highest bid, the **best ask** is the lowest ask. The best bid is always below the best ask,
otherwise the two would trade.

## 2. Why does the bot keep its own copy?

The exchange sends the bot a **snapshot** (the current levels) and the bot only knows what the last snapshot said.
To make decisions (where is the mid price? how wide is the spread? how much is queued at my price?) the bot needs to ask
questions about that data quickly and in many places. So it keeps its own `OrderBook` and asks it. Later steps also
use the book to take the bot's own orders out of the picture, so the book must not depend on the order of the input.

## 3. The methods you write

All prices are `Price(i64)` in ticks, quantities are `Qty(i64)` in lots.

| Method | Meaning |
| --- | --- |
| `apply_snapshot(&mut self, snap)` | Throw away the old book and rebuild it from `snap`. Ignore levels with qty 0. Input may be in any order. |
| `best_bid(&self)` | The bid level with the highest price, or `None` if there are no bids. |
| `best_ask(&self)` | The ask level with the lowest price, or `None`. |
| `mid(&self)` | `(best bid + best ask) / 2`, rounded down. `None` unless both sides exist. |
| `spread(&self)` | `best ask - best bid` in ticks. `None` unless both sides exist. |
| `depth_at(&self, side, price)` | Quantity at exactly that price on that side. `Qty(0)` if there is nothing there. |

## 4. The Rust you need

- **`struct`**: a bundle of named fields. `MyBook` has two: `bids` and `asks`.
- **`impl OrderBook for MyBook`**: says "MyBook does everything the `OrderBook` trait promises". Each method is written inside it.
- **`&self` vs `&mut self`**: `&self` can only read the book. `&mut self` may change it. Only `apply_snapshot` changes it.
- **`Option<T>`**: either `Some(value)` or `None`. Use `match`, or `if let`, or the `?` operator to unpack it:
  `let bid = self.best_bid()?;` returns `None` from your function early if there is no bid.
- **`match`**: choose by shape, e.g. `match side { Side::Buy => ..., Side::Sell => ... }`.
- **`BTreeMap<Price, Qty>`**: a map that stays sorted by key.
  - `map.insert(price, qty)`, `map.get(&price)`, `map.clear()`.
  - `map.first_key_value()` gives the smallest key with its value, as `Option<(&Price, &Qty)>`.
  - `map.last_key_value()` gives the largest. `map.iter().next_back()` does the same thing.
  - Smallest key is the best ask, largest key is the best bid.
- Types are tuple structs, so `Price(101)` wraps a number and `price.0` reads it back out.
  `Price - Price` gives an `i64`. `Level` has fields `price` and `qty`, and you build one with `Level { price, qty }`.
- Values you read out of a map are references. Dereference with `*` or copy with `.clone()`. `Price` and `Qty` are `Copy`, so `*qty` just works.

Open `crates/book/src/standin.rs` for a finished example of the same trait.

## 5. Run the tests

From the repo root:

```
make exercise-book
```

The eight tests in `crates/book/tests/mine.rs` start out failing with `not yet implemented: exercise: ...`.
That message tells you which method to write next. Work through them one at a time. When all eight pass, you are done.

The normal `make check` skips these tests (they are marked `#[ignore]`), so it stays green while you work.

## 6. Switch the app to your book

Once the tests pass, run the app with your book instead of the stand-in:

```
cargo run --release -p app -- --book mine
```

(Leave the flag off to use the stand-in. `make run ARGS="--book mine"` does the same thing. The other two components keep their stand-ins.)

## 7. Reading

- The Rust Book: [ch. 5 Structs](https://doc.rust-lang.org/book/ch05-00-structs.html),
  [ch. 6 Enums and `match`](https://doc.rust-lang.org/book/ch06-00-enums.html),
  [ch. 8 Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html),
  [ch. 10.2 Traits](https://doc.rust-lang.org/book/ch10-02-traits.html).
- [`BTreeMap` documentation](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html).
