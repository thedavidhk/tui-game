//! Modal dialog keyboard handling, pointer picking, and full-screen dialog rendering.

use std::path::PathBuf;

use tui_game_core::input::{InputEvent, Key, KeyChord, MouseButton, MouseEventKind};
use tui_game_core::rect::Rect;
use tui_game_core::render::FrameBuffer;
use tui_game_core::ui::{
    centered_rect, chrome_inner_rect, draw_modal_world_scrim, draw_rounded_panel,
    draw_text_block_theme, draw_text_field, EditorHitTarget, GameUiPalette, PanelBorderEmphasis,
    SearchListPickerOutput, TextFieldOutput, UiHitTarget,
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
            if matches!(chord.key, Key::Char('n') | Key::Char('N') | Key::Esc) {
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
    ///
    /// Picker rows are picked from the shared [`tui_game_core::ui::UiHitState`] populated by the
    /// previous frame's compose, the same path the game shell uses.
    pub fn handle_dialog_mouse(&mut self, ev: &InputEvent) -> bool {
        let InputEvent::Mouse { kind, cell, .. } = ev else {
            return false;
        };
        if self.dialog.is_none() {
            return false;
        }
        // A modal swallows all pointer input; only a left-click on a picker row does anything.
        if let MouseEventKind::Down(MouseButton::Left) = kind {
            if let Some(UiHitTarget::Editor(EditorHitTarget::PickerRow(idx))) =
                self.ui_hits.pick(*cell)
            {
                let is_terrain = matches!(self.dialog, Some(Dialog::PickTerrain { .. }));
                let picked = match self.dialog.as_mut() {
                    Some(Dialog::PickTerrain { picker } | Dialog::PickEntity { picker }) => {
                        picker.set_cursor(idx);
                        picker.pick_filtered_row(idx)
                    }
                    _ => None,
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

    pub fn draw_dialog_layer(&mut self, fb: &mut FrameBuffer) {
        let palette = GameUiPalette::DEFAULT;
        let screen = Rect::new(0, 0, fb.width, fb.height);
        let last_mouse = self.last_mouse_cell;
        match &self.dialog {
            None => {}
            Some(Dialog::SavePath { field }) => {
                draw_modal_world_scrim(fb, screen, &palette);
                let r = centered_rect(fb, 64, 7);
                draw_rounded_panel(fb, r, "Save as", PanelBorderEmphasis::Highlighted, &palette);
                let inner = chrome_inner_rect(r);
                draw_text_field(
                    fb,
                    inner.x,
                    inner.y.saturating_add(1),
                    inner.w.saturating_sub(6),
                    "Path: ",
                    field,
                    true,
                    &palette,
                );
                draw_text_block_theme(
                    fb,
                    Rect::new(inner.x, inner.y.saturating_add(3), inner.w, 2),
                    &[String::from(
                        "Enter: save & close   Esc: cancel   (.ron added if no extension)",
                    )],
                    &palette,
                );
            }
            Some(Dialog::PickTerrain { picker }) => {
                draw_modal_world_scrim(fb, screen, &palette);
                let r = centered_rect(fb, 72, dialog_picker_panel_height(picker.list_visible));
                draw_rounded_panel(
                    fb,
                    r,
                    "Pick terrain",
                    PanelBorderEmphasis::Highlighted,
                    &palette,
                );
                picker.draw(fb, r, &palette, last_mouse, &mut self.ui_hits);
            }
            Some(Dialog::PickEntity { picker }) => {
                draw_modal_world_scrim(fb, screen, &palette);
                let r = centered_rect(fb, 72, dialog_picker_panel_height(picker.list_visible));
                draw_rounded_panel(
                    fb,
                    r,
                    "Pick entity",
                    PanelBorderEmphasis::Highlighted,
                    &palette,
                );
                picker.draw(fb, r, &palette, last_mouse, &mut self.ui_hits);
            }
            Some(Dialog::HotReloadUnsaved) => {
                draw_modal_world_scrim(fb, screen, &palette);
                let r = centered_rect(fb, 72, 10);
                draw_rounded_panel(
                    fb,
                    r,
                    "File changed on disk",
                    PanelBorderEmphasis::Highlighted,
                    &palette,
                );
                let inner = chrome_inner_rect(r);
                draw_text_block_theme(
                    fb,
                    Rect::new(
                        inner.x,
                        inner.y.saturating_add(1),
                        inner.w,
                        inner.h.saturating_sub(1),
                    ),
                    &[
                        "The .ron file was modified outside this editor.".into(),
                        "You have unsaved changes here.".into(),
                        String::new(),
                        "Y — reload from disk (discard local edits)".into(),
                        "N / Esc — keep editing; ignore this revision".into(),
                    ],
                    &palette,
                );
            }
        }
    }
}

fn dialog_picker_panel_height(list_visible: usize) -> u16 {
    // Bordered panel: query row + list + two hint lines + title row inside border.
    let inner = 3_u16.saturating_add(list_visible as u16).saturating_add(2);
    inner.saturating_add(2).max(8)
}
