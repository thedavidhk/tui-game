//! Right-hand column: status, mode/layer controls, picker buttons, player row.

use tui_game_core::rect::Rect;
use tui_game_core::render::{Cell, Color, FrameBuffer, Style};
use tui_game_core::ui::{
    chrome_inner_rect, draw_rounded_panel, EditorHitTarget, GameUiPalette, PanelBorderEmphasis,
    UiHitTarget,
};
use tui_game_core::world::EMPTY_PROP_ID;

use super::{Editor, Mode, PaintLayer};

pub(crate) fn trunc_visual(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    s.chars().take(max_cols).collect()
}

impl Editor {
    pub fn compose_sidebar(&mut self, fb: &mut FrameBuffer, help: Rect) {
        let palette = GameUiPalette::DEFAULT;
        draw_rounded_panel(fb, help, "Editor", PanelBorderEmphasis::Subtle, &palette);
        let inner = chrome_inner_rect(help);
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
            EditorHitTarget::OpenTerrainPicker,
            false,
        );
        self.sidebar_hit_row(
            fb,
            inner,
            &mut y,
            "> Search / pick entity…",
            EditorHitTarget::OpenEntityPicker,
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
            EditorHitTarget::ModePaint,
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
            EditorHitTarget::ModePlace,
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
            EditorHitTarget::ModeErase,
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
            EditorHitTarget::ModePlayer,
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
            EditorHitTarget::ModeAtmos,
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
            EditorHitTarget::LayerGround,
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
            EditorHitTarget::LayerProp,
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
        let palette = GameUiPalette::DEFAULT;
        let row_y = *y;
        let right = inner.right();
        let preview_bg = palette.panel_bg_soft;
        let label_fg = palette.text;
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
        let palette = GameUiPalette::DEFAULT;
        let row_y = *y;
        let right = inner.right();
        let preview_bg = palette.panel_bg_soft;
        let label_fg = palette.text;
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
        hit: EditorHitTarget,
        accent: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
        let palette = GameUiPalette::DEFAULT;
        let row_y = *y;
        let right = inner.right();
        let (fg, bg) = if accent {
            (palette.selected_fg, palette.selected_bg)
        } else {
            (palette.text, palette.panel_bg)
        };
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
                    style: Style {
                        bold: accent,
                        dim: false,
                        underline: false,
                    },
                },
            );
            x = x.saturating_add(1);
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.ui_hits.push(
            UiHitTarget::Editor(hit),
            Rect::new(inner.x, row_y, row_w, 1),
        );
        *y = y.saturating_add(1);
    }

    fn sidebar_player_spawn_row(&mut self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let palette = GameUiPalette::DEFAULT;
        let row_y = *y;
        let right = inner.right();
        let bg = palette.panel_bg;
        let meta_fg = palette.text_dim;
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
        self.ui_hits.push(
            UiHitTarget::Editor(EditorHitTarget::PlayerSpawnRow),
            Rect::new(inner.x, row_y, row_w, 1),
        );
        *y = y.saturating_add(1);
    }

    fn sidebar_clear_prop_row(&mut self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let palette = GameUiPalette::DEFAULT;
        let row_y = *y;
        let right = inner.right();
        let bg = palette.panel_bg;
        let meta_fg = palette.text_dim;
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
        self.ui_hits.push(
            UiHitTarget::Editor(EditorHitTarget::ClearPropOverlay),
            Rect::new(inner.x, row_y, row_w, 1),
        );
        *y = y.saturating_add(1);
    }

    fn sidebar_plain(fb: &mut FrameBuffer, inner: Rect, y: &mut u16, text: &str) {
        if *y >= inner.bottom() {
            return;
        }
        let palette = GameUiPalette::DEFAULT;
        let fg = palette.text;
        let bg = palette.panel_bg;
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
