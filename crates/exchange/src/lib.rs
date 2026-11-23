//! The simulated exchange: a price-time matching engine with maker/taker fees.

pub mod engine;

pub use engine::{MatchOutput, MatchingEngine};
