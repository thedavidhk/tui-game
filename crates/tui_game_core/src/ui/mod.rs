//! Layout-only UI helpers writing into `FrameBuffer`.
//!
//! Screen chrome sizes and positions: `layout` (`GameShellLayout`, `OverlaySplitConfig`,
//! `FloatingPanelLayout`).

mod dialogue;
pub mod hit;
pub mod layout;
mod log;
mod menu;
pub mod mouse;
pub mod palette;
mod panel;
mod text_field;
pub mod wrap;

pub use dialogue::draw_dialogue;
pub use hit::{UiHitState, UiHitTarget};
pub use mouse::{
    cell_in_axis_rect, cell_in_brush, cell_local_in_rect, for_each_in_brush, for_each_in_rect,
    map_view_rect,
};
pub use log::draw_log;
pub use menu::draw_menu;
pub use palette::PRESET_COLORS;
pub use panel::{draw_bordered_panel, draw_text_block};
pub use text_field::{centered_rect, draw_text_field, TextField, TextFieldOutput, TextFilter};
