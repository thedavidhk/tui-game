//! Baked tile appearance: weighted static variants, connector masks, and runtime animation.
//!
//! Static visuals are resolved in [`crate::world::map::MapGrid::rebuild_display_cache`]; only
//! [`resolve_animated`] runs each frame for animated tile types.

use crate::render::Color;

use super::map::TileTable;
use super::tiles::{AnimMode, TileDef, TileId, TileSurface, WeightedGlyph};

#[inline]
fn cell_from_def(ch: char, fg: Color, def: &TileDef) -> TileDisplayCell {
    TileDisplayCell {
        ch,
        fg,
        bg: def.terrain_bg(),
    }
}

use serde::{Deserialize, Serialize};

/// Resolved glyph + colors for one map cell (terrain `bg` is baked; FoW and ambiance adjust at compose).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TileDisplayCell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
}

impl Default for TileDisplayCell {
    fn default() -> Self {
        Self {
            ch: '.',
            fg: Color::rgb(180, 180, 170),
            bg: Color::rgb(14, 14, 20),
        }
    }
}

#[inline]
pub fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 32;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 32;
    x
}

#[inline]
pub fn hash_cell(visual_seed: u64, wx: i32, wy: i32, tag: u32) -> u64 {
    let a = wx as u64;
    let b = wy as u64;
    mix64(
        visual_seed
            ^ a.wrapping_mul(0x9E3779B185EBCA87)
            ^ b.rotate_left(17)
            ^ ((tag as u64) << 32),
    )
}

/// Borrowed grid slice for baking without depending on [`crate::world::map::MapGrid`].
#[derive(Clone, Copy)]
pub struct TileBakeView<'a> {
    pub table: &'a TileTable,
    pub tiles: &'a [TileId],
    pub width: u16,
    pub height: u16,
}

impl<'a> TileBakeView<'a> {
    #[inline]
    pub fn in_bounds(self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u16) < self.width && (y as u16) < self.height
    }

    #[inline]
    pub fn tile_at(self, x: i32, y: i32) -> Option<TileId> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.tiles.get(i).copied()
    }
}

/// True when this tile uses per-tick [`resolve_animated`] instead of the baked display cache.
#[must_use]
pub fn def_is_animated(def: &TileDef) -> bool {
    matches!(&def.surface, Some(TileSurface::Animated { .. }))
}

#[must_use]
pub fn links_for_mask(v: TileBakeView<'_>, wx: i32, wy: i32, dx: i32, dy: i32) -> bool {
    let nx = wx + dx;
    let ny = wy + dy;
    if !v.in_bounds(nx, ny) {
        return false;
    }
    let Some(ta) = v.tile_at(wx, wy) else {
        return false;
    };
    let Some(tb) = v.tile_at(nx, ny) else {
        return false;
    };
    let Some(a) = v.table.def(ta) else {
        return false;
    };
    let Some(b) = v.table.def(tb) else {
        return false;
    };
    a.connect_mask != 0 && b.connect_mask != 0 && (a.connect_mask & b.connect_mask) != 0
}

/// Cardinal link bitmask for connector autotiles: **bit0 = N**, **bit1 = E**, **bit2 = S**, **bit3 = W**
/// (index `mask` into a 16-entry `glyphs` table is `mask` = N + 2×E + 4×S + 8×W, values `0..=15`).
///
/// Canonical `glyphs` order (wall segments open toward missing neighbors):
///
/// | `mask` | bits (N E S W) | typical |
/// |--------|----------------|---------|
/// | 0 | 0000 | isolated `·` |
/// | 1 | 0001 | `╵` (N only) |
/// | 2 | 0010 | `╶` (E only) |
/// | 3 | 0011 | `└` (N+E) |
/// | 4 | 0100 | `╷` (S only) |
/// | 5 | 0101 | `│` (N+S) |
/// | 6 | 0110 | `┌` (S+E) |
/// | 7 | 0111 | `├` (N+E+S) |
/// | 8 | 1000 | `╴` (W only) |
/// | 9 | 1001 | `┘` (N+W) |
/// | 10 | 1010 | `─` (E+W) |
/// | 11 | 1011 | `┴` (N+E+W) |
/// | 12 | 1100 | `┐` (S+W) |
/// | 13 | 1101 | `┤` (N+S+W) |
/// | 14 | 1110 | `┬` (E+S+W) |
/// | 15 | 1111 | `┼` (all) |
#[must_use]
pub fn neighbor_link_mask(v: TileBakeView<'_>, wx: i32, wy: i32) -> u8 {
    let mut m = 0u8;
    if links_for_mask(v, wx, wy, 0, -1) {
        m |= 1;
    }
    if links_for_mask(v, wx, wy, 1, 0) {
        m |= 2;
    }
    if links_for_mask(v, wx, wy, 0, 1) {
        m |= 4;
    }
    if links_for_mask(v, wx, wy, -1, 0) {
        m |= 8;
    }
    m
}

