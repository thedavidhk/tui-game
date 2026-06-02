//! Filtered searchable list for modal pickers (type to narrow, Enter or click to confirm).

use crate::input::{Key, KeyChord, MouseCell};
use crate::rect::Rect;
use crate::render::FrameBuffer;

use super::chrome::chrome_inner_rect;
use super::hit::{EditorHitTarget, UiHitState, UiHitTarget};
use super::list::{draw_selectable_list, SelectableList};
use super::theme::GameUiPalette;
use super::{draw_text_block_theme, draw_text_field, TextField, TextFieldOutput, TextFilter};

#[derive(Clone, Debug)]
struct Entry {
    id: usize,
    line: String,
    haystack: String,
}

/// Stateful filter + selected row for a modal picker. Scrolling is handled by
/// [`draw_selectable_list`] at draw time, keyed off `list_cursor`.
#[derive(Debug)]
pub struct SearchListPicker {
    pub query: TextField,
    entries: Vec<Entry>,
    /// Indices into `entries` matching the current filter.
    filtered: Vec<usize>,
    /// Index into `filtered`.
    list_cursor: usize,
    /// Preferred number of list rows (used by callers to size the modal panel).
    pub list_visible: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchListPickerOutput {
    /// Keep the modal open.
    Continue,
    Cancel,
    /// Stable `id` from [`SearchListPicker::set_entries`].
    Picked(usize),
}

impl SearchListPicker {
    pub fn new(list_visible: usize, query_max_chars: usize) -> Self {
        let list_visible = list_visible.max(1);
        Self {
            query: TextField::new(query_max_chars, "", TextFilter::Text),
            entries: Vec::new(),
            filtered: Vec::new(),
            list_cursor: 0,
            list_visible,
        }
    }

    /// Replace catalog entries. Each item is `(id, display_line, search_blob)`; matching is
    /// ASCII case-insensitive substring on `search_blob`.
    pub fn set_entries(&mut self, items: impl IntoIterator<Item = (usize, String, String)>) {
        self.entries.clear();
        for (id, line, search_blob) in items {
            self.entries.push(Entry {
                id,
                line,
                haystack: search_blob.to_ascii_lowercase(),
            });
        }
        self.rebuild_filtered();
    }

    /// Recompute matches after changing `query.text` outside [`apply_key`].
    pub fn sync_filter(&mut self) {
        self.rebuild_filtered();
    }

    fn rebuild_filtered(&mut self) {
        let q = self.query.text.to_ascii_lowercase();
        self.filtered.clear();
        for (i, e) in self.entries.iter().enumerate() {
            if q.is_empty() || e.haystack.contains(&q) {
                self.filtered.push(i);
            }
        }
        if self.filtered.is_empty() {
            self.list_cursor = 0;
        } else {
            self.list_cursor = self.list_cursor.min(self.filtered.len() - 1);
        }
    }

    pub fn apply_key(&mut self, chord: &KeyChord) -> SearchListPickerOutput {
        if !chord.ctrl && matches!(chord.key, Key::Esc) {
            return SearchListPickerOutput::Cancel;
        }
        if !chord.ctrl && matches!(chord.key, Key::Enter) {
            return if self.filtered.is_empty() {
                SearchListPickerOutput::Continue
            } else {
                let entry_i = self.filtered[self.list_cursor];
                SearchListPickerOutput::Picked(self.entries[entry_i].id)
            };
        }
        if !chord.ctrl && matches!(chord.key, Key::Up) {
            self.list_cursor = self.list_cursor.saturating_sub(1);
            return SearchListPickerOutput::Continue;
        }
        if !chord.ctrl && matches!(chord.key, Key::Down) {
            if !self.filtered.is_empty() && self.list_cursor + 1 < self.filtered.len() {
                self.list_cursor += 1;
            }
            return SearchListPickerOutput::Continue;
        }

        let before = self.query.text.clone();
        match self.query.apply_key(chord) {
            TextFieldOutput::Cancel => SearchListPickerOutput::Cancel,
            TextFieldOutput::Tab | TextFieldOutput::Edited => {
                if before != self.query.text {
                    self.rebuild_filtered();
                }
                SearchListPickerOutput::Continue
            }
        }
    }

