//! Keyboard: modes, save, brush tweaks, map pan, and save-as dialog.

use tui_game_core::input::{Key, KeyChord};
use tui_game_core::ui::{TextField, TextFilter};

use super::{Editor, Dialog, Mode, MAX_BRUSH_SIZE};

impl Editor {
    pub fn step_main_key(&mut self, chord: &KeyChord) {
        match chord {
            KeyChord {
                key: Key::Tab,
                ctrl: false,
                ..
            } => {
                self.cycle_mode();
            }
            KeyChord {
                key: Key::Char('=') | Key::Char('+'),
                ctrl: false,
                ..
            } => {
                self.brush_radius = (self.brush_radius + 1).min(MAX_BRUSH_SIZE);
                self.status = format!("Brush radius {}", self.brush_radius);
            }
            KeyChord {
                key: Key::Char('-') | Key::Char('_'),
                ctrl: false,
                ..
            } => {
                self.brush_radius = self.brush_radius.saturating_sub(1);
                self.status = format!("Brush radius {}", self.brush_radius);
            }
            KeyChord {
                key: Key::Char('0'),
                ctrl: false,
                ..
            } if self.mode == Mode::PaintTiles => {
                self.brush_sparse_pct = 0;
                self.status =
                    "Prop brush: dense fill. , . sparse ±1%   Ctrl+wheel on map (prop layer)".into();
            }
            KeyChord {
                key: Key::Char(','),
                ctrl: false,
                ..
            } if self.mode == Mode::PaintTiles => {
                self.brush_sparse_pct = self.brush_sparse_pct.saturating_sub(1);
                self.status = if self.brush_sparse_pct == 0 {
                    "Prop brush: dense. , . sparse   Ctrl+wheel (prop layer)".into()
                } else {
                    format!("Prop sparse {}% (else clear)", self.brush_sparse_pct)
                };
            }
            KeyChord {
                key: Key::Char('.'),
                ctrl: false,
                ..
            } if self.mode == Mode::PaintTiles => {
                self.brush_sparse_pct = (self.brush_sparse_pct + 1).min(100);
                self.status = if self.brush_sparse_pct >= 100 {
                    "Prop brush: dense (100%).".into()
                } else {
                    format!("Prop sparse {}% (else clear)", self.brush_sparse_pct)
                };
            }
            KeyChord {
                key: Key::Esc,
                ctrl: false,
                ..
            } => {
                self.rect_drag_start = None;
                self.last_paint_cell = None;
            }
            KeyChord {
                key: Key::Backspace,
                ctrl: false,
                ..
            } if self.mode == Mode::SetPlayerSpawn => {
                self.clear_player_spawn();
            }
            KeyChord {
                key: Key::Backspace,
                ctrl: false,
                ..
            } if self.mode == Mode::AtmosphereZones => {
                self.remove_last_atmosphere_zone();
            }
            KeyChord {
                key: Key::Char('s'),
                ctrl: true,
                ..
            } => {
                if let Err(e) = self.save() {
                    self.status = e;
                }
            }
            KeyChord {
                key: Key::Char('m'),
                ctrl: false,
                ..
            } => {
                self.cycle_mode();
            }
            KeyChord {
                key: Key::F(2),
                ctrl: false,
                ..
            } => {
                let initial = self.path.to_string_lossy().into_owned();
                self.dialog = Some(Dialog::SavePath {
                    field: TextField::new(96, initial, TextFilter::Text),
                });
            }
            KeyChord {
                key: Key::Up,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_y -= 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Down,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_y += 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Left,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_x -= 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Right,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_x += 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Char('q'),
                ctrl: true,
                ..
            } => {
                self.status = "QUIT".into();
            }
            _ => {}
        }
    }
}
