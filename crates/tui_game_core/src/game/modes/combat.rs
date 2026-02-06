use crate::combat::{CombatAction, CombatState};
use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, KeyChord};

pub(crate) fn handle(game: &mut Game, ev: InputEvent, state: CombatState) {
    let mut next = state.clone();
    match ev {
        InputEvent::Key(KeyChord {
            key: Key::Tab | Key::Char(' '),
            ..
        }) => {
            let report = next.apply_action(
                CombatAction::Pass,
                &mut game.entities,
                &mut game.rng_seed,
                |_x, _y| false,
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
            );
            game.apply_combat_report(&next, report);
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
        InputEvent::Key(KeyChord {
            key: Key::Char('x'),
            ..
        }) => {
            game.combat_try_attack(&mut next);
        }
        InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
            game.finish_combat(&next, "Combat ended.");
            return;
        }
        _ => {}
    }
    if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
        *cs = next;
    }
}
