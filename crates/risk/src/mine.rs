//! The owner's exercise: fill in `check` and `on_pnl` below. See GUIDE.md.

use crate::{Order, RiskCheck, RiskContext, RiskLimits, RiskViolation};
use types::Cash;

// The fields are unused until you write the rules. Delete this line once they are used.
#[allow(dead_code)]
#[derive(Debug)]
pub struct MyRisk {
    limits: RiskLimits,
    halted: bool,
}

impl MyRisk {
    pub fn new(limits: RiskLimits) -> MyRisk {
        MyRisk {
            limits,
            halted: false,
        }
    }
}

impl RiskCheck for MyRisk {
    fn check(&mut self, _order: &Order, _ctx: &RiskContext) -> Result<(), RiskViolation> {
        todo!("exercise: apply the seven rules in GUIDE.md, in order, and return the first failure")
    }

    fn on_pnl(&mut self, _total_pnl: Cash) {
        todo!("exercise: halt (and stay halted) when total_pnl is below -max_loss")
    }

    fn is_halted(&self) -> bool {
        todo!("exercise: return whether trading has been halted")
    }
}
