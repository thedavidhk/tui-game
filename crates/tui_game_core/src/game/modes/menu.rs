use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::ui::hit::UiHitTarget;

pub(crate) fn handle(game: &mut Game, ev: GameInput, selected: usize) {
    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Moved,
            cell,
            ..
        })
        | GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Down(MouseButton::Left),
            cell,
            ..
        }) => {
            if let Some(UiHitTarget::MainMenuItem(i)) = game.ui_hits.pick(cell) {
                if i < game.menu_items.len() {
                    if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                        *s = i;
                    }
                }
            }
        }
        GameInput::Command(GameCommand::ListPrev) => {
            let sel = selected.saturating_sub(1);
            if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                *s = sel;
            }
        }
        GameInput::Command(GameCommand::ListNext) => {
            let n = game.menu_items.len();
            let sel = (selected + 1).min(n.saturating_sub(1));
            if let Some(GameMode::MainMenu { selected: s }) = game.modes.current_mut() {
                *s = sel;
            }
        }
        GameInput::Command(GameCommand::Back) => {
            game.quit_requested = true;
        }
        GameInput::Command(GameCommand::Confirm) => {
            match selected {
                0 => {
                    let needs_full_reset = game.player_id().is_none_or(|pid| {
                        !game.entities.is_alive(pid) || game.entities.pos(pid).is_none()
                    });
                    if needs_full_reset {
                        match game.restart_new_game() {
                            Ok(()) => {}
                            Err(e) => game.log.push(format!("Could not start new game: {e}")),
                        }
                    } else {
                        game.modes.stack = vec![GameMode::Exploration];
                        game.log.push("Entered world.".into());
                        game.refresh_fow();
                    }
                }
                1 => {
                    game.quit_requested = true;
                }
                _ => {}
            }
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
