//! Discrete-event simulation: the bot, the exchange and the synthetic market,
//! connected by one-way latency.

use types::{BOT, BotEvent, Command, FeeSchedule, LIQUIDITY, MarketData, Price, Timestamp};

use crate::engine::{MatchOutput, MatchingEngine};
use crate::scheduler::Scheduler;
use crate::synthetic::{SyntheticConfig, SyntheticMarket};

/// Everything needed to set up one simulation run.
#[derive(Clone, Debug)]
pub struct SimConfig {
    /// Seed for all randomness.
    pub seed: u64,
    /// Simulated length of the run, in nanoseconds.
    pub duration_ns: u64,
    /// One-way message delay between bot and exchange, in nanoseconds.
    pub latency_ns: u64,
    /// Exchange fees.
    pub fees: FeeSchedule,
    /// Synthetic market parameters.
    pub synthetic: SyntheticConfig,
    /// Number of price levels per side in book snapshots.
    pub depth: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            seed: 42,
            duration_ns: 3_600_000_000_000,
            latency_ns: 1_000_000,
            fees: FeeSchedule {
                maker_ppm: 200,
                taker_ppm: 500,
            },
            synthetic: SyntheticConfig::default(),
            depth: 10,
        }
    }
}

enum SimEvent {
    FairStep,
    LpRefresh,
    TakerArrival,
    ToExchange(Command),
    ToBot(BotEvent),
}

/// The simulated world as seen from the bot's side of the wire.
pub struct Simulation {
    cfg: SimConfig,
    engine: MatchingEngine,
    market: SyntheticMarket,
    queue: Scheduler<SimEvent>,
    now: Timestamp,
}

impl Simulation {
    /// Builds a simulation at time zero.
    pub fn new(cfg: SimConfig) -> Self {
        let engine = MatchingEngine::new(cfg.fees);
        let mut market = SyntheticMarket::new(cfg.synthetic.clone(), cfg.seed);
        let mut queue = Scheduler::new();
        if cfg.synthetic.enabled {
            queue.push(Timestamp(0), SimEvent::LpRefresh);
            queue.push(
                Timestamp(cfg.synthetic.fair_step_interval_ns),
                SimEvent::FairStep,
            );
            queue.push(
                Timestamp(market.next_taker_delay_ns()),
                SimEvent::TakerArrival,
            );
        }
        Simulation {
            cfg,
            engine,
            market,
            queue,
            now: Timestamp(0),
        }
    }

    /// The current simulated time.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Sends a command from the bot; it reaches the exchange after the latency.
    pub fn send(&mut self, cmd: Command) {
        let at = self.now.plus_nanos(self.cfg.latency_ns);
        self.queue.push(at, SimEvent::ToExchange(cmd));
    }

    /// Makes a command arrive at the exchange exactly at `at` (tests, scenarios).
    pub fn inject(&mut self, at: Timestamp, cmd: Command) {
        self.queue.push(at, SimEvent::ToExchange(cmd));
    }

    /// Midpoint of the exchange's best bid and ask, rounded down.
    pub fn exchange_mid(&self) -> Option<Price> {
        match (self.engine.best_bid(), self.engine.best_ask()) {
            (Some(b), Some(a)) => Some(Price((b.0 + a.0).div_euclid(2))),
            _ => None,
        }
    }

    /// Runs the world forward until the next message for the bot, or `None`
    /// when the queue is empty or the next event is past the run's duration.
    pub fn next_bot_event(&mut self) -> Option<(Timestamp, BotEvent)> {
        loop {
            let next = self.queue.peek_time()?;
            if next.0 > self.cfg.duration_ns {
                return None;
            }
            let (at, event) = self.queue.pop()?;
            self.now = at;
            if let Some(bot_event) = self.process(event) {
                return Some((at, bot_event));
            }
        }
    }

    fn process(&mut self, event: SimEvent) -> Option<BotEvent> {
        let mut out = MatchOutput {
            exec: Vec::new(),
            trades: Vec::new(),
            book_changed: false,
        };
        match event {
            SimEvent::ToBot(ev) => return Some(ev),
            SimEvent::ToExchange(cmd) => out = self.engine.submit(&cmd),
            SimEvent::FairStep => {
                self.market.step_fair();
                let at = self
                    .now
                    .plus_nanos(self.cfg.synthetic.fair_step_interval_ns);
                self.queue.push(at, SimEvent::FairStep);
            }
            SimEvent::LpRefresh => {
                let resting = self.engine.resting_orders(LIQUIDITY);
                for cmd in self.market.refresh_liquidity(&resting) {
                    let o = self.engine.submit(&cmd);
                    out.trades.extend(o.trades);
                    out.book_changed |= o.book_changed;
                }
                let at = self
                    .now
                    .plus_nanos(self.cfg.synthetic.lp_refresh_interval_ns);
                self.queue.push(at, SimEvent::LpRefresh);
            }
            SimEvent::TakerArrival => {
                let mid = self.exchange_mid();
                let cmd = self.market.taker_order(mid);
                out = self.engine.submit(&cmd);
                let delay = self.market.next_taker_delay_ns();
                self.queue
                    .push(self.now.plus_nanos(delay), SimEvent::TakerArrival);
            }
        }
        self.deliver(out);
        None
    }

    /// Schedules what the bot will hear about `out`: its own exec events,
    /// then every trade, then at most one snapshot, all after the latency.
    fn deliver(&mut self, out: MatchOutput) {
        let at = self.now.plus_nanos(self.cfg.latency_ns);
        for (trader, ev) in out.exec {
            if trader == BOT {
                self.queue.push(at, SimEvent::ToBot(BotEvent::Exec(ev)));
            }
        }
        for trade in out.trades {
            self.queue
                .push(at, SimEvent::ToBot(BotEvent::Market(trade)));
        }
        if out.book_changed {
            let snap = self.engine.snapshot(self.cfg.depth);
            self.queue.push(
                at,
                SimEvent::ToBot(BotEvent::Market(MarketData::Snapshot(snap))),
            );
        }
    }
}
