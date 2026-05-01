use crate::combat::{CombatAction, CombatState};
use crate::game::{Game, GameMode};
use crate::game::services::hover;
use crate::input::{InputEvent, Key, KeyChord, MouseButton, MouseEventKind};
use crate::ui::layout::GameShellLayout;

pub(crate) fn handle(game: &mut Game, ev: InputEvent, state: CombatState) {
    let mut next = state.clone();
    let world_r = GameShellLayout::root_panels(game.viewport_w, game.viewport_h).0;

    match ev {
        InputEvent::Mouse {
            kind,
            cell,
            ..
        } => {
            hover::sync_combat_hover(game, cell, world_r);
            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    game.combat_try_primary_click(&mut next, cell);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    game.combat_rmb_march_toward(&mut next, cell);
                }
                _ => {}
            }
        }
        InputEvent::Key(KeyChord {
            key: Key::Enter,
            ..
        }) => {
            let report = next.apply_action(
                CombatAction::Pass,
                &mut game.entities,
                &mut game.rng_seed,
                |_x, _y| false,
                None,
                None,
            );
            game.apply_combat_report(&next, report);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('f'),
            ..
        }) => {
            let report = next.apply_action(
                CombatAction::Flee,
                &mut game.entities,
                &mut game.rng_seed,
                |_x, _y| false,
                None,
                None,
            );
            game.apply_combat_report(&next, report);
        }
        InputEvent::Key(KeyChord { key: Key::Up, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, -1);
        }
        InputEvent::Key(KeyChord { key: Key::Down, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, 1);
        }
        InputEvent::Key(KeyChord { key: Key::Left, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(-1, 0);
        }
        InputEvent::Key(KeyChord {
            key: Key::Right,
            ..
        }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(1, 0);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('w'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Up, .. }) => {
            game.combat_try_move(&mut next, 0, -1);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('s'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Down, .. }) => {
            game.combat_try_move(&mut next, 0, 1);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('a'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Left, .. }) => {
            game.combat_try_move(&mut next, -1, 0);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('d'),
            ..
        })
        | InputEvent::Key(KeyChord {
            key: Key::Right,
            ..
        }) => {
            game.combat_try_move(&mut next, 1, 0);
        }
        InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
            game.finish_combat_player_quit(&next);
            return;
        }
        _ => {}
    }
    if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
        *cs = next;
    }
}
