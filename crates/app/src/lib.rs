//! app crate: wires venue, book, strategy, risk, order tracker and PnL into one run.

use std::fmt;

use book::{MyBook, OrderBook, TopOfBook};
use exchange::SimConfig;
use risk::{MaxPositionOnly, MyRisk, Order, RiskCheck, RiskContext, RiskLimits};
use strategy::{
    Action, FixedSpreadQuoter, MarketMaker, MarketMakerConfig, OrderTracker, Strategy,
    StrategyContext,
};
use types::{
    BotEvent, CASH_UNITS_PER_USDT, Cash, ExecEvent, FeeSchedule, MarketData, Qty, Side, fmt_cash,
    fmt_ns_as_ms, fmt_ppm_pct, fmt_qty,
};
use venue::{SimVenue, Venue};

pub mod pnl;

pub use pnl::PnlTracker;

/// Which implementation of a component to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Impl {
    /// The ready-made stand-in.
    Standin,
    /// The owner's own implementation.
    Mine,
}

/// Everything that can be configured for one run.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub seed: u64,
    pub duration_secs: u64,
    pub latency_us: u64,
    pub maker_fee_ppm: i64,
    pub taker_fee_ppm: i64,
    pub half_spread_ticks: i64,
    pub skew_ticks: i64,
    pub quote_lots: i64,
    pub max_position_lots: i64,
    pub max_order_lots: i64,
    pub price_band_ticks: i64,
    pub max_open_orders: usize,
    pub max_loss_usdt: i64,
    pub book: Impl,
    pub strategy: Impl,
    pub risk: Impl,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            seed: 42,
            duration_secs: 3600,
            latency_us: 1000,
            maker_fee_ppm: 200,
            taker_fee_ppm: 500,
            half_spread_ticks: 2500,
            skew_ticks: 500,
            quote_lots: 100,
            max_position_lots: 1000,
            max_order_lots: 500,
            price_band_ticks: 20_000,
            max_open_orders: 10,
            max_loss_usdt: 100,
            book: Impl::Standin,
            strategy: Impl::Standin,
            risk: Impl::Standin,
        }
    }
}

impl AppConfig {
    /// Checks the values that would make a run meaningless. The message names the field.
    pub fn validate(&self) -> Result<(), String> {
        if self.duration_secs == 0 {
            return Err("duration_secs must be > 0".to_string());
        }
        if self.quote_lots <= 0 {
            return Err("quote_lots must be > 0".to_string());
        }
        if self.half_spread_ticks < 1 {
            return Err("half_spread_ticks must be >= 1".to_string());
        }
        if self.max_open_orders == 0 {
            return Err("max_open_orders must be > 0".to_string());
        }
        if self.max_position_lots <= 0 {
            return Err("max_position_lots must be > 0".to_string());
        }
        if self.max_order_lots <= 0 {
            return Err("max_order_lots must be > 0".to_string());
        }
        Ok(())
    }
}

/// The result of one run. It holds no floats, so two runs can be compared exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub venue: String,
    pub seed: u64,
    /// The configured simulated duration (not the last event time), in nanoseconds.
    pub sim_time_ns: u64,
    pub latency_ns: u64,
    pub maker_fee_ppm: i64,
    pub taker_fee_ppm: i64,
    /// Orders and cancels sent to the venue, including the cancels sent on a halt.
    pub orders_sent: u64,
    pub risk_rejections: u64,
    pub fills: u64,
    pub buys: u64,
    pub sells: u64,
    pub maker_fills: u64,
    pub taker_fills: u64,
    pub volume: Qty,
    pub final_position: Qty,
    pub gross: Cash,
    pub fees_paid: Cash,
    pub realized_net: Cash,
    pub unrealized: Cash,
    pub total: Cash,
    pub max_drawdown: Cash,
    /// `Some(reason)` if risk halted the bot.
    pub halted: Option<String>,
}

