use serde::{Deserialize, Serialize};

use super::tile_surface::{bake_tile_display, TileBakeView, TileDisplayCell};
use super::tiles::{TileDef, TileId};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TileTable {
    pub defs: Vec<TileDef>,
}

impl TileTable {
    pub fn default_pack() -> Result<Self, String> {
        let level = crate::game_content::embedded_demo_level();
        if level.tile_defs.is_empty() {
            return Err("embedded demo level has no tile_defs".into());
        }
        Ok(Self {
            defs: level.tile_defs,
        })
    }

    pub fn def(&self, id: TileId) -> Option<&TileDef> {
        self.defs.iter().find(|d| d.id == id)
    }

    pub fn index_of(&self, id: TileId) -> Option<usize> {
        self.defs.iter().position(|d| d.id == id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MapGrid {
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<TileId>,
    pub table: TileTable,
    /// Baked glyph/fg per cell (static surfaces). Rebuilt at load and after edits; animated
    /// tiles still read `tiles` + defs at compose time.
    #[serde(default)]
    pub display: Vec<TileDisplayCell>,
    /// Per-cell ambiance weight `0..=255`, same length as `tiles` when present (filled with `0` on load).
    #[serde(default)]
    pub ambiance: Vec<u8>,
}

impl MapGrid {
    pub fn filled(width: u16, height: u16, tile: TileId, table: TileTable) -> Self {
        let n = (width as usize) * (height as usize);
        let mut m = Self {
            width,
            height,
            tiles: vec![tile; n],
            table,
            display: vec![TileDisplayCell::default(); n],
            ambiance: vec![0u8; n],
        };
        m.rebuild_display_cache(0);
        m
    }

    /// Full recompute of [`MapGrid::display`] (e.g. level load, resize, save load).
    pub fn rebuild_display_cache(&mut self, visual_seed: u64) {
        let n = (self.width as usize) * (self.height as usize);
        if self.display.len() != n {
            self.display.resize(n, TileDisplayCell::default());
        }
        let v = TileBakeView {
            table: &self.table,
            tiles: &self.tiles,
            width: self.width,
            height: self.height,
        };
        for y in 0..self.height {
            for x in 0..self.width {
                let wx = i32::from(x);
                let wy = i32::from(y);
                let i = wy as usize * self.width as usize + wx as usize;
                self.display[i] = bake_tile_display(v, wx, wy, visual_seed);
            }
        }
    }

    /// After a single-cell edit, refresh this cell and neighbors (connector mask locality).
    pub fn rebuild_display_local(&mut self, wx: i32, wy: i32, visual_seed: u64) {
        let v = TileBakeView {
            table: &self.table,
            tiles: &self.tiles,
            width: self.width,
            height: self.height,
        };
        for (x, y) in [
            (wx, wy),
            (wx, wy - 1),
            (wx + 1, wy),
            (wx, wy + 1),
            (wx - 1, wy),
        ] {
            if !self.in_bounds(x, y) {
                continue;
            }
            let i = y as usize * self.width as usize + x as usize;
            if i < self.display.len() {
                self.display[i] = bake_tile_display(v, x, y, visual_seed);
            }
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u16) < self.width && (y as u16) < self.height
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<TileId> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.tiles.get(i).copied()
    }

    pub fn set_tile(&mut self, x: i32, y: i32, t: TileId) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.tiles[i] = t;
        true
    }

    /// Like [`set_tile`] then [`rebuild_display_local`] (pass `visual_seed` from level / editor).
    pub fn set_tile_with_display(&mut self, x: i32, y: i32, t: TileId, visual_seed: u64) -> bool {
        if !self.set_tile(x, y, t) {
            return false;
        }
        self.rebuild_display_local(x, y, visual_seed);
        true
    }

    pub fn blocks_movement(&self, x: i32, y: i32) -> bool {
        self.tile_at(x, y)
            .and_then(|id| self.table.def(id))
            .map(|d| d.blocks_movement)
            .unwrap_or(true)
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.tile_at(x, y)
            .and_then(|id| self.table.def(id))
            .map(|d| d.blocks_sight)
            .unwrap_or(true)
    }
}
