//! Geometric atmosphere zones, per-cell resolution at bake time, and baked fog colors.
//!
//! **Bake pipeline:** [`rebuild_atmosphere_bake`] fills [`FogBakedTrio`] per map cell from
//! [`MapGrid::default_atmosphere`], [`MapGrid::atmosphere_zones`], and static [`MapGrid::display`].
//! Runtime [`compose_fog_from_luminance`] only lerps the three baked endpoints using smoothed FoW.
//!
//! **Future (Part C):** per-ray visibility budget using [`AtmosphereRecipe::sight_strength`] per cell
//! along LOS — not implemented in v1.

use serde::{Deserialize, Serialize};

use crate::render::Color;

use crate::world::MapGrid;

// --------------------------------------------------------------------------- tunables (single place)

/// How strongly explored-but-not-visible terrain pulls toward the void palette (`0..=100`).
pub const EXPLORED_BLEND_TOWARDS_VOID_PCT: u8 = 48;

/// Default `visible_background_pull` when migrating from legacy `visible_boost`-only data.
pub const DEFAULT_VISIBLE_BACKGROUND_PULL: u8 = 12;

/// Default `sight_strength` (multiplier on base FoW radius).
pub const DEFAULT_SIGHT_STRENGTH: f32 = 1.0;

/// Clamp [`AtmosphereRecipe::sight_strength`] after multiply against base FoW radius.
pub const SIGHT_RADIUS_MIN: i32 = 4;
pub const SIGHT_RADIUS_MAX: i32 = 48;

use crate::level::{AtmosphereRecipe, AtmosphereShape, AtmosphereZone, MapTileFog};
use crate::world::fog_visual::FOG_LUMINANCE_EXPLORED;

/// Resolved numeric/color mix for one cell after zone weighting (bake-time only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedAtmosphere {
    pub void_background: Color,
    pub void_glyph_foreground: Color,
    pub visible_background_pull: u8,
    pub sight_strength: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FogPaint {
    pub fg: Color,
    pub bg: Color,
}

/// Baked fog colors for one cell: three discrete FoW presentation states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FogBakedTrio {
    pub unseen: FogPaint,
    pub explored: FogPaint,
    pub visible: FogPaint,
}

// --------------------------------------------------------------------------- resolution + bake

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Signed distance from `(x,y)` to the **outside** of the hard shape (negative = inside, 0 = boundary).
fn shape_signed_distance(shape: AtmosphereShape, ax: i32, ay: i32, x: i32, y: i32) -> f32 {
    let dx = (f64::from(x - ax)).abs();
    let dy = (f64::from(y - ay)).abs();
    match shape {
        AtmosphereShape::Rectangle {
            width_tiles: w,
            height_tiles: h,
        } => {
            let hw = f64::from(w) / 2.0;
            let hh = f64::from(h) / 2.0;
            let qx = dx - hw;
            let qy = dy - hh;
            let ox = qx.max(0.0);
            let oy = qy.max(0.0);
            let outside = (ox * ox + oy * oy).sqrt();
            let inside = qx.max(qy).min(0.0);
            (outside + inside) as f32
        }
        AtmosphereShape::Circle { radius_tiles: r } => {
            let dist = ((dx * dx + dy * dy).sqrt()) as f32;
            dist - r as f32
        }
    }
}

/// Weight in `0..=1` for one zone at `(wx, wy)`.
pub fn zone_influence_weight(zone: &AtmosphereZone, wx: i32, wy: i32) -> f32 {
    let sd = shape_signed_distance(zone.shape, zone.anchor_x, zone.anchor_y, wx, wy);
    if sd <= 0.0 {
        return 1.0;
    }
    let fall = f32::from(zone.edge_falloff_tiles.max(1));
    if sd >= fall {
        return 0.0;
    }
    1.0 - smoothstep(0.0, fall, sd)
}

