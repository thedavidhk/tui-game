//! Build ANSI/CSI output from `FrameBuffer` without terminal I/O.

use std::io::Write;

use unicode_width::UnicodeWidthChar;

use super::buffer::{Cell, Color, FrameBuffer};

fn push_truecolor_fg(out: &mut Vec<u8>, c: Color) {
    let _ = write!(out, "\x1b[38;2;{};{};{}m", c.r, c.g, c.b);
}

fn push_truecolor_bg(out: &mut Vec<u8>, c: Color) {
    let _ = write!(out, "\x1b[48;2;{};{};{}m", c.r, c.g, c.b);
}

fn push_cell_attrs(out: &mut Vec<u8>, c: &Cell) {
    out.extend_from_slice(b"\x1b[0m");
    push_truecolor_fg(out, c.fg);
    push_truecolor_bg(out, c.bg);
    if c.style.bold {
        out.extend_from_slice(b"\x1b[1m");
    }
    if c.style.dim {
        out.extend_from_slice(b"\x1b[2m");
    }
    if c.style.underline {
        out.extend_from_slice(b"\x1b[4m");
    }
}

fn push_cursor_pos(out: &mut Vec<u8>, x: u16, y: u16) {
    // CSI y;x H — 1-based for typical terminals; crossterm uses 0-based internally;
    // here we use 1-based row/col to match common ANSI expectations.
    let row = y.saturating_add(1);
    let col = x.saturating_add(1);
    let _ = write!(out, "\x1b[{};{}H", row, col);
}

/// Full redraw: assume cursor home + clear optional is done by caller if needed.
pub fn encode_frame_full(fb: &FrameBuffer) -> Vec<u8> {
    let mut out = Vec::with_capacity(fb.cells().len() * 32);
    for y in 0..fb.height {
        for x in 0..fb.width {
            push_cursor_pos(&mut out, x, y);
            let c = fb.get(x, y).unwrap();
            push_cell_attrs(&mut out, c);
            push_char_utf8(&mut out, c.ch);
        }
    }
    out.extend_from_slice(b"\x1b[0m");
    out
}

fn push_char_utf8(out: &mut Vec<u8>, ch: char) {
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    out.extend_from_slice(s.as_bytes());
}

/// Delta against `fb.prev_cells()` (caller must not have called `commit_frame` yet this frame).
pub fn encode_frame_delta(fb: &FrameBuffer) -> (Vec<u8>, u32) {
    let mut out = Vec::new();
    let mut dirty: u32 = 0;
    for y in 0..fb.height {
        for x in 0..fb.width {
            let i = y as usize * fb.width as usize + x as usize;
            let cur = &fb.cells()[i];
            let prev = &fb.prev_cells()[i];
            if cur == prev {
                continue;
            }
            dirty = dirty.saturating_add(1);
            push_cursor_pos(&mut out, x, y);
            push_cell_attrs(&mut out, cur);
            push_char_utf8(&mut out, cur.ch);
        }
    }
    if dirty > 0 {
        out.extend_from_slice(b"\x1b[0m");
    }
    (out, dirty)
}

/// Display width of a cell character (for layout/debug).
#[allow(dead_code)]
pub fn cell_display_width(ch: char) -> u16 {
    UnicodeWidthChar::width(ch)
        .map(|w| w as u16)
        .unwrap_or(0)
        .max(1)
}
