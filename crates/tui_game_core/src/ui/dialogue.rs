//! Dialogue band rendering: rounded panel, wrapped body, `›` choice markers, dimmed unavailable
//! choices, and mouse hit targets (`UiHitTarget::DialogueChoice` / `DialogueContinue`).

use std::collections::HashSet;

use crate::content::DialogueNode;
use crate::input::MouseCell;
use crate::rect::Rect;
use crate::render::{FrameBuffer, Style};

use super::chrome::{
    chrome_inner_rect, draw_clipped_line, draw_rounded_panel, PanelBorderEmphasis,
};
use super::hit::{UiHitState, UiHitTarget};
use super::theme::GameUiPalette;
use super::wrap::wrap_words;

pub fn draw_dialogue(
    fb: &mut FrameBuffer,
    panel: Rect,
    palette: &GameUiPalette,
    speaker: &str,
    node: &DialogueNode,
    body: &str,
    visible_choice_indices: &[usize],
    choice_cursor: usize,
    continue_only: bool,
    last_mouse: Option<MouseCell>,
    hits: &mut UiHitState,
) {
    draw_rounded_panel(
        fb,
        panel,
        speaker,
        PanelBorderEmphasis::Highlighted,
        palette,
    );
    let inner = chrome_inner_rect(panel);
    if inner.h < 1 || inner.w < 1 {
        return;
    }
    let line_w = inner.w as usize;
    let mut y = inner.y;

    if continue_only {
        let wrapped = wrap_words(body, line_w.max(8));
        for line in &wrapped {
            if y >= inner.bottom() {
                break;
            }
            draw_clipped_line(
                fb,
                inner.x,
                y,
                inner.w,
                line,
                palette.text,
                palette.panel_bg,
                Style::default(),
            );
            y = y.saturating_add(1);
        }
        y = y.saturating_add(1);
        let hint = "  (Enter, Space, or click to continue)";
        if y < inner.bottom() {
            draw_clipped_line(
                fb,
                inner.x,
                y,
                inner.w,
                hint,
                palette.text_dim,
                palette.panel_bg,
                Style {
                    dim: true,
                    ..Default::default()
                },
            );
            hits.push(
                UiHitTarget::DialogueContinue,
                Rect::new(inner.x, y, inner.w, 1),
            );
        }
        return;
    }

    let visible_set: HashSet<usize> = visible_choice_indices.iter().copied().collect();
    let wrapped = wrap_words(body, line_w.max(8));
    for line in &wrapped {
        if y >= inner.bottom() {
            break;
        }
        draw_clipped_line(
            fb,
            inner.x,
            y,
            inner.w,
            line,
            palette.text,
            palette.panel_bg,
            Style::default(),
        );
        y = y.saturating_add(1);
    }
    if y < inner.bottom() {
        y = y.saturating_add(1);
    }

    let max_vis = visible_choice_indices.len().saturating_sub(1);
    let cur = choice_cursor.min(max_vis);

    for (i, choice) in node.choices.iter().enumerate() {
        if y >= inner.bottom() {
            break;
        }
        if !visible_set.contains(&i) {
            continue;
        }
        let vis_pos = visible_choice_indices.iter().position(|&g| g == i);
        let row_rect = Rect::new(inner.x, y, inner.w, 1);
        let mouse_hot = last_mouse.is_some_and(|m| row_rect.contains(m.x, m.y));
        let selected = vis_pos == Some(cur);
        let highlight = selected || mouse_hot;

        let prefix = if highlight { "› " } else { "  " };
        let label = format!("{prefix}{}", choice.label);
        let (fg, bg, st) = if highlight {
            (
                palette.selected_fg,
                palette.selected_bg,
                Style {
                    bold: true,
                    ..Default::default()
                },
            )
        } else {
            (palette.text, palette.panel_bg, Style::default())
        };

        draw_clipped_line(fb, inner.x, y, inner.w, &label, fg, bg, st);

        if let Some(vp) = vis_pos {
            hits.push(UiHitTarget::DialogueChoice(vp), row_rect);
        }
        y = y.saturating_add(1);
    }
}
