//! Single-line rounded panels, modal scrims, and other **game chrome** drawing helpers.
//!
//! Level editor dialogs keep using [`super::panel::draw_bordered_panel`]; the shipped game uses
//! this module for lighter borders and consistent emphasis (dim vs active) per `docs/ui_design.md`.

use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

use super::theme::GameUiPalette;

/// Whether a panel border reads as ambient chrome or as the focused surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelBorderEmphasis {
    /// Idle / secondary panels: dim border.
    Subtle,
    /// Primary context (dialogue, focused transfer column): brighter border.
    Highlighted,
}

/// Content area inside [`draw_rounded_panel`] (one cell inset on all sides).
#[must_use]
pub fn chrome_inner_rect(panel: Rect) -> Rect {
    Rect::new(
        panel.x + 1,
        panel.y + 1,
        panel.w.saturating_sub(2),
        panel.h.saturating_sub(2),
    )
}

/// Fills `world` with a dim wash so modal overlays read as a separate layer (`docs/ui_design.md` §10).
pub fn draw_modal_world_scrim(fb: &mut FrameBuffer, world: Rect, palette: &GameUiPalette) {
    if world.w == 0 || world.h == 0 {
        return;
    }
    let style = Style {
        bold: false,
        dim: true,
        underline: false,
    };
    for y in world.y..world.bottom() {
        for x in world.x..world.right() {
            fb.set(
                x,
                y,
                Cell {
                    ch: ' ',
                    fg: palette.text_dim,
                    bg: palette.modal_scrim_bg,
                    style,
                },
            );
        }
    }
}

/// Rounded single-line frame with embedded title on the top edge (`╭─ Title ───╮` style).
pub fn draw_rounded_panel(
    fb: &mut FrameBuffer,
    r: Rect,
    title: &str,
    emphasis: PanelBorderEmphasis,
    palette: &GameUiPalette,
) {
    if r.w < 2 || r.h < 2 {
        return;
    }
    let border = match emphasis {
        PanelBorderEmphasis::Subtle => palette.border_dim,
        PanelBorderEmphasis::Highlighted => palette.border_active,
    };

    // Base fill
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            fb.set(
                x,
                y,
                Cell {
                    ch: ' ',
                    fg: palette.text,
                    bg: palette.panel_bg,
                    style: Style::default(),
                },
            );
        }
    }

    let inner_w = r.w.saturating_sub(2) as usize;
    let top_inner = top_border_inner_title(title, inner_w);

    // Top border row (corners + title run)
    fb.set(
        r.x,
        r.y,
        Cell {
            ch: '╭',
            fg: border,
            bg: palette.panel_bg,
            style: Style::default(),
        },
    );
    fb.set(
        r.right().saturating_sub(1),
        r.y,
        Cell {
            ch: '╮',
            fg: border,
            bg: palette.panel_bg,
            style: Style::default(),
        },
    );
    let mut x = r.x + 1;
    for (i, ch) in top_inner.chars().enumerate() {
        if i >= inner_w {
            break;
        }
        let fg = if ch == '─' {
            border
        } else {
            palette.title
        };
        let st = Style {
            bold: ch != '─',
            dim: false,
            underline: false,
        };
        fb.set(
            x,
            r.y,
            Cell {
                ch,
                fg,
                bg: palette.panel_bg,
                style: st,
            },
        );
        x = x.saturating_add(1);
    }

    // Vertical sides
    if r.h > 2 {
        for y in r.y + 1..r.bottom().saturating_sub(1) {
            fb.set(
                r.x,
                y,
                Cell {
                    ch: '│',
                    fg: border,
                    bg: palette.panel_bg,
                    style: Style::default(),
                },
            );
            fb.set(
                r.right().saturating_sub(1),
                y,
                Cell {
                    ch: '│',
                    fg: border,
                    bg: palette.panel_bg,
                    style: Style::default(),
                },
            );
        }
    }

    // Bottom border
    fb.set(
        r.x,
        r.bottom().saturating_sub(1),
        Cell {
            ch: '╰',
            fg: border,
            bg: palette.panel_bg,
            style: Style::default(),
        },
    );
    fb.set(
        r.right().saturating_sub(1),
        r.bottom().saturating_sub(1),
        Cell {
            ch: '╯',
            fg: border,
            bg: palette.panel_bg,
            style: Style::default(),
        },
    );
    for col in 1..r.w.saturating_sub(1) {
        let x = r.x + col;
        fb.set(
            x,
            r.bottom().saturating_sub(1),
            Cell {
                ch: '─',
                fg: border,
                bg: palette.panel_bg,
                style: Style::default(),
            },
        );
    }
}

/// Top edge between corners: `─ Title ───` filling exactly `inner_w` **terminal columns**.
///
/// `inner_w` counts Unicode scalar values (one column each in this TUI). Using [`str::len`] or
/// [`String::truncate`] with `inner_w` is wrong: UTF-8 byte length can split multibyte sequences.
fn top_border_inner_title(title: &str, inner_w: usize) -> String {
    if inner_w == 0 {
        return String::new();
    }
    if title.is_empty() {
        return "─".repeat(inner_w);
    }
    if inner_w == 1 {
        return "─".into();
    }

    let max_title_chars = inner_w.saturating_sub(2);
    let title_part: String = title.chars().take(max_title_chars).collect();

    let mut out = String::new();
    out.push('─');
    out.push(' ');
    out.push_str(&title_part);

    let pad = inner_w.saturating_sub(out.chars().count());
    out.extend(std::iter::repeat_n('─', pad));

    if out.chars().count() > inner_w {
        return out.chars().take(inner_w).collect();
    }
    out
}

/// Draws one text line left-aligned inside `[x0, x0+width)` with clipping; pads with spaces.
pub fn draw_clipped_line(
    fb: &mut FrameBuffer,
    x0: u16,
    y: u16,
    width: u16,
    text: &str,
    fg: Color,
    bg: Color,
    style: Style,
) {
    if width == 0 {
        return;
    }
    let mut x = x0;
    for ch in text.chars() {
        if x >= x0.saturating_add(width) {
            break;
        }
        fb.set(
            x,
            y,
            Cell {
                ch,
                fg,
                bg,
                style,
            },
        );
        x = x.saturating_add(1);
    }
    while x < x0.saturating_add(width) {
        fb.set(
            x,
            y,
            Cell {
                ch: ' ',
                fg,
                bg,
                style: Style::default(),
            },
        );
        x = x.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::top_border_inner_title;

    #[test]
    fn top_border_title_never_panics_on_multibyte_utf8() {
        // Regression: old code used byte len + truncate(inner_w) and could split UTF-8.
        let t = "Загружено"; // Cyrillic, 2+ bytes per char
        for w in 1..=32 {
            let s = top_border_inner_title(t, w);
            assert_eq!(s.chars().count(), w, "w={w} s={s:?}");
        }
    }

    #[test]
    fn top_border_title_fills_ascii() {
        let s = top_border_inner_title("Log", 12);
        assert_eq!(s.chars().count(), 12);
        assert!(s.starts_with('─'));
    }
}
