use serde::{Deserialize, Serialize};

use super::tiles::{TileDef, TileId};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TileTable {
    pub defs: Vec<TileDef>,
}

impl TileTable {
    pub fn default_pack() -> Self {
        Self {
            defs: vec![
                TileDef::floor(0, '.', "floor"),
                TileDef::wall(1, '#', "wall"),
            ],
        }
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
}

impl MapGrid {
    pub fn filled(width: u16, height: u16, tile: TileId, table: TileTable) -> Self {
        let n = (width as usize) * (height as usize);
        Self {
            width,
            height,
            tiles: vec![tile; n],
            table,
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