fn pick_weighted(entries: &[WeightedGlyph], h: u64) -> Option<(char, Color)> {
    let total: u64 = entries.iter().map(|e| e.weight as u64).sum();
    if total == 0 {
        return None;
    }
    let r = h % total;
    let mut acc: u64 = 0;
    for e in entries {
        acc += e.weight as u64;
        if r < acc {
            return Some((e.ch, e.fg));
        }
    }
    entries.last().map(|e| (e.ch, e.fg))
}

fn connector_glyph(glyphs: &[char], mask: usize, fallback: char) -> char {
    if glyphs.len() >= 16 {
        glyphs[mask.min(15)]
    } else if !glyphs.is_empty() {
        glyphs[mask.min(glyphs.len() - 1)]
    } else {
        fallback
    }
}

/// Bake static appearance for one cell (not used for animated tiles except placeholder).
#[must_use]
pub fn bake_tile_display(v: TileBakeView<'_>, wx: i32, wy: i32, visual_seed: u64) -> TileDisplayCell {
    let tid = v.tile_at(wx, wy).unwrap_or(0);
    let Some(def) = v.table.def(tid) else {
        return TileDisplayCell {
            ch: '?',
            fg: Color::rgb(200, 100, 100),
            bg: Color::rgb(40, 10, 12),
        };
    };
    if def_is_animated(def) {
        return cell_from_def(def.glyph, def.fg, def);
    }
    match &def.surface {
        None => cell_from_def(def.glyph, def.fg, def),
        Some(TileSurface::StaticVariants { entries }) => {
            let h = hash_cell(visual_seed, wx, wy, tid as u32);
            pick_weighted(entries, h).map_or(
                cell_from_def(def.glyph, def.fg, def),
                |(ch, fg)| cell_from_def(ch, fg, def),
            )
        }
        Some(TileSurface::Connector { glyphs }) => {
            let mask = neighbor_link_mask(v, wx, wy) as usize;
            cell_from_def(connector_glyph(glyphs, mask, def.glyph), def.fg, def)
        }
        Some(TileSurface::Animated { .. }) => cell_from_def(def.glyph, def.fg, def),
    }
}

