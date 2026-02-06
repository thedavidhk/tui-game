use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, KeyChord};

pub(crate) fn handle(game: &mut Game, ev: InputEvent) {
    match ev {
        InputEvent::Key(KeyChord { key: Key::F(1), .. }) => {
            game.debug_overlay = !game.debug_overlay;
        }
        InputEvent::Key(KeyChord { key: Key::F(5), .. }) => match game.save_to_path("save.ron") {
            Ok(()) => game.log.push("Saved save.ron (F5).".into()),
            Err(e) => game.log.push(format!("Save failed: {e}")),
        },
        InputEvent::Key(KeyChord { key: Key::F(9), .. }) => match game.load_from_path("save.ron") {
            Ok(()) => {}
            Err(e) => game.log.push(format!("Load failed: {e}")),
        },
        InputEvent::Key(KeyChord {
            key: Key::Char('i'),
            ..
        }) => {
            game.modes.push(GameMode::Inventory { cursor: 0 });
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('j'),
            ..
        }) => {
            game.modes.push(GameMode::Journal { quest_cursor: 0 });
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('e'),
            ..
        })
        | InputEvent::Key(KeyChord {
            key: Key::Enter, ..
        }) => game.try_interact(),
        InputEvent::Key(KeyChord {
            key: Key::Char('c'),
            ..
        }) => game.try_start_combat(),
        InputEvent::Key(KeyChord {
            key: Key::Char('w'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Up, .. }) => game.try_move_player(0, -1),
        InputEvent::Key(KeyChord {
            key: Key::Char('s'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Down, .. }) => game.try_move_player(0, 1),
        InputEvent::Key(KeyChord {
            key: Key::Char('a'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Left, .. }) => game.try_move_player(-1, 0),
        InputEvent::Key(KeyChord {
            key: Key::Char('d'),
            ..
        })
        | InputEvent::Key(KeyChord {
            key: Key::Right,
            ..
        }) => game.try_move_player(1, 0),
        _ => {}
    }
}
