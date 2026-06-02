//! Exploration-phase NPC routines (idle, roam, patrol).

use crate::content::{NpcRoutineDef, PatrolStopDef};
use crate::entity::{EntityId, GridPos, NpcBrainState};
use crate::math::{chebyshev, lcg_next_u32};

use super::ctx::BehaviorCtx;
use super::navigation::next_step_toward;

/// What an NPC chose to do on the exploration map this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplorationIntent {
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

fn patrol_goal(state: &NpcBrainState, stops: &[PatrolStopDef]) -> Option<GridPos> {
    let stop = stops.get(state.patrol_next_stop as usize % stops.len())?;
    Some(GridPos {
        x: state.home.x + i32::from(stop.dx),
        y: state.home.y + i32::from(stop.dy),
    })
}

/// Advance roam/patrol/idle state and return the next grid step, if any.
#[must_use]
pub fn routine_tick(
    ctx: &mut BehaviorCtx<'_>,
    actor: EntityId,
    from: GridPos,
    routine: NpcRoutineDef,
    brain: &mut NpcBrainState,
) -> ExplorationIntent {
    match routine {
        NpcRoutineDef::Idle => ExplorationIntent::Idle,
        NpcRoutineDef::Roam { radius, wait_ticks } => {
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
                    return ExplorationIntent::Idle;
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
                return ExplorationIntent::Idle;
            };
            let Some(next) = next_step_toward(ctx, actor, from, goal) else {
                return ExplorationIntent::Idle;
            };
            if next == goal {
                brain.roam_goal = None;
            }
            ExplorationIntent::Step(next)
        }
        NpcRoutineDef::Patrol { stops } => {
            if stops.is_empty() {
                return ExplorationIntent::Idle;
            }
            let mut goal = patrol_goal(brain, stops).unwrap_or(from);
            if from == goal {
                let stop = stops.get(brain.patrol_next_stop as usize % stops.len()).unwrap();
                if brain.patrol_wait_ticks < stop.wait_ticks {
                    brain.patrol_wait_ticks = brain.patrol_wait_ticks.saturating_add(1);
                    return ExplorationIntent::Idle;
                }
                brain.patrol_wait_ticks = 0;
                brain.patrol_next_stop =
                    ((brain.patrol_next_stop as usize + 1) % stops.len()) as u16;
                goal = patrol_goal(brain, stops).unwrap_or(from);
            }
            next_step_toward(ctx, actor, from, goal)
                .map(ExplorationIntent::Step)
                .unwrap_or(ExplorationIntent::Idle)
        }
    }
}
