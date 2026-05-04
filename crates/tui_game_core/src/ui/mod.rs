//! Layout-only UI helpers writing into `FrameBuffer`.
//!
//! Screen chrome sizes and positions: `layout` (`GameShellLayout`, `split_horizontal_columns`,
//! `FloatingPanelLayout`).

mod dialogue;
pub mod hit;
pub mod layout;
mod log;
mod menu;
pub mod mouse;
pub mod palette;
pub mod viewport_scroll;
mod panel;
mod text_field;
pub mod wrap;

pub use dialogue::draw_dialogue;
pub use hit::{UiHitState, UiHitTarget};
pub use mouse::{
    cell_in_axis_rect, cell_in_brush, cell_local_in_rect, for_each_in_brush, for_each_in_rect,
    map_view_rect,
};
pub use viewport_scroll::{
    clamp_origin, edge_scroll_pan_delta, map_larger_than_view, screen_cell_to_world,
    world_view_origin, EDGE_MARGIN_CELLS, EDGE_SCROLL_COOLDOWN_TICKS,
};
pub use log::draw_log;
pub use menu::draw_menu;
pub use palette::PRESET_COLORS;
pub use panel::{draw_bordered_panel, draw_text_block};
pub use text_field::{centered_rect, draw_text_field, TextField, TextFieldOutput, TextFilter};
