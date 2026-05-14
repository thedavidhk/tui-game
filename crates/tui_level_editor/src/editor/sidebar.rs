//! Right-hand column: status, mode/layer controls, picker buttons, player row.

use tui_game_core::rect::Rect;
use tui_game_core::render::{Cell, Color, FrameBuffer, Style};
use tui_game_core::world::EMPTY_PROP_ID;

use super::{Editor, Mode, PaintLayer, SidebarHit};

pub(crate) fn trunc_visual(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    s.chars().take(max_cols).collect()
}

impl Editor {
    pub fn compose_sidebar(&mut self, fb: &mut FrameBuffer, help: Rect) {
        let inner = Rect::new(
            help.x.saturating_add(1),
            help.y.saturating_add(1),
            help.w.saturating_sub(2),
            help.h.saturating_sub(2),
        );
        let wlim = inner.right().saturating_sub(inner.x) as usize;
        let mut y = inner.y;
        let row = |s: &str| trunc_visual(s, wlim);
        Self::sidebar_plain(fb, inner, &mut y, &row(&self.status));
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!("File: {}", self.path.display())),
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!("Level: {}", self.level.name)),
        );
        Self::sidebar_plain(fb, inner, &mut y, "");
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "LMB map: paint / place / erase / zones   Shift+L: rect",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "Wheel on map: brush r   Ctrl+wheel (props): sparse %",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "Tab / m: cycle mode   Arrows: pan big maps   F2: save as",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "Ctrl+S save   Esc: cancel drag   Ctrl+Q quit   ext: hot-reload",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!(
                "r{}  zones:{}  pull:{}%",
                self.brush_radius,
                self.level.atmosphere_zones.len(),
                self.level.default_atmosphere.visible_background_pull
            )),
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(if self.dirty {
                "Edits: unsaved (reload asks)"
            } else {
                "Edits: clean (auto-reload from disk)"
            }),
        );
        Self::sidebar_plain(fb, inner, &mut y, "");
        Self::sidebar_plain(fb, inner, &mut y, "-- Brush / place --");
        self.sidebar_terrain_preview_row(fb, inner, &mut y);
        self.sidebar_entity_preview_row(fb, inner, &mut y);
        Self::sidebar_plain(fb, inner, &mut y, "");

        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            "> Search / pick terrain…",
            SidebarHit::OpenTerrainPicker,
            false,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            "> Search / pick entity…",
            SidebarHit::OpenEntityPicker,
            false,
        );
        Self::sidebar_plain(fb, inner, &mut y, "");

        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Paint tiles",
                if self.mode == Mode::PaintTiles {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::ModePaint,
            self.mode == Mode::PaintTiles,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Place entities",
                if self.mode == Mode::PlaceSpawns {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::ModePlace,
            self.mode == Mode::PlaceSpawns,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Erase spawns",
                if self.mode == Mode::EraseSpawns {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::ModeErase,
            self.mode == Mode::EraseSpawns,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Player spawn",
                if self.mode == Mode::SetPlayerSpawn {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::ModePlayer,
            self.mode == Mode::SetPlayerSpawn,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Atmosphere zones",
                if self.mode == Mode::AtmosphereZones {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::ModeAtmos,
            self.mode == Mode::AtmosphereZones,
        );
        Self::sidebar_plain(fb, inner, &mut y, "");

        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Layer: ground",
                if self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Ground {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::LayerGround,
            self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Ground,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            &format!(
                "{} Layer: props",
                if self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Prop {
                    '>'
                } else {
                    ' '
                }
            ),
            SidebarHit::LayerProp,
            self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Prop,
        );
        if self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Prop {
            self.sidebar_clear_prop_row(fb, inner, &mut y);
        }
        Self::sidebar_plain(fb, inner, &mut y, "");
        if self.mode == Mode::PaintTiles {
            let layer = match self.paint_layer {
                PaintLayer::Ground => "ground",
                PaintLayer::Prop => "prop",
            };
            let sp = if self.paint_layer == PaintLayer::Prop {
                if self.brush_sparse_pct == 0 || self.brush_sparse_pct >= 100 {
                    "  prop brush: dense".into()
                } else {
                    format!("  prop sparse {}%", self.brush_sparse_pct)
                }
            } else {
                String::new()
            };
            Self::sidebar_plain(fb, inner, &mut y, &row(&format!("Painting {layer}.{sp}")));
            if self.paint_layer == PaintLayer::Prop && self.current_tile == EMPTY_PROP_ID {
                Self::sidebar_plain(
                    fb,
                    inner,
                    &mut y,
                    &row("Prop brush clears overlay (see preview above)."),
                );
            }
        } else if self.mode == Mode::PlaceSpawns && self.current_spawn_blueprint().is_none() {
            Self::sidebar_plain(fb, inner, &mut y, "(no blueprints)");
        }
        Self::sidebar_plain(fb, inner, &mut y, "");
        Self::sidebar_plain(fb, inner, &mut y, "-- Player --");
        self.sidebar_player_spawn_row(fb, inner, &mut y);
    }

    fn sidebar_terrain_preview_row(&self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let preview_bg = Color::rgb(34, 30, 48);
        let label_fg = Color::rgb(205, 200, 190);
        if let Some(d) = self.preview_terrain_def() {
            fb.set(
                inner.x,
                row_y,
                Cell {
                    ch: d.glyph,
                    fg: d.fg,
                    bg: d.bg.unwrap_or(Color::rgb(22, 20, 28)),
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                },
            );
            let wlim = inner.w.saturating_sub(4) as usize;
            let rest = format!(
                " {} id{} {}",
                if d.solid() { "solid" } else { "open " },
                d.idx,
                trunc_visual(d.description(), wlim.saturating_sub(12))
            );
            let mut x = inner.x.saturating_add(2);
            for ch in rest.chars() {
                if x >= right {
                    break;
                }
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch,
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
            while x < right {
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch: ' ',
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
        } else {
            let mut x = inner.x;
            fb.set(
                x,
                row_y,
                Cell {
                    ch: '·',
                    fg: Color::rgb(150, 140, 125),
                    bg: preview_bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            for ch in " clear prop overlay".chars() {
                if x >= right {
                    break;
                }
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch,
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
            while x < right {
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch: ' ',
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
        }
        *y = y.saturating_add(1);
    }

    fn sidebar_entity_preview_row(&self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let preview_bg = Color::rgb(34, 30, 48);
        let label_fg = Color::rgb(205, 200, 190);
        if let Some(bp) = self.preview_entity_blueprint() {
            let gcol = bp.default_fg.to_render_color();
            fb.set(
                inner.x,
                row_y,
                Cell {
                    ch: bp.default_glyph,
                    fg: gcol,
                    bg: preview_bg,
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                },
            );
            let wlim = inner.w.saturating_sub(4) as usize;
            let name_budget = wlim.saturating_sub(bp.kind.len().saturating_add(3));
            let rest = format!(
                " {}  {}",
                bp.kind,
                trunc_visual(bp.display_name, name_budget)
            );
            let mut x = inner.x.saturating_add(2);
            for ch in rest.chars() {
                if x >= right {
                    break;
                }
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch,
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
            while x < right {
                fb.set(
                    x,
                    row_y,
                    Cell {
                        ch: ' ',
                        fg: label_fg,
                        bg: preview_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
        } else {
            Self::sidebar_plain(fb, inner, y, "(no entity blueprint)");
            return;
        }
        *y = y.saturating_add(1);
    }

    fn sidebar_hit_row(
        &mut self,
        fb: &mut FrameBuffer,
        inner: Rect,
        y: &mut u16,
        text: &str,
        hit: SidebarHit,
        accent: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = if accent {
            Color::rgb(40, 36, 55)
        } else {
            Color::rgb(18, 16, 22)
        };
        let fg = Color::rgb(210, 205, 195);
        let mut x = inner.x;
        for ch in text.chars() {
            if x >= right {
                break;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits
            .push((hit, Rect::new(inner.x, row_y, row_w, 1)));
        *y = y.saturating_add(1);
    }

    fn sidebar_player_spawn_row(&mut self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let mark_fg = Color::rgb(120, 220, 255);
        let mut x = inner.x;
        let mut put = |ch: char, fg: Color| -> bool {
            if x >= right {
                return false;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            true
        };
        let sel = self.mode == Mode::SetPlayerSpawn;
        let _ = put(if sel { '>' } else { ' ' }, meta_fg);
        let _ = put('@', mark_fg);
        let _ = put(' ', meta_fg);
        for ch in "Player spawn (click row)".chars() {
            let _ = put(ch, meta_fg);
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits.push((
            SidebarHit::PlayerSpawnRow,
            Rect::new(inner.x, row_y, row_w, 1),
        ));
        *y = y.saturating_add(1);
    }

    fn sidebar_clear_prop_row(&mut self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let mark = Color::rgb(200, 160, 120);
        let sel = self.current_tile == EMPTY_PROP_ID;
        let mut x = inner.x;
        let mut put = |ch: char, fg: Color| -> bool {
            if x >= right {
                return false;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            true
        };
        let _ = put(if sel { '>' } else { ' ' }, meta_fg);
        for ch in "(clear) ".chars() {
            let _ = put(ch, mark);
        }
        for ch in "no prop overlay".chars() {
            let _ = put(ch, meta_fg);
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits.push((
            SidebarHit::ClearPropOverlay,
            Rect::new(inner.x, row_y, row_w, 1),
        ));
        *y = y.saturating_add(1);
    }

    fn sidebar_plain(fb: &mut FrameBuffer, inner: Rect, y: &mut u16, text: &str) {
        if *y >= inner.bottom() {
            return;
        }
        let fg = Color::rgb(210, 205, 195);
        let bg = Color::rgb(18, 16, 22);
        let mut x = inner.x;
        for ch in text.chars() {
            if x >= inner.right() {
                break;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
        *y = y.saturating_add(1);
    }
}
