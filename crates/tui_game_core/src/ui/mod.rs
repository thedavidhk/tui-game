//! UI helpers writing into [`crate::render::FrameBuffer`].
//!
//! - **Layout** — `layout` (`GameShellLayout`, [`layout::OverlayBandLayout`], `FloatingPanelLayout`).
//! - **Game chrome** — `theme` ([`GameUiPalette`]), `chrome` (rounded panels, modal scrim), and
//!   widgets (`menu`, `log`, `dialogue`) used by [`crate::game::Game::compose`].
//! - **Hit testing** — `hit` ([`UiHitTarget`]) for mouse picking in the shell binary, modes, and
//!   the level editor ([`hit::EditorHitTarget`]).
//! - Both the game shell and the level editor share the same chrome (`chrome`), palette
//!   (`theme`), list widget (`list`), and hit registry (`hit`).

pub mod chrome;
mod dialogue;
pub mod hit;
pub mod layout;
mod list;
mod log;
mod menu;
pub mod mouse;
mod panel;
mod search_list_picker;
pub mod swatches;
mod text_field;
pub mod theme;
pub mod viewport_scroll;
pub mod wrap;

pub use chrome::{
    chrome_inner_rect, draw_clipped_line, draw_modal_world_scrim, draw_rounded_panel,
    PanelBorderEmphasis,
};
pub use dialogue::draw_dialogue;
pub use hit::{EditorHitTarget, UiHitState, UiHitTarget};
pub use layout::{centered_rect, centered_rect_dims};
pub use list::{draw_selectable_list, SelectableList};
pub use log::draw_log;
pub use menu::draw_menu;
pub use mouse::{
    cell_in_axis_rect, cell_in_brush, cell_local_in_rect, for_each_in_brush, for_each_in_rect,
    map_view_rect,
};
pub use panel::draw_text_block_theme;
pub use search_list_picker::{SearchListPicker, SearchListPickerOutput};
pub use swatches::PRESET_COLORS;
pub use text_field::{draw_text_field, TextField, TextFieldOutput, TextFilter};
pub use theme::GameUiPalette;
pub use viewport_scroll::{
    clamp_origin, edge_scroll_pan_delta, map_larger_than_view, screen_cell_to_world,
    world_view_origin, EDGE_MARGIN_CELLS, EDGE_SCROLL_COOLDOWN_TICKS,
};
