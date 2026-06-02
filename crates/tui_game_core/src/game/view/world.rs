//! World viewport composition: terrain and fog per cell, then visible entities on top.

use crate::content::Relation;
use crate::entity::{EntityId, GridPos};
use crate::game::spell;
use crate::game::GameMode;
use crate::math::{euclidean_dist_sq, within_euclidean_radius};
use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};
use crate::world::{compose_fog_from_luminance, smooth_fog_luminance};

use super::super::{unseen_fog_glyph, Game};

// Screen<->world coordinate mapping uses many small `as` casts on grid indices that are
// bounds-checked above each use, and the per-cell color bindings share `_fg`/`_bg` suffixes.
#[allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::too_many_lines
)]
pub(super) fn compose_world(game: &Game, fb: &mut FrameBuffer, area: Rect) {
    let Some(_) = game.player_pos() else {
        return;
    };
    let cam_w = area.w as i32;
    let cam_h = area.h as i32;
    let (ox, oy) = game.world_screen_origin();

    for row in 0..area.h {
        for col in 0..area.w {
            let wx = ox + col as i32;
            let wy = oy + row as i32;
            let screen_x = area.x + col;
            let screen_y = area.y + row;
            let mut cell = Cell::default();
            if !game.map.in_bounds(wx, wy) {
                cell.ch = unseen_fog_glyph(wx, wy, game.map_visual_seed);
                let d = &game.map.default_atmosphere;
                let oob = d.void_background.lighten(6);
                cell.bg = oob;
                cell.fg = d.void_glyph_foreground;
                fb.set(screen_x, screen_y, cell);
                continue;
            }
            let idx = wy as usize * game.map.width as usize + wx as usize;
            let seen = game.explored.get(idx).copied().unwrap_or(false);
            let composed =
                game.map
                    .composed_terrain_cell(wx, wy, game.surface_tick, game.map_visual_seed);
            let terrain_ch = composed.ch;
            let l = smooth_fog_luminance(
                game.map.width,
                game.map.height,
                &game.explored,
                &game.visible,
                wx,
                wy,
            );
            let fog_baked = game.atmosphere_bake.get(idx).copied().unwrap_or_default();
            let (out_fg, out_bg) = compose_fog_from_luminance(fog_baked, l);
            cell.ch = if seen {
                terrain_ch
            } else {
                unseen_fog_glyph(wx, wy, game.map_visual_seed)
            };
            cell.fg = out_fg;
            cell.bg = out_bg;
            fb.set(screen_x, screen_y, cell);
        }
    }

    // Entities on top
    for (i, alive) in game.entities.alive.iter().enumerate() {
        if !alive {
            continue;
        }
        let Some(ep) = game.entities.position[i] else {
            continue;
        };
        let wx = ep.x;
        let wy = ep.y;
        let sx = wx - ox;
        let sy = wy - oy;
        if sx < 0 || sy < 0 || sx >= cam_w || sy >= cam_h {
            continue;
        }
        let idx = wy as usize * game.map.width as usize + wx as usize;
        let vis = game.visible.get(idx).copied().unwrap_or(false);
        if !vis {
            continue;
        }
        let screen_x = area.x + sx as u16;
        let screen_y = area.y + sy as u16;
        let g = game.entities.glyph[i];
        let eid = EntityId(i as u32);
        let is_npc = game.entities.npc_kind[i].is_some();
        let base_fg = game.entities.fg[i];
        let relation_fg = if is_npc {
            match game.relation_to_player(eid) {
                Relation::Hostile => Some(Color::rgb(240, 95, 95)),
                // Reserve green for true allies only (party / joins the player in fights).
                Relation::Allied => Some(Color::rgb(120, 240, 140)),
                Relation::Friendly | Relation::Neutral => Some(base_fg),
            }
        } else {
            None
        };
        let ent_fg = relation_fg.unwrap_or(base_fg);
        let fog_baked = game.atmosphere_bake.get(idx).copied().unwrap_or_default();
        let ent_bg = fog_baked.visible.bg;
        let c = Cell {
            ch: g,
            fg: ent_fg,
            bg: ent_bg,
            style: Style {
                bold: true,
                dim: false,
                underline: false,
            },
        };
        fb.set(screen_x, screen_y, c);
    }

    // Spell targeting overlay: range ring, AoE preview, crosshair cursor.
    if let Some(GameMode::SpellTargeting { spell, cursor }) = game.modes.current().cloned() {
        let def = spell::def(spell);
        let player_pos = game.player_pos().unwrap_or(cursor);
        draw_targeting_overlay(fb, area, ox, oy, cam_w, cam_h, player_pos, cursor, def);
    }

    // Active projectiles / melee flashes on top of everything.
    for proj in &game.active_projectiles {
        let wp = proj.current_pos();
        let sx = wp.x - ox;
        let sy = wp.y - oy;
        if sx < 0 || sy < 0 || sx >= cam_w || sy >= cam_h {
            continue;
        }
        let screen_x = area.x + sx as u16;
        let screen_y = area.y + sy as u16;

        // Blend the projectile over whatever is already on that cell.
        let bg = fb.get(screen_x, screen_y).map_or(Color::rgb(0, 0, 0), |c| c.bg);
        fb.set(
            screen_x,
            screen_y,
            Cell {
                ch: proj.glyph,
                fg: proj.color,
                bg,
                style: Style {
                    bold: true,
                    dim: false,
                    underline: false,
                },
            },
        );
    }
}

/// Draw the spell targeting overlay: subtle range tint, area-of-effect preview circle, and cursor crosshair.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn draw_targeting_overlay(
    fb: &mut FrameBuffer,
    area: Rect,
    ox: i32,
    oy: i32,
    cam_w: i32,
    cam_h: i32,
    player_pos: GridPos,
    cursor: GridPos,
    def: &spell::SpellDef,
) {
    let aoe_r = i64::from(def.aoe_radius);
    let aoe_r_sq = aoe_r * aoe_r;
    let cursor_in_range = within_euclidean_radius(player_pos, cursor, def.range);

    // AoE preview: orange tint within explosion radius — only shown when cursor is in range.
    if cursor_in_range {
        for row in 0..cam_h {
            for col in 0..cam_w {
                let wx = ox + col;
                let wy = oy + row;
                if euclidean_dist_sq(wx - cursor.x, wy - cursor.y) > aoe_r_sq {
                    continue;
                }
                let screen_x = area.x + col as u16;
                let screen_y = area.y + row as u16;
                if let Some(existing) = fb.get(screen_x, screen_y) {
                    let tinted = existing.bg.blend_weight(spell::AOE_PREVIEW_COLOR, 55);
                    fb.set(screen_x, screen_y, Cell { bg: tinted, ..*existing });
                }
            }
        }
    }

    // Cursor crosshair on top.
    let csx = cursor.x - ox;
    let csy = cursor.y - oy;
    if csx >= 0 && csy >= 0 && csx < cam_w && csy < cam_h {
        let screen_x = area.x + csx as u16;
        let screen_y = area.y + csy as u16;
        let crosshair_fg = if cursor_in_range {
            Color::rgb(255, 220, 80)
        } else {
            Color::rgb(200, 60, 60)
        };
        let bg = fb.get(screen_x, screen_y).map_or(Color::rgb(0, 0, 0), |c| c.bg);
        fb.set(
            screen_x,
            screen_y,
            Cell {
                ch: '◎',
                fg: crosshair_fg,
                bg,
                style: Style { bold: true, dim: false, underline: false },
            },
        );
    }
}

