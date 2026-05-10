use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::ui::hit::UiHitTarget;

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    let n = game.narrative.quest_journal.len();
    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Command(GameCommand::Back) => {
            let _ = game.modes.pop();
        }
        GameInput::Command(GameCommand::ListPrev) => {
            if let Some(GameMode::Journal { quest_cursor }) = game.modes.current_mut() {
                *quest_cursor = quest_cursor.saturating_sub(1);
            }
        }
        GameInput::Command(GameCommand::ListNext) => {
            if let Some(GameMode::Journal { quest_cursor }) = game.modes.current_mut() {
                let max = n.saturating_sub(1);
                *quest_cursor = (*quest_cursor + 1).min(max);
            }
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
            if let Some(UiHitTarget::JournalQuest(i)) = game.ui_hits.pick(cell) {
                if i < n {
                    if let Some(GameMode::Journal { quest_cursor }) = game.modes.current_mut() {
                        *quest_cursor = i;
                    }
                }
            }
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
