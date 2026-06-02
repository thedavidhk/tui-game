//! Frame composition for the game: world viewport, HUD, log, and active overlays.
//!
//! [`compose`] is the single entry point ([`crate::game::Game::compose`] delegates here).
//! World rendering lives in [`world`]; full-screen overlays in [`overlays`]; this module
//! orchestrates them and draws the persistent shell (HUD + log) plus floating panels.

mod overlays;
mod world;

use crate::game::hud;
use crate::game::{effects, Game, GameMode};
use crate::rect::Rect;
use crate::render::FrameBuffer;
use crate::ui::layout::FloatingPanelLayout;
use crate::ui::{
    chrome_inner_rect, draw_log, draw_menu, draw_modal_world_scrim, draw_rounded_panel,
    draw_text_block_theme, GameUiPalette, PanelBorderEmphasis,
};

pub(crate) fn compose(
    game: &mut Game,
    fb: &mut FrameBuffer,
    world_rect: Rect,
    hud_rect: Rect,
    log_rect: Rect,
) {
    let palette = GameUiPalette::DEFAULT;
    game.ui_hits.clear();

    world::compose_world(game, fb, world_rect);

    // World-space area effects (fire, poison clouds, …) are blended over the world view
    // before screen-space post-processing, so the vignette sits on top of them.
    let area_effects = effects::frame_area_effects(game);
    if !area_effects.is_empty() {
        let (ox, oy) = game.world_screen_origin();
        crate::render::area_effects::apply_area_effects(fb, (ox, oy), world_rect, &area_effects);
    }

    // Screen-space effects sit on top of the world view but under HUD/overlays/dialogue,
    // so panels stay legible. Which effects are active is decided in `game::effects`.
    let screen_effects = effects::active_screen_effects(game);
    crate::render::effects::apply_screen_effects(fb, world_rect, &screen_effects);

    compose_hud(game, fb, hud_rect, &palette);
    compose_log(game, fb, log_rect, &palette);
    compose_active_overlay(game, fb, world_rect, &palette);
    compose_debug_overlay(game, fb, &palette);
}

fn compose_hud(game: &Game, fb: &mut FrameBuffer, hud_rect: Rect, palette: &GameUiPalette) {
    let turn_hud = crate::game::services::behavior::active_turn_clock(game);
    let hud_emphasis = if turn_hud.is_some() {
        PanelBorderEmphasis::Highlighted
    } else {
        PanelBorderEmphasis::Subtle
    };
    draw_rounded_panel(fb, hud_rect, "Status", hud_emphasis, palette);
    let hud_inner = chrome_inner_rect(hud_rect);
    let raw_lines: Vec<String> = if let Some(c) = turn_hud {
        hud::combat_status_lines(game, c)
    } else {
        hud::exploration_status_lines(game)
    };
    let line_w = hud_inner.w.max(1) as usize;
    let lines = crate::ui::wrap::wrap_panel_lines(&raw_lines, line_w);
    draw_text_block_theme(fb, hud_inner, &lines, palette);
}

fn compose_log(game: &Game, fb: &mut FrameBuffer, log_rect: Rect, palette: &GameUiPalette) {
    draw_rounded_panel(fb, log_rect, "Log", PanelBorderEmphasis::Subtle, palette);
    let log_inner = chrome_inner_rect(log_rect);
    let footer = hud::command_footer(game);
    // One row reserved for the command footer inside `log_inner`; `draw_log` subtracts it again
    // from `inner.h` for the scroll region — do not pre-shrink the rect or the newest line is dropped.
    let body_rows = log_inner.h.saturating_sub(1);
    let max_rows = body_rows as usize;
    let n = max_rows.min(game.log.len());
    let start = game.log.len().saturating_sub(n);
    let log_lines: Vec<String> = game.log[start..].to_vec();
    draw_log(fb, log_inner, &log_lines, Some(footer), palette);
}

fn compose_active_overlay(
    game: &mut Game,
    fb: &mut FrameBuffer,
    world_rect: Rect,
    palette: &GameUiPalette,
) {
    if let Some(GameMode::MainMenu { selected }) = game.modes.current().cloned() {
        let menu_r = FloatingPanelLayout::main_menu();
        draw_menu(
            fb,
            menu_r,
            "Main menu",
            &game.menu_items,
            selected,
            palette,
            game.last_mouse_cell,
            &mut game.ui_hits,
        );
    }

    compose_dialogue_overlay(game, fb, palette);

    if matches!(game.modes.current(), Some(GameMode::GameOver)) {
        draw_modal_world_scrim(fb, world_rect, palette);
        let gr = FloatingPanelLayout::game_over(fb.width, fb.height);
        draw_rounded_panel(
            fb,
            gr,
            "Game over",
            PanelBorderEmphasis::Highlighted,
            palette,
        );
        let inner = chrome_inner_rect(gr);
        let lines = vec![
            "Your journey ends here.".into(),
            String::new(),
            "Enter · Space · Esc — main menu".into(),
        ];
        draw_text_block_theme(fb, inner, &lines, palette);
    }

    match game.modes.current() {
        Some(GameMode::Inventory { cursor }) => overlays::compose_inventory(game, fb, *cursor),
        Some(GameMode::Journal { quest_cursor }) => {
            overlays::compose_journal(game, fb, *quest_cursor);
        }
        Some(GameMode::ItemTransfer {
            container,
            focus,
            cursor_player,
            cursor_container,
        }) => {
            overlays::compose_item_transfer(
                game,
                fb,
                *container,
                *focus,
                *cursor_player,
                *cursor_container,
            );
        }
        _ => {}
    }
}

fn compose_dialogue_overlay(game: &mut Game, fb: &mut FrameBuffer, palette: &GameUiPalette) {
    let Some(GameMode::Dialogue {
        ref dialogue_id,
        node_index,
        choice_cursor,
        npc_entity,
        ..
    }) = game.modes.current().cloned()
    else {
        return;
    };
    let tree = game
        .content
        .dialogues
        .get(dialogue_id.as_str())
        .copied()
        .unwrap_or(game.content.default_dialogue);
    let Some(node) = tree.nodes.get(node_index) else {
        return;
    };
    let visible = game.dialogue_visible_choice_indices(node);
    let node_text = game
        .content
        .runtime_hooks
        .resolve_dialogue_text(node, &game.narrative);
    let speaker_name = game
        .entities
        .name
        .get(npc_entity.0 as usize)
        .map_or("NPC", String::as_str);
    let dr = FloatingPanelLayout::dialogue_band(fb.width, fb.height);
    let continue_only = node.choices.is_empty() && node.auto_next.is_some();
    crate::ui::draw_dialogue(
        fb,
        dr,
        palette,
        speaker_name,
        node,
        node_text.as_str(),
        &visible,
        choice_cursor.min(visible.len().saturating_sub(1)),
        continue_only,
        game.last_mouse_cell,
        &mut game.ui_hits,
    );
}

fn compose_debug_overlay(game: &Game, fb: &mut FrameBuffer, palette: &GameUiPalette) {
    if !game.debug_overlay {
        return;
    }
    let dbg = FloatingPanelLayout::debug_panel(fb.width);
    draw_rounded_panel(
        fb,
        dbg,
        "Debug (F1)",
        PanelBorderEmphasis::Highlighted,
        palette,
    );
    let inner = chrome_inner_rect(dbg);
    let dirty = fb.dirty_count();
    let lines = hud::debug_overlay_lines(game, dirty);
    draw_text_block_theme(fb, inner, &lines, palette);
}
