//! The simulated exchange: a price-time matching engine with maker/taker fees.

pub mod engine;
pub mod scheduler;
pub mod sim;
pub mod synthetic;

pub use engine::{MatchOutput, MatchingEngine};
pub use scheduler::Scheduler;
pub use sim::{SimConfig, Simulation};
pub use synthetic::{SyntheticConfig, SyntheticMarket};
