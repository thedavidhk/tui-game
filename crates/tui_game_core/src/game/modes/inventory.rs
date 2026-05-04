use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::item::{ItemCategory, ItemStack};

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    let n = game.narrative.inventory.stacks.len();
    match ev {
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
        GameInput::Command(GameCommand::InventoryUse) => {
            let Some(GameMode::Inventory { cursor }) = game.modes.current().cloned() else {
                return;
            };
            if n == 0 {
                return;
            }
            let idx = cursor.min(n.saturating_sub(1));
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
                        game.log.push(format!("Used {name} (no effect yet)."));
                    }
                }
                _ => game.log.push("That item is not consumable.".into()),
            }
            if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                *cursor = (*cursor).min(game.narrative.inventory.stacks.len().saturating_sub(1));
            }
        }
        GameInput::Command(GameCommand::InventoryEquip) => {
            let Some(GameMode::Inventory { cursor }) = game.modes.current().cloned() else {
                return;
            };
            if n == 0 {
                return;
            }
            let idx = cursor.min(n.saturating_sub(1));
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
                    if game.narrative.inventory.try_remove(&id_owned, 1).is_err() {
                        game.log.push("Could not equip.".into());
                        return;
                    }
                    if let Some(prev) = game.narrative.equipment.insert(slot, id_owned.clone()) {
                        game.narrative.inventory.add(prev, 1);
                    }
                    game.log.push(format!("Equipped {}.", def.name));
                }
                ItemCategory::Ammo => {
                    let load_count = stack.count;
                    if game.narrative.inventory.try_remove(&id_owned, load_count).is_err() {
                        game.log.push("Could not load quiver.".into());
                        return;
                    }
                    let incoming = ItemStack {
                        id: id_owned.clone(),
                        count: load_count,
                    };
                    match game.narrative.equipped_ammo.take() {
                        None => {
                            game.narrative.equipped_ammo = Some(incoming);
                        }
                        Some(prev) if prev.id == incoming.id => {
                            let merged = prev.count.saturating_add(incoming.count);
                            game.narrative.equipped_ammo =
                                Some(ItemStack::new(incoming.id.clone(), merged));
                        }
                        Some(prev) => {
                            game.narrative.inventory.add(prev.id, prev.count);
                            game.narrative.equipped_ammo = Some(incoming);
                        }
                    }
                    game.log.push(format!("Loaded {} into quiver.", def.name));
                }
                ItemCategory::Mundane | ItemCategory::Consumable => {
                    game.log.push("Use e to equip weapons or load ammo; u uses consumables.".into());
                }
            }
            if let Some(GameMode::Inventory { cursor }) = game.modes.current_mut() {
                *cursor = (*cursor).min(game.narrative.inventory.stacks.len().saturating_sub(1));
            }
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
