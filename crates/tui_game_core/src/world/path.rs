use std::collections::{HashMap, VecDeque};

use crate::entity::{EntityArena, EntityId, GridPos};

use super::MapGrid;

#[derive(Clone, Copy, Debug)]
pub struct PathQueryCtx<'a> {
    pub map: &'a MapGrid,
    pub entities: &'a EntityArena,
}

#[derive(Clone, Copy, Debug)]
pub struct PathRequest {
    pub start: GridPos,
    pub goal: GridPos,
    pub mover: Option<EntityId>,
    pub max_steps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    Unreachable,
    InvalidStart,
    InvalidGoal,
}

pub fn shortest_path(ctx: &PathQueryCtx<'_>, req: &PathRequest) -> Result<Vec<GridPos>, PathError> {
    if !ctx.map.in_bounds(req.start.x, req.start.y) {
        return Err(PathError::InvalidStart);
    }
    if !ctx.map.in_bounds(req.goal.x, req.goal.y) {
        return Err(PathError::InvalidGoal);
    }
    if req.start == req.goal {
        return Ok(vec![req.start]);
    }

    let mut q = VecDeque::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    q.push_back((req.start.x, req.start.y, 0u16));
    came_from.insert((req.start.x, req.start.y), (req.start.x, req.start.y));

    while let Some((x, y, steps)) = q.pop_front() {
        if steps >= req.max_steps {
            continue;
        }
        for (nx, ny) in [(x, y - 1), (x + 1, y), (x, y + 1), (x - 1, y)] {
            if !ctx.map.in_bounds(nx, ny) {
                continue;
            }
            if came_from.contains_key(&(nx, ny)) {
                continue;
            }
            let blocked = ctx.map.blocks_movement(nx, ny);
            if !ctx.entities.can_move_to(blocked, GridPos { x: nx, y: ny }, req.mover) {
                continue;
            }
            came_from.insert((nx, ny), (x, y));
            if nx == req.goal.x && ny == req.goal.y {
                return Ok(reconstruct_path(req.start, req.goal, &came_from));
            }
            q.push_back((nx, ny, steps.saturating_add(1)));
        }
    }

    Err(PathError::Unreachable)
}

pub fn next_step_toward(ctx: &PathQueryCtx<'_>, req: &PathRequest) -> Result<GridPos, PathError> {
    let path = shortest_path(ctx, req)?;
    path.get(1).copied().ok_or(PathError::Unreachable)
}

fn reconstruct_path(
    start: GridPos,
    goal: GridPos,
    came_from: &HashMap<(i32, i32), (i32, i32)>,
) -> Vec<GridPos> {
    let mut current = (goal.x, goal.y);
    let mut path_rev = vec![GridPos {
        x: current.0,
        y: current.1,
    }];
    while current != (start.x, start.y) {
        if let Some(prev) = came_from.get(&current).copied() {
            current = prev;
            path_rev.push(GridPos {
                x: current.0,
                y: current.1,
            });
        } else {
            break;
        }
    }
    path_rev.reverse();
    path_rev
}

#[cfg(test)]
mod tests {
    use super::{next_step_toward, shortest_path, PathQueryCtx, PathRequest};
    use crate::entity::{EntityArena, GridPos};
    use crate::world::TileTable;
    use crate::world::MapGrid;

    #[test]
    fn shortest_path_finds_route_in_open_grid() {
        let map = MapGrid::filled(5, 5, 0, TileTable::default_pack());
        let arena = EntityArena::new();
        let ctx = PathQueryCtx {
            map: &map,
            entities: &arena,
        };
        let req = PathRequest {
            start: GridPos { x: 1, y: 1 },
            goal: GridPos { x: 3, y: 1 },
            mover: None,
            max_steps: 10,
        };
        let path = shortest_path(&ctx, &req).expect("path should exist");
        assert_eq!(path.first().copied(), Some(req.start));
        assert_eq!(path.last().copied(), Some(req.goal));
    }

    #[test]
    fn next_step_returns_immediate_neighbor() {
        let map = MapGrid::filled(5, 5, 0, TileTable::default_pack());
        let arena = EntityArena::new();
        let ctx = PathQueryCtx {
            map: &map,
            entities: &arena,
        };
        let req = PathRequest {
            start: GridPos { x: 1, y: 1 },
            goal: GridPos { x: 4, y: 1 },
            mover: None,
            max_steps: 10,
        };
        let step = next_step_toward(&ctx, &req).expect("next step should exist");
        assert!(step == GridPos { x: 2, y: 1 } || step == GridPos { x: 1, y: 2 });
    }
}