#[must_use]
pub fn resolve_atmosphere_cell(
    default: &AtmosphereRecipe,
    zones: &[AtmosphereZone],
    wx: i32,
    wy: i32,
) -> ResolvedAtmosphere {
    let mut wsum = 0.0_f32;
    let mut vb_acc = 0.0_f32;
    let mut ss_acc = 0.0_f32;
    let mut vr = 0.0_f64;
    let mut vg = 0.0_f64;
    let mut vb = 0.0_f64;
    let mut gr = 0.0_f64;
    let mut gg = 0.0_f64;
    let mut gb = 0.0_f64;

    for z in zones {
        let w = zone_influence_weight(z, wx, wy);
        if w <= f32::EPSILON {
            continue;
        }
        let r = &z.recipe;
        wsum += w;
        vb_acc += w * f32::from(r.visible_background_pull.min(100));
        ss_acc += w * r.sight_strength;
        let wf = f64::from(w);
        vr += f64::from(r.void_background.r) * wf;
        vg += f64::from(r.void_background.g) * wf;
        vb += f64::from(r.void_background.b) * wf;
        gr += f64::from(r.void_glyph_foreground.r) * wf;
        gg += f64::from(r.void_glyph_foreground.g) * wf;
        gb += f64::from(r.void_glyph_foreground.b) * wf;
    }

    if wsum <= f32::EPSILON {
        return ResolvedAtmosphere {
            void_background: default.void_background,
            void_glyph_foreground: default.void_glyph_foreground,
            visible_background_pull: default.visible_background_pull.min(100),
            sight_strength: default.sight_strength,
        };
    }

    let inv = f64::from(wsum);
    ResolvedAtmosphere {
        void_background: Color::rgb(
            (vr / inv).round().clamp(0.0, 255.0) as u8,
            (vg / inv).round().clamp(0.0, 255.0) as u8,
            (vb / inv).round().clamp(0.0, 255.0) as u8,
        ),
        void_glyph_foreground: Color::rgb(
            (gr / inv).round().clamp(0.0, 255.0) as u8,
            (gg / inv).round().clamp(0.0, 255.0) as u8,
            (gb / inv).round().clamp(0.0, 255.0) as u8,
        ),
        visible_background_pull: (vb_acc / wsum).round().clamp(0.0, 100.0) as u8,
        sight_strength: ss_acc / wsum,
    }
}

#[inline]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lerp_color_f32(a: Color, b: Color, t: f32) -> Color {
    let t = f64::from(t.clamp(0.0, 1.0));
    let mix = |x: u8, y: u8| -> u8 {
        let xi = f64::from(x);
        let yi = f64::from(y);
        (xi * (1.0 - t) + yi * t).round().clamp(0.0, 255.0) as u8
    };
    Color::rgb(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
}

/// Lerp baked unseen → explored → visible using smoothed luminance `l_smooth` in `0..=1`.
#[must_use]
pub fn compose_fog_from_luminance(baked: FogBakedTrio, l_smooth: f32) -> (Color, Color) {
    let l = l_smooth.clamp(0.0, 1.0);
    let mid = FOG_LUMINANCE_EXPLORED;
    let u_fg = baked.unseen.fg;
    let u_bg = baked.unseen.bg;
    let e_fg = baked.explored.fg;
    let e_bg = baked.explored.bg;
    let v_fg = baked.visible.fg;
    let v_bg = baked.visible.bg;
    if l <= mid {
        let t = if mid > f32::EPSILON { l / mid } else { 0.0 };
        (lerp_color_f32(u_fg, e_fg, t), lerp_color_f32(u_bg, e_bg, t))
    } else {
        let t = (l - mid) / (1.0 - mid).max(f32::EPSILON);
        (lerp_color_f32(e_fg, v_fg, t), lerp_color_f32(e_bg, v_bg, t))
    }
}

