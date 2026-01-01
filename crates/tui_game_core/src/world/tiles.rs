use serde::{Deserialize, Serialize};

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
}

impl TileDef {
    pub fn floor(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: false,
            blocks_sight: false,
            name: name.into(),
        }
    }

    pub fn wall(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: true,
            blocks_sight: true,
            name: name.into(),
        }
    }
}
