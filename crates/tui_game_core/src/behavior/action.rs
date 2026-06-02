//! Unified NPC action intents (exploration and turn-based).

use crate::combat::AttackStyle;
use crate::entity::{EntityId, GridPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcAction {
    Step(GridPos),
    Attack {
        target: EntityId,
        style: AttackStyle,
    },
    Pass,
    Idle,
}
