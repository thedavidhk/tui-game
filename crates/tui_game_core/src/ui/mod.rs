//! UI helpers writing into [`crate::render::FrameBuffer`].
//!
//! - **Layout** — `layout` (`GameShellLayout`, [`layout::OverlayBandLayout`], `FloatingPanelLayout`).
//! - **Game chrome** — `theme` ([`GameUiPalette`]), `chrome` (rounded panels, modal scrim), and
//!   widgets (`menu`, `log`, `dialogue`) used by [`crate::game::Game::compose`].
//! - **Hit testing** — `hit` ([`UiHitTarget`]) for mouse picking in the shell binary / modes.
//! - **Editor** — the level editor still uses [`panel::draw_bordered_panel`] for tool dialogs.

pub mod chrome;
mod dialogue;
pub mod hit;
pub mod layout;
mod log;
mod menu;
pub mod theme;
pub mod mouse;
pub mod palette;
pub mod viewport_scroll;
mod panel;
mod text_field;
mod search_list_picker;
pub mod wrap;

pub use chrome::{
    chrome_inner_rect, draw_clipped_line, draw_modal_world_scrim, draw_rounded_panel,
    PanelBorderEmphasis,
};
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
pub use panel::{draw_bordered_panel, draw_text_block, draw_text_block_theme};
pub use theme::GameUiPalette;
pub use text_field::{
    centered_rect, centered_rect_dims, draw_text_field, TextField, TextFieldOutput, TextFilter,
};
pub use search_list_picker::{SearchListPicker, SearchListPickerHit, SearchListPickerOutput};
