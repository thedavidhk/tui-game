//! Single entry point for NPC decisions.

use crate::combat::CombatState;
use crate::entity::EntityId;

use super::action::NpcAction;
use super::constraints::ActionConstraints;
use super::ctx::BehaviorCtx;
use super::reactions;

/// Decide what `actor` does this tick.
#[must_use]
pub fn decide_actor_action(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> NpcAction {
    let Some(bp) = ctx.blueprint_for(actor) else {
        return NpcAction::Pass;
    };

    if let Some(action) = reactions::try_forced(ctx, actor, constraints, clock) {
        return action;
    }

    for reaction in bp.behavior.reactions {
        if let Some(action) = reactions::try_reaction(ctx, actor, *reaction, constraints, clock) {
            return action;
        }
    }
    NpcAction::Idle
}
