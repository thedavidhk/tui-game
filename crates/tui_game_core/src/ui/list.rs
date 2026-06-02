//! Shared single-select, scrollable list of text rows.
//!
//! Rendered inside a panel's inner rect with hover highlighting and one mouse hit target
//! per visible row. This is the one home for the "column of selectable rows" idiom used by
//! the main menu, dialogue choices, and the inventory / journal / transfer overlays, so the
//! prefix, selection styling, scrolling, and hit registration are not reinvented per screen.

use crate::input::MouseCell;
use crate::rect::Rect;
use crate::render::{FrameBuffer, Style};

use super::chrome::draw_clipped_line;
use super::hit::{UiHitState, UiHitTarget};
use super::theme::GameUiPalette;

/// Inputs for [`draw_selectable_list`].
pub struct SelectableList<'a> {
    /// Panel content area (typically from [`super::chrome::chrome_inner_rect`]).
    pub inner: Rect,
    /// One display string per row, already formatted; the `›`/blank prefix is added here.
    pub rows: &'a [String],
    /// Keyboard-selected row index (clamped into range), or `None` for hover-only highlight
    /// (e.g. an unfocused column).
    pub selected: Option<usize>,
    /// Pointer cell for hover highlighting, if any.
    pub last_mouse: Option<MouseCell>,
    /// Shown dimmed at the top of `inner` when `rows` is empty.
    pub empty_text: Option<&'a str>,
    /// Rows at the bottom of `inner` the caller draws itself (hints, separators). The list
    /// never draws into them and scrolls within the remaining height.
    pub reserved_footer_rows: u16,
}

/// Draw `list` and register a hit target per visible row via `target_for(row_index)`.
///
/// Scrolls so the selected row stays visible. Stateless (recomputed each frame) since the
/// caller owns `selected`.
pub fn draw_selectable_list(
    fb: &mut FrameBuffer,
    palette: &GameUiPalette,
    list: &SelectableList<'_>,
    hits: &mut UiHitState,
    target_for: impl Fn(usize) -> UiHitTarget,
) {
    let inner = list.inner;
    let visible_rows = inner.h.saturating_sub(list.reserved_footer_rows);
    if visible_rows == 0 || inner.w == 0 {
        return;
    }

    if list.rows.is_empty() {
        if let Some(text) = list.empty_text {
            draw_clipped_line(
                fb,
                inner.x,
                inner.y,
                inner.w,
                text,
                palette.text_dim,
                palette.panel_bg,
                Style {
                    dim: true,
                    ..Default::default()
                },
            );
        }
        return;
    }

    let cap = usize::from(visible_rows);
    let selected = list.selected.map(|s| s.min(list.rows.len() - 1));
    let first = scroll_first(list.rows.len(), cap, selected.unwrap_or(0));

    for screen_row in 0..visible_rows {
        let idx = first + usize::from(screen_row);
        if idx >= list.rows.len() {
            break;
        }
        let y = inner.y.saturating_add(screen_row);
        let row = Rect::new(inner.x, y, inner.w, 1);
        let hot = list.last_mouse.is_some_and(|m| row.contains(m.x, m.y));
        let sel = selected == Some(idx) || hot;
        let prefix = if sel { "› " } else { "  " };
        let line = format!("{prefix}{}", list.rows[idx]);
        let (fg, bg) = if sel {
            (palette.selected_fg, palette.selected_bg)
        } else {
            (palette.text, palette.panel_bg)
        };
        let style = Style {
            bold: sel,
            dim: false,
            underline: false,
        };
        draw_clipped_line(fb, inner.x, y, inner.w, &line, fg, bg, style);
        hits.push(target_for(idx), row);
    }
}

/// First visible row index so `selected` stays on screen within `cap` rows.
///
/// Anchors the window so the selected row sits at or above the last visible line, clamped to
/// the end of the list. When everything fits (`len <= cap`) the window starts at `0`.
fn scroll_first(len: usize, cap: usize, selected: usize) -> usize {
    if cap == 0 || len <= cap {
        return 0;
    }
    let selected = selected.min(len - 1);
    let max_first = len - cap;
    selected.saturating_sub(cap - 1).min(max_first)
}

