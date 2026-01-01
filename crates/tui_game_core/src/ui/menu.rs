use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

pub fn draw_menu(
    fb: &mut FrameBuffer,
    r: Rect,
    title: &str,
    items: &[&str],
    selected: usize,
    mouse_regions: &mut Vec<Rect>,
) {
    super::draw_bordered_panel(fb, r, title);
    let inner = Rect::new(r.x + 1, r.y + 1, r.w.saturating_sub(2), r.h.saturating_sub(2));
    for (i, item) in items.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let sel = i == selected;
        let fg = if sel {
            Color::rgb(40, 35, 20)
        } else {
            Color::rgb(210, 200, 180)
        };
        let bg = if sel {
            Color::rgb(220, 190, 120)
        } else {
            Color::rgb(20, 18, 24)
        };
        let mut x = inner.x + 1;
        let prefix = if sel { "> " } else { "  " };
        for ch in prefix.chars().chain(item.chars()) {
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
                    style: Style {
                        bold: sel,
                        dim: false,
                        underline: false,
                    },
                },
            );
            x = x.saturating_add(1);
        }
        mouse_regions.push(Rect::new(inner.x, y, inner.w, 1));
    }
}
