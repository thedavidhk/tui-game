//! Serializable level files (RON).

use serde::{Deserialize, Serialize};

use crate::world::{MapGrid, TileDef, TileId, TileTable};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EntitySpawn {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub glyph: char,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LevelFile {
    pub schema_version: u32,
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<TileId>,
    pub tile_defs: Vec<TileDef>,
    pub spawns: Vec<EntitySpawn>,
}

impl LevelFile {
    pub const SCHEMA: u32 = 1;

    pub fn to_map(&self) -> Result<MapGrid, String> {
        let expected = (self.width as usize) * (self.height as usize);
        if self.tiles.len() != expected {
            return Err(format!(
                "tiles len {} != width*height {}",
                self.tiles.len(),
                expected
            ));
        }
        let table = TileTable {
            defs: self.tile_defs.clone(),
        };
        Ok(MapGrid {
            width: self.width,
            height: self.height,
            tiles: self.tiles.clone(),
            table,
        })
    }

    pub fn from_map(map: &MapGrid, name: impl Into<String>, spawns: Vec<EntitySpawn>) -> Self {
        Self {
            schema_version: Self::SCHEMA,
            name: name.into(),
            width: map.width,
            height: map.height,
            tiles: map.tiles.clone(),
            tile_defs: map.table.defs.clone(),
            spawns,
        }
    }
}

pub fn level_to_ron(level: &LevelFile) -> Result<String, ron::Error> {
    ron::ser::to_string_pretty(level, ron::ser::PrettyConfig::new())
}

pub fn level_from_ron(s: &str) -> Result<LevelFile, ron::de::SpannedError> {
    ron::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::TileTable;

    #[test]
    fn round_trip_level_ron() {
        let table = TileTable::default_pack();
        let mut map = MapGrid::filled(4, 3, 0, table);
        map.set_tile(1, 1, 1);
        let level = LevelFile::from_map(
            &map,
            "test",
            vec![EntitySpawn {
                kind: "guide".into(),
                x: 2,
                y: 1,
                glyph: 'g',
                name: "Guide".into(),
            }],
        );
        let s = level_to_ron(&level).unwrap();
        let back = level_from_ron(&s).unwrap();
        assert_eq!(back, level);
    }
}
