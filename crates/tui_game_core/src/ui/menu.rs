use crate::input::MouseCell;
use crate::rect::Rect;
use crate::render::{FrameBuffer, Style};

use super::chrome::{chrome_inner_rect, draw_clipped_line, draw_rounded_panel, PanelBorderEmphasis};
use super::hit::{UiHitState, UiHitTarget};
use super::theme::GameUiPalette;

pub fn draw_menu(
    fb: &mut FrameBuffer,
    r: Rect,
    title: &str,
    items: &[&str],
    selected: usize,
    palette: &GameUiPalette,
    last_mouse: Option<MouseCell>,
    hits: &mut UiHitState,
) {
    draw_rounded_panel(
        fb,
        r,
        title,
        PanelBorderEmphasis::Highlighted,
        palette,
    );
    let inner = chrome_inner_rect(r);
    for (i, item) in items.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let row = Rect::new(inner.x, y, inner.w, 1);
        let mouse_hot = last_mouse.is_some_and(|m| row.contains(m.x, m.y));
        let sel = i == selected || mouse_hot;
        let fg = if sel {
            palette.selected_fg
        } else {
            palette.text
        };
        let bg = if sel {
            palette.selected_bg
        } else {
            palette.panel_bg
        };
        let prefix = if sel { "› " } else { "  " };
        let line = format!("{prefix}{item}");
        let st = Style {
            bold: sel,
            dim: false,
            underline: false,
        };
        draw_clipped_line(fb, inner.x, y, inner.w, &line, fg, bg, st);
        hits.push(UiHitTarget::MainMenuItem(i), row);
    }
}
