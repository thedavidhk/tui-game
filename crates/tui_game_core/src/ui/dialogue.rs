//! Dialogue band rendering: rounded panel, wrapped body, `›` choice markers, dimmed unavailable
//! choices, and mouse hit targets (`UiHitTarget::DialogueChoice` / `DialogueContinue`).

use crate::content::DialogueNode;
use crate::input::MouseCell;
use crate::rect::Rect;
use crate::render::{FrameBuffer, Style};

use super::chrome::{
    chrome_inner_rect, draw_clipped_line, draw_rounded_panel, PanelBorderEmphasis,
};
use super::hit::{UiHitState, UiHitTarget};
use super::list::{draw_selectable_list, SelectableList};
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

    // Choices are a selectable list in the area below the wrapped body. Rows follow
    // `visible_choice_indices` order, so a row index is the visible position the dialogue
    // mode expects in `UiHitTarget::DialogueChoice` / `choice_cursor`.
    let rows: Vec<String> = visible_choice_indices
        .iter()
        .filter_map(|&gi| node.choices.get(gi).map(|c| c.label.to_string()))
        .collect();
    let choices_rect = Rect::new(inner.x, y, inner.w, inner.bottom().saturating_sub(y));
    let list = SelectableList {
        inner: choices_rect,
        rows: &rows,
        selected: Some(choice_cursor),
        last_mouse,
        empty_text: None,
        reserved_footer_rows: 0,
    };
    draw_selectable_list(fb, palette, &list, hits, UiHitTarget::DialogueChoice);
}