    /// Resolve a click on filtered row `idx` to a stable entry id, or `None` if out of range.
    #[must_use]
    pub fn pick_filtered_row(&self, idx: usize) -> Option<usize> {
        let entry_i = *self.filtered.get(idx)?;
        Some(self.entries[entry_i].id)
    }

    /// Move the keyboard selection to filtered row `idx` (no-op if out of range).
    pub fn set_cursor(&mut self, idx: usize) {
        if idx < self.filtered.len() {
            self.list_cursor = idx;
        }
    }

    /// Draw the picker body inside an already-drawn rounded `panel`: query field, scrollable
    /// match list (hover + click registered as [`EditorHitTarget::PickerRow`]), then a hint.
    pub fn draw(
        &self,
        fb: &mut FrameBuffer,
        panel: Rect,
        palette: &GameUiPalette,
        last_mouse: Option<MouseCell>,
        hits: &mut UiHitState,
    ) {
        let inner = chrome_inner_rect(panel);
        if inner.w == 0 || inner.h == 0 {
            return;
        }
        let label = "Filter: ";
        let label_w = u16::try_from(label.chars().count()).unwrap_or(0);
        let field_w = inner.w.saturating_sub(label_w).max(1);
        draw_text_field(
            fb,
            inner.x,
            inner.y,
            field_w,
            label,
            &self.query,
            true,
            palette,
        );

        let list_top = inner.y.saturating_add(2);
        let list_rect = Rect::new(
            inner.x,
            list_top,
            inner.w,
            inner.bottom().saturating_sub(list_top),
        );
        let rows: Vec<String> = self
            .filtered
            .iter()
            .map(|&ei| self.entries[ei].line.clone())
            .collect();
        let list = SelectableList {
            inner: list_rect,
            rows: &rows,
            selected: Some(self.list_cursor),
            last_mouse,
            empty_text: Some("(no matches)"),
            reserved_footer_rows: 2,
        };
        draw_selectable_list(fb, palette, &list, hits, |i| {
            UiHitTarget::Editor(EditorHitTarget::PickerRow(i))
        });

        let hint_y = inner.bottom().saturating_sub(2);
        if hint_y >= list_top {
            draw_text_block_theme(
                fb,
                Rect::new(inner.x, hint_y, inner.w, 2),
                &[
                    "↑↓ choose   Enter pick   Click row   Esc close".into(),
                    "Type to filter".into(),
                ],
                palette,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, KeyChord};

    fn chord_key(key: Key) -> KeyChord {
        KeyChord {
            key,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    #[test]
    fn filter_narrows_and_pick_returns_stable_id() {
        let mut p = SearchListPicker::new(5, 32);
        p.set_entries([
            (10, "alpha one".into(), "alpha one x".into()),
            (20, "beta two".into(), "beta two y".into()),
            (30, "alpha three".into(), "alpha three z".into()),
        ]);
        assert_eq!(p.filtered.len(), 3);
        p.query.text = "beta".into();
        p.query.cursor = p.query.char_len();
        p.sync_filter();
        assert_eq!(p.filtered.len(), 1);
        assert_eq!(
            p.apply_key(&chord_key(Key::Enter)),
            SearchListPickerOutput::Picked(20)
        );
    }

    #[test]
    fn down_moves_selection() {
        let mut p = SearchListPicker::new(5, 16);
        p.set_entries((0..4).map(|i| (i, format!("id {i}"), format!("id {i}"))));
        assert_eq!(
            p.apply_key(&chord_key(Key::Down)),
            SearchListPickerOutput::Continue
        );
        assert_eq!(p.list_cursor, 1);
        assert_eq!(
            p.apply_key(&chord_key(Key::Enter)),
            SearchListPickerOutput::Picked(1)
        );
    }
}