/// `1234567` becomes `"1,234,567"`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn fmt_duration(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

fn fmt_signed_qty(qty: Qty) -> String {
    if qty.0 >= 0 {
        format!("+{}", fmt_qty(qty))
    } else {
        fmt_qty(qty)
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "hft-lab run summary")?;
        writeln!(f, "  venue:            {}", self.venue)?;
        writeln!(f, "  seed:             {}", self.seed)?;
        writeln!(f, "  simulated time:   {}", fmt_duration(self.sim_time_ns))?;
        writeln!(
            f,
            "  latency:          {} ms each way",
            fmt_ns_as_ms(self.latency_ns)
        )?;
        writeln!(
            f,
            "  fees:             maker {}% / taker {}%",
            fmt_ppm_pct(self.maker_fee_ppm),
            fmt_ppm_pct(self.taker_fee_ppm)
        )?;
        writeln!(
            f,
            "  orders sent:      {}   (rejected by risk: {})",
            thousands(self.orders_sent),
            thousands(self.risk_rejections)
        )?;
        writeln!(
            f,
            "  fills:            {}   (buy {} / sell {} · maker {} / taker {})",
            thousands(self.fills),
            thousands(self.buys),
            thousands(self.sells),
            thousands(self.maker_fills),
            thousands(self.taker_fills)
        )?;
        writeln!(f, "  volume:           {} BTC", fmt_qty(self.volume))?;
        writeln!(
            f,
            "  final inventory:  {} BTC",
            fmt_signed_qty(self.final_position)
        )?;
        writeln!(
            f,
            "  gross PnL:        {} USDT (before fees)",
            fmt_cash(self.gross)
        )?;
        // Fees paid are shown as a PnL contribution: a cost is negative.
        writeln!(
            f,
            "  fees paid:        {} USDT",
            fmt_cash(Cash(-self.fees_paid.0))
        )?;
        writeln!(
            f,
            "  realized PnL:     {} USDT (after fees)",
            fmt_cash(self.realized_net)
        )?;
        writeln!(
            f,
            "  unrealized PnL:   {} USDT (marked at final mid)",
            fmt_cash(self.unrealized)
        )?;
        writeln!(f, "  total PnL:        {} USDT", fmt_cash(self.total))?;
        writeln!(
            f,
            "  max drawdown:     {} USDT",
            fmt_cash(self.max_drawdown)
        )?;
        match &self.halted {
            None => writeln!(f, "  halted:           no"),
            Some(reason) => writeln!(f, "  halted:           yes ({reason})"),
        }
    }
}

/// Runs one simulation with the risk checker chosen by `cfg.risk`.
pub fn run(cfg: &AppConfig) -> Summary {
    let limits = RiskLimits {
        max_position: Qty(cfg.max_position_lots),
        max_order_qty: Qty(cfg.max_order_lots),
        price_band_ticks: cfg.price_band_ticks,
        max_open_orders: cfg.max_open_orders,
        max_loss: Cash(cfg.max_loss_usdt * CASH_UNITS_PER_USDT),
    };
    let risk: Box<dyn RiskCheck> = match cfg.risk {
        Impl::Standin => Box::new(MaxPositionOnly::new(limits)),
        Impl::Mine => Box::new(MyRisk::new(limits)),
    };
    run_with(cfg, risk)
}

