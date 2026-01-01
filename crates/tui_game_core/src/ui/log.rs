use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

pub fn draw_log(fb: &mut FrameBuffer, inner: Rect, lines: &[String], mouse_regions: &mut Vec<Rect>) {
    let _ = mouse_regions;
    let fg = Color::rgb(190, 200, 175);
    let bg = Color::rgb(14, 18, 14);
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
