use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::entity::{EntityArena, EntityId, GridPos};

use super::MapGrid;

const ORTHOGONAL_COST: u32 = 10;
const DIAGONAL_COST: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    Unreachable,
    InvalidStart,
    InvalidGoal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathPlan {
    pub path: Vec<GridPos>,
    pub reached_goal: bool,
}

pub fn plan_path(
    map: &MapGrid,
    entities: &EntityArena,
    start: GridPos,
    goal: GridPos,
    mover: Option<EntityId>,
    allow_partial: bool,
    max_cost: u32,
) -> Result<PathPlan, PathError> {
    find_path(
        map,
        entities,
        start,
        goal,
        mover,
        allow_partial,
        max_cost,
        |_, _| false,
    )
}

pub fn plan_path_player_fow(
    map: &MapGrid,
    entities: &EntityArena,
    explored: &[bool],
    start: GridPos,
    goal: GridPos,
    mover: Option<EntityId>,
    allow_partial: bool,
    max_cost: u32,
) -> Result<PathPlan, PathError> {
    find_path(
        map,
        entities,
        start,
        goal,
        mover,
        allow_partial,
        max_cost,
        |x, y| {
            let idx = y as usize * map.width as usize + x as usize;
            !explored.get(idx).copied().unwrap_or(false)
        },
    )
}

#[must_use]
pub fn first_step_on_line(from: GridPos, to: GridPos) -> Option<GridPos> {
    bresenham_line(from, to).get(1).copied()
}

fn find_path(
    map: &MapGrid,
    entities: &EntityArena,
    start: GridPos,
    goal: GridPos,
    mover: Option<EntityId>,
    allow_partial: bool,
    max_cost: u32,
    mut treat_as_passable: impl FnMut(i32, i32) -> bool,
) -> Result<PathPlan, PathError> {
    if !map.in_bounds(start.x, start.y) {
        return Err(PathError::InvalidStart);
    }
    if !map.in_bounds(goal.x, goal.y) {
        return Err(PathError::InvalidGoal);
    }
    if start == goal {
        return Ok(PathPlan {
            path: vec![start],
            reached_goal: true,
        });
    }
    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), u32> = HashMap::new();
    let start_key = (start.x, start.y);
    open.push(OpenNode {
        pos: start_key,
        g: 0,
        f: octile_cost(start_key, (goal.x, goal.y)),
    });
    came_from.insert(start_key, start_key);
    g_score.insert(start_key, 0);

    let mut best = start_key;
    let mut best_dist = euclidean_distance_sq(start_key, (goal.x, goal.y));
    let mut best_cost = 0;

    while let Some(node) = open.pop() {
        if node.pos == (goal.x, goal.y) {
            let raw_path = reconstruct_path(start, goal, &came_from);
            let path = smooth_path(map, entities, mover, raw_path, &mut treat_as_passable);
            return Ok(PathPlan {
                path,
                reached_goal: true,
            });
        }
        if node.g > max_cost {
            continue;
        }
        if let Some(stored) = g_score.get(&node.pos) {
            if node.g > *stored {
                continue;
            }
        }
        for (dx, dy, step_cost) in neighbors8() {
            let nx = node.pos.0 + dx;
            let ny = node.pos.1 + dy;
            if !map.in_bounds(nx, ny) {
                continue;
            }
            if !is_walkable(map, entities, mover, nx, ny, &mut treat_as_passable) {
                continue;
            }
            if dx != 0
                && dy != 0
                && (!is_walkable(
                    map,
                    entities,
                    mover,
                    node.pos.0 + dx,
                    node.pos.1,
                    &mut treat_as_passable,
                ) || !is_walkable(
                    map,
                    entities,
                    mover,
                    node.pos.0,
                    node.pos.1 + dy,
                    &mut treat_as_passable,
                ))
            {
                continue;
            }
            let tentative_g = node.g.saturating_add(step_cost);
            if tentative_g > max_cost {
                continue;
            }
            let key = (nx, ny);
            if g_score.get(&key).is_some_and(|old| tentative_g >= *old) {
                continue;
            }
            came_from.insert(key, node.pos);
            g_score.insert(key, tentative_g);

            let d = euclidean_distance_sq(key, (goal.x, goal.y));
            if d < best_dist || (d == best_dist && tentative_g < best_cost) {
                best_dist = d;
                best = key;
                best_cost = tentative_g;
            }
            open.push(OpenNode {
                pos: key,
                g: tentative_g,
                f: tentative_g.saturating_add(octile_cost(key, (goal.x, goal.y))),
            });
        }
    }

    if !allow_partial || best == start_key {
        return Err(PathError::Unreachable);
    }
    let raw_path = reconstruct_path(
        start,
        GridPos {
            x: best.0,
            y: best.1,
        },
        &came_from,
    );
    let path = smooth_path(map, entities, mover, raw_path, &mut treat_as_passable);
    Ok(PathPlan {
        path,
        reached_goal: best == (goal.x, goal.y),
    })
}

