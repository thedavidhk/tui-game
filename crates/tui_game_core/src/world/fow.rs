//! Simple grid FoW: Bresenham ray per target cell from origin (classic roguelike style).

use crate::math::euclidean_dist_sq;

use super::map::MapGrid;

/// Bitset for `width * height` cells: visible this frame.
pub fn compute_visible(map: &MapGrid, origin_x: i32, origin_y: i32, radius: i32, out: &mut [bool]) {
    let n = (map.width as usize) * (map.height as usize);
    if out.len() < n {
        return;
    }
    out[..n].fill(false);

    let r = radius.max(0);
    let x0 = origin_x.saturating_sub(r);
    let y0 = origin_y.saturating_sub(r);
    let x1 = origin_x.saturating_add(r);
    let y1 = origin_y.saturating_add(r);

    for ty in y0..=y1 {
        for tx in x0..=x1 {
            let dx = tx - origin_x;
            let dy = ty - origin_y;
            let r_sq = i64::from(r) * i64::from(r);
            if euclidean_dist_sq(dx, dy) > r_sq {
                continue;
            }
            if los_clear(map, origin_x, origin_y, tx, ty) {
                if let Some(i) = map_index(map, tx, ty) {
                    out[i] = true;
                }
            }
        }
    }
}

fn map_index(map: &MapGrid, x: i32, y: i32) -> Option<usize> {
    if !map.in_bounds(x, y) {
        return None;
    }
    Some(y as usize * map.width as usize + x as usize)
}

/// Bresenham line: visible if every cell before the target is transparent; target may be opaque.
fn los_clear(map: &MapGrid, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let max_steps = (map.width + map.height) as i32 + 4;
    for _ in 0..max_steps {
        if x == x1 && y == y1 {
            return true;
        }
        if map.blocks_sight(x, y) {
            return false;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    false
}

/// Mark explored where visible OR already explored (caller merges).
pub fn merge_explored(map: &MapGrid, visible: &[bool], explored: &mut [bool]) {
    let n = (map.width as usize) * (map.height as usize);
    for i in 0..n.min(visible.len()).min(explored.len()) {
        if visible[i] {
            explored[i] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::TileTable;

    #[test]
    fn origin_visible_open_map() {
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let map = MapGrid::filled(7, 7, 0, table);
        let mut vis = vec![false; 49];
        compute_visible(&map, 3, 3, 3, &mut vis);
        assert!(vis[3 + 3 * 7]);
    }

    #[test]
    fn wall_blocks_los() {
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let mut map = MapGrid::filled(7, 7, 0, table);
        map.set_tile(3, 3, 1);
        let mut vis = vec![false; 49];
        compute_visible(&map, 3, 0, 4, &mut vis);
        assert!(vis[3]);
        assert!(
            !vis[3 + 5 * 7],
            "cell behind wall along column should stay hidden"
        );
    }
}
