use crate::content::DialogueNode;
use crate::rect::Rect;
use crate::render::FrameBuffer;

pub fn draw_dialogue(
    fb: &mut FrameBuffer,
    r: Rect,
    node: &DialogueNode,
    choice_cursor: usize,
    mouse_regions: &mut Vec<Rect>,
) {
    super::draw_bordered_panel(fb, r, "Dialogue");
    let inner = Rect::new(r.x + 1, r.y + 1, r.w.saturating_sub(2), r.h.saturating_sub(2));
    let mut lines: Vec<String> = Vec::new();
    lines.push(node.text.to_string());
    lines.push(String::new());
    for (i, c) in node.choices.iter().enumerate() {
        let prefix = if i == choice_cursor { "> " } else { "  " };
        lines.push(format!("{}{}", prefix, c.label));
    }
    super::draw_text_block(fb, inner, &lines);
    let choice_y0 = inner.y + 3;
    for i in 0..node.choices.len() {
        let y = choice_y0 + i as u16;
        mouse_regions.push(Rect::new(inner.x, y, inner.w, 1));
    }
}
