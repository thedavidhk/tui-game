use serde::{Deserialize, Serialize};

use crate::render::Color;

/// Stable tile type id; properties resolved via `TileDef` table.
pub type TileId = u16;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TileDef {
    pub id: TileId,
    pub glyph: char,
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    /// Display name for editor / debug.
    pub name: String,
    /// Glyph foreground when the tile is drawn in truecolor terminals.
    #[serde(default)]
    pub fg: Color,
}

impl TileDef {
    pub fn floor(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: false,
            blocks_sight: false,
            name: name.into(),
            fg: Color::rgb(190, 188, 175),
        }
    }

    pub fn wall(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: true,
            blocks_sight: true,
            name: name.into(),
            fg: Color::rgb(140, 135, 125),
        }
    }

    /// Single “solid” flag for editors: maps to both movement and line-of-sight blocking.
    pub fn solid(&self) -> bool {
        self.blocks_movement && self.blocks_sight
    }

    pub fn set_solid(&mut self, solid: bool) {
        self.blocks_movement = solid;
        self.blocks_sight = solid;
    }
}