fn bake_cell_trio(res: &ResolvedAtmosphere, base_fg: Color, base_bg: Color) -> FogBakedTrio {
    let void_fg = res.void_glyph_foreground;
    let void_bg = res.void_background;
    let pct = EXPLORED_BLEND_TOWARDS_VOID_PCT.min(100);
    let explored_fg = base_fg.mix_towards(void_fg, pct);
    let explored_bg = base_bg.mix_towards(void_bg, pct);
    let pull = res.visible_background_pull.min(100);
    let visible_bg = base_bg.mix_towards(void_bg, pull);
    FogBakedTrio {
        unseen: FogPaint {
            fg: void_fg,
            bg: void_bg,
        },
        explored: FogPaint {
            fg: explored_fg,
            bg: explored_bg,
        },
        visible: FogPaint {
            fg: base_fg,
            bg: visible_bg,
        },
    }
}

/// Fills `out` with [`FogBakedTrio`] per cell (length `width * height`). Clears and resizes `out`.
pub fn rebuild_atmosphere_bake(map: &MapGrid, out: &mut Vec<FogBakedTrio>) {
    let n = (map.width as usize) * (map.height as usize);
    out.clear();
    out.resize(n, FogBakedTrio::default());

    let zones = map.atmosphere_zones.as_slice();
    let default = &map.default_atmosphere;

    for y in 0..map.height {
        for x in 0..map.width {
            let wx = i32::from(x);
            let wy = i32::from(y);
            let i = wy as usize * map.width as usize + wx as usize;
            let baked_cell = map.display.get(i).copied().unwrap_or_default();
            let res = resolve_atmosphere_cell(default, zones, wx, wy);
            out[i] = bake_cell_trio(&res, baked_cell.fg, baked_cell.bg);
        }
    }
}

/// Effective FoW radius (tiles) from level default recipe only (v1).
#[must_use]
pub fn effective_fow_radius_cells(base_radius: i32, level_default: &AtmosphereRecipe) -> i32 {
    let s = f64::from(level_default.sight_strength);
    let r = (f64::from(base_radius) * s).round() as i32;
    r.clamp(SIGHT_RADIUS_MIN, SIGHT_RADIUS_MAX)
}

/// Discrete fog compose (editor visible preview).
#[must_use]
pub fn compose_map_tile_discrete(baked: FogBakedTrio, fog: MapTileFog) -> (Color, Color) {
    match fog {
        MapTileFog::Unseen => (baked.unseen.fg, baked.unseen.bg),
        MapTileFog::Explored => (baked.explored.fg, baked.explored.bg),
        MapTileFog::Visible => (baked.visible.fg, baked.visible.bg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::AtmosphereRecipe;

    #[test]
    fn resolve_default_without_zones() {
        let d = AtmosphereRecipe {
            void_background: Color::rgb(1, 2, 3),
            void_glyph_foreground: Color::rgb(4, 5, 6),
            visible_background_pull: 20,
            sight_strength: 0.75,
        };
        let r = resolve_atmosphere_cell(&d, &[], 0, 0);
        assert_eq!(r.void_background, d.void_background);
        assert_eq!(r.visible_background_pull, 20);
    }

    #[test]
    fn bake_luminance_matches_discrete_endpoints() {
        let d = AtmosphereRecipe::default();
        let res = resolve_atmosphere_cell(&d, &[], 0, 0);
        let baked = bake_cell_trio(&res, Color::rgb(100, 90, 80), Color::rgb(20, 22, 28));
        for &(l, fog) in &[
            (0.0_f32, MapTileFog::Unseen),
            (FOG_LUMINANCE_EXPLORED, MapTileFog::Explored),
            (1.0_f32, MapTileFog::Visible),
        ] {
            let (a_fg, a_bg) = compose_map_tile_discrete(baked, fog);
            let (b_fg, b_bg) = compose_fog_from_luminance(baked, l);
            assert_eq!((a_fg, a_bg), (b_fg, b_bg), "l={l} fog={fog:?}");
        }
    }
}
