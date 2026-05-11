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
///
/// For levels and terrain packs, numeric tile ids in `tiles` / maps match **`tile_defs` index**
/// (first def is `0`, etc.). RON omits per-def `id`; see [`normalize_tile_def_ids`].
pub type TileId = u16;

/// Sentinel for [`crate::world::MapGrid::props`]: no prop overlay on that cell.
///
/// This value is never assigned to [`TileDef::id`] by [`normalize_tile_def_ids`] (indices only).
pub const EMPTY_PROP_ID: TileId = u16::MAX;

/// Assign `defs[i].id = i as u16` for every entry. Call after deserializing `tile_defs` / `TileTable.defs`
/// so runtime ids match placement data (`tiles` stores indices into this slice).
///
/// **Caveat:** reordering `tile_defs` changes every numeric id — keep the same order as when the
/// level grid was authored (or renumber `tiles` accordingly).
#[inline]
pub fn normalize_tile_def_ids(defs: &mut [TileDef]) {
    for (i, d) in defs.iter_mut().enumerate() {
        d.id = TileId::try_from(i).expect("tile_defs length must fit u16");
    }
}

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

mod connector_line_style_ron {
    use super::ConnectorLineStyle;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &ConnectorLineStyle, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            ConnectorLineStyle::SingleLine => "single_line",
            ConnectorLineStyle::DoubleLine => "double_line",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ConnectorLineStyle, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = ConnectorLineStyle;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "\"single_line\" or \"double_line\" (in RON quote `double_line` — bare `double_line` parses as subtraction)",
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
        }

        fn parse<E: de::Error>(s: &str) -> Result<ConnectorLineStyle, E> {
            let t = s.trim();
            if t.eq_ignore_ascii_case("single_line") || t == "SingleLine" {
                Ok(ConnectorLineStyle::SingleLine)
            } else if t.eq_ignore_ascii_case("double_line") || t == "DoubleLine" {
                Ok(ConnectorLineStyle::DoubleLine)
            } else {
                Err(de::Error::unknown_variant(
                    t,
                    &["single_line", "double_line"],
                ))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Built-in 16-glyph connector autotile tables (mask order: N, E, S, W — see [`crate::world::tile_surface::neighbor_link_mask`]).
///
/// **RON:** write `style: "double_line"` (quoted). Bare `double_line` is parsed as `double` `-` `line`, so the field is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorLineStyle {
    /// Light box-drawing (`─│┌…┼`); shared by any terrain with the same line weight.
    SingleLine,
    /// Double box-drawing (`═║╔…╬`); same topology as single.
    DoubleLine,
}

impl serde::Serialize for ConnectorLineStyle {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        connector_line_style_ron::serialize(self, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ConnectorLineStyle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        connector_line_style_ron::deserialize(deserializer)
    }
}

/// Payload for [`TileSurface::Connector`]: reuse a preset line weight, or override with 16 custom glyphs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConnectorSurface {
    /// When `glyphs` is missing or shorter than 16, selects the shared glyph table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ConnectorLineStyle>,
    /// Legacy / custom: full 16-entry table (same mask order as presets). When present with len ≥ 16, overrides `style`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyphs: Option<Vec<char>>,
}

/// Preset glyphs for [`ConnectorLineStyle::SingleLine`] (light box drawing).
pub const CONNECTOR_GLYPHS_SINGLE: [char; 16] = [
    '·', '╵', '╶', '└', '╷', '│', '┌', '├', '╴', '┘', '─', '┴', '┐', '┤', '┬', '┼',
];

/// Preset glyphs for [`ConnectorLineStyle::DoubleLine`] (double rules; stubs use light terminals).
pub const CONNECTOR_GLYPHS_DOUBLE: [char; 16] = [
    '#', '╨', '╞', '╚', '╥', '║', '╔', '╠', '╡', '╝', '═', '╩', '╗', '╣', '╦', '╬',
];

impl ConnectorSurface {
    /// Resolved 16-glyph row for baking: custom table if long enough, else the preset for `style` (default single).
    #[must_use]
    pub fn glyph_table(&self) -> &[char] {
        if let Some(ref g) = self.glyphs {
            if g.len() >= 16 {
                return g.as_slice();
            }
        }
        match self.style.unwrap_or(ConnectorLineStyle::SingleLine) {
            ConnectorLineStyle::SingleLine => &CONNECTOR_GLYPHS_SINGLE,
            ConnectorLineStyle::DoubleLine => &CONNECTOR_GLYPHS_DOUBLE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TileSurface {
    StaticVariants {
        entries: Vec<WeightedGlyph>,
    },
    Connector(ConnectorSurface),
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
    /// Implicit index in `tile_defs` / `TileTable.defs`; not written to RON (see [`normalize_tile_def_ids`]).
    #[serde(default, skip_serializing)]
    pub id: TileId,
    pub glyph: char,
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    /// Stable id for tools, tests, and search (often snake_case).
    pub name: String,
    /// Short immersive line for the in-game HUD; when empty, [`Self::name`] is shown.
    #[serde(default)]
    pub description: String,
    /// Glyph foreground when the tile is drawn in truecolor terminals.
    #[serde(default)]
    pub fg: Color,
    /// Tile background when drawn; `None` uses [`Color::terrain_bg_from_fg`] on `fg`.
    #[serde(default)]
    pub bg: Option<Color>,
    /// Non-zero masks participate in connector neighbor tests: a link exists
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

    /// Text shown in exploration HUD etc.; falls back to [`Self::name`] if empty.
    #[must_use]
    pub fn description(&self) -> &str {
        if self.description.is_empty() {
            self.name.as_str()
        } else {
            self.description.as_str()
        }
    }

    pub fn floor(id: TileId, glyph: char, name: impl Into<String>) -> Self {
        Self {
            id,
            glyph,
            blocks_movement: false,
            blocks_sight: false,
            name: name.into(),
            description: String::new(),
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
            description: String::new(),
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
