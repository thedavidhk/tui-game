use crate::game::{Game, GameCommand, GameInput, GameMode};

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    let GameInput::Command(cmd) = ev else {
        return;
    };
    if matches!(cmd, GameCommand::Confirm | GameCommand::Back) {
        game.modes.stack = vec![GameMode::MainMenu { selected: 0 }];
        game.log.push("Main menu.".into());
    }
}
