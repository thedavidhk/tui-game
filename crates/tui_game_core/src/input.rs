//! Normalized input events (no crossterm types).
//!
//! Mouse events carry **modifier flags** (`shift` / `ctrl` / `alt`) so UIs can distinguish
//! chorded clicks (e.g. shift–drag) without duplicating platform state.

use crate::rect::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Backspace,
    Enter,
    Esc,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
    Insert,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    /// Cursor moved to a new cell (no button action).
    Moved,
    ScrollUp,
    ScrollDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseCell {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyChord),
    Mouse {
        kind: MouseEventKind,
        cell: MouseCell,
        /// Raw column if needed for wide-char alignment (optional).
        column: u16,
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
    Resize {
        width: u16,
        height: u16,
    },
}

/// Batched input for one simulation step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputBatch {
    pub events: Vec<InputEvent>,
}

impl InputBatch {
    pub fn push(&mut self, ev: InputEvent) {
        self.events.push(ev);
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Hit-test: returns index of the first rect containing `cell`, or None.
pub fn hit_rect_index(cell: MouseCell, rects: &[Rect]) -> Option<usize> {
    rects.iter().position(|r| r.contains(cell.x, cell.y))
}
