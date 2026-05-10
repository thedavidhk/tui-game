//! Per-frame input dispatch.

use tui_game_core::input::{InputBatch, InputEvent, MouseCell};

use super::{Editor, SidebarHit};

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

    pub fn sidebar_pick(&self, cell: MouseCell) -> Option<SidebarHit> {
        self.sidebar_hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(cell.x, cell.y))
            .map(|(h, _)| *h)
    }
}
