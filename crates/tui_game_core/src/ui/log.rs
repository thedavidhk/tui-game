//! Bottom log strip: semantic coloring (`docs/ui_design.md` §9) and optional dim footer row.

use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, Style};

use super::theme::GameUiPalette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogTone {
    Mundane,
    Success,
    Danger,
    Important,
    Magic,
}

fn classify_log_tone(line: &str) -> LogTone {
    let t = line.trim();
    if t.starts_with("[+]") {
        return LogTone::Success;
    }
    if t.starts_with("[!]") {
        return LogTone::Danger;
    }
    if t.starts_with("[→]") || t.starts_with("[->]") {
        return LogTone::Magic;
    }
    if t.starts_with("[•]") {
        return LogTone::Important;
    }
    let lower = t.to_ascii_lowercase();
    if lower.contains("picked up")
        || lower.contains("equipped")
        || lower.contains("loaded")
        || lower.contains("saved ")
    {
        return LogTone::Success;
    }
    if lower.contains("damage")
        || lower.contains("hostile")
        || lower.contains("failed")
        || lower.contains("error")
    {
        return LogTone::Danger;
    }
    if lower.contains("contact") || lower.contains("warning") {
        return LogTone::Important;
    }
    LogTone::Mundane
}

fn tone_fg(tone: LogTone, palette: &GameUiPalette) -> Color {
    match tone {
        LogTone::Mundane => palette.text_dim,
        LogTone::Success => palette.good,
        LogTone::Danger => palette.warning,
        LogTone::Important => palette.title,
        LogTone::Magic => palette.magic,
    }
}

/// Renders recent log lines with light semantic coloring.
///
/// `inner` is the full content [`Rect`] inside the log panel (including space for a footer). When
/// `footer` is [`Some`], the **last row** of `inner` is reserved for it; log lines use the rows
/// above. Do not pre-shrink `inner` in the caller — that would double-reserve the footer row and
/// hide the newest log line.
pub fn draw_log(
    fb: &mut FrameBuffer,
    inner: Rect,
    lines: &[String],
    footer: Option<&str>,
    palette: &GameUiPalette,
) {
    let footer_rows = u16::from(footer.is_some());
    let body_h = inner.h.saturating_sub(footer_rows);
    let log_bg = palette.panel_bg_soft;

    for (row, line) in lines.iter().enumerate() {
        let y = inner.y + row as u16;
        if y >= inner.y + body_h {
            break;
        }
        let tone = classify_log_tone(line);
        let fg = tone_fg(tone, palette);
        let st = Style {
            bold: matches!(tone, LogTone::Important | LogTone::Danger),
            dim: matches!(tone, LogTone::Mundane),
            underline: false,
        };
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
                    bg: log_bg,
                    style: st,
                },
            );
            x = x.saturating_add(1);
        }
        while x < inner.right() {
            fb.set(
                x,
                y,
                Cell {
                    ch: ' ',
                    fg,
                    bg: log_bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
    }

    if let Some(foot) = footer {
        let y = inner.bottom().saturating_sub(1);
        if y >= inner.y && y < inner.bottom() {
            let footer_bg = palette.panel_bg;
            let fg = palette.text_dim;
            let st = Style {
                dim: true,
                underline: true,
                ..Default::default()
            };
            let mut x = inner.x;
            while x < inner.right() {
                fb.set(
                    x,
                    y,
                    Cell {
                        ch: ' ',
                        fg,
                        bg: footer_bg,
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
            let mut x = inner.x;
            for ch in foot.chars() {
                if x >= inner.right() {
                    break;
                }
                fb.set(
                    x,
                    y,
                    Cell {
                        ch,
                        fg,
                        bg: footer_bg,
                        style: st,
                    },
                );
                x = x.saturating_add(1);
            }
        }
    }
}
