//! Filtered searchable list for modal pickers (type to narrow, Enter or click to confirm).

use crate::input::{Key, KeyChord};
use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

use super::{draw_text_block, draw_text_field, TextField, TextFieldOutput, TextFilter};

#[derive(Clone, Debug)]
struct Entry {
    id: usize,
    line: String,
    haystack: String,
}

/// Stateful filter + scrollable matches for a modal picker.
#[derive(Debug)]
pub struct SearchListPicker {
    pub query: TextField,
    entries: Vec<Entry>,
    /// Indices into `entries` matching the current filter.
    filtered: Vec<usize>,
    /// Index into `filtered`.
    list_cursor: usize,
    /// Index into `filtered` of the top visible row.
    scroll: usize,
    /// Max number of list rows drawn (scroll for the rest).
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchListPickerHit {
    Outside,
    QueryField,
    /// Row index `0..list_visible` within the visible window.
    ListRow(usize),
}

impl SearchListPicker {
    pub fn new(list_visible: usize, query_max_chars: usize) -> Self {
        let list_visible = list_visible.max(1);
        Self {
            query: TextField::new(query_max_chars, "", TextFilter::Text),
            entries: Vec::new(),
            filtered: Vec::new(),
            list_cursor: 0,
            scroll: 0,
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
            self.scroll = 0;
            return;
        }
        self.list_cursor = self.list_cursor.min(self.filtered.len() - 1);
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let n = self.filtered.len();
        if n == 0 {
            self.scroll = 0;
            return;
        }
        let vis = self.list_visible.min(n);
        if self.list_cursor < self.scroll {
            self.scroll = self.list_cursor;
        }
        if self.list_cursor >= self.scroll + vis {
            self.scroll = self.list_cursor + 1 - vis;
        }
        let max_scroll = n.saturating_sub(vis);
        self.scroll = self.scroll.min(max_scroll);
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
            if !self.filtered.is_empty() && self.list_cursor > 0 {
                self.list_cursor -= 1;
                self.clamp_scroll();
            }
            return SearchListPickerOutput::Continue;
        }
        if !chord.ctrl && matches!(chord.key, Key::Down) {
            if !self.filtered.is_empty() && self.list_cursor + 1 < self.filtered.len() {
                self.list_cursor += 1;
                self.clamp_scroll();
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

    /// First screen row of the list (single-line query is above this).
    pub fn list_origin_y(panel: Rect) -> u16 {
        panel.y.saturating_add(3)
    }

    /// Hit-test against the same layout as [`draw_search_list_picker`].
    pub fn hit(panel: Rect, cell_x: u16, cell_y: u16, list_visible: usize) -> SearchListPickerHit {
        if !panel.contains(cell_x, cell_y) {
            return SearchListPickerHit::Outside;
        }
        let inner_left = panel.x.saturating_add(2);
        let inner_right = panel.right().saturating_sub(2);
        if cell_x < inner_left || cell_x >= inner_right {
            return SearchListPickerHit::Outside;
        }
        let qy = panel.y.saturating_add(2);
        if cell_y == qy {
            return SearchListPickerHit::QueryField;
        }
        let list_top = Self::list_origin_y(panel);
        if cell_y < list_top {
            return SearchListPickerHit::Outside;
        }
        let row = (cell_y - list_top) as usize;
        if row < list_visible {
            SearchListPickerHit::ListRow(row)
        } else {
            SearchListPickerHit::Outside
        }
    }

    /// Resolve a visible list row click to a stable entry id, or `None` if out of range.
    pub fn pick_visible_row(&self, visible_row: usize) -> Option<usize> {
        let i = self.scroll.checked_add(visible_row)?;
        let entry_i = *self.filtered.get(i)?;
        Some(self.entries[entry_i].id)
    }

    pub fn move_cursor_to_visible_row(&mut self, visible_row: usize) {
        let i = self.scroll.saturating_add(visible_row);
        if i < self.filtered.len() {
            self.list_cursor = i;
            self.clamp_scroll();
        }
    }

    /// Draw body inside an already bordered `panel` (title drawn separately). Uses `panel` metrics
    /// consistent with [`hit`].
    pub fn draw(&self, fb: &mut FrameBuffer, panel: Rect) {
        let inner_w = panel.w.saturating_sub(4);
        let iy = panel.y.saturating_add(2);
        draw_text_field(
            fb,
            panel.x.saturating_add(2),
            iy,
            inner_w.saturating_sub(8).max(1),
            "Filter: ",
            &self.query,
            true,
        );
        let list_top = Self::list_origin_y(panel);
        let fg_sel = Color::rgb(255, 250, 220);
        let bg_sel = Color::rgb(55, 48, 78);
        let fg = Color::rgb(210, 205, 195);
        let bg = Color::rgb(18, 16, 22);
        for vis in 0..self.list_visible {
            let y = list_top.saturating_add(vis as u16);
            if y >= panel.bottom().saturating_sub(1) {
                break;
            }
            let i = self.scroll + vis;
            let text = if let Some(&ei) = self.filtered.get(i) {
                trunc_cols(&self.entries[ei].line, inner_w as usize)
            } else {
                String::new()
            };
            let sel = i == self.list_cursor;
            let mut x = panel.x.saturating_add(2);
            for ch in text.chars() {
                if x >= panel.right().saturating_sub(2) {
                    break;
                }
                fb.set(
                    x,
                    y,
                    Cell {
                        ch,
                        fg: if sel { fg_sel } else { fg },
                        bg: if sel { bg_sel } else { bg },
                        style: Style {
                            bold: sel,
                            dim: false,
                            underline: false,
                        },
                    },
                );
                x = x.saturating_add(1);
            }
            while x < panel.right().saturating_sub(2) {
                fb.set(
                    x,
                    y,
                    Cell {
                        ch: ' ',
                        fg: if sel { fg_sel } else { fg },
                        bg: if sel { bg_sel } else { bg },
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
        }
        let hint_y = list_top.saturating_add(self.list_visible as u16);
        if hint_y < panel.bottom().saturating_sub(1) {
            let hint = Rect::new(
                panel.x.saturating_add(2),
                hint_y,
                inner_w,
                panel.bottom().saturating_sub(hint_y).saturating_sub(1),
            );
            draw_text_block(
                fb,
                hint,
                &[
                    "↑↓ choose   Enter pick   Click row   Esc close".into(),
                    "Type to filter".into(),
                ],
            );
        }
    }
}

fn trunc_cols(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    s.chars().take(max_cols).collect()
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
