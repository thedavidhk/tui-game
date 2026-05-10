//! Modal dialog keyboard handling, pointer picking, and full-screen dialog rendering.

use std::path::PathBuf;

use tui_game_core::input::{InputEvent, Key, KeyChord, MouseButton, MouseEventKind};
use tui_game_core::rect::Rect;
use tui_game_core::render::{Cell, Color, FrameBuffer, Style};
use tui_game_core::ui::{
    centered_rect, centered_rect_dims, draw_bordered_panel, draw_text_block, draw_text_field,
    SearchListPicker, SearchListPickerHit, SearchListPickerOutput, TextFieldOutput,
};

use super::{Dialog, Editor};

impl Editor {
    pub fn handle_dialog(&mut self, chord: &KeyChord) -> bool {
        if chord.ctrl && matches!(chord.key, Key::Char('q')) {
            self.dialog = None;
            self.status = "QUIT".into();
            return true;
        }
        if matches!(&self.dialog, Some(Dialog::HotReloadUnsaved)) {
            if matches!(chord.key, Key::Char('y') | Key::Char('Y')) {
                self.dialog = None;
                match self.try_hot_reload_replace() {
                    Ok(()) => {}
                    Err(e) => {
                        self.status = format!("Hot-reload failed: {e}");
                        self.mark_dirty();
                        self.refresh_disk_fingerprint();
                    }
                }
                return true;
            }
            if matches!(
                chord.key,
                Key::Char('n') | Key::Char('N') | Key::Esc
            ) {
                self.dialog = None;
                self.refresh_disk_fingerprint();
                self.status = "Kept in-memory edits; ignored this disk revision.".into();
                return true;
            }
            return true;
        }
        let Some(d) = self.dialog.as_mut() else {
            return false;
        };
        match d {
            Dialog::SavePath { field } => {
                if matches!(chord.key, Key::Enter) && !chord.ctrl {
                    let s = field.text.trim();
                    if s.is_empty() {
                        self.status = "Filename cannot be empty.".into();
                    } else {
                        let mut p = PathBuf::from(s);
                        if p.extension().is_none()
                            || p.extension() == Some(std::ffi::OsStr::new(""))
                        {
                            p.set_extension("ron");
                        }
                        self.path = p;
                        if let Err(e) = self.save() {
                            self.status = e;
                        }
                        self.dialog = None;
                    }
                    return true;
                }
                match field.apply_key(chord) {
                    TextFieldOutput::Cancel => self.dialog = None,
                    TextFieldOutput::Tab => {}
                    TextFieldOutput::Edited => {}
                }
                true
            }
            Dialog::PickTerrain { picker } => match picker.apply_key(chord) {
                SearchListPickerOutput::Continue => true,
                SearchListPickerOutput::Cancel => {
                    self.dialog = None;
                    true
                }
                SearchListPickerOutput::Picked(i) => {
                    self.apply_terrain_pick_by_def_index(i);
                    self.dialog = None;
                    true
                }
            },
            Dialog::PickEntity { picker } => match picker.apply_key(chord) {
                SearchListPickerOutput::Continue => true,
                SearchListPickerOutput::Cancel => {
                    self.dialog = None;
                    true
                }
                SearchListPickerOutput::Picked(i) => {
                    self.apply_entity_pick_by_index(i);
                    self.dialog = None;
                    true
                }
            },
            Dialog::HotReloadUnsaved => true,
        }
    }

