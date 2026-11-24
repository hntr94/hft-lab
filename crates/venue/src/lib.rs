//! venue crate: the only way the bot talks to an exchange.

use exchange::{SimConfig, Simulation};
use types::{BotEvent, Command, Timestamp};

/// A connection to an exchange, from the bot's side.
///
/// A real venue (H1) would implement this over the network, so callers must
/// tolerate rejects and late acks.
pub trait Venue {
    /// Human-readable name of the venue, shown in the run summary.
    fn name(&self) -> String;

    /// Sends a command to the exchange. Delivery and the reply are asynchronous.
    fn send(&mut self, cmd: Command);

    /// Blocks or advances until the next event for the bot.
    /// `None` means the session ended.
    fn next_event(&mut self) -> Option<(Timestamp, BotEvent)>;
}

/// The in-process simulated exchange, wrapped as a [`Venue`].
pub struct SimVenue {
    sim: Simulation,
}

impl SimVenue {
    /// Builds a simulated venue at time zero.
    pub fn new(cfg: SimConfig) -> Self {
        SimVenue {
            sim: Simulation::new(cfg),
        }
    }
}

impl Venue for SimVenue {
    fn name(&self) -> String {
        "sim (in-process) · BTC/USDT".to_string()
    }

    fn send(&mut self, cmd: Command) {
        self.sim.send(cmd);
    }

    fn next_event(&mut self) -> Option<(Timestamp, BotEvent)> {
        self.sim.next_bot_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exchange::SimConfig;
    use types::{BotEvent, MarketData};

    #[test]
    fn name_is_sim() {
        let venue = SimVenue::new(SimConfig::default());
        assert_eq!(venue.name(), "sim (in-process) · BTC/USDT");
    }

    #[test]
    fn delegates_to_simulation() {
        let cfg = SimConfig {
            duration_ns: 5_000_000_000,
            ..SimConfig::default()
        };
        let mut venue = SimVenue::new(cfg);

        let mut snapshots = 0;
        while let Some((_, event)) = venue.next_event() {
            if let BotEvent::Market(MarketData::Snapshot(_)) = event {
                snapshots += 1;
            }
        }

        assert!(
            snapshots >= 1,
            "expected at least one snapshot, got {snapshots}"
        );
    }
}
