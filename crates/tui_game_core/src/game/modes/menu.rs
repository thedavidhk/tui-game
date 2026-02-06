use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, MouseButton, MouseEventKind};
use crate::ui::hit::UiHitTarget;

pub(crate) fn handle(game: &mut Game, ev: InputEvent, selected: usize) {
    match ev {
        InputEvent::Mouse {
            kind: MouseEventKind::Down(MouseButton::Left),
            cell,
            ..
        } => {
            if let Some(UiHitTarget::MainMenuItem(i)) = game.ui_hits.pick(cell) {
                if i < game.menu_items.len() {
                    if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                        *s = i;
                    }
                }
            }
        }
        InputEvent::Key(k) => {
            if k.key == Key::Char('q') || k.key == Key::Esc {
                // stay on menu; quit only via selection
            }
            if matches!(k.key, Key::Up | Key::Char('k')) {
                let sel = selected.saturating_sub(1);
                if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                    *s = sel;
                }
            }
            if matches!(k.key, Key::Down | Key::Char('j')) {
                let n = game.menu_items.len();
                let sel = (selected + 1).min(n.saturating_sub(1));
                if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                    *s = sel;
                }
            }
            if matches!(k.key, Key::Enter) {
                match selected {
                    0 => {
                        game.modes.stack = vec![GameMode::Exploration];
                        game.log.push("Entered world.".into());
                        game.refresh_fow();
                    }
                    1 => {
                        game.quit_requested = true;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
