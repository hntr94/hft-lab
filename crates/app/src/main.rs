use app::{AppConfig, Impl, run};
use clap::Parser;

/// Runs the simulated market-making bot and prints a summary.
#[derive(Parser, Debug)]
#[command(version, allow_negative_numbers = true)]
struct Cli {
    /// Random seed for the synthetic market.
    #[arg(long, default_value_t = AppConfig::default().seed)]
    seed: u64,
    /// Simulated run length in seconds.
    #[arg(long, default_value_t = AppConfig::default().duration_secs)]
    duration_secs: u64,
    /// One-way latency in microseconds.
    #[arg(long, default_value_t = AppConfig::default().latency_us)]
    latency_us: u64,
    /// Maker fee in parts per million (negative is a rebate).
    #[arg(long, default_value_t = AppConfig::default().maker_fee_ppm)]
    maker_fee_ppm: i64,
    /// Taker fee in parts per million.
    #[arg(long, default_value_t = AppConfig::default().taker_fee_ppm)]
    taker_fee_ppm: i64,
    /// Distance of each quote from the mid, in ticks.
    #[arg(long, default_value_t = AppConfig::default().half_spread_ticks)]
    half_spread_ticks: i64,
    /// Quote shift per quote_qty (one quote size) of position, in ticks.
    #[arg(long, default_value_t = AppConfig::default().skew_ticks)]
    skew_ticks: i64,
    /// Size of each quote, in lots.
    #[arg(long, default_value_t = AppConfig::default().quote_lots)]
    quote_lots: i64,
    /// Largest position allowed, in lots.
    #[arg(long, default_value_t = AppConfig::default().max_position_lots)]
    max_position_lots: i64,
    /// Largest single order, in lots.
    #[arg(long, default_value_t = AppConfig::default().max_order_lots)]
    max_order_lots: i64,
    /// How far an order may be from the mid, in ticks.
    #[arg(long, default_value_t = AppConfig::default().price_band_ticks)]
    price_band_ticks: i64,
    /// How many orders may be open at once.
    #[arg(long, default_value_t = AppConfig::default().max_open_orders)]
    max_open_orders: usize,
    /// Halt when total PnL falls below minus this many USDT.
    #[arg(long, default_value_t = AppConfig::default().max_loss_usdt)]
    max_loss_usdt: i64,
    /// Order book implementation.
    #[arg(long, value_enum, default_value_t = AppConfig::default().book)]
    book: Impl,
    /// Strategy implementation.
    #[arg(long, value_enum, default_value_t = AppConfig::default().strategy)]
    strategy: Impl,
    /// Risk checker implementation.
    #[arg(long, value_enum, default_value_t = AppConfig::default().risk)]
    risk: Impl,
}

impl From<Cli> for AppConfig {
    fn from(c: Cli) -> Self {
        AppConfig {
            seed: c.seed,
            duration_secs: c.duration_secs,
            latency_us: c.latency_us,
            maker_fee_ppm: c.maker_fee_ppm,
            taker_fee_ppm: c.taker_fee_ppm,
            half_spread_ticks: c.half_spread_ticks,
            skew_ticks: c.skew_ticks,
            quote_lots: c.quote_lots,
            max_position_lots: c.max_position_lots,
            max_order_lots: c.max_order_lots,
            price_band_ticks: c.price_band_ticks,
            max_open_orders: c.max_open_orders,
            max_loss_usdt: c.max_loss_usdt,
            book: c.book,
            strategy: c.strategy,
            risk: c.risk,
        }
    }
}

fn main() {
    let cfg = AppConfig::from(Cli::parse());
    if let Err(msg) = cfg.validate() {
        eprintln!("error: {msg}");
        std::process::exit(2);
    }
    print!("{}", run(&cfg));
}
