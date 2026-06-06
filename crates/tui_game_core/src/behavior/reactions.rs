//! Reaction stack evaluation.
//!
//! `try_reaction` returns:
//! - `None` — this reaction does not apply; try the next entry in the blueprint list.
//! - `Some(Pass | Idle)` — handled for this tick without a grid step.
//! - `Some(Step | Attack)` — movement or combat intent.

use crate::combat::{move_cost_units, AttackStyle, CombatState, ATTACK_COST_UNITS};
use crate::content::ReactionDef;
use crate::entity::{ActiveReaction, EntityId, ForcedReaction, GridPos};
use crate::math::manhattan;

use super::action::NpcAction;
use super::constraints::{ActionConstraints, ActionPhase};
use super::ctx::BehaviorCtx;
use super::exploration::routine_action;
use super::flee::{try_flee_from_threat, try_forced_flee};
use super::navigation::next_step_toward;

#[must_use]
pub fn attack_style_for_actor(_ctx: &BehaviorCtx<'_>, _actor: EntityId) -> AttackStyle {
    AttackStyle::Unarmed
}

/// Try a forced override before the blueprint reaction list.
pub fn try_forced(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let brain = ctx.entities.npc_brain.get(actor.0 as usize)?;
    let forced = brain.forced_reaction?;
    let actor_pos = ctx.entities.pos(actor)?;
    match forced {
        ForcedReaction::Flee { from } => {
            try_forced_flee(ctx, actor, actor_pos, from, constraints, clock)
        }
    }
}

#[must_use]
pub fn try_reaction(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    def: ReactionDef,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let actor_pos = ctx.entities.pos(actor)?;
    if !ctx.entities.is_alive(actor) {
        return Some(NpcAction::Pass);
    }
    if ctx.entities.stats(actor).is_none_or(|s| s.hp == 0) {
        return Some(NpcAction::Pass);
    }

    match def {
        ReactionDef::FleeFromThreat { range } => {
            try_flee_from_threat(ctx, actor, actor_pos, range, constraints, clock)
        }
        ReactionDef::FightNearestInTurn => {
            if !constraints.encounter_active {
                return None;
            }
            let clock = clock?;
            if !constraints.in_turn_session() || !constraints.is_current_turn(clock) {
                return None;
            }
            fight_nearest(ctx, clock, actor, actor_pos)
        }
        ReactionDef::InvestigateLastHit => {
            let goal = match ctx.entities.npc_brain.get(actor.0 as usize)?.active {
                ActiveReaction::Investigate(pos) => pos,
                _ => return None,
            };
            if actor_pos == goal {
                if let Some(brain) = ctx.entities.npc_brain.get_mut(actor.0 as usize) {
                    brain.active = ActiveReaction::None;
                }
                return Some(NpcAction::Pass);
            }
            next_step_toward(ctx, actor, actor_pos, goal).map(NpcAction::step)
        }
        ReactionDef::Routine(routine) => try_routine(ctx, actor, actor_pos, routine, constraints, clock),
        ReactionDef::CallForHelp { range: _ } => Some(NpcAction::Pass),
        ReactionDef::Pass => Some(NpcAction::Pass),
    }
}

fn try_routine(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    routine: crate::content::NpcRoutineDef,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    if constraints.in_turn_session() {
        let clock = clock?;
        if !constraints.is_current_turn(clock) {
            return None;
        }
    } else if !matches!(constraints.phase, ActionPhase::RealtimeExplore) {
        return None;
    }
    routine_action(ctx, actor, actor_pos, routine, constraints, clock)
}

fn fight_nearest(
    ctx: &BehaviorCtx<'_>,
    state: &CombatState,
    actor: EntityId,
    actor_pos: GridPos,
) -> Option<NpcAction> {
    let ap = state.current_ap_units().unwrap_or(0);
    let style = attack_style_for_actor(ctx, actor);
    let mut closest: Option<(EntityId, i32)> = None;
    for target in &state.initiative {
        if *target == actor {
            continue;
        }
        if !ctx.entities.is_alive(*target) || ctx.entities.stats(*target).is_none_or(|s| s.hp == 0)
        {
            continue;
        }
        let Some(p) = ctx.entities.pos(*target) else {
            continue;
        };
        let d = manhattan(actor_pos, p);
        if closest.is_none_or(|(_, best)| d < best) {
            closest = Some((*target, d));
        }
    }
    let (target_id, _) = closest?;
    let target_pos = ctx.entities.pos(target_id)?;
    let dx = (actor_pos.x - target_pos.x).abs();
    let dy = (actor_pos.y - target_pos.y).abs();
    if dx.max(dy) == 1 {
        if ap >= ATTACK_COST_UNITS {
            return Some(NpcAction::Attack {
                target: target_id,
                style,
            });
        }
        return Some(NpcAction::Pass);
    }
    let next = next_step_toward(ctx, actor, actor_pos, target_pos)?;
    let step_cost = move_cost_units(actor_pos, next)?;
    if ap < step_cost {
        return Some(NpcAction::Pass);
    }
    Some(NpcAction::step(next))
}
