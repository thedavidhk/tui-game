use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

use super::theme::GameUiPalette;

pub fn draw_bordered_panel(fb: &mut FrameBuffer, r: Rect, title: &str) {
    let fg = Color::rgb(180, 170, 140);
    let bg = Color::rgb(25, 22, 30);
    if r.w == 0 || r.h == 0 {
        return;
    }
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let top = y == r.y;
            let bot = y == r.bottom().saturating_sub(1);
            let left = x == r.x;
            let right = x == r.right().saturating_sub(1);
            let ch = match (top, bot, left, right) {
                (true, _, true, _) => '╔',
                (true, _, _, true) => '╗',
                (_, true, true, _) => '╚',
                (_, true, _, true) => '╝',
                (true, _, _, _) | (_, true, _, _) => '═',
                (_, _, true, _) | (_, _, _, true) => '║',
                _ => ' ',
            };
            fb.set(
                x,
                y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
        }
    }
    // Title on top inner
    let mut cx = r.x + 2;
    let title_y = r.y;
    for ch in title.chars() {
        if cx >= r.right().saturating_sub(1) {
            break;
        }
        fb.set(
            cx,
            title_y,
            Cell {
                ch,
                fg: Color::rgb(255, 230, 160),
                bg,
                style: Style {
                    bold: true,
                    dim: false,
                    underline: false,
                },
            },
        );
        cx = cx.saturating_add(1);
    }
}

pub fn draw_text_block(fb: &mut FrameBuffer, inner: Rect, lines: &[String]) {
    let fg = Color::rgb(210, 205, 195);
    let bg = Color::rgb(18, 16, 22);
    for (row, line) in lines.iter().enumerate() {
        let y = inner.y + row as u16;
        if y >= inner.bottom() {
            break;
        }
        let mut x = inner.x;
        for ch in line.chars() {
            if x >= inner.right() {
                break;
            }
            fb.set(
                x,
                y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
    }
}

/// Same layout as [`draw_text_block`], using [`GameUiPalette`] game-chrome colors.
pub fn draw_text_block_theme(fb: &mut FrameBuffer, inner: Rect, lines: &[String], palette: &GameUiPalette) {
    for (row, line) in lines.iter().enumerate() {
        let y = inner.y + row as u16;
        if y >= inner.bottom() {
            break;
        }
        let mut x = inner.x;
        for ch in line.chars() {
            if x >= inner.right() {
                break;
            }
            fb.set(
                x,
                y,
                Cell {
                    ch,
                    fg: palette.text,
                    bg: palette.panel_bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
    }
}
