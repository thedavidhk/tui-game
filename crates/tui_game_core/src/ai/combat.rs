use crate::ai::{AiIntent, CombatAiCtx, CombatDecisionPolicy};
use crate::entity::EntityId;

/// Baseline policy placeholder: always wait/pass.
pub struct HoldPositionPolicy;

impl CombatDecisionPolicy for HoldPositionPolicy {
    fn decide(&self, _actor: EntityId, _ctx: &CombatAiCtx<'_>) -> AiIntent {
        AiIntent::Wait
    }
}
