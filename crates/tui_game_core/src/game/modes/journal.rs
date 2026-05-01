use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, KeyChord};

pub(crate) fn handle(game: &mut Game, ev: InputEvent) {
    let n = game.narrative.quest_journal.len();
    match ev {
        InputEvent::Key(KeyChord { key: Key::Char('q'), .. }) => {
            let _ = game.modes.pop();
        }
        InputEvent::Key(KeyChord {
            key: Key::Up,
            ..
        }) => {
            if let Some(GameMode::Journal { quest_cursor }) = game.modes.current_mut() {
                *quest_cursor = quest_cursor.saturating_sub(1);
            }
        }
        InputEvent::Key(KeyChord {
            key: Key::Down,
            ..
        }) => {
            if let Some(GameMode::Journal { quest_cursor }) = game.modes.current_mut() {
                let max = n.saturating_sub(1);
                *quest_cursor = (*quest_cursor + 1).min(max);
            }
        }
        _ => {}
    }
}
