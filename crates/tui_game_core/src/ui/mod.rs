//! Layout-only UI helpers writing into `FrameBuffer`.

mod dialogue;
mod log;
mod menu;
mod panel;

pub use dialogue::draw_dialogue;
pub use log::draw_log;
pub use menu::draw_menu;
pub use panel::{draw_bordered_panel, draw_text_block};
