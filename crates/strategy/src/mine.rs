//! The owner's exercise: fill in `on_update` below. See GUIDE.md.

use crate::{Action, MarketMakerConfig, Strategy, StrategyContext};

// The field is unused until you write `on_update`. Delete this line once it is used.
#[allow(dead_code)]
#[derive(Debug)]
pub struct MarketMaker {
    cfg: MarketMakerConfig,
}

impl MarketMaker {
    pub fn new(cfg: MarketMakerConfig) -> MarketMaker {
        MarketMaker { cfg }
    }
}

impl Strategy for MarketMaker {
    fn on_update(&mut self, _ctx: &StrategyContext) -> Vec<Action> {
        todo!("exercise: follow the quoting rules in GUIDE.md and return the actions to take")
    }
}