fn euclidean_distance_sq(a: (i32, i32), b: (i32, i32)) -> i64 {
    let dx = i64::from(a.0 - b.0);
    let dy = i64::from(a.1 - b.1);
    dx * dx + dy * dy
}

fn octile_cost(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    let diag = dx.min(dy);
    let straight = dx.max(dy) - diag;
    DIAGONAL_COST * diag + ORTHOGONAL_COST * straight
}

fn is_walkable(
    map: &MapGrid,
    entities: &EntityArena,
    mover: Option<EntityId>,
    x: i32,
    y: i32,
    treat_as_passable: &mut impl FnMut(i32, i32) -> bool,
) -> bool {
    let blocked = if treat_as_passable(x, y) {
        false
    } else {
        map.blocks_movement(x, y)
    };
    entities.can_move_to(blocked, GridPos { x, y }, mover)
}

fn smooth_path(
    map: &MapGrid,
    entities: &EntityArena,
    mover: Option<EntityId>,
    path: Vec<GridPos>,
    treat_as_passable: &mut impl FnMut(i32, i32) -> bool,
) -> Vec<GridPos> {
    if path.len() <= 2 {
        return path;
    }
    let mut out = Vec::with_capacity(path.len());
    out.push(path[0]);
    let mut anchor_idx = 0usize;
    while anchor_idx < path.len() - 1 {
        let mut furthest = anchor_idx + 1;
        for candidate in (anchor_idx + 2)..path.len() {
            if has_line_of_sight(
                map,
                entities,
                mover,
                path[anchor_idx],
                path[candidate],
                treat_as_passable,
            ) {
                furthest = candidate;
            } else {
                break;
            }
        }
        out.push(path[furthest]);
        anchor_idx = furthest;
    }
    out
}

fn has_line_of_sight(
    map: &MapGrid,
    entities: &EntityArena,
    mover: Option<EntityId>,
    from: GridPos,
    to: GridPos,
    treat_as_passable: &mut impl FnMut(i32, i32) -> bool,
) -> bool {
    let line = bresenham_line(from, to);
    if line.is_empty() {
        return false;
    }
    for &p in line.iter().skip(1) {
        if !is_walkable(map, entities, mover, p.x, p.y, treat_as_passable) {
            return false;
        }
    }
    for pair in line.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        if dx.abs() > 1 || dy.abs() > 1 || (dx == 0 && dy == 0) {
            return false;
        }
        if dx != 0
            && dy != 0
            && (!is_walkable(map, entities, mover, a.x + dx, a.y, treat_as_passable)
                || !is_walkable(map, entities, mover, a.x, a.y + dy, treat_as_passable))
        {
            return false;
        }
    }
    true
}

/// Bresenham tile ray from `from` through `to` (inclusive).
#[must_use]
pub fn bresenham_tile_line(from: GridPos, to: GridPos) -> Vec<GridPos> {
    bresenham_line(from, to)
}

/// True if every cell **strictly between** `from` and `to` does not block sight.
/// Adjacent endpoints (`line` length 2) have no interior tiles to check.
#[must_use]
pub fn projectile_sight_clear(map: &MapGrid, from: GridPos, to: GridPos) -> bool {
    let cells = bresenham_line(from, to);
    if cells.len() < 2 {
        return false;
    }
    for p in cells.iter().skip(1).take(cells.len().saturating_sub(2)) {
        if map.blocks_sight(p.x, p.y) {
            return false;
        }
    }
    true
}

fn bresenham_line(from: GridPos, to: GridPos) -> Vec<GridPos> {
    let mut out = Vec::new();
    let mut x0 = from.x;
    let mut y0 = from.y;
    let x1 = to.x;
    let y1 = to.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        out.push(GridPos { x: x0, y: y0 });
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    out
}

