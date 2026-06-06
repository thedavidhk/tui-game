//! Flee-from-threat movement and latch/settle state.

use crate::combat::{move_cost_units, CombatState};
use crate::content::ReactionDef;
use crate::entity::{ActiveReaction, EntityId, GridPos, NpcBrainState};
use crate::math::chebyshev;

use super::action::NpcAction;
use super::constraints::ActionConstraints;
use super::ctx::BehaviorCtx;
use super::navigation::next_step_toward;
use super::threat;

/// Exploration ticks to idle after fleeing before roam/patrol resumes.
const FLEE_SETTLE_TICKS: u16 = 36;

#[derive(Clone, Copy, Debug)]
struct ThreatRef {
    id: Option<EntityId>,
    pos: GridPos,
}

/// How far (Chebyshev) to run beyond merely leaving the trigger disc.
#[must_use]
pub fn flee_clearance_chebyshev(trigger_range: u16) -> i32 {
    i32::from(trigger_range).saturating_mul(2)
}

/// Clear flee latch and start post-flee settle.
pub(crate) fn complete_flee(brain: &mut NpcBrainState) {
    brain.active = ActiveReaction::None;
    brain.alarmed_ticks = FLEE_SETTLE_TICKS;
}

#[must_use]
pub(crate) fn clearance_for_actor(ctx: &BehaviorCtx<'_>, actor: EntityId) -> i32 {
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

/// Forced flee (magic, abilities): latch and use the same path as blueprint flee.
#[must_use]
pub(crate) fn try_forced_flee(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    from: GridPos,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let clearance = clearance_for_actor(ctx, actor);
    {
        let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
        brain.forced_reaction = None;
        brain.active = ActiveReaction::Flee {
            threat: None,
            from,
        };
    }
    try_flee_latched(
        ctx,
        actor,
        actor_pos,
        ThreatRef { id: None, pos: from },
        clearance,
        constraints,
        clock,
    )
}

/// Evaluate [`ReactionDef::FleeFromThreat`].
#[must_use]
pub(crate) fn try_flee_from_threat(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    range: u16,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let clearance = flee_clearance_chebyshev(range);
    let fleeing = matches!(
        ctx.entities.npc_brain.get(actor.0 as usize)?.active,
        ActiveReaction::Flee { .. }
    );
    let Some(threat_ref) = resolve_flee_threat(ctx, actor, range, clearance, fleeing) else {
        return None;
    };
    try_flee_latched(
        ctx,
        actor,
        actor_pos,
        threat_ref,
        clearance,
        constraints,
        clock,
    )
}

fn try_flee_latched(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    threat_ref: ThreatRef,
    clearance: i32,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
    let threat_pos = refresh_threat_pos(ctx, threat_ref);
    let dist = effective_threat_distance(ctx, actor, actor_pos, threat_pos);
    if dist >= clearance {
        let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
        complete_flee(brain);
        return Some(NpcAction::Idle);
    }
    {
        let brain = ctx.entities.npc_brain.get_mut(actor.0 as usize)?;
        brain.active = ActiveReaction::Flee {
            threat: threat_ref.id,
            from: threat_pos,
        };
    }
    if let Some(action) = flee_step(
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
        complete_flee(brain);
        return Some(NpcAction::Idle);
    }
    None
}

fn flee_step(
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
    Some(NpcAction::step(next))
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

fn resolve_flee_threat(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    trigger_range: u16,
    clearance: i32,
    fleeing: bool,
) -> Option<ThreatRef> {
    let search_range = if fleeing {
        u16::try_from(clearance.min(i32::from(u16::MAX))).unwrap_or(u16::MAX)
    } else {
        trigger_range
    };
    if let Some((id, pos)) = threat::nearest_non_allied_threat(ctx, actor, search_range) {
        return Some(ThreatRef { id: Some(id), pos });
    }
    if fleeing {
        let brain = ctx.entities.npc_brain.get(actor.0 as usize)?;
        if let ActiveReaction::Flee { threat, from } = brain.active {
            return Some(ThreatRef {
                id: threat,
                pos: from,
            });
        }
    }
    None
}

fn refresh_threat_pos(ctx: &BehaviorCtx<'_>, threat_ref: ThreatRef) -> GridPos {
    threat_ref
        .id
        .and_then(|id| ctx.entities.pos(id))
        .unwrap_or(threat_ref.pos)
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