/// Runtime-only glyph for animated tiles (deterministic; no per-cell mutable state).
#[must_use]
pub fn resolve_animated(
    def: &TileDef,
    wx: i32,
    wy: i32,
    surface_tick: u64,
    visual_seed: u64,
) -> TileDisplayCell {
    let Some(TileSurface::Animated {
        frames,
        mode,
        ticks_per_frame,
        p_step_num,
        p_step_den,
    }) = &def.surface
    else {
        return cell_from_def(def.glyph, def.fg, def);
    };
    if frames.is_empty() {
        return cell_from_def(def.glyph, def.fg, def);
    }
    let n = frames.len();
    let phase = hash_cell(visual_seed, wx, wy, def.id as u32) as usize % n;
    let idx = match mode {
        AnimMode::Cycle => {
            let tpf = (*ticks_per_frame).max(1) as u64;
            let step = (surface_tick / tpf) as usize;
            (phase + step) % n
        }
        AnimMode::Drift => {
            let den = (*p_step_den).max(1) as u64;
            let num = (*p_step_num).min(den as u32) as u64;
            let adv = surface_tick
                .saturating_mul(num)
                .checked_div(den)
                .unwrap_or(0) as usize;
            (phase + adv) % n
        }
    };
    let f = &frames[idx];
    cell_from_def(f.ch, f.fg, def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::{MapGrid, TileTable};
    use crate::world::tiles::AnimatedFrame;

    #[test]
    fn mix64_stable() {
        assert_eq!(mix64(1), mix64(1));
        assert_ne!(mix64(1), mix64(2));
    }

    #[test]
    fn static_variant_deterministic_per_cell() {
        let mut table = TileTable::default_pack().expect("default terrain pack must load");
        table.defs.push(TileDef {
            id: 9,
            glyph: ',',
            blocks_movement: false,
            blocks_sight: false,
            name: "grass".into(),
            fg: Color::rgb(100, 200, 100),
            bg: None,
            connect_mask: 0,
            surface: Some(TileSurface::StaticVariants {
                entries: vec![
                    WeightedGlyph {
                        weight: 1,
                        ch: ',',
                        fg: Color::rgb(80, 160, 80),
                    },
                    WeightedGlyph {
                        weight: 1,
                        ch: '.',
                        fg: Color::rgb(100, 180, 90),
                    },
                ],
            }),
        });
        let map = MapGrid::filled(4, 4, 9, table);
        let v = TileBakeView {
            table: &map.table,
            tiles: &map.tiles,
            width: map.width,
            height: map.height,
        };
        let a = bake_tile_display(v, 0, 0, 12345);
        let b = bake_tile_display(v, 0, 0, 12345);
        let c = bake_tile_display(v, 1, 0, 12345);
        assert_eq!(a, b);
        assert_ne!(a.ch, '?' );
        // Different coords often differ (not strictly required).
        let _ = c;
    }

    #[test]
    fn connector_mask_center() {
        let mut table = TileTable::default_pack().expect("default terrain pack must load");
        // id 1 wall with connect 1
        table.defs[1].connect_mask = 1;
        table.defs.push(TileDef {
            id: 5,
            glyph: '#',
            blocks_movement: true,
            blocks_sight: true,
            name: "stone".into(),
            fg: Color::rgb(120, 120, 120),
            bg: None,
            connect_mask: 1,
            surface: Some(TileSurface::Connector {
                glyphs: (0..16).map(|i| char::from_u32('a' as u32 + i as u32).unwrap()).collect(),
            }),
        });
        let mut map = MapGrid::filled(3, 3, 0, table);
        map.set_tile(1, 0, 5);
        map.set_tile(1, 1, 5);
        map.set_tile(1, 2, 5);
        map.set_tile(0, 1, 5);
        map.set_tile(2, 1, 5);
        let v = TileBakeView {
            table: &map.table,
            tiles: &map.tiles,
            width: map.width,
            height: map.height,
        };
        let m = neighbor_link_mask(v, 1, 1);
        assert_eq!(m, 0b1111);
        let cell = bake_tile_display(v, 1, 1, 0);
        assert_eq!(cell.ch, 'p'); // mask 0b1111 = 15 -> 'a' + 15
    }

    #[test]
    fn animated_cycle_advances() {
        let def = TileDef {
            id: 7,
            glyph: '~',
            blocks_movement: false,
            blocks_sight: false,
            name: "water".into(),
            fg: Color::rgb(50, 80, 200),
            bg: None,
            connect_mask: 0,
            surface: Some(TileSurface::Animated {
                frames: vec![
                    AnimatedFrame {
                        ch: '~',
                        fg: Color::rgb(50, 100, 200),
                    },
                    AnimatedFrame {
                        ch: '≈',
                        fg: Color::rgb(70, 120, 220),
                    },
                ],
                mode: AnimMode::Cycle,
                ticks_per_frame: 1,
                p_step_num: 0,
                p_step_den: 60,
            }),
        };
        let a = resolve_animated(&def, 3, 4, 0, 99);
        let b = resolve_animated(&def, 3, 4, 1, 99);
        assert_ne!(a.ch, b.ch);
    }
}