fn neighbors8() -> [(i32, i32, u32); 8] {
    [
        (0, -1, ORTHOGONAL_COST),
        (1, 0, ORTHOGONAL_COST),
        (0, 1, ORTHOGONAL_COST),
        (-1, 0, ORTHOGONAL_COST),
        (1, -1, DIAGONAL_COST),
        (1, 1, DIAGONAL_COST),
        (-1, 1, DIAGONAL_COST),
        (-1, -1, DIAGONAL_COST),
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenNode {
    pos: (i32, i32),
    g: u32,
    f: u32,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .cmp(&self.f)
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| self.pos.cmp(&other.pos))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
    use super::{plan_path, plan_path_player_fow};
    use crate::entity::{EntityArena, GridPos};
    use crate::world::MapGrid;
    use crate::world::TileTable;

    #[test]
    fn shortest_path_finds_route_in_open_grid() {
        let map = MapGrid::filled(
            5,
            5,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 1 };
        let goal = GridPos { x: 3, y: 1 };
        let plan =
            plan_path(&map, &arena, start, goal, None, false, u32::MAX).expect("path should exist");
        assert_eq!(plan.path.first().copied(), Some(start));
        assert_eq!(plan.path.last().copied(), Some(goal));
        assert!(plan.reached_goal);
    }

    #[test]
    fn closest_path_returns_partial_when_goal_blocked() {
        let mut map = MapGrid::filled(
            5,
            5,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        // Create a wall line at x=3 except one side route blocked by map bounds.
        map.set_tile(3, 0, 1);
        map.set_tile(3, 1, 1);
        map.set_tile(3, 2, 1);
        map.set_tile(3, 3, 1);
        map.set_tile(3, 4, 1);
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 2 };
        let goal = GridPos { x: 4, y: 2 };
        let plan = plan_path(&map, &arena, start, goal, None, true, u32::MAX)
            .expect("closest route should still exist");
        assert!(!plan.reached_goal);
        assert_ne!(plan.path.last().copied(), Some(goal));
    }

    #[test]
    fn custom_blocking_policy_can_treat_unknown_as_passable() {
        let mut map = MapGrid::filled(
            5,
            5,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        map.set_tile(2, 1, 1);
        let explored = vec![
            true, true, false, false, false, true, true, false, false, false, true, true, false,
            false, false, true, true, false, false, false, true, true, false, false, false,
        ];
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 1 };
        let goal = GridPos { x: 4, y: 1 };
        let plan = plan_path_player_fow(&map, &arena, &explored, start, goal, None, true, u32::MAX)
            .expect("path should exist under unknown-passable policy");
        assert!(plan.reached_goal);
        assert_eq!(plan.path.last().copied(), Some(goal));
    }

    #[test]
    fn prefers_diagonal_when_open() {
        let map = MapGrid::filled(
            5,
            5,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 1 };
        let goal = GridPos { x: 3, y: 3 };
        let plan =
            plan_path(&map, &arena, start, goal, None, false, u32::MAX).expect("path should exist");
        assert!(plan.path.len() <= 3, "expected diagonal path");
    }

    #[test]
    fn smoothing_collapses_to_single_segment_in_open_space() {
        let map = MapGrid::filled(
            8,
            8,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 1 };
        let goal = GridPos { x: 6, y: 4 };
        let plan =
            plan_path(&map, &arena, start, goal, None, false, u32::MAX).expect("path should exist");
        assert_eq!(plan.path.first().copied(), Some(start));
        assert_eq!(plan.path.last().copied(), Some(goal));
        assert_eq!(
            plan.path.len(),
            2,
            "open map should smooth to direct segment"
        );
    }

    #[test]
    fn smoothing_respects_corner_blocking() {
        let mut map = MapGrid::filled(
            6,
            6,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        map.set_tile(2, 1, 1);
        map.set_tile(1, 2, 1);
        let arena = EntityArena::new();
        let start = GridPos { x: 1, y: 1 };
        let goal = GridPos { x: 3, y: 3 };
        let plan = plan_path(&map, &arena, start, goal, None, true, u32::MAX)
            .expect("partial route should exist");
        assert!(
            plan.path.len() > 2 || !plan.reached_goal,
            "smoothing must not cut diagonally through blocked corner"
        );
    }
}
