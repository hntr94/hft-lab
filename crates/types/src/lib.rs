//! Shared vocabulary for the whole project: integer units, ids and messages.
//!
//! Every other crate (matching engine, simulation, bot) builds on these types,
//! so you can write `types::Price` without caring which file it lives in.

pub mod ids;
pub mod messages;
pub mod units;

pub use ids::*;
pub use messages::*;
pub use units::*;