#[cfg(test)]
mod tests {
    use super::{draw_selectable_list, scroll_first, SelectableList};
    use crate::input::MouseCell;
    use crate::rect::Rect;
    use crate::render::FrameBuffer;
    use crate::ui::hit::{UiHitState, UiHitTarget};
    use crate::ui::theme::GameUiPalette;

    #[test]
    fn scroll_keeps_selection_visible() {
        // 10 rows, window of 5.
        assert_eq!(scroll_first(10, 5, 0), 0);
        assert_eq!(scroll_first(10, 5, 4), 0);
        assert_eq!(scroll_first(10, 5, 5), 1);
        assert_eq!(scroll_first(10, 5, 9), 5);
        // Selected stays within [first, first + cap).
        for sel in 0..10 {
            let first = scroll_first(10, 5, sel);
            assert!(sel >= first && sel < first + 5, "sel={sel} first={first}");
        }
    }

    #[test]
    fn scroll_no_offset_when_everything_fits() {
        assert_eq!(scroll_first(3, 5, 2), 0);
        assert_eq!(scroll_first(5, 5, 4), 0);
        assert_eq!(scroll_first(0, 5, 0), 0);
    }

    #[test]
    fn empty_list_draws_placeholder() {
        let mut fb = FrameBuffer::new(20, 6);
        let mut hits = UiHitState::default();
        let rows: Vec<String> = Vec::new();
        let list = SelectableList {
            inner: Rect::new(0, 0, 20, 6),
            rows: &rows,
            selected: Some(0),
            last_mouse: None,
            empty_text: Some("(empty)"),
            reserved_footer_rows: 0,
        };
        draw_selectable_list(&mut fb, &GameUiPalette::DEFAULT, &list, &mut hits, |_| {
            UiHitTarget::MainMenuItem(0)
        });
        assert_eq!(fb.get(0, 0).unwrap().ch, '(');
    }

    #[test]
    fn selected_row_has_marker_and_registers_hit() {
        let mut fb = FrameBuffer::new(20, 6);
        let mut hits = UiHitState::default();
        let rows: Vec<String> = vec!["alpha".into(), "beta".into(), "gamma".into()];
        let list = SelectableList {
            inner: Rect::new(0, 0, 20, 6),
            rows: &rows,
            selected: Some(1),
            last_mouse: None,
            empty_text: None,
            reserved_footer_rows: 0,
        };
        draw_selectable_list(&mut fb, &GameUiPalette::DEFAULT, &list, &mut hits, |i| {
            UiHitTarget::InventoryStack(i)
        });
        // Row 1 (y=1) is selected: marker glyph in the first column.
        assert_eq!(fb.get(0, 1).unwrap().ch, '›');
        assert_eq!(fb.get(0, 0).unwrap().ch, ' ');
        assert_eq!(
            hits.pick(MouseCell { x: 3, y: 1 }),
            Some(UiHitTarget::InventoryStack(1))
        );
    }

    #[test]
    fn footer_rows_are_left_untouched() {
        let mut fb = FrameBuffer::new(20, 6);
        let mut hits = UiHitState::default();
        // More rows than fit so scrolling would otherwise fill every line.
        let rows: Vec<String> = (0..20).map(|i| format!("row{i}")).collect();
        let list = SelectableList {
            inner: Rect::new(0, 0, 20, 6),
            rows: &rows,
            selected: Some(19),
            last_mouse: None,
            empty_text: None,
            reserved_footer_rows: 2,
        };
        draw_selectable_list(&mut fb, &GameUiPalette::DEFAULT, &list, &mut hits, |i| {
            UiHitTarget::InventoryStack(i)
        });
        // No hit target registered inside the reserved footer (last two rows).
        assert_eq!(hits.pick(MouseCell { x: 1, y: 4 }), None);
        assert_eq!(hits.pick(MouseCell { x: 1, y: 5 }), None);
    }
}
