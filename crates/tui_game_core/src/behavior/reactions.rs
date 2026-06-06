//! Reaction stack evaluation.

use crate::combat::{move_cost_units, AttackStyle, CombatState, ATTACK_COST_UNITS};
use crate::content::ReactionDef;
use crate::entity::{ActiveReaction, EntityId, ForcedReaction, GridPos};
use crate::math::{chebyshev, manhattan};

use super::action::NpcAction;
use super::constraints::ActionConstraints;
use super::ctx::BehaviorCtx;
use super::exploration::{routine_tick, ExplorationIntent};
use super::navigation::next_step_toward;
use super::threat;

/// Exploration ticks to idle after reaching flee clearance before roaming resumes.
const FLEE_SETTLE_TICKS: u16 = 36;

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
            let clearance = flee_clearance_for_actor(ctx, actor);
            flee_from(ctx, actor, actor_pos, from, clearance, constraints, clock)
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
            let clearance = flee_clearance_chebyshev(range);
            let fleeing = matches!(
                ctx.entities.npc_brain.get(actor.0 as usize)?.active,
                ActiveReaction::Flee { .. }
            );
            let Some(threat_pos) = flee_threat_pos(ctx, actor, range, clearance, fleeing) else {
                return None;
            };
            let dist = effective_threat_distance(ctx, actor, actor_pos, threat_pos);
            if dist >= clearance {
                let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
                brain.alarmed_ticks = FLEE_SETTLE_TICKS;
                brain.active = ActiveReaction::None;
                return Some(NpcAction::Idle);
            }
            {
                let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
                brain.active = ActiveReaction::Flee {
                    from: threat_pos,
                };
            }
            if let Some(action) = flee_from(
                ctx,
                actor,
                actor_pos,
                threat_pos,
                clearance,
                constraints,
                clock,
            ) {
                return Some(action);
            }
            if greedy_flee_step(ctx, actor, actor_pos, threat_pos).is_none() {
                let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
                brain.active = ActiveReaction::None;
                brain.alarmed_ticks = FLEE_SETTLE_TICKS;
                return Some(NpcAction::Idle);
            }
            None
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
            next_step_toward(ctx, actor, actor_pos, goal).map(NpcAction::Step)
        }
        ReactionDef::Routine(routine) => {
            if constraints.in_turn_session() {
                let clock = clock?;
                if !constraints.is_current_turn(clock) {
                    return None;
                }
                let mut brain = ctx
                    .entities
                    .npc_brain
                    .get(actor.0 as usize)
                    .copied()
                    .unwrap_or_default();
                let intent = routine_tick(ctx, actor, actor_pos, routine, &mut brain);
                if let Some(slot) = ctx.entities.npc_brain.get_mut(actor.0 as usize) {
                    *slot = brain;
                }
                return match intent {
                    ExplorationIntent::Step(target) => {
                        if move_cost_units(actor_pos, target)
                            .is_some_and(|cost| clock.current_ap_units().unwrap_or(0) >= cost)
                        {
                            Some(NpcAction::RoamStep(target))
                        } else {
                            Some(NpcAction::Pass)
                        }
                    }
                    ExplorationIntent::Idle => Some(NpcAction::Idle),
                };
            }
            if !matches!(
                constraints.phase,
                super::constraints::ActionPhase::RealtimeExplore
            ) {
                return None;
            }
            let mut brain = ctx
                .entities
                .npc_brain
                .get(actor.0 as usize)
                .copied()
                .unwrap_or_default();
            let intent = routine_tick(ctx, actor, actor_pos, routine, &mut brain);
            if let Some(slot) = ctx.entities.npc_brain.get_mut(actor.0 as usize) {
                *slot = brain;
            }
            match intent {
                ExplorationIntent::Step(target) => Some(NpcAction::RoamStep(target)),
                ExplorationIntent::Idle => Some(NpcAction::Idle),
            }
        }
        ReactionDef::CallForHelp { range: _ } => {
            // Stub: no ally system yet.
            Some(NpcAction::Pass)
        }
        ReactionDef::Pass => Some(NpcAction::Pass),
    }
}

/// How far (Chebyshev) to run beyond merely leaving the trigger disc.
#[must_use]
pub fn flee_clearance_chebyshev(trigger_range: u16) -> i32 {
    i32::from(trigger_range).saturating_mul(2)
}

fn effective_threat_distance(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    fallback_threat: GridPos,
) -> i32 {
    if let Some((_, live)) = threat::nearest_non_allied_threat(ctx, actor, u16::MAX) {
        chebyshev(actor_pos, live)
    } else {
        chebyshev(actor_pos, fallback_threat)
    }
}

