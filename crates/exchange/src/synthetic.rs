//! The synthetic market: a random-walk fair price, a liquidity provider that
//! refreshes a ladder of quotes, and Poisson-arriving takers, some of them
//! informed about the fair price. It only produces commands; the simulation
//! decides when to call it and submits the commands to the matching engine.

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use types::{Command, LIQUIDITY, NewOrder, OrderId, OrderType, Price, Qty, Side, TAKER, TraderId};

/// Tunable parameters of the synthetic market.
#[derive(Clone, Debug)]
pub struct SyntheticConfig {
    /// Whether the synthetic market runs at all.
    pub enabled: bool,
    /// Starting fair price.
    pub initial_fair: Price,
    /// Time between fair-price steps, in nanoseconds.
    pub fair_step_interval_ns: u64,
    /// Largest fair-price move per step, in ticks (either direction).
    pub fair_step_max_ticks: i64,
    /// Time between liquidity refreshes, in nanoseconds.
    pub lp_refresh_interval_ns: u64,
    /// Number of price levels per side.
    pub lp_levels: usize,
    /// Distance of the innermost level from fair, in ticks.
    pub lp_offset_ticks: i64,
    /// Distance between consecutive levels, in ticks.
    pub lp_level_step_ticks: i64,
    /// Inclusive range of liquidity order sizes, in lots.
    pub lp_qty_range: (i64, i64),
    /// Average taker orders per second.
    pub taker_rate_per_sec: f64,
    /// Inclusive range of taker order sizes, in lots.
    pub taker_qty_range: (i64, i64),
    /// Share of takers that know which side of the exchange mid fair is on.
    pub informed_fraction: f64,
}

impl Default for SyntheticConfig {
    fn default() -> Self {
        SyntheticConfig {
            enabled: true,
            initial_fair: Price(10_000_000),
            fair_step_interval_ns: 100_000_000,
            fair_step_max_ticks: 300,
            lp_refresh_interval_ns: 250_000_000,
            lp_levels: 10,
            lp_offset_ticks: 3_000,
            lp_level_step_ticks: 500,
            lp_qty_range: (50, 500),
            taker_rate_per_sec: 5.0,
            taker_qty_range: (10, 200),
            informed_fraction: 0.2,
        }
    }
}

/// The synthetic market's state: fair price, three random streams, id counters.
pub struct SyntheticMarket {
    cfg: SyntheticConfig,
    fair: Price,
    fair_rng: ChaCha8Rng,
    lp_rng: ChaCha8Rng,
    taker_rng: ChaCha8Rng,
    lp_seq: u64,
    taker_seq: u64,
}

impl SyntheticMarket {
    /// Creates a market; each concern gets its own stream seeded from `seed`.
    pub fn new(cfg: SyntheticConfig, seed: u64) -> Self {
        SyntheticMarket {
            fair: cfg.initial_fair,
            fair_rng: ChaCha8Rng::seed_from_u64(seed.wrapping_add(1)),
            lp_rng: ChaCha8Rng::seed_from_u64(seed.wrapping_add(2)),
            taker_rng: ChaCha8Rng::seed_from_u64(seed.wrapping_add(3)),
            lp_seq: 0,
            taker_seq: 0,
            cfg,
        }
    }

    /// The current hidden fair price.
    pub fn fair(&self) -> Price {
        self.fair
    }

    /// Moves the fair price by a uniform whole number of ticks in `[-max, +max]`.
    pub fn step_fair(&mut self) {
        let max = self.cfg.fair_step_max_ticks;
        self.fair = self.fair + self.fair_rng.random_range(-max..=max);
    }

    /// Cancels every id in `resting`, then places a fresh ladder around fair:
    /// bids for each level, then asks for each level.
    pub fn refresh_liquidity(&mut self, resting: &[OrderId]) -> Vec<Command> {
        let mut cmds: Vec<Command> = resting.iter().map(|id| Command::Cancel(*id)).collect();
        for side in [Side::Buy, Side::Sell] {
            for i in 0..self.cfg.lp_levels {
                let dist = self.cfg.lp_offset_ticks + i as i64 * self.cfg.lp_level_step_ticks;
                let price = match side {
                    Side::Buy => self.fair - dist,
                    Side::Sell => self.fair + dist,
                };
                let (lo, hi) = self.cfg.lp_qty_range;
                let qty = Qty(self.lp_rng.random_range(lo..=hi));
                let id = next_id(LIQUIDITY, &mut self.lp_seq);
                cmds.push(Command::New(NewOrder {
                    id,
                    side,
                    qty,
                    kind: OrderType::Limit { price },
                }));
            }
        }
        cmds
    }

