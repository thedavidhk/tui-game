use crate::game::{Game, GameMode};
use crate::game_content;
use crate::rect::Rect;
use crate::render::FrameBuffer;

pub(crate) fn compose(
    game: &mut Game,
    fb: &mut FrameBuffer,
    world_rect: Rect,
    hud_rect: Rect,
    log_rect: Rect,
) {
    game.ui_hits.clear();
    game.compose_world(fb, world_rect);
    crate::ui::draw_bordered_panel(fb, hud_rect, "Status");
    let inner = Rect::new(
        hud_rect.x + 1,
        hud_rect.y + 1,
        hud_rect.w.saturating_sub(2),
        hud_rect.h.saturating_sub(2),
    );
    let mut lines = vec![format!("Mode: {}", game.mode_label())];
    lines.extend(Game::quest_status_lines(&game.narrative));
    lines.push(format!("Demo quest line: {:?}", game.narrative.quests));
    lines.push("I: inventory  J: journal  E: talk/chest  C: combat".into());
    lines.push("F1: debug  F5/F9: save/load".into());
    crate::ui::draw_text_block(fb, inner, &lines);

    crate::ui::draw_bordered_panel(fb, log_rect, "Log");
    let log_inner = Rect::new(
        log_rect.x + 1,
        log_rect.y + 1,
        log_rect.w.saturating_sub(2),
        log_rect.h.saturating_sub(2),
    );
    // Only as many lines as fit vertically; always the newest entries (bottom of `game.log`).
    // A fixed `take(6)` with a shorter `log_inner` used to clip the newest lines off-screen.
    let max_rows = log_inner.h.max(1) as usize;
    let n = max_rows.min(game.log.len());
    let start = game.log.len().saturating_sub(n);
    let log_lines: Vec<String> = game.log[start..].to_vec();
    crate::ui::draw_log(fb, log_inner, &log_lines, &mut Vec::new());

    if let Some(GameMode::MainMenu { selected }) = game.modes.current().cloned() {
        let menu_r = Rect::new(2, 2, 30, 10);
        crate::ui::draw_menu(
            fb,
            menu_r,
            "Main menu",
            &game.menu_items,
            selected,
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
            .unwrap_or(game.content.guide_dialogue);
        if let Some(node) = tree.nodes.get(node_index) {
            let visible = game.dialogue_visible_choice_indices(node);
            let visible_labels: Vec<&'static str> = visible.iter().map(|i| node.choices[*i].label).collect();
            let node_text = game_content::resolve_dialogue_text(node, &game.narrative);
            let speaker_name = game
                .entities
                .name
                .get(npc_entity.0 as usize)
                .map_or("NPC", String::as_str);
            let dr = Rect::new(2, fb.height.saturating_sub(12), fb.width.saturating_sub(4), 10);
            let continue_only = node.choices.is_empty() && node.auto_next.is_some();
            crate::ui::draw_dialogue(
                fb,
                dr,
                speaker_name,
                node_text.as_str(),
                &visible_labels,
                choice_cursor.min(visible_labels.len().saturating_sub(1)),
                continue_only,
                &mut game.ui_hits,
            );
        }
    }

    if let Some(GameMode::Combat(ref c)) = game.modes.current() {
        let cr = Rect::new(2, 10, 40, 6);
        let actor = c.current_actor();
        let who = actor
            .map(|id| game.entities.name.get(id.0 as usize).cloned().unwrap_or_default())
            .unwrap_or_else(|| "n/a".into());
        let hp_line = actor
            .and_then(|id| game.entities.stats(id))
            .map(|s| format!("HP: {}/{}  AP: {}", s.hp, s.max_hp, c.current_ap().unwrap_or(0)))
            .unwrap_or_else(|| "HP: n/a  AP: n/a".into());
        let lines = vec![
            if c.friendly {
                "Combat (training)".into()
            } else {
                "Combat".into()
            },
            format!("Turn: {}", who),
            hp_line,
            "Move WASD, x attack, Tab pass, f flee".into(),
        ];
        crate::ui::draw_bordered_panel(fb, cr, "Combat");
        let inner = Rect::new(cr.x + 1, cr.y + 1, cr.w.saturating_sub(2), cr.h.saturating_sub(2));
        crate::ui::draw_text_block(fb, inner, &lines);
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
        game.compose_item_transfer_overlay(fb, *container, *focus, *cursor_player, *cursor_container);
    }

    if game.debug_overlay {
        let dbg = Rect::new(2, fb.height.saturating_sub(8), fb.width.saturating_sub(4), 6);
        let dirty = fb.dirty_count();
        let enc = game
            .last_perf
            .map(|p| format!("encode_us {}", p.encode_nanos / 1000))
            .unwrap_or_else(|| "encode_us —".into());
        let lines = vec![
            format!("debug: dirty_cells(prev) ~{}", dirty),
            format!("map {}x{}", game.map.width, game.map.height),
            enc,
        ];
        crate::ui::draw_bordered_panel(fb, dbg, "Debug");
        let inner = Rect::new(dbg.x + 1, dbg.y + 1, dbg.w.saturating_sub(2), dbg.h.saturating_sub(2));
        crate::ui::draw_text_block(fb, inner, &lines);
    }
}
