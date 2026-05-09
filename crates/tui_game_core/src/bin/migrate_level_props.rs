//! One-shot helper: move wall/tree tile indices from `tiles` (ground) into `props`, inferring ground.
//!
//! Usage: `cargo run -p tui_game_core --bin migrate_level_props -- path/to/level.ron`
//! Writes the same path (pretty RON). Tile indices must match `demo_terrain_pack.ron` order.

use std::collections::HashSet;
use std::env;
use std::path::Path;

use tui_game_core::level::{level_from_ron, level_to_ron, materialize_tile_defs_from_pack};
use tui_game_core::world::EMPTY_PROP_ID;

/// Pack order: 0 floor, 1 wall, 2 grass, 3 water, 4–7 connector walls, 8 tree.
fn prop_tile_ids() -> HashSet<u16> {
    [1u16, 4, 5, 6, 7, 8].into_iter().collect()
}

fn infer_ground(i: usize, orig: &[u16], w: usize, h: usize, prop_ids: &HashSet<u16>) -> u16 {
    const PREFER: &[u16] = &[2, 3, 0];
    let x = (i % w) as i32;
    let y = (i / w) as i32;
    let mut seen = Vec::new();
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let ni = ny as usize * w + nx as usize;
            let t = orig[ni];
            if !prop_ids.contains(&t) {
                seen.push(t);
            }
        }
    }
    for &p in PREFER {
        if seen.iter().any(|&t| t == p) {
            return p;
        }
    }
    seen.first().copied().unwrap_or(2)
}

fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| "usage: migrate_level_props <level.ron>".to_string())?;
    let p = Path::new(&path);
    let raw = std::fs::read_to_string(p).map_err(|e| format!("read: {e}"))?;
    let mut level = level_from_ron(&raw).map_err(|e| format!("parse: {e}"))?;
    materialize_tile_defs_from_pack(&mut level, p.parent())
        .map_err(|e| format!("terrain pack: {e}"))?;

    let prop_ids = prop_tile_ids();
    let w = level.width as usize;
    let h = level.height as usize;
    let n = w * h;
    if level.tiles.len() != n {
        return Err(format!("tiles len {} != {n}", level.tiles.len()));
    }

    let orig = level.tiles.clone();
    let mut props = vec![EMPTY_PROP_ID; n];
    for i in 0..n {
        let t = orig[i];
        if prop_ids.contains(&t) {
            props[i] = t;
            level.tiles[i] = infer_ground(i, &orig, w, h, &prop_ids);
        } else {
            props[i] = EMPTY_PROP_ID;
        }
    }
    level.props = props;

    let out = level_to_ron(&level).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(p, out).map_err(|e| format!("write: {e}"))?;
    println!("OK: wrote {path}");
    Ok(())
}
