use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, KeyChord};

pub(crate) fn handle(game: &mut Game, ev: InputEvent) {
    let InputEvent::Key(KeyChord { key, .. }) = ev else {
        return;
    };
    if matches!(key, Key::Enter | Key::Char(' ')) {
        game.modes.stack = vec![GameMode::MainMenu { selected: 0 }];
        game.log.push("Main menu.".into());
    }
}