fn flee_threat_pos(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    trigger_range: u16,
    clearance: i32,
    fleeing: bool,
) -> Option<GridPos> {
    let search_range = if fleeing {
        u16::try_from(clearance.min(i32::from(u16::MAX))).unwrap_or(u16::MAX)
    } else {
        trigger_range
    };
    if let Some((_, pos)) = threat::nearest_non_allied_threat(ctx, actor, search_range) {
        return Some(pos);
    }
    if fleeing {
        let brain = ctx.entities.npc_brain.get(actor.0 as usize)?;
        if let ActiveReaction::Flee { from } = brain.active {
            return Some(from);
        }
    }
    None
}

#[must_use]
fn flee_clearance_for_actor(ctx: &BehaviorCtx<'_>, actor: EntityId) -> i32 {
    ctx.blueprint_for(actor)
        .and_then(|bp| {
            bp.behavior.reactions.iter().find_map(|r| {
                let ReactionDef::FleeFromThreat { range } = r else {
                    return None;
                };
                Some(flee_clearance_chebyshev(*range))
            })
        })
        .unwrap_or(16)
}

fn flee_from(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    threat_pos: GridPos,
    clearance: i32,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let current_dist = chebyshev(actor_pos, threat_pos);
    let flee_goal = clamp_flee_goal(ctx.map, flee_goal_cell(actor_pos, threat_pos, clearance));
    let path_step = next_step_toward(ctx, actor, actor_pos, flee_goal);
    let greedy_step = greedy_flee_step(ctx, actor, actor_pos, threat_pos);
    let next = [path_step, greedy_step]
        .into_iter()
        .flatten()
        .filter(|candidate| chebyshev(*candidate, threat_pos) > current_dist)
        .max_by_key(|candidate| chebyshev(*candidate, threat_pos))?;
    if constraints.in_turn_session() {
        let clock = clock?;
        if !constraints.is_current_turn(clock) {
            return None;
        }
        let cost = move_cost_units(actor_pos, next)?;
        if clock.current_ap_units().unwrap_or(0) < cost {
            return Some(NpcAction::Pass);
        }
    }
    Some(NpcAction::Step(next))
}

#[must_use]
fn clamp_flee_goal(map: &crate::world::MapGrid, goal: GridPos) -> GridPos {
    let max_x = i32::from(map.width.saturating_sub(1));
    let max_y = i32::from(map.height.saturating_sub(1));
    GridPos {
        x: goal.x.clamp(0, max_x),
        y: goal.y.clamp(0, max_y),
    }
}

/// One step to any walkable neighbor farther from `threat` (map edge / wall fallback).
fn greedy_flee_step(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    from: GridPos,
    threat: GridPos,
) -> Option<GridPos> {
    const DELTAS: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let current = chebyshev(from, threat);
    let mut best: Option<(GridPos, i32)> = None;
    for (dx, dy) in DELTAS {
        let candidate = GridPos {
            x: from.x + dx,
            y: from.y + dy,
        };
        if !ctx.map.in_bounds(candidate.x, candidate.y) {
            continue;
        }
        if !ctx
            .entities
            .can_move_to(ctx.map.blocks_movement(candidate.x, candidate.y), candidate, Some(actor))
        {
            continue;
        }
        let dist = chebyshev(candidate, threat);
        if dist <= current {
            continue;
        }
        if best.is_none_or(|(_, best_dist)| dist > best_dist) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(pos, _)| pos)
}

#[must_use]
fn flee_goal_cell(from: GridPos, threat: GridPos, clearance: i32) -> GridPos {
    let dx = (from.x - threat.x).signum();
    let dy = (from.y - threat.y).signum();
    if dx == 0 && dy == 0 {
        return GridPos {
            x: from.x + 1,
            y: from.y,
        };
    }
    let current = chebyshev(from, threat);
    if current >= clearance {
        return from;
    }
    let extra = clearance - current;
    GridPos {
        x: from.x + dx * extra,
        y: from.y + dy * extra,
    }
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
    Some(NpcAction::Step(next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flee_clearance_is_double_trigger_range() {
        assert_eq!(flee_clearance_chebyshev(10), 20);
        assert_eq!(flee_clearance_chebyshev(7), 14);
    }

    #[test]
    fn flee_goal_is_clearance_away_from_threat() {
        let from = GridPos { x: 6, y: 5 };
        let threat = GridPos { x: 5, y: 5 };
        let goal = flee_goal_cell(from, threat, 20);
        assert_eq!(goal, GridPos { x: 25, y: 5 });
        assert!(chebyshev(goal, threat) >= 20);
    }

    #[test]
    fn flee_goal_stays_put_when_already_clear() {
        let from = GridPos { x: 30, y: 5 };
        let threat = GridPos { x: 5, y: 5 };
        assert_eq!(flee_goal_cell(from, threat, 20), from);
    }
}
