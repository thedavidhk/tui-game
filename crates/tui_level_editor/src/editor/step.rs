//! Per-frame input dispatch.

use tui_game_core::input::{InputBatch, InputEvent, MouseCell};
use tui_game_core::ui::UiHitTarget;

use super::Editor;

impl Editor {
    pub fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            match ev {
                InputEvent::Key(chord) => {
                    if self.handle_dialog(chord) {
                        continue;
                    }
                    self.step_main_key(chord);
                }
                InputEvent::Mouse { .. } => {
                    self.update_map_hover_from_mouse(ev);
                    if !self.handle_dialog_mouse(ev) {
                        self.step_main_mouse(ev);
                    }
                }
                InputEvent::Resize { .. } => {}
            }
        }
    }

    pub fn ui_hit_at(&self, cell: MouseCell) -> Option<UiHitTarget> {
        self.ui_hits.pick(cell)
    }
}
