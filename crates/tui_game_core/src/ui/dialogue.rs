use crate::rect::Rect;
use crate::render::FrameBuffer;

use super::hit::{UiHitState, UiHitTarget};

pub fn draw_dialogue(
    fb: &mut FrameBuffer,
    r: Rect,
    speaker_name: &str,
    node_text: &str,
    choice_labels: &[&str],
    choice_cursor: usize,
    hits: &mut UiHitState,
) {
    super::draw_bordered_panel(fb, r, "Dialogue");
    let inner = Rect::new(
        r.x + 1,
        r.y + 1,
        r.w.saturating_sub(2),
        r.h.saturating_sub(2),
    );
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("{speaker_name}: {node_text}"));
    lines.push(String::new());
    for (i, c) in choice_labels.iter().enumerate() {
        let prefix = if i == choice_cursor { "> " } else { "  " };
        lines.push(format!("{}{}", prefix, c));
    }
    super::draw_text_block(fb, inner, &lines);
    let choice_y0 = inner.y + 3;
    for i in 0..choice_labels.len() {
        let y = choice_y0 + i as u16;
        hits.push(
            UiHitTarget::DialogueChoice(i),
            Rect::new(inner.x, y, inner.w, 1),
        );
    }
}