    /// A market order from a taker. An informed taker buys when fair is above
    /// the exchange mid and sells when below; otherwise the side is a coin flip.
    pub fn taker_order(&mut self, exchange_mid: Option<Price>) -> Command {
        let (lo, hi) = self.cfg.taker_qty_range;
        let qty = Qty(self.taker_rng.random_range(lo..=hi));
        let informed = self.taker_rng.random::<f64>() < self.cfg.informed_fraction;
        let coin_flip = self.taker_rng.random::<bool>();
        let random_side = if coin_flip { Side::Buy } else { Side::Sell };
        let side = match (informed, exchange_mid) {
            (true, Some(mid)) if self.fair > mid => Side::Buy,
            (true, Some(mid)) if self.fair < mid => Side::Sell,
            _ => random_side,
        };
        let id = next_id(TAKER, &mut self.taker_seq);
        Command::New(NewOrder {
            id,
            side,
            qty,
            kind: OrderType::Market,
        })
    }

    /// Nanoseconds until the next taker arrives (exponential, at least 1).
    pub fn next_taker_delay_ns(&mut self) -> u64 {
        let u: f64 = self.taker_rng.random();
        let secs = -(1.0 - u).ln() / self.cfg.taker_rate_per_sec;
        ((secs * 1e9) as u64).max(1)
    }
}

/// Hands out the next order id for `trader`, counting from 1.
fn next_id(trader: TraderId, seq: &mut u64) -> OrderId {
    *seq += 1;
    OrderId { trader, seq: *seq }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let run = || {
            let mut m = SyntheticMarket::new(SyntheticConfig::default(), 7);
            let mut out = Vec::new();
            for _ in 0..100 {
                m.step_fair();
                out.push((m.fair(), m.taker_order(Some(m.fair()))));
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn refresh_places_symmetric_ladder() {
        let mut m = SyntheticMarket::new(SyntheticConfig::default(), 1);
        assert_eq!(m.fair(), Price(10_000_000));
        let resting = [
            OrderId {
                trader: LIQUIDITY,
                seq: 1,
            },
            OrderId {
                trader: LIQUIDITY,
                seq: 2,
            },
        ];
        let cmds = m.refresh_liquidity(&resting);
        assert_eq!(cmds.len(), 22);
        assert_eq!(cmds[0], Command::Cancel(resting[0]));
        assert_eq!(cmds[1], Command::Cancel(resting[1]));
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        for c in &cmds[2..] {
            match c {
                Command::New(o) => match (&o.side, &o.kind) {
                    (Side::Buy, OrderType::Limit { price }) => bids.push(*price),
                    (Side::Sell, OrderType::Limit { price }) => asks.push(*price),
                    _ => panic!("unexpected order"),
                },
                Command::Cancel(_) => panic!("cancel after news"),
            }
        }
        assert_eq!(bids.len(), 10);
        assert_eq!(asks.len(), 10);
        assert_eq!(bids.iter().max(), Some(&Price(9_997_000)));
        assert_eq!(asks.iter().min(), Some(&Price(10_003_000)));
    }

    #[test]
    fn informed_takers_follow_fair() {
        let cfg = SyntheticConfig {
            informed_fraction: 1.0,
            ..SyntheticConfig::default()
        };
        let mut m = SyntheticMarket::new(cfg, 3);
        let below = Price(m.fair().0 - 100);
        let above = Price(m.fair().0 + 100);
        for _ in 0..50 {
            match m.taker_order(Some(below)) {
                Command::New(o) => assert_eq!(o.side, Side::Buy),
                _ => panic!("expected new"),
            }
            match m.taker_order(Some(above)) {
                Command::New(o) => assert_eq!(o.side, Side::Sell),
                _ => panic!("expected new"),
            }
        }
    }

    #[test]
    fn fair_step_bounded() {
        let mut m = SyntheticMarket::new(SyntheticConfig::default(), 5);
        for _ in 0..1_000 {
            let before = m.fair();
            m.step_fair();
            assert!((m.fair() - before).abs() <= 300);
        }
    }

    #[test]
    fn taker_delay_mean_close_to_rate() {
        let mut m = SyntheticMarket::new(SyntheticConfig::default(), 9);
        let n = 10_000u64;
        let total: u64 = (0..n).map(|_| m.next_taker_delay_ns()).sum();
        let mean = total as f64 / n as f64;
        assert!((mean - 200_000_000.0).abs() < 20_000_000.0, "mean {mean}");
    }
}
