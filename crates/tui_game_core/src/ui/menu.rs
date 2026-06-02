use crate::input::MouseCell;
use crate::render::FrameBuffer;

use super::chrome::{chrome_inner_rect, draw_rounded_panel, PanelBorderEmphasis};
use super::hit::{UiHitState, UiHitTarget};
use super::list::{draw_selectable_list, SelectableList};
use super::theme::GameUiPalette;

pub fn draw_menu(
    fb: &mut FrameBuffer,
    r: crate::rect::Rect,
    title: &str,
    items: &[&str],
    selected: usize,
    palette: &GameUiPalette,
    last_mouse: Option<MouseCell>,
    hits: &mut UiHitState,
) {
    draw_rounded_panel(fb, r, title, PanelBorderEmphasis::Highlighted, palette);
    let rows: Vec<String> = items.iter().map(|s| (*s).to_string()).collect();
    let list = SelectableList {
        inner: chrome_inner_rect(r),
        rows: &rows,
        selected: Some(selected),
        last_mouse,
        empty_text: None,
        reserved_footer_rows: 0,
    };
    draw_selectable_list(fb, palette, &list, hits, UiHitTarget::MainMenuItem);
}
