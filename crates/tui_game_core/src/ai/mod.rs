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
