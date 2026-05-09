//! Serializable level files (RON), atmosphere recipes, and terrain pack merging.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::render::Color;
use crate::world::{
    mix64, normalize_tile_def_ids, MapGrid, TileDef, TileId, TileTable, EMPTY_PROP_ID,
};

fn level_props_omittable(props: &[TileId]) -> bool {
    props.is_empty() || props.iter().all(|&p| p == EMPTY_PROP_ID)
}

fn default_void_glyph_fg() -> Color {
    Color::rgb(40, 40, 43)
}

/// Tunable atmosphere parameters (global default or per-zone recipe).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtmosphereRecipe {
    pub void_background: Color,
    #[serde(default = "default_void_glyph_fg")]
    pub void_glyph_foreground: Color,
    /// Blend visible terrain **background** toward [`Self::void_background`] (`0..=100`).
    pub visible_background_pull: u8,
    /// Multiplier on base fog-of-war radius at the **level** (v1; zonal LOS deferred).
    pub sight_strength: f32,
}

impl Default for AtmosphereRecipe {
    fn default() -> Self {
        Self {
            void_background: Color::rgb(5, 5, 8),
            void_glyph_foreground: default_void_glyph_fg(),
            visible_background_pull: 12,
            sight_strength: 1.0,
        }
    }
}

/// Axis-aligned rectangle or disk in tile space, centered on [`AtmosphereZone`] anchor.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AtmosphereShape {
    Rectangle {
        width_tiles: u16,
        height_tiles: u16,
    },
    Circle {
        radius_tiles: u16,
    },
}

/// Placed atmosphere volume. Influence falls off outside the hard shape across [`Self::edge_falloff_tiles`].
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtmosphereZone {
    pub anchor_x: i32,
    pub anchor_y: i32,
    pub shape: AtmosphereShape,
    pub edge_falloff_tiles: u16,
    pub recipe: AtmosphereRecipe,
}

/// Fog-of-war state for a map cell when composing colors (discrete / editor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapTileFog {
    Unseen,
    Explored,
    Visible,
}

/// Explicit player start cell in a [`LevelFile`]. When absent, the game uses map center.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerSpawn {
    pub x: i32,
    pub y: i32,
}

/// Serializable terrain definitions (referenced by [`LevelFile::terrain_pack`]).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TerrainPack {
    pub schema_version: u32,
    pub tile_defs: Vec<TileDef>,
}

impl TerrainPack {
    pub const SCHEMA: u32 = 1;
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
    /// Ground terrain indices into [`Self::tile_defs`] (RON field `tiles`).
    pub tiles: Vec<TileId>,
    /// Prop overlay per cell; [`EMPTY_PROP_ID`] = none. Omitted from RON when empty or all-clear.
    #[serde(default, skip_serializing_if = "level_props_omittable")]
    pub props: Vec<TileId>,
    /// Relative path from the level file’s directory (e.g. `"../terrains/demo_terrain_pack.ron"`).
    /// When empty, [`Self::tile_defs`] must be populated (tests / legacy).
    #[serde(default)]
    pub terrain_pack: String,
    /// Inline tile definitions when [`Self::terrain_pack`] is empty.
    #[serde(default)]
    pub tile_defs: Vec<TileDef>,
    pub spawns: Vec<EntitySpawn>,
    #[serde(default)]
    pub player_spawn: Option<PlayerSpawn>,
    #[serde(default)]
    pub visual_seed: Option<u64>,
    #[serde(default)]
    pub default_atmosphere: AtmosphereRecipe,
    #[serde(default)]
    pub atmosphere_zones: Vec<AtmosphereZone>,
}