/// Runs one simulation with an injected risk checker (`cfg.risk` is ignored).
pub fn run_with(cfg: &AppConfig, mut risk: Box<dyn RiskCheck>) -> Summary {
    let sim_cfg = SimConfig {
        seed: cfg.seed,
        duration_ns: cfg.duration_secs * 1_000_000_000,
        latency_ns: cfg.latency_us * 1_000,
        fees: FeeSchedule {
            maker_ppm: cfg.maker_fee_ppm,
            taker_ppm: cfg.taker_fee_ppm,
        },
        ..Default::default()
    };
    let mut venue = SimVenue::new(sim_cfg.clone());
    let mut book: Box<dyn OrderBook> = match cfg.book {
        Impl::Standin => Box::new(TopOfBook::new()),
        Impl::Mine => Box::new(MyBook::new()),
    };
    let mm_cfg = MarketMakerConfig {
        half_spread_ticks: cfg.half_spread_ticks,
        quote_qty: Qty(cfg.quote_lots),
        skew_ticks: cfg.skew_ticks,
    };
    let mut strategy: Box<dyn Strategy> = match cfg.strategy {
        Impl::Standin => Box::new(FixedSpreadQuoter::new(mm_cfg)),
        Impl::Mine => Box::new(MarketMaker::new(mm_cfg)),
    };
    let mut tracker = OrderTracker::new();
    let mut pnl = PnlTracker::new();

    let mut orders_sent = 0u64;
    let mut risk_rejections = 0u64;
    let mut halted: Option<String> = None;

    while let Some((now, ev)) = venue.next_event() {
        match &ev {
            BotEvent::Market(MarketData::Snapshot(snap)) => {
                book.apply_snapshot(&tracker.exclude_own(snap));
            }
            BotEvent::Market(MarketData::Trade { .. }) => {}
            BotEvent::Exec(exec) => {
                tracker.on_exec(exec);
                if let ExecEvent::Fill {
                    side,
                    price,
                    qty,
                    liquidity,
                    fee,
                    ..
                } = exec
                {
                    pnl.on_fill(*side, *price, *qty, liquidity.clone(), *fee);
                }
            }
        }
        pnl.mark(book.mid());
        risk.on_pnl(pnl.total());

        if risk.is_halted() {
            if halted.is_none() {
                halted = Some("loss limit".to_string());
                for order in tracker.open_orders() {
                    if let Some(cmd) = tracker.cancel(order.id) {
                        venue.send(cmd);
                        orders_sent += 1;
                    }
                }
            }
            continue;
        }

        let open = tracker.open_orders();
        let ctx = StrategyContext {
            now,
            book: book.as_ref(),
            position: tracker.position(),
            open_orders: &open,
        };
        let actions = strategy.on_update(&ctx);
        for action in actions {
            let risk_ctx = RiskContext {
                position: tracker.position(),
                open_buy_qty: tracker.open_qty(Side::Buy),
                open_sell_qty: tracker.open_qty(Side::Sell),
                open_orders: tracker.open_orders().len(),
                mid: book.mid(),
            };
            let order = match &action {
                Action::Place { side, price, qty } => Order::Place {
                    side: *side,
                    price: *price,
                    qty: *qty,
                },
                Action::Cancel(_) => Order::Cancel,
            };
            if risk.check(&order, &risk_ctx).is_err() {
                risk_rejections += 1;
                continue;
            }
            let cmd = match action {
                Action::Place { side, price, qty } => Some(tracker.place(side, price, qty)),
                Action::Cancel(id) => tracker.cancel(id),
            };
            if let Some(cmd) = cmd {
                venue.send(cmd);
                orders_sent += 1;
            }
        }
    }

    Summary {
        venue: venue.name(),
        seed: cfg.seed,
        sim_time_ns: sim_cfg.duration_ns,
        latency_ns: sim_cfg.latency_ns,
        maker_fee_ppm: cfg.maker_fee_ppm,
        taker_fee_ppm: cfg.taker_fee_ppm,
        orders_sent,
        risk_rejections,
        fills: pnl.fills(),
        buys: pnl.buys(),
        sells: pnl.sells(),
        maker_fills: pnl.maker_fills(),
        taker_fills: pnl.taker_fills(),
        volume: pnl.volume(),
        final_position: pnl.position(),
        gross: pnl.gross(),
        fees_paid: pnl.fees(),
        realized_net: pnl.realized_net(),
        unrealized: pnl.unrealized(),
        total: pnl.total(),
        max_drawdown: pnl.max_drawdown(),
        halted,
    }
}
