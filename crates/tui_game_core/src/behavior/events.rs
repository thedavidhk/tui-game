//! World events that affect NPC behavior.

use crate::content::ReactionDef;
use crate::entity::{ActiveReaction, EntityId, ForcedReaction, GridPos, NpcBrainState};

/// Apply damage-driven reactions for `target` based on its blueprint list.
pub fn on_actor_damaged(
    brain: &mut NpcBrainState,
    reactions: &[ReactionDef],
    attacker: EntityId,
    attacker_pos: GridPos,
) {
    if reactions.iter().any(|r| matches!(r, ReactionDef::FleeFromThreat { .. })) {
        brain.active = ActiveReaction::Flee {
            threat: Some(attacker),
            from: attacker_pos,
        };
        brain.forced_reaction = None;
        return;
    }
    if reactions
        .iter()
        .any(|r| matches!(r, ReactionDef::InvestigateLastHit))
    {
        brain.active = ActiveReaction::Investigate(attacker_pos);
    }
}

/// Set a forced flee override (magic / abilities).
pub fn force_flee(brain: &mut NpcBrainState, from: GridPos) {
    brain.forced_reaction = Some(ForcedReaction::Flee { from });
}
