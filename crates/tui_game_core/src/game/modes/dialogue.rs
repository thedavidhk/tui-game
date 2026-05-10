use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::ui::hit::UiHitTarget;

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    let (dialogue_id, node_index) = match game.modes.current() {
        Some(GameMode::Dialogue {
            dialogue_id,
            node_index,
            ..
        }) => (dialogue_id.clone(), *node_index),
        _ => return,
    };
    let tree = game
        .content
        .dialogues
        .get(dialogue_id.as_str())
        .copied()
        .unwrap_or(game.content.default_dialogue);
    let Some(node) = tree.nodes.get(node_index) else {
        let _ = game.modes.pop();
        return;
    };
    let exit_sentinel = tree.nodes.len();

    if node.choices.is_empty() {
        if node.auto_next.is_some() {
            match ev {
                GameInput::Command(GameCommand::ToggleDebug) => {
                    game.debug_overlay = !game.debug_overlay;
                }
                GameInput::Raw(InputEvent::Mouse {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    cell,
                    ..
                }) => {
                    if matches!(game.ui_hits.pick(cell), Some(UiHitTarget::DialogueContinue)) {
                        game.apply_dialogue_continue(tree, exit_sentinel);
                    }
                }
                GameInput::Command(GameCommand::Back) => {
                    let _ = game.modes.pop();
                }
                GameInput::Command(GameCommand::Confirm) => {
                    game.apply_dialogue_continue(tree, exit_sentinel);
                }
                GameInput::Command(_) | GameInput::Raw(_) => {}
            }
            return;
        }
        let _ = game.modes.pop();
        game.log.push("No available dialogue choices.".into());
        return;
    }

    let visible = game.dialogue_visible_choice_indices(node);
    if visible.is_empty() {
        let _ = game.modes.pop();
        game.log.push("No available dialogue choices.".into());
        return;
    }

    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Moved,
            cell,
            ..
        }) => {
            if let Some(UiHitTarget::DialogueChoice(i)) = game.ui_hits.pick(cell) {
                if let Some(GameMode::Dialogue { choice_cursor: c, .. }) = game.modes.current_mut()
                {
                    let max = visible.len().saturating_sub(1);
                    *c = i.min(max);
                }
            }
        }
        GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Down(MouseButton::Left),
            cell,
            ..
        }) => {
            if let Some(UiHitTarget::DialogueChoice(i)) = game.ui_hits.pick(cell) {
                if let Some(GameMode::Dialogue { choice_cursor: c, .. }) = game.modes.current_mut() {
                    let max = visible.len().saturating_sub(1);
                    *c = i.min(max);
                }
                game.apply_dialogue_choice(tree, exit_sentinel);
            }
        }
        GameInput::Command(GameCommand::Back) => {
            let _ = game.modes.pop();
        }
        GameInput::Command(GameCommand::ListPrev) => {
            if let Some(GameMode::Dialogue { choice_cursor: c, .. }) = game.modes.current_mut() {
                *c = c.saturating_sub(1);
            }
        }
        GameInput::Command(GameCommand::ListNext) => {
            if let Some(GameMode::Dialogue { choice_cursor: c, .. }) = game.modes.current_mut() {
                let max = visible.len().saturating_sub(1);
                *c = (*c + 1).min(max);
            }
        }
        GameInput::Command(GameCommand::Confirm) => {
            game.apply_dialogue_choice(tree, exit_sentinel);
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
