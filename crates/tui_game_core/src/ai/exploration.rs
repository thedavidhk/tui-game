use crate::content::{NpcRoutineDef, PatrolStopDef};
use crate::entity::{EntityArena, EntityId, GridPos, NpcBrainState};
use crate::math::{chebyshev, lcg_next_u32};
use crate::world::{first_step_on_line, plan_path, MapGrid};

fn random_inclusive(seed: &mut u64, lo: i32, hi: i32) -> i32 {
    let span = (hi - lo + 1).max(1) as u32;
    lo + (lcg_next_u32(seed) % span) as i32
}

fn step_toward(
    actor: EntityId,
    from: GridPos,
    goal: GridPos,
    map: &MapGrid,
    entities: &EntityArena,
) -> Option<GridPos> {
    let plan = plan_path(map, entities, from, goal, Some(actor), true, u32::MAX).ok()?;
    let waypoint = plan.path.get(1).copied()?;
    first_step_on_line(from, waypoint)
}

fn pick_roam_goal(
    state: &NpcBrainState,
    radius: u16,
    map: &MapGrid,
    entities: &EntityArena,
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

pub fn next_exploration_step(
    actor: EntityId,
    from: GridPos,
    routine: NpcRoutineDef,
    state: &mut NpcBrainState,
    map: &MapGrid,
    entities: &EntityArena,
    rng_seed: &mut u64,
) -> Option<GridPos> {
    match routine {
        NpcRoutineDef::Idle => None,
        NpcRoutineDef::Roam { radius, wait_ticks } => {
            let out_of_range = chebyshev(from, state.home) > i32::from(radius);
            if out_of_range {
                state.roam_goal = Some(state.home);
            }
            if state.roam_goal == Some(from) {
                state.roam_goal = None;
            }
            if state.roam_goal.is_none() {
                if state.roam_wait_ticks < wait_ticks {
                    state.roam_wait_ticks = state.roam_wait_ticks.saturating_add(1);
                    return None;
                }
                state.roam_wait_ticks = 0;
                state.roam_goal = pick_roam_goal(state, radius, map, entities, actor, rng_seed);
            }
            let goal = state.roam_goal?;
            let next = step_toward(actor, from, goal, map, entities)?;
            if next == goal {
                state.roam_goal = None;
            }
            Some(next)
        }
        NpcRoutineDef::Patrol { stops } => {
            if stops.is_empty() {
                return None;
            }
            let mut goal = patrol_goal(state, stops)?;
            if from == goal {
                let stop = stops.get(state.patrol_next_stop as usize % stops.len())?;
                if state.patrol_wait_ticks < stop.wait_ticks {
                    state.patrol_wait_ticks = state.patrol_wait_ticks.saturating_add(1);
                    return None;
                }
                state.patrol_wait_ticks = 0;
                state.patrol_next_stop =
                    ((state.patrol_next_stop as usize + 1) % stops.len()) as u16;
                goal = patrol_goal(state, stops)?;
            }
            step_toward(actor, from, goal, map, entities)
        }
    }
}
