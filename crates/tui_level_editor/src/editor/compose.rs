//! Main framebuffer composition: map viewport, spawns, player marker, sidebar, dialogs.

use tui_game_core::level::MapTileFog;
use tui_game_core::render::{Cell, Color, FrameBuffer, Style};
use tui_game_core::ui::{cell_in_axis_rect, cell_in_brush};
use tui_game_core::world::compose_map_tile_discrete;

use super::{Editor, Mode};

impl Editor {
    pub fn compose(&mut self, fb: &mut FrameBuffer) {
        self.viewport_w = fb.width;
        self.viewport_h = fb.height;
        self.clamp_editor_view();
        self.surface_tick = self.surface_tick.wrapping_add(1);
        self.ui_hits.clear();

        let fg = Color::rgb(210, 210, 200);
        let bg = Color::rgb(12, 12, 18);
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.set(
                    x,
                    y,
                    Cell {
                        ch: ' ',
                        fg,
                        bg,
                        style: Style::default(),
                    },
                );
            }
        }
        let map = self.map_area_rect();
        let ox = map.x;
        let oy = map.y;
        let vw = map.w as usize;
        let vh = map.h as usize;
        let vo_x = self.view_origin_x;
        let vo_y = self.view_origin_y;
        let lw = self.level.width as i32;
        let lh = self.level.height as i32;

        for j in 0..vh {
            for i in 0..vw {
                let tx = vo_x + i as i32;
                let ty = vo_y + j as i32;
                let sx = ox.saturating_add(i as u16);
                let sy = oy.saturating_add(j as u16);
                if tx < 0 || ty < 0 || tx >= lw || ty >= lh {
                    fb.set(
                        sx,
                        sy,
                        Cell {
                            ch: ' ',
                            fg,
                            bg,
                            style: Style::default(),
                        },
                    );
                    continue;
                }
                let wi = self.level.width as usize;
                let idx = ty as usize * wi + tx as usize;
                let (ch, tile_fg) = if let Some(ref map) = self.level_map {
                    let c =
                        map.composed_terrain_cell(tx, ty, self.surface_tick, self.map_visual_seed);
                    (c.ch, c.fg)
                } else {
                    ('?', Color::rgb(200, 190, 170))
                };
                let fog_baked = self.atmosphere_bake.get(idx).copied().unwrap_or_default();
                let (_, tile_bg) = compose_map_tile_discrete(fog_baked, MapTileFog::Visible);
                let mut c = Cell {
                    ch,
                    fg: tile_fg,
                    bg: tile_bg,
                    style: Style::default(),
                };
                if !self.dialog_covers_map() {
                    if let Some((hx, hy)) = self.hover_map_cell {
                        let txi = tx;
                        let tyi = ty;
                        let mut lift: u8 = 0;
                        match self.mode {
                            Mode::PaintTiles => {
                                if cell_in_brush(txi, tyi, hx, hy, self.brush_radius) {
                                    lift = lift.max(14);
                                }
                                if let Some((sx, sy)) = self.rect_drag_start {
                                    if cell_in_axis_rect(txi, tyi, sx, sy, hx, hy) {
                                        lift = lift.max(20);
                                    }
                                }
                            }
                            Mode::PlaceSpawns => {
                                if txi == hx && tyi == hy {
                                    lift = lift.max(14);
                                }
                            }
                            Mode::SetPlayerSpawn => {
                                if txi == hx && tyi == hy {
                                    lift = lift.max(16);
                                }
                            }
                            Mode::EraseSpawns => {
                                if cell_in_brush(txi, tyi, hx, hy, self.brush_radius) {
                                    let mut l: u8 = 12;
                                    if self.cell_has_spawn(txi, tyi) {
                                        l = l.max(28);
                                    }
                                    lift = lift.max(l);
                                }
                                if let Some((sx, sy)) = self.rect_drag_start {
                                    if cell_in_axis_rect(txi, tyi, sx, sy, hx, hy) {
                                        let mut l: u8 = 10;
                                        if self.cell_has_spawn(txi, tyi) {
                                            l = l.max(26);
                                        }
                                        lift = lift.max(l);
                                    }
                                }
                            }
                            Mode::AtmosphereZones => {
                                if cell_in_brush(txi, tyi, hx, hy, self.brush_radius) {
                                    lift = lift.max(14);
                                }
                            }
                        }
                        if lift > 0 {
                            c.bg = c.bg.lighten(lift);
                        }
                    }
                }
                if !self.dialog_covers_map()
                    && self
                        .hover_map_cell
                        .is_some_and(|(hx, hy)| hx == tx && hy == ty)
                {
                    c.style.bold = true;
                    c.fg = Color::rgb(255, 255, 120);
                }
                fb.set(sx, sy, c);
            }
        }
        for s in &self.level.spawns {
            if s.x >= 0
                && s.y >= 0
                && (s.x as u16) < self.level.width
                && (s.y as u16) < self.level.height
            {
                if s.x < vo_x || s.y < vo_y || s.x >= vo_x + vw as i32 || s.y >= vo_y + vh as i32 {
                    continue;
                }
                let mut spawn_bg = bg;
                if !self.dialog_covers_map() && self.mode == Mode::EraseSpawns {
                    if let Some((hx, hy)) = self.hover_map_cell {
                        if cell_in_brush(s.x, s.y, hx, hy, self.brush_radius) {
                            spawn_bg = spawn_bg.lighten(18);
                        }
                        if let Some((sx, sy)) = self.rect_drag_start {
                            if cell_in_axis_rect(s.x, s.y, sx, sy, hx, hy) {
                                spawn_bg = spawn_bg.lighten(14);
                            }
                        }
                    }
                }
                let c = Cell {
                    ch: self.spawn_glyph(s),
                    fg: self.spawn_fg(s),
                    bg: spawn_bg,
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                };
                let px = ox.saturating_add((s.x - vo_x) as u16);
                let py = oy.saturating_add((s.y - vo_y) as u16);
                fb.set(px, py, c);
            }
        }
        if let Some(ps) = self.level.player_spawn {
            if ps.x >= 0
                && ps.y >= 0
                && (ps.x as u16) < self.level.width
                && (ps.y as u16) < self.level.height
                && ps.x >= vo_x
                && ps.y >= vo_y
                && ps.x < vo_x + vw as i32
                && ps.y < vo_y + vh as i32
            {
                let px = ox.saturating_add((ps.x - vo_x) as u16);
                let py = oy.saturating_add((ps.y - vo_y) as u16);
                let mut spawn_bg = bg;
                if !self.dialog_covers_map() && self.mode == Mode::SetPlayerSpawn {
                    if let Some((hx, hy)) = self.hover_map_cell {
                        if ps.x == hx && ps.y == hy {
                            spawn_bg = spawn_bg.lighten(20);
                        }
                    }
                }
                let c = Cell {
                    ch: '@',
                    fg: Color::rgb(120, 220, 255),
                    bg: spawn_bg,
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                };
                fb.set(px, py, c);
            }
        }
        self.compose_sidebar(fb, self.sidebar_rect());

        if self.dialog.is_some() {
            self.draw_dialog_layer(fb);
        }
    }
}
