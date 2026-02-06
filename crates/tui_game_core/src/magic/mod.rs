use crate::combat::CombatState;
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::narrative::NarrativeState;
use crate::world::MapGrid;

pub mod combat;
pub mod exploration;

pub struct MagicCtx<'a> {
    pub entities: &'a mut EntityArena,
    pub narrative: &'a mut NarrativeState,
    pub map: &'a mut MapGrid,
    pub log: &'a mut Vec<String>,
    pub rng_seed: &'a mut u64,
}

pub enum MagicPhase<'a> {
    Combat { state: &'a mut CombatState },
    Exploration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastRequest {
    pub caster: EntityId,
    pub spell_id: &'static str,
    pub target: GridPos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastError {
    UnknownSpell,
    InvalidTarget,
    NotEnoughResource,
    NotAllowedInPhase,
}

pub fn try_cast(
    _ctx: &mut MagicCtx<'_>,
    _phase: MagicPhase<'_>,
    _req: CastRequest,
) -> Result<(), CastError> {
    Err(CastError::UnknownSpell)
}
