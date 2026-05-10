use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::item::{ItemCategory, StackEquipped};
use crate::ui::hit::UiHitTarget;

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    let n = game.narrative.inventory.stacks.len();
    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Command(GameCommand::Back) => {
            let _ = game.modes.pop();
        }
        GameInput::Command(GameCommand::ListPrev) => {
            if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                *cursor = cursor.saturating_sub(1);
            }
        }
        GameInput::Command(GameCommand::ListNext) => {
            if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                let max = n.saturating_sub(1);
                *cursor = (*cursor + 1).min(max);
            }
        }
        GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Moved,
            cell,
            ..
        }) => {
            if let Some(UiHitTarget::InventoryStack(i)) = game.ui_hits.pick(cell) {
                if i < n {
                    if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                        *cursor = i;
                    }
                }
            }
        }
        GameInput::Raw(InputEvent::Mouse {
            kind: MouseEventKind::Down(MouseButton::Left),
            cell,
            ..
        }) => {
            if let Some(UiHitTarget::InventoryStack(i)) = game.ui_hits.pick(cell) {
                if i < n {
                    if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                        *cursor = i;
                    }
                    inventory_click_activate(game, i);
                }
            }
        }
        GameInput::Command(GameCommand::InventoryUse) => {
            let Some(GameMode::Inventory { cursor }) = game.modes.current().cloned() else {
                return;
            };
            if n == 0 {
                return;
            }
            let idx = cursor.min(n.saturating_sub(1));
            inventory_use_stack(game, idx);
        }
        GameInput::Command(GameCommand::InventoryEquip) => {
            let Some(GameMode::Inventory { cursor }) = game.modes.current().cloned() else {
                return;
            };
            if n == 0 {
                return;
            }
            let idx = cursor.min(n.saturating_sub(1));
            inventory_equip_stack(game, idx);
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}

fn inventory_click_activate(game: &mut Game, idx: usize) {
    let n = game.narrative.inventory.stacks.len();
    if n == 0 {
        return;
    }
    let idx = idx.min(n.saturating_sub(1));
    let Some(stack) = game.narrative.inventory.stacks.get(idx) else {
        return;
    };
    let id_owned = stack.id.clone();
    let catlog = game.content.item_catalog();
    let Some(def) = catlog.get(id_owned.as_str()) else {
        game.log.push(format!(
            "{}: unknown item.",
            catlog.display_name(id_owned.as_str())
        ));
        return;
    };
    match def.category {
        ItemCategory::Consumable => inventory_use_stack(game, idx),
        ItemCategory::Equippable(_) | ItemCategory::Ammo => inventory_equip_stack(game, idx),
        ItemCategory::Mundane => {}
    }
}

fn inventory_use_stack(game: &mut Game, idx: usize) {
    let n = game.narrative.inventory.stacks.len();
    if n == 0 {
        return;
    }
    let idx = idx.min(n.saturating_sub(1));
    let Some(stack) = game.narrative.inventory.stacks.get(idx) else {
        return;
    };
    let id_owned = stack.id.clone();
    let catlog = game.content.item_catalog();
    let Some(def) = catlog.get(id_owned.as_str()) else {
        game.log.push(format!(
            "{}: unknown item.",
            catlog.display_name(id_owned.as_str())
        ));
        return;
    };
    match def.category {
        ItemCategory::Consumable => {
            let name = def.name;
            if game.narrative.inventory.try_remove(&id_owned, 1).is_ok() {
                game.log.push(format!("[+] Used {name} (no effect yet)."));
            }
        }
        _ => game.log.push("That item is not consumable.".into()),
    }
    if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
        *cursor = (*cursor).min(game.narrative.inventory.stacks.len().saturating_sub(1));
    }
}

fn inventory_equip_stack(game: &mut Game, idx: usize) {
    let n = game.narrative.inventory.stacks.len();
    if n == 0 {
        return;
    }
    let idx = idx.min(n.saturating_sub(1));
    let Some(stack) = game.narrative.inventory.stacks.get(idx) else {
        return;
    };
    let id_owned = stack.id.clone();
    let catlog = game.content.item_catalog();
    let Some(def) = catlog.get(id_owned.as_str()) else {
        game.log.push(format!(
            "{}: unknown item.",
            catlog.display_name(id_owned.as_str())
        ));
        return;
    };
    match def.category {
        ItemCategory::Equippable(slot) => {
            if game.narrative.inventory.count_of(&id_owned) < 1 {
                game.log.push("Could not equip.".into());
                return;
            }
            let toggling_off =
                matches!(stack.equipped, Some(StackEquipped::Wear(s)) if s == slot);
            game.narrative.equip_wear_stack(idx, slot);
            if toggling_off {
                game.log.push(format!("[-] Unequipped {}.", def.name));
            } else {
                game.log.push(format!("[+] Equipped {}.", def.name));
            }
        }
        ItemCategory::Ammo => {
            if game.narrative.inventory.count_of(&id_owned) < 1 {
                game.log.push("Could not load quiver.".into());
                return;
            }
            let toggling_off = matches!(stack.equipped, Some(StackEquipped::Quiver));
            game.narrative.toggle_ammo_quiver(idx);
            if toggling_off {
                game.log
                    .push(format!("[-] Unloaded {} from quiver.", def.name));
            } else {
                game.log
                    .push(format!("[+] Loaded {} into quiver.", def.name));
            }
        }
        ItemCategory::Mundane | ItemCategory::Consumable => {
            game.log
                .push("Use e to equip weapons or load ammo; u uses consumables.".into());
        }
    }
    if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
        *cursor = (*cursor).min(game.narrative.inventory.stacks.len().saturating_sub(1));
    }
}
