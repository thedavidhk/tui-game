use crate::combat::{CombatAction, CombatState};
use crate::entity::{EntityArena, EntityId};
use crate::world::MapGrid;

pub mod combat;
pub mod exploration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiIntent {
    Combat(CombatAction),
    Wait,
}

pub struct CombatAiCtx<'a> {
    pub state: &'a CombatState,
    pub map: &'a MapGrid,
    pub entities: &'a EntityArena,
}

pub trait CombatDecisionPolicy {
    fn decide(&self, actor: EntityId, ctx: &CombatAiCtx<'_>) -> AiIntent;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcTask {
    Idle,
    Investigate {
        target: crate::entity::GridPos,
    },
    DefendArea {
        center: crate::entity::GridPos,
        radius: u16,
    },
    Flee {
        from: EntityId,
    },
    SeekHelp {
        from: EntityId,
    },
    YieldTo {
        target: EntityId,
    },
    Engage {
        target: EntityId,
    },
}

pub trait NpcTaskPlanner {
    fn plan_task(&self, actor: EntityId, map: &MapGrid, arena: &EntityArena) -> NpcTask;
}
