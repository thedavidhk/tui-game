//! Exploration-phase NPC routines (idle, roam, patrol).

use crate::combat::{move_cost_units, CombatState};
use crate::content::{NpcRoutineDef, PatrolStopDef};
use crate::entity::{EntityId, GridPos, NpcBrainState};
use crate::math::{chebyshev, lcg_next_u32};

use super::action::NpcAction;
use super::constraints::ActionConstraints;
use super::ctx::BehaviorCtx;
use super::navigation::next_step_toward;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoutineIntent {
    Idle,
    Step(GridPos),
}

fn random_inclusive(seed: &mut u64, lo: i32, hi: i32) -> i32 {
    let span = (hi - lo + 1).max(1) as u32;
    lo + (lcg_next_u32(seed) % span) as i32
}

fn pick_roam_goal(
    map: &crate::world::MapGrid,
    entities: &crate::entity::EntityArena,
    state: &NpcBrainState,
    radius: u16,
    actor: EntityId,
    seed: &mut u64,
) -> Option<GridPos> {
    if radius == 0 {
        return None;
    }
    let r = i32::from(radius);
    for _ in 0..24 {
        let dx = random_inclusive(seed, -r, r);
        let dy = random_inclusive(seed, -r, r);
        let candidate = GridPos {
            x: state.home.x + dx,
            y: state.home.y + dy,
        };
        if chebyshev(state.home, candidate) > r {
            continue;
        }
        if !map.in_bounds(candidate.x, candidate.y) {
            continue;
        }
        if !entities.can_move_to(
            map.blocks_movement(candidate.x, candidate.y),
            candidate,
            Some(actor),
        ) {
            continue;
        }
        return Some(candidate);
    }
    None
}

/// Returns `true` while the actor is still in post-flee settle (counts down one tick).
fn consume_alarmed_settle(brain: &mut NpcBrainState) -> bool {
    if brain.alarmed_ticks == 0 {
        return false;
    }
    brain.alarmed_ticks = brain.alarmed_ticks.saturating_sub(1);
    true
}

fn patrol_goal(state: &NpcBrainState, stops: &[PatrolStopDef]) -> Option<GridPos> {
    let stop = stops.get(state.patrol_next_stop as usize % stops.len())?;
    Some(GridPos {
        x: state.home.x + i32::from(stop.dx),
        y: state.home.y + i32::from(stop.dy),
    })
}

fn routine_tick(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    from: GridPos,
    routine: NpcRoutineDef,
    brain: &mut NpcBrainState,
) -> RoutineIntent {
    match routine {
        NpcRoutineDef::Idle => RoutineIntent::Idle,
        NpcRoutineDef::Roam { radius, wait_ticks } => {
            if consume_alarmed_settle(brain) {
                return RoutineIntent::Idle;
            }
            let out_of_range = chebyshev(from, brain.home) > i32::from(radius);
            if out_of_range {
                brain.roam_goal = Some(brain.home);
            }
            if brain.roam_goal == Some(from) {
                brain.roam_goal = None;
            }
            if brain.roam_goal.is_none() {
                if brain.roam_wait_ticks < wait_ticks {
                    brain.roam_wait_ticks = brain.roam_wait_ticks.saturating_add(1);
                    return RoutineIntent::Idle;
                }
                brain.roam_wait_ticks = 0;
                brain.roam_goal = pick_roam_goal(
                    ctx.map,
                    &*ctx.entities,
                    brain,
                    radius,
                    actor,
                    ctx.rng,
                );
            }
            let Some(goal) = brain.roam_goal else {
                return RoutineIntent::Idle;
            };
            let Some(next) = next_step_toward(ctx, actor, from, goal) else {
                return RoutineIntent::Idle;
            };
            if next == goal {
                brain.roam_goal = None;
            }
            RoutineIntent::Step(next)
        }
        NpcRoutineDef::Patrol { stops } => {
            if consume_alarmed_settle(brain) {
                return RoutineIntent::Idle;
            }
            if stops.is_empty() {
                return RoutineIntent::Idle;
            }
            let mut goal = patrol_goal(brain, stops).unwrap_or(from);
            if from == goal {
                let stop = stops.get(brain.patrol_next_stop as usize % stops.len()).unwrap();
                if brain.patrol_wait_ticks < stop.wait_ticks {
                    brain.patrol_wait_ticks = brain.patrol_wait_ticks.saturating_add(1);
                    return RoutineIntent::Idle;
                }
                brain.patrol_wait_ticks = 0;
                brain.patrol_next_stop =
                    ((brain.patrol_next_stop as usize + 1) % stops.len()) as u16;
                goal = patrol_goal(brain, stops).unwrap_or(from);
            }
            next_step_toward(ctx, actor, from, goal)
                .map(RoutineIntent::Step)
                .unwrap_or(RoutineIntent::Idle)
        }
    }
}

/// Advance roam/patrol/idle and map to an exploration [`NpcAction`].
#[must_use]
pub(crate) fn routine_action(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    actor_pos: GridPos,
    routine: NpcRoutineDef,
    constraints: ActionConstraints,
    clock: Option<&CombatState>,
) -> Option<NpcAction> {
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
        RoutineIntent::Idle => Some(NpcAction::Idle),
        RoutineIntent::Step(target) => {
            if constraints.in_turn_session() {
                let clock = clock?;
                if move_cost_units(actor_pos, target)
                    .is_some_and(|cost| clock.current_ap_units().unwrap_or(0) >= cost)
                {
                    Some(NpcAction::roam_step(target))
                } else {
                    Some(NpcAction::Pass)
                }
            } else {
                Some(NpcAction::roam_step(target))
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn routine_tick_for_test(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    from: GridPos,
    routine: NpcRoutineDef,
    brain: &mut NpcBrainState,
) -> Option<GridPos> {
    match routine_tick(ctx, actor, from, routine, brain) {
        RoutineIntent::Idle => None,
        RoutineIntent::Step(target) => Some(target),
    }
}
