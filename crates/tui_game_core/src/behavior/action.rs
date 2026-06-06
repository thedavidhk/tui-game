//! Unified NPC action intents (exploration and turn-based).

use crate::combat::AttackStyle;
use crate::entity::{EntityId, GridPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcAction {
    /// Urgent movement (flee, chase, investigate).
    Step(GridPos),
    /// Casual roam/patrol pace (half exploration speed).
    RoamStep(GridPos),
    Attack {
        target: EntityId,
        style: AttackStyle,
    },
    Pass,
    Idle,
}

impl NpcAction {
    #[must_use]
    pub const fn step_target(self) -> Option<GridPos> {
        match self {
            Self::Step(target) | Self::RoamStep(target) => Some(target),
            Self::Attack { .. } | Self::Pass | Self::Idle => None,
        }
    }

    #[must_use]
    pub const fn is_leisurely_step(self) -> bool {
        matches!(self, Self::RoamStep(_))
    }
}
