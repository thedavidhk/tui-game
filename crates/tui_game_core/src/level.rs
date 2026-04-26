//! Serializable level files (RON).

use serde::{Deserialize, Serialize};

use crate::render::Color;
use crate::world::{mix64, MapGrid, TileDef, TileId, TileTable};

/// Explicit player start cell in a [`LevelFile`]. When absent, the game uses map center.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerSpawn {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EntitySpawn {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub glyph_override: Option<char>,
    #[serde(default)]
    pub name_override: Option<String>,
    #[serde(default)]
    pub fg_override: Option<Color>,
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
    /// Player start position. If `None`, the runtime uses the map center (legacy behavior).
    #[serde(default)]
    pub player_spawn: Option<PlayerSpawn>,
    /// Stable seed for baked static tile variants (grass). If omitted, derived from level data.
    #[serde(default)]
    pub visual_seed: Option<u64>,
}

/// Default `visual_seed` when [`LevelFile::visual_seed`] is `None`.
#[must_use]
pub fn derive_visual_seed(level: &LevelFile) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in level.name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= (level.width as u64) << 32 | level.height as u64;
    for d in &level.tile_defs {
        h ^= (d.id as u64).wrapping_mul(0x9e3779b97f4a7c15);
    }
    mix64(h)
}

/// Derive a seed from map geometry + tile ids (e.g. after loading a save without a stored display cache).
#[must_use]
pub fn derive_visual_seed_from_map(map: &MapGrid) -> u64 {
    let mut h: u64 = 0xdeadbeefcafe0000;
    h ^= (map.width as u64) << 16 | map.height as u64;
    for &t in &map.tiles {
        h = h.wrapping_add(t as u64);
        h = mix64(h);
    }
    for d in &map.table.defs {
        h ^= (d.id as u64).rotate_left(11);
    }
    mix64(h)
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
        let n = expected;
        Ok(MapGrid {
            width: self.width,
            height: self.height,
            tiles: self.tiles.clone(),
            table,
            display: vec![crate::world::TileDisplayCell::default(); n],
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
            player_spawn: None,
            visual_seed: None,
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
    use crate::world::{MapGrid, TileTable};

    #[test]
    fn ron_roundtrips_anim_mode() {
        use crate::render::Color;
        use crate::world::{AnimMode, AnimatedFrame, TileSurface};
        let animated = TileSurface::Animated {
            frames: vec![AnimatedFrame {
                ch: '~',
                fg: Color::rgb(1, 2, 3),
            }],
            mode: AnimMode::Cycle,
            ticks_per_frame: 6,
            p_step_num: 0,
            p_step_den: 60,
        };
        let raw = ron::ser::to_string_pretty(&animated, ron::ser::PrettyConfig::new()).unwrap();
        let back: TileSurface = ron::from_str(&raw).unwrap_or_else(|e| {
            panic!("ron error {e}; serialized:\n{raw}");
        });
        assert_eq!(back, animated);
    }

    #[test]
    fn ron_roundtrips_anim_mode_drift() {
        use crate::render::Color;
        use crate::world::{AnimMode, AnimatedFrame, TileSurface};
        let animated = TileSurface::Animated {
            frames: vec![AnimatedFrame {
                ch: '~',
                fg: Color::rgb(1, 2, 3),
            }],
            mode: AnimMode::Drift,
            ticks_per_frame: 6,
            p_step_num: 1,
            p_step_den: 60,
        };
        let raw = ron::ser::to_string_pretty(&animated, ron::ser::PrettyConfig::new()).unwrap();
        let back: TileSurface = ron::from_str(&raw).unwrap_or_else(|e| {
            panic!("ron error {e}; serialized:\n{raw}");
        });
        assert_eq!(back, animated);
    }

    /// Editor / older RON used bare `mode: cycle,` which RON parses as unit, not a string.
    #[test]
    fn ron_deserializes_legacy_bare_anim_mode_token() {
        use crate::render::Color;
        use crate::world::{AnimMode, AnimatedFrame, TileSurface};
        let raw = r#"(
            kind: "animated",
            frames: [(ch: '~', fg: (r: 1, g: 2, b: 3))],
            mode: cycle,
            ticks_per_frame: 6,
            p_step_num: 0,
            p_step_den: 60,
        )"#;
        let ts: TileSurface = ron::from_str(raw).unwrap();
        assert_eq!(
            ts,
            TileSurface::Animated {
                frames: vec![AnimatedFrame {
                    ch: '~',
                    fg: Color::rgb(1, 2, 3),
                }],
                mode: AnimMode::Cycle,
                ticks_per_frame: 6,
                p_step_num: 0,
                p_step_den: 60,
            }
        );
    }

    #[test]
    fn derive_visual_seed_is_stable() {
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let map = MapGrid::filled(4, 3, 0, table);
        let level = LevelFile::from_map(&map, "test", vec![]);
        let a = super::derive_visual_seed(&level);
        let b = super::derive_visual_seed(&level);
        assert_eq!(a, b);
    }

    #[test]
    fn round_trip_level_ron() {
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let mut map = MapGrid::filled(4, 3, 0, table);
        map.set_tile(1, 1, 1);
        let level = LevelFile::from_map(
            &map,
            "test",
            vec![EntitySpawn {
                kind: "guide".into(),
                x: 2,
                y: 1,
                glyph_override: None,
                name_override: None,
                fg_override: None,
            }],
        );
        let s = level_to_ron(&level).unwrap();
        let back = level_from_ron(&s).unwrap();
        assert_eq!(back, level);
    }
}
