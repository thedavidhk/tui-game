use serde::{Deserialize, Serialize};

use crate::render::Color;

mod anim_mode_ron {
    use super::AnimMode;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &AnimMode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            AnimMode::Cycle => "cycle",
            AnimMode::Drift => "drift",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AnimMode, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct AnimModeVisitor;

        impl<'de> de::Visitor<'de> for AnimModeVisitor {
            type Value = AnimMode;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "anim mode: quoted \"cycle\" or \"drift\"; legacy RON may use a bare word (read as cycle)",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                parse(v)
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                parse(&v)
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                parse(std::str::from_utf8(v).map_err(de::Error::custom)?)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                // RON parses bare identifiers like `cycle` or `Cycle` as unit; we cannot tell drift apart.
                Ok(AnimMode::Cycle)
            }
        }

        fn parse<E: de::Error>(s: &str) -> Result<AnimMode, E> {
            let s = s.trim();
            if s.eq_ignore_ascii_case("cycle") {
                Ok(AnimMode::Cycle)
            } else if s.eq_ignore_ascii_case("drift") {
                Ok(AnimMode::Drift)
            } else {
                Err(de::Error::custom(format!(
                    "unknown anim mode {s:?}; expected \"cycle\" or \"drift\""
                )))
            }
        }

        deserializer.deserialize_any(AnimModeVisitor)
    }
}

/// Stable tile type id; properties resolved via `TileDef` table.
pub type TileId = u16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightedGlyph {
    pub weight: u32,
    pub ch: char,
    pub fg: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimatedFrame {
    pub ch: char,
    pub fg: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimMode {
    /// `frame_index = (surface_tick / ticks_per_frame + phase) % len`.
    #[default]
    Cycle,
    /// Cheap deterministic shimmer: `((surface_tick * numer) / denom + phase) % len`.
    Drift,
}

fn default_ticks_per_frame() -> u32 {
    4
}

fn default_drift_denom() -> u32 {
    60
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TileSurface {
    StaticVariants {
        entries: Vec<WeightedGlyph>,
    },
    Connector {
        /// Index = 4-bit mask: bit0 = north, bit1 = east, bit2 = south, bit3 = west.
        glyphs: Vec<char>,
    },
    Animated {
        frames: Vec<AnimatedFrame>,
        #[serde(default, with = "anim_mode_ron")]
        mode: AnimMode,
        #[serde(default = "default_ticks_per_frame")]
        ticks_per_frame: u32,
        #[serde(default)]
        p_step_num: u32,
        #[serde(default = "default_drift_denom")]
        p_step_den: u32,
    },
}

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
    /// Tile background when drawn; `None` uses [`Color::terrain_bg_from_fg`] on `fg`.
    #[serde(default)]
    pub bg: Option<Color>,
    /// Non-zero masks participate in [`TileSurface::Connector`] neighbor tests: a link exists
    /// when `(self.connect_mask & neighbor.connect_mask) != 0`.
    #[serde(default)]
    pub connect_mask: u8,
    /// Optional baked / animated visuals; `None` uses `glyph` + `fg` only.
    #[serde(default)]
    pub surface: Option<TileSurface>,
}

impl TileDef {
    /// Background for this terrain (explicit or derived from `fg`).
    #[must_use]
    pub fn terrain_bg(&self) -> Color {
        self.bg
            .unwrap_or_else(|| Color::terrain_bg_from_fg(self.fg))
    }

    pub fn floor(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: false,
            blocks_sight: false,
            name: name.into(),
            fg: Color::rgb(190, 188, 175),
            bg: None,
            connect_mask: 0,
            surface: None,
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
            bg: None,
            connect_mask: 0,
            surface: None,
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
