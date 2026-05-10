use crate::game::hud;
use crate::game::{Game, GameMode};
use crate::rect::Rect;
use crate::render::FrameBuffer;
use crate::ui::layout::FloatingPanelLayout;
use crate::ui::{
    chrome_inner_rect, draw_log, draw_menu, draw_modal_world_scrim, draw_rounded_panel,
    draw_text_block_theme, PanelBorderEmphasis, GameUiPalette,
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

    game.compose_world(fb, world_rect);

    let hud_emphasis = if matches!(game.modes.current(), Some(GameMode::Combat(_))) {
        PanelBorderEmphasis::Highlighted
    } else {
        PanelBorderEmphasis::Subtle
    };
    draw_rounded_panel(
        fb,
        hud_rect,
        "Status",
        hud_emphasis,
        &palette,
    );
    let hud_inner = chrome_inner_rect(hud_rect);
    let lines: Vec<String> = match game.modes.current() {
        Some(GameMode::Combat(c)) => hud::combat_status_lines(game, c),
        _ => hud::exploration_status_lines(game),
    };
    draw_text_block_theme(fb, hud_inner, &lines, &palette);

    draw_rounded_panel(
        fb,
        log_rect,
        "Log",
        PanelBorderEmphasis::Subtle,
        &palette,
    );
    let log_inner = chrome_inner_rect(log_rect);
    let footer = hud::command_footer(game);
    // One row reserved for the command footer inside `log_inner`; `draw_log` subtracts it again
    // from `inner.h` for the scroll region — do not pre-shrink the rect or the newest line is dropped.
    let body_rows = log_inner.h.saturating_sub(1);
    let max_rows = body_rows as usize;
    let n = max_rows.min(game.log.len());
    let start = game.log.len().saturating_sub(n);
    let log_lines: Vec<String> = game.log[start..].to_vec();
    draw_log(
        fb,
        log_inner,
        &log_lines,
        Some(footer),
        &palette,
        &mut Vec::new(),
    );

    if let Some(GameMode::MainMenu { selected }) = game.modes.current().cloned() {
        let menu_r = FloatingPanelLayout::main_menu();
        draw_menu(
            fb,
            menu_r,
            "Main menu",
            &game.menu_items,
            selected,
            &palette,
            game.last_mouse_cell,
            &mut game.ui_hits,
        );
    }

    if let Some(GameMode::Dialogue {
        ref dialogue_id,
        node_index,
        choice_cursor,
        npc_entity,
        ..
    }) = game.modes.current().cloned()
    {
        let tree = game
            .content
            .dialogues
            .get(dialogue_id.as_str())
            .copied()
            .unwrap_or(game.content.default_dialogue);
        if let Some(node) = tree.nodes.get(node_index) {
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
                &palette,
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
    }

    if matches!(game.modes.current(), Some(GameMode::GameOver)) {
        draw_modal_world_scrim(fb, world_rect, &palette);
        let gr = FloatingPanelLayout::game_over(fb.width, fb.height);
        draw_rounded_panel(
            fb,
            gr,
            "Game over",
            PanelBorderEmphasis::Highlighted,
            &palette,
        );
        let inner = chrome_inner_rect(gr);
        let lines = vec![
            "Your journey ends here.".into(),
            String::new(),
            "Enter · Space · Esc — main menu".into(),
        ];
        draw_text_block_theme(fb, inner, &lines, &palette);
    }

    if let Some(GameMode::Inventory { cursor }) = game.modes.current() {
        game.compose_inventory_overlay(fb, *cursor);
    }
    if let Some(GameMode::Journal { quest_cursor }) = game.modes.current() {
        game.compose_journal_overlay(fb, *quest_cursor);
    }
    if let Some(GameMode::ItemTransfer {
        container,
        focus,
        cursor_player,
        cursor_container,
    }) = game.modes.current()
    {
        game.compose_item_transfer_overlay(
            fb,
            *container,
            *focus,
            *cursor_player,
            *cursor_container,
        );
    }

    if game.debug_overlay {
        let dbg = FloatingPanelLayout::debug_panel(fb.width, fb.height);
        draw_rounded_panel(
            fb,
            dbg,
            "Debug (F1)",
            PanelBorderEmphasis::Highlighted,
            &palette,
        );
        let inner = chrome_inner_rect(dbg);
        let dirty = fb.dirty_count();
        let lines = hud::debug_overlay_lines(game, dirty);
        draw_text_block_theme(fb, inner, &lines, &palette);
    }
}
