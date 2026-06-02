//! Crossterm → [`tui_game_core::input`] bridge shared by the game and editor binaries.
//!
//! `tui_game_core` stays crossterm-free (see `docs/DESIGN.md` §1); this thin crate is the one
//! place that translates terminal events into the engine's backend-agnostic [`InputEvent`]. Both
//! binaries map the full key/mouse set and ignore events they do not bind.

use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton as CMouseButton, MouseEvent,
    MouseEventKind as CMouseKind,
};

use tui_game_core::input::{InputEvent, Key, KeyChord, MouseButton, MouseCell, MouseEventKind};

/// Translate a crossterm key event, or `None` for keys with no engine mapping.
#[must_use]
pub fn map_key(k: KeyEvent) -> Option<InputEvent> {
    let key = match k.code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Tab => Key::Tab,
        KeyCode::Esc => Key::Esc,
        KeyCode::Delete => Key::Delete,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::F(n) => Key::F(n.min(12)),
        _ => return None,
    };
    Some(InputEvent::Key(KeyChord {
        key,
        ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
        alt: k.modifiers.contains(KeyModifiers::ALT),
        shift: k.modifiers.contains(KeyModifiers::SHIFT),
    }))
}

/// Translate a crossterm mouse event, or `None` for horizontal scroll (unused by the engine).
#[must_use]
pub fn map_mouse(m: MouseEvent) -> Option<InputEvent> {
    let map_button = |b: CMouseButton| match b {
        CMouseButton::Left => MouseButton::Left,
        CMouseButton::Right => MouseButton::Right,
        CMouseButton::Middle => MouseButton::Middle,
    };
    let kind = match m.kind {
        CMouseKind::Down(b) => MouseEventKind::Down(map_button(b)),
        CMouseKind::Up(b) => MouseEventKind::Up(map_button(b)),
        CMouseKind::Drag(b) => MouseEventKind::Drag(map_button(b)),
        CMouseKind::ScrollUp => MouseEventKind::ScrollUp,
        CMouseKind::ScrollDown => MouseEventKind::ScrollDown,
        CMouseKind::ScrollLeft | CMouseKind::ScrollRight => return None,
        CMouseKind::Moved => MouseEventKind::Moved,
    };
    Some(InputEvent::Mouse {
        kind,
        cell: MouseCell {
            x: m.column,
            y: m.row,
        },
        column: m.column,
        shift: m.modifiers.contains(KeyModifiers::SHIFT),
        ctrl: m.modifiers.contains(KeyModifiers::CONTROL),
        alt: m.modifiers.contains(KeyModifiers::ALT),
    })
}
