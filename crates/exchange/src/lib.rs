//! The simulated exchange: a price-time matching engine with maker/taker fees.

pub mod engine;
pub mod scheduler;

pub use engine::{MatchOutput, MatchingEngine};
pub use scheduler::Scheduler;
