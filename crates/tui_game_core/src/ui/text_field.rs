//! Single-line text input for TUI panels (no terminal I/O).

use crate::input::{Key, KeyChord};
use crate::render::{Cell, FrameBuffer, Style};

use super::theme::GameUiPalette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFilter {
    /// Printable ASCII plus space (level titles, terrain names, save paths).
    Text,
    /// ASCII digits only (unsigned dimensions).
    Digits,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextField {
    pub text: String,
    /// Cursor as a character boundary index in `0..=char_count`.
    pub cursor: usize,
    pub max_chars: usize,
    pub filter: TextFilter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldOutput {
    /// Key was handled; text may have changed.
    Edited,
    /// User pressed Esc.
    Cancel,
    /// User pressed Tab (caller moves focus).
    Tab,
}

impl TextField {
    pub fn new(max_chars: usize, initial: impl AsRef<str>, filter: TextFilter) -> Self {
        let t = initial.as_ref().to_string();
        let cursor = t.chars().count();
        Self {
            text: t,
            cursor,
            max_chars,
            filter,
        }
    }

    pub fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index_of_char_cursor(&self) -> usize {
        self.text
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    fn allowed_char(&self, c: char) -> bool {
        match self.filter {
            TextFilter::Text => c.is_ascii_graphic() || c == ' ',
            TextFilter::Digits => c.is_ascii_digit(),
        }
    }

    pub fn apply_key(&mut self, key: &KeyChord) -> TextFieldOutput {
        if key.ctrl || key.alt {
            return TextFieldOutput::Edited;
        }
        match key.key {
            Key::Esc => TextFieldOutput::Cancel,
            Key::Enter => TextFieldOutput::Edited,
            Key::Tab => TextFieldOutput::Tab,
            Key::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                TextFieldOutput::Edited
            }
            Key::Right => {
                let n = self.char_len();
                self.cursor = (self.cursor + 1).min(n);
                TextFieldOutput::Edited
            }
            Key::Home => {
                self.cursor = 0;
                TextFieldOutput::Edited
            }
            Key::End => {
                self.cursor = self.char_len();
                TextFieldOutput::Edited
            }
            Key::Backspace => {
                if self.cursor == 0 {
                    return TextFieldOutput::Edited;
                }
                let at = self.cursor - 1;
                let bi = self
                    .text
                    .char_indices()
                    .nth(at)
                    .map(|(i, c)| (i, c.len_utf8()))
                    .unwrap_or((0, 0));
                self.text.drain(bi.0..bi.0 + bi.1);
                self.cursor = at;
                TextFieldOutput::Edited
            }
            Key::Delete => {
                let n = self.char_len();
                if self.cursor >= n {
                    return TextFieldOutput::Edited;
                }
                let bi = self
                    .text
                    .char_indices()
                    .nth(self.cursor)
                    .map(|(i, c)| (i, c.len_utf8()))
                    .unwrap_or((0, 0));
                self.text.drain(bi.0..bi.0 + bi.1);
                TextFieldOutput::Edited
            }
            Key::Char(c) => {
                if !self.allowed_char(c) {
                    return TextFieldOutput::Edited;
                }
                if self.char_len() >= self.max_chars {
                    return TextFieldOutput::Edited;
                }
                let byte = self.byte_index_of_char_cursor();
                self.text.insert(byte, c);
                self.cursor = self.cursor.saturating_add(1);
                TextFieldOutput::Edited
            }
            _ => TextFieldOutput::Edited,
        }
    }
}

/// Draw `label` then an input area of width `field_w` with an inverted cursor cell, using
/// [`GameUiPalette`] chrome colors.
// A drawing primitive: position, width, label, state, focus, and palette are all independent
// inputs; bundling them into a struct would not make call sites clearer.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_field(
    fb: &mut FrameBuffer,
    origin_x: u16,
    origin_y: u16,
    field_w: u16,
    label: &str,
    field: &TextField,
    active: bool,
    palette: &GameUiPalette,
) {
    let fg = if active {
        palette.selected_fg
    } else {
        palette.text
    };
    let mut x = origin_x;
    for ch in label.chars() {
        if x >= fb.width {
            return;
        }
        fb.set(
            x,
            origin_y,
            Cell {
                ch,
                fg: palette.text_dim,
                bg: palette.panel_bg,
                style: Style::default(),
            },
        );
        x = x.saturating_add(1);
    }
    for col in 0..field_w {
        if x >= fb.width {
            break;
        }
        let idx = col as usize;
        let ch = field.text.chars().nth(idx).unwrap_or(' ');
        let at_cursor = active && field.cursor == idx;
        fb.set(
            x,
            origin_y,
            Cell {
                ch,
                // Block cursor: inverse video against the field background.
                fg: if at_cursor { palette.panel_bg } else { fg },
                bg: if at_cursor {
                    palette.selected_fg
                } else {
                    palette.panel_bg_soft
                },
                style: Style::default(),
            },
        );
        x = x.saturating_add(1);
    }
}
