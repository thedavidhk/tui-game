//! World hover description and click-range helpers (exploration + combat).

use crate::combat::CombatState;
use crate::content::ContentPack;
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::game::Game;
use crate::game::GameMode;
use crate::input::MouseCell;
use crate::rect::Rect;
use crate::world::{MapGrid, EMPTY_PROP_ID};

/// Max Manhattan distance from the player for starting dialogue (after walking into range).
pub const TALK_RANGE_MANHATTAN: i32 = 4;

#[must_use]
pub fn manhattan(a: GridPos, b: GridPos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

#[must_use]
pub fn chebyshev(a: GridPos, b: GridPos) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

fn blueprint_for<'a>(
    entities: &EntityArena,
    content: &'a ContentPack,
    eid: EntityId,
) -> Option<&'a crate::content::EntityBlueprint> {
    let kind = entities.npc_kind.get(eid.0 as usize)?.as_deref()?;
    content.blueprint(kind)
}

fn tile_label(map: &MapGrid, pos: GridPos) -> String {
    let g = map
        .ground_at(pos.x, pos.y)
        .and_then(|id| map.table.def(id))
        .map(|d| d.name.as_str())
        .unwrap_or("?");
    let p = map
        .prop_at(pos.x, pos.y)
        .filter(|&id| id != EMPTY_PROP_ID)
        .and_then(|id| map.table.def(id))
        .map(|d| d.name.as_str());
    match p {
        Some(pn) => format!("{g} / {pn}"),
        None => g.into(),
    }
}

/// Lines appended under the status HUD while exploring (short lines for narrow HUD).
#[must_use]
pub fn exploration_hover_lines(game: &Game) -> Vec<String> {
    let Some(cell) = game.exploration_hover_cell else {
        return vec!["Look: —".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["Look: (outside map)".into()];
    };
    let mut out = Vec::new();
    out.push(format!("Tile: {}", tile_label(&game.map, wp)));
    let pid = game.player_id();
    let pp = pid.and_then(|id| game.entities.pos(id));
    let occupants = game.entities.occupants_at(wp.x, wp.y);
    let mut shown_entity = false;
    for &eid in &occupants {
        if Some(eid) == pid {
            continue;
        }
        let name = game
            .entities
            .name
            .get(eid.0 as usize)
            .cloned()
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        shown_entity = true;
        if let Some(ppos) = pp {
            let d = manhattan(ppos, wp);
            out.push(format!("{name} (dist {d})"));
            if let Some(bp) = blueprint_for(&game.entities, &game.content, eid) {
                if game.entities.is_container[eid.0 as usize] {
                    if chebyshev(ppos, wp) <= 1 {
                        out.push("LMB: open".into());
                    } else {
                        out.push("Closer to open".into());
                    }
                } else if crate::game::services::relation::is_hostile_to_player(game, eid) {
                    if chebyshev(ppos, wp) <= 1 {
                        out.push("LMB: attack".into());
                    } else {
                        out.push("LMB: move & fight".into());
                    }
                } else if bp.dialogue_id.is_some() {
                    if manhattan(ppos, wp) <= TALK_RANGE_MANHATTAN {
                        out.push("LMB: talk".into());
                    } else {
                        out.push("Too far to talk".into());
                    }
                }
            }
        } else {
            out.push(name);
        }
        break;
    }
    if !shown_entity {
        out.push("No creature here.".into());
    }
    out
}

/// Hover text during combat (player's world cursor).
#[must_use]
pub fn combat_hover_lines(game: &Game, state: &CombatState) -> Vec<String> {
    let Some(cell) = game.combat_hover_cell else {
        return vec!["Look: —".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["Look: (outside map)".into()];
    };
    let mut out = vec![format!("Tile: {}", tile_label(&game.map, wp))];
    let pid = game.player_id();
    let player_turn = pid.is_some_and(|p| state.current_actor() == Some(p));

    let mut label = None::<String>;
    for &eid in &game.entities.occupants_at(wp.x, wp.y) {
        if !state.contains_actor(eid) {
            continue;
        }
        let n = game
            .entities
            .name
            .get(eid.0 as usize)
            .cloned()
            .unwrap_or_default();
        if !n.is_empty() {
            label = Some(n);
        }
        break;
    }
    if let Some(n) = label {
        out.push(format!("Fighter: {n}"));
    } else {
        out.push("Empty cell".into());
    }

    if player_turn {
        if matches!(
            state.profile.ruleset,
            crate::combat::CombatRuleset::NonLethalSpar | crate::combat::CombatRuleset::NonLethalBrawl
        ) {
            out.push("LMB: engage trainer".into());
        } else {
            out.push("LMB: hostile / move".into());
        }
        out.push("RMB: move here".into());
    } else {
        out.push("(enemy turn)".into());
    }
    out
}

/// Updates hover cell when the cursor is over the world panel during exploration.
pub fn sync_exploration_hover(game: &mut Game, cell: MouseCell, world_screen: Rect) {
    if !matches!(game.modes.current(), Some(GameMode::Exploration)) {
        game.exploration_hover_cell = None;
        return;
    }
    if world_screen.contains(cell.x, cell.y) {
        game.exploration_hover_cell = Some(cell);
    } else {
        game.exploration_hover_cell = None;
    }
    game.register_world_pointer(cell, world_screen);
}

/// Updates hover cell when the cursor is over the world panel during combat.
pub fn sync_combat_hover(game: &mut Game, cell: MouseCell, world_screen: Rect) {
    if !matches!(game.modes.current(), Some(GameMode::Combat(_))) {
        game.combat_hover_cell = None;
        return;
    }
    if world_screen.contains(cell.x, cell.y) {
        game.combat_hover_cell = Some(cell);
    } else {
        game.combat_hover_cell = None;
    }
    game.register_world_pointer(cell, world_screen);
}
