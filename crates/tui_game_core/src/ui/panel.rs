use crate::rect::Rect;
use crate::render::{Cell, FrameBuffer, Style};

use super::theme::GameUiPalette;

/// Draw left-aligned `lines` into `inner` using [`GameUiPalette`] chrome colors (clipped to the
/// rect; no padding of short lines). Used for static panel bodies (HUD, dialogs, hints).
pub fn draw_text_block_theme(
    fb: &mut FrameBuffer,
    inner: Rect,
    lines: &[String],
    palette: &GameUiPalette,
) {
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
