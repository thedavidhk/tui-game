use serde::{Deserialize, Serialize};

use crate::level::{AtmosphereRecipe, AtmosphereZone};

use super::tile_surface::{
    bake_layered_tile_display, bake_tile_display, def_is_animated, resolve_animated, TileBakeView,
    TileDisplayCell,
};
use super::tiles::{TileDef, TileId, EMPTY_PROP_ID};

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
        self.defs.iter().find(|d| d.idx == id)
    }

    pub fn index_of(&self, id: TileId) -> Option<usize> {
        self.defs.iter().position(|d| d.idx == id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MapGrid {
    pub width: u16,
    pub height: u16,
    /// Ground terrain per cell (serialized as `tiles` for saves / legacy).
    #[serde(rename = "tiles")]
    pub ground: Vec<TileId>,
    /// Optional overlay (trees, …). [`EMPTY_PROP_ID`] = no overlay; background comes from ground.
    #[serde(default)]
    pub props: Vec<TileId>,
    pub table: TileTable,
    /// Baked glyph/fg per cell (static surfaces). Rebuilt at load and after edits; animated
    /// tiles still read `ground` / `props` + defs at compose time.
    #[serde(default)]
    pub display: Vec<TileDisplayCell>,
    #[serde(default)]
    pub default_atmosphere: AtmosphereRecipe,
    #[serde(default)]
    pub atmosphere_zones: Vec<AtmosphereZone>,
}

impl MapGrid {
    pub fn filled(width: u16, height: u16, ground_tile: TileId, table: TileTable) -> Self {
        let n = (width as usize) * (height as usize);
        let mut m = Self {
            width,
            height,
            ground: vec![ground_tile; n],
            props: vec![EMPTY_PROP_ID; n],
            table,
            display: vec![TileDisplayCell::default(); n],
            default_atmosphere: AtmosphereRecipe::default(),
            atmosphere_zones: Vec::new(),
        };
        m.rebuild_display_cache(0);
        m
    }

    /// Fix `props` length after deserializing older saves (missing or short `props`).
    pub fn normalize_layer_sizes(&mut self) {
        let n = (self.width as usize) * (self.height as usize);
        if self.ground.len() != n {
            self.ground.resize(n, 0);
        }
        if self.props.len() != n {
            self.props.resize(n, EMPTY_PROP_ID);
        }
    }

    /// Full recompute of [`MapGrid::display`] (e.g. level load, resize, save load).
    pub fn rebuild_display_cache(&mut self, visual_seed: u64) {
        self.normalize_layer_sizes();
        let n = (self.width as usize) * (self.height as usize);
        if self.display.len() != n {
            self.display.resize(n, TileDisplayCell::default());
        }
        let gv = TileBakeView {
            table: &self.table,
            tiles: &self.ground,
            width: self.width,
            height: self.height,
        };
        for y in 0..self.height {
            for x in 0..self.width {
                let wx = i32::from(x);
                let wy = i32::from(y);
                let i = wy as usize * self.width as usize + wx as usize;
                self.display[i] = bake_layered_tile_display(gv, &self.props, wx, wy, visual_seed);
            }
        }
    }

    /// After a single-cell edit, refresh this cell and neighbors (connector mask locality on each layer).
    pub fn rebuild_display_local(&mut self, wx: i32, wy: i32, visual_seed: u64) {
        self.normalize_layer_sizes();
        let gv = TileBakeView {
            table: &self.table,
            tiles: &self.ground,
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
                self.display[i] = bake_layered_tile_display(gv, &self.props, x, y, visual_seed);
            }
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u16) < self.width && (y as u16) < self.height
    }

    #[inline]
    pub fn ground_at(&self, x: i32, y: i32) -> Option<TileId> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.ground.get(i).copied()
    }

    #[inline]
    pub fn prop_at(&self, x: i32, y: i32) -> Option<TileId> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.props.get(i).copied()
    }

    /// Same as [`Self::ground_at`] (legacy name).
    #[inline]
    pub fn tile_at(&self, x: i32, y: i32) -> Option<TileId> {
        self.ground_at(x, y)
    }

    pub fn set_ground(&mut self, x: i32, y: i32, t: TileId) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.ground[i] = t;
        true
    }

    pub fn set_prop(&mut self, x: i32, y: i32, t: TileId) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = y as usize * self.width as usize + x as usize;
        self.props[i] = t;
        true
    }

    /// Like [`set_ground`] then [`rebuild_display_local`].
    pub fn set_ground_with_display(&mut self, x: i32, y: i32, t: TileId, visual_seed: u64) -> bool {
        if !self.set_ground(x, y, t) {
            return false;
        }
        self.rebuild_display_local(x, y, visual_seed);
        true
    }

    /// Like [`set_prop`] then [`rebuild_display_local`].
    pub fn set_prop_with_display(&mut self, x: i32, y: i32, t: TileId, visual_seed: u64) -> bool {
        if !self.set_prop(x, y, t) {
            return false;
        }
        self.rebuild_display_local(x, y, visual_seed);
        true
    }

    /// Same as [`Self::set_ground`] (tests / call sites).
    #[inline]
    pub fn set_tile(&mut self, x: i32, y: i32, t: TileId) -> bool {
        self.set_ground(x, y, t)
    }

    /// Same as [`Self::set_ground_with_display`].
    #[inline]
    pub fn set_tile_with_display(&mut self, x: i32, y: i32, t: TileId, visual_seed: u64) -> bool {
        self.set_ground_with_display(x, y, t, visual_seed)
    }

    fn cell_blocks(def: Option<&TileDef>) -> bool {
        def.is_some_and(|d| d.blocks_movement)
    }

    pub fn blocks_movement(&self, x: i32, y: i32) -> bool {
        let g = self.ground_at(x, y).and_then(|id| self.table.def(id));
        let p = self
            .prop_at(x, y)
            .filter(|&id| id != EMPTY_PROP_ID)
            .and_then(|id| self.table.def(id));
        Self::cell_blocks(g) || Self::cell_blocks(p)
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        let g = self.ground_at(x, y).and_then(|id| self.table.def(id));
        let p = self
            .prop_at(x, y)
            .filter(|&id| id != EMPTY_PROP_ID)
            .and_then(|id| self.table.def(id));
        g.map(|d| d.blocks_sight).unwrap_or(true) || p.is_some_and(|d| d.blocks_sight)
    }

    /// Composed terrain glyph and colors (animated layers resolved).
    #[must_use]
    pub fn composed_terrain_cell(
        &self,
        wx: i32,
        wy: i32,
        surface_tick: u64,
        visual_seed: u64,
    ) -> TileDisplayCell {
        let gv = TileBakeView {
            table: &self.table,
            tiles: &self.ground,
            width: self.width,
            height: self.height,
        };
        let g_tid = self.ground_at(wx, wy).unwrap_or(0);
        let ground_cell = match self.table.def(g_tid) {
            Some(d) if def_is_animated(d) => resolve_animated(d, wx, wy, surface_tick, visual_seed),
            Some(_) => bake_tile_display(gv, wx, wy, visual_seed),
            None => TileDisplayCell {
                ch: '?',
                fg: crate::render::Color::rgb(200, 100, 100),
                bg: crate::render::Color::rgb(40, 10, 12),
            },
        };
        let p_tid = self.prop_at(wx, wy).unwrap_or(EMPTY_PROP_ID);
        if p_tid == EMPTY_PROP_ID {
            return ground_cell;
        }
        let pv = TileBakeView {
            table: &self.table,
            tiles: &self.props,
            width: self.width,
            height: self.height,
        };
        let prop_cell = match self.table.def(p_tid) {
            Some(d) if def_is_animated(d) => resolve_animated(d, wx, wy, surface_tick, visual_seed),
            Some(_) => bake_tile_display(pv, wx, wy, visual_seed),
            None => return ground_cell,
        };
        TileDisplayCell {
            ch: prop_cell.ch,
            fg: prop_cell.fg,
            bg: ground_cell.bg,
        }
    }
}
