//! World hover description and click-range helpers (exploration + combat).

use crate::combat::CombatState;
use crate::content::ContentPack;
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::game::Game;
use crate::game::GameMode;
use crate::input::MouseCell;
use crate::math::{chebyshev, manhattan};
use crate::rect::Rect;
use crate::world::{MapGrid, EMPTY_PROP_ID};

/// Max Manhattan distance from the player for starting dialogue (after walking into range).
pub const TALK_RANGE_MANHATTAN: i32 = 4;

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
        .map(|d| d.description())
        .unwrap_or("?");
    let p = map
        .prop_at(pos.x, pos.y)
        .filter(|&id| id != EMPTY_PROP_ID)
        .and_then(|id| map.table.def(id))
        .map(|d| d.description());
    match p {
        Some(pn) => format!("{g} / {pn}"),
        None => g.into(),
    }
}

fn terrain_primary_name(map: &MapGrid, wp: GridPos) -> String {
    let g = map
        .ground_at(wp.x, wp.y)
        .and_then(|id| map.table.def(id))
        .map(|d| d.description().to_string())
        .unwrap_or_else(|| "Unknown".into());
    let p = map
        .prop_at(wp.x, wp.y)
        .filter(|&id| id != EMPTY_PROP_ID)
        .and_then(|id| map.table.def(id))
        .map(|d| d.description());
    match p {
        Some(pn) if !pn.is_empty() => format!("{pn} ({g})"),
        _ => g,
    }
}

/// Short “Here” lines for the default HUD (`docs/ui_design.md` §4).
#[must_use]
pub fn exploration_here_lines(game: &Game) -> Vec<String> {
    let Some(cell) = game.exploration_hover_cell else {
        return vec!["—".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["(off map)".into()];
    };
    let mut out = vec![terrain_primary_name(&game.map, wp)];
    let pid = game.player_id();
    let pp = pid.and_then(|id| game.entities.pos(id));
    let occupants = game.entities.occupants_at(wp.x, wp.y);
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
        if let Some(ppos) = pp {
            let d = manhattan(ppos, wp);
            out.push(format!("{name} · dist {d}"));
        } else {
            out.push(name);
        }
        break;
    }
    out
}

/// Verbose hover (tile ids, interaction hints) for the F1 debug overlay only.
#[must_use]
pub fn exploration_hover_debug_lines(game: &Game) -> Vec<String> {
    let Some(cell) = game.exploration_hover_cell else {
        return vec!["hover: —".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["hover: outside world rect".into()];
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

/// Combat “Target” block for the right HUD (player-facing).
#[must_use]
pub fn combat_target_lines(game: &Game, state: &CombatState) -> Vec<String> {
    let Some(cell) = game.combat_hover_cell else {
        return vec!["—".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["(off map)".into()];
    };
    let pid = game.player_id();
    let pp = pid.and_then(|id| game.entities.pos(id));
    let range = pp.map(|p| manhattan(p, wp));

    let mut foe: Option<(EntityId, String)> = None;
    for &eid in &game.entities.occupants_at(wp.x, wp.y) {
        if !state.contains_actor(eid) || Some(eid) == pid {
            continue;
        }
        let n = game
            .entities
            .name
            .get(eid.0 as usize)
            .cloned()
            .unwrap_or_default();
        if !n.is_empty() {
            foe = Some((eid, n));
        }
        break;
    }
    let mut out = Vec::new();
    if let Some((eid, n)) = foe {
        out.push(n);
        if let Some(s) = game.entities.stats(eid) {
            out.push(format!("HP       {}/{}", s.hp, s.max_hp));
        }
        if let Some(r) = range {
            out.push(format!("Range    {r}"));
        }
        if crate::game::services::relation::is_hostile_to_player(game, eid) {
            out.push("Hostile".into());
        }
    } else {
        out.push("—".into());
        out.push("(no fighter here)".into());
        if let Some(r) = range {
            out.push(format!("Range    {r}"));
        }
    }
    out
}

/// Verbose combat hover for the F1 overlay.
#[must_use]
pub fn combat_hover_debug_lines(game: &Game, state: &CombatState) -> Vec<String> {
    let Some(cell) = game.combat_hover_cell else {
        return vec!["hover: —".into()];
    };
    let Some(wp) = game.screen_cell_to_world(cell) else {
        return vec!["hover: outside world rect".into()];
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
            crate::combat::CombatRuleset::NonLethalSpar
                | crate::combat::CombatRuleset::NonLethalBrawl
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
