use crate::game::{Game, GameCommand, GameInput, GameMode};

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Command(cmd)
            if matches!(cmd, GameCommand::Confirm | GameCommand::Back) =>
        {
            game.modes.stack = vec![GameMode::MainMenu { selected: 0 }];
            game.log.push("Main menu.".into());
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