    /// Returns `true` if the event was consumed by a modal (including ignored wheel on modal).
    pub fn handle_dialog_mouse(&mut self, ev: &InputEvent) -> bool {
        let InputEvent::Mouse {
            kind,
            cell,
            column,
            ..
        } = ev
        else {
            return false;
        };

        let picker_list_vis = match self.dialog.as_ref() {
            None => return false,
            Some(Dialog::SavePath { .. } | Dialog::HotReloadUnsaved) => return true,
            Some(Dialog::PickTerrain { picker }) => picker.list_visible,
            Some(Dialog::PickEntity { picker }) => picker.list_visible,
        };

        let h = dialog_picker_panel_height(picker_list_vis);
        let r = centered_rect_dims(self.viewport_w, self.viewport_h, 72, h);
        if !r.contains(*column, cell.y) {
            return true;
        }
        if matches!(
            kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            return true;
        }
        let hit = SearchListPicker::hit(r, *column, cell.y, picker_list_vis);
        if let MouseEventKind::Down(MouseButton::Left) = kind {
            if let SearchListPickerHit::ListRow(row) = hit {
                let is_terrain = matches!(self.dialog, Some(Dialog::PickTerrain { .. }));
                let picked = if let Some(d) = self.dialog.as_mut() {
                    match d {
                        Dialog::PickTerrain { picker } | Dialog::PickEntity { picker } => {
                            picker.move_cursor_to_visible_row(row);
                            picker.pick_visible_row(row)
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(id) = picked {
                    if is_terrain {
                        self.apply_terrain_pick_by_def_index(id);
                    } else {
                        self.apply_entity_pick_by_index(id);
                    }
                    self.dialog = None;
                }
            }
        }
        true
    }

    pub fn draw_dialog_layer(&self, fb: &mut FrameBuffer, d: &Dialog) {
        match d {
            Dialog::SavePath { field } => {
                let dim = Cell {
                    ch: ' ',
                    fg: Color::rgb(100, 100, 110),
                    bg: Color::rgb(8, 8, 12),
                    style: Style {
                        bold: false,
                        dim: true,
                        underline: false,
                    },
                };
                fb.fill_rect(Rect::new(0, 0, fb.width, fb.height), dim);
                let r = centered_rect(fb, 64, 7);
                draw_bordered_panel(fb, r, " Save as ");
                let iy = r.y + 2;
                draw_text_field(
                    fb,
                    r.x + 2,
                    iy,
                    r.w.saturating_sub(6),
                    "Path: ",
                    field,
                    true,
                );
                let hint = Rect::new(r.x + 2, iy + 2, r.w.saturating_sub(4), 2);
                draw_text_block(
                    fb,
                    hint,
                    &[String::from(
                        "Enter: save & close   Esc: cancel   (.ron added if no extension)",
                    )],
                );
            }
            Dialog::PickTerrain { picker } => {
                let h = dialog_picker_panel_height(picker.list_visible);
                let r = centered_rect(fb, 72, h);
                draw_bordered_panel(fb, r, " Pick terrain ");
                picker.draw(fb, r);
            }
            Dialog::PickEntity { picker } => {
                let h = dialog_picker_panel_height(picker.list_visible);
                let r = centered_rect(fb, 72, h);
                draw_bordered_panel(fb, r, " Pick entity ");
                picker.draw(fb, r);
            }
            Dialog::HotReloadUnsaved => {
                let dim = Cell {
                    ch: ' ',
                    fg: Color::rgb(100, 100, 110),
                    bg: Color::rgb(8, 8, 12),
                    style: Style {
                        bold: false,
                        dim: true,
                        underline: false,
                    },
                };
                fb.fill_rect(Rect::new(0, 0, fb.width, fb.height), dim);
                let r = centered_rect(fb, 72, 10);
                draw_bordered_panel(fb, r, " File changed on disk ");
                let body = Rect::new(r.x + 2, r.y + 2, r.w.saturating_sub(4), r.h.saturating_sub(4));
                draw_text_block(
                    fb,
                    body,
                    &[
                        "The .ron file was modified outside this editor.".into(),
                        "You have unsaved changes here.".into(),
                        String::new(),
                        "Y — reload from disk (discard local edits)".into(),
                        "N / Esc — keep editing; ignore this revision".into(),
                    ],
                );
            }
        }
    }
}

fn dialog_picker_panel_height(list_visible: usize) -> u16 {
    // Bordered panel: query row + list + two hint lines + title row inside border.
    let inner = 3_u16
        .saturating_add(list_visible as u16)
        .saturating_add(2);
    inner.saturating_add(2).max(8)
}