/// Load a [`TerrainPack`] from disk and assign [`LevelFile::tile_defs`].
pub fn materialize_tile_defs_from_pack(
    level: &mut LevelFile,
    level_file_parent: Option<&Path>,
) -> Result<(), String> {
    let path = level.terrain_pack.trim();
    if path.is_empty() {
        if level.tile_defs.is_empty() {
            return Err("level has empty terrain_pack and no inline tile_defs".into());
        }
        normalize_tile_def_ids(&mut level.tile_defs);
        return Ok(());
    }
    let base = level_file_parent.unwrap_or_else(|| Path::new("."));
    let full = base.join(path);
    let raw = std::fs::read_to_string(&full)
        .map_err(|e| format!("read terrain pack {}: {e}", full.display()))?;
    let pack: TerrainPack = ron::from_str(&raw)
        .map_err(|e| format!("parse terrain pack {}: {e}", full.display()))?;
    if pack.schema_version != TerrainPack::SCHEMA {
        return Err(format!(
            "terrain pack {} schema_version {} != {}",
            full.display(),
            pack.schema_version,
            TerrainPack::SCHEMA
        ));
    }
    if pack.tile_defs.is_empty() {
        return Err(format!("terrain pack {} has no tile_defs", full.display()));
    }
    level.tile_defs = pack.tile_defs;
    normalize_tile_def_ids(&mut level.tile_defs);
    Ok(())
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
    for &t in &map.ground {
        h = h.wrapping_add(t as u64);
        h = mix64(h);
    }
    for &t in &map.props {
        if t != EMPTY_PROP_ID {
            h = h.wrapping_add(t as u64).rotate_left(3);
            h = mix64(h);
        }
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
        if self.tile_defs.is_empty() {
            return Err("to_map: tile_defs is empty (load terrain pack first)".into());
        }
        let props = if self.props.len() == expected {
            self.props.clone()
        } else {
            vec![EMPTY_PROP_ID; expected]
        };
        let mut defs = self.tile_defs.clone();
        normalize_tile_def_ids(&mut defs);
        let table = TileTable { defs };
        Ok(MapGrid {
            width: self.width,
            height: self.height,
            ground: self.tiles.clone(),
            props,
            table,
            display: vec![crate::world::TileDisplayCell::default(); expected],
            default_atmosphere: self.default_atmosphere,
            atmosphere_zones: self.atmosphere_zones.clone(),
        })
    }

    pub fn from_map(map: &MapGrid, name: impl Into<String>, spawns: Vec<EntitySpawn>) -> Self {
        let mut tile_defs = map.table.defs.clone();
        normalize_tile_def_ids(&mut tile_defs);
        let mut props = map.props.clone();
        if level_props_omittable(&props) {
            props.clear();
        }
        Self {
            schema_version: Self::SCHEMA,
            name: name.into(),
            width: map.width,
            height: map.height,
            tiles: map.ground.clone(),
            props,
            terrain_pack: String::new(),
            tile_defs,
            spawns,
            player_spawn: None,
            visual_seed: None,
            default_atmosphere: map.default_atmosphere,
            atmosphere_zones: map.atmosphere_zones.clone(),
        }
    }
}

pub fn level_to_ron(level: &LevelFile) -> Result<String, ron::Error> {
    ron::ser::to_string_pretty(level, ron::ser::PrettyConfig::new())
}

pub fn level_from_ron(s: &str) -> Result<LevelFile, ron::de::SpannedError> {
    let mut level: LevelFile = ron::from_str(s)?;
    normalize_tile_def_ids(&mut level.tile_defs);
    Ok(level)
}

pub fn terrain_pack_to_ron(pack: &TerrainPack) -> Result<String, ron::Error> {
    ron::ser::to_string_pretty(pack, ron::ser::PrettyConfig::new())
}

pub fn terrain_pack_from_ron(s: &str) -> Result<TerrainPack, ron::de::SpannedError> {
    let mut pack: TerrainPack = ron::from_str(s)?;
    normalize_tile_def_ids(&mut pack.tile_defs);
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{MapGrid, TileTable};

    #[test]
    fn ron_roundtrips_anim_mode() {
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

    #[test]
    fn ron_deserializes_legacy_bare_anim_mode_token() {
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

    #[test]
    fn round_trip_level_ron_prop_layer() {
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let mut map = MapGrid::filled(3, 3, 0, table);
        map.set_prop(1, 1, 1);
        let level = LevelFile::from_map(&map, "props", vec![]);
        let s = level_to_ron(&level).unwrap();
        let back = level_from_ron(&s).unwrap();
        let m2 = back.to_map().expect("to_map");
        assert_eq!(m2.ground, map.ground);
        assert_eq!(m2.props, map.props);
    }
}
