//! Full-screen overlay composition (inventory, journal, item transfer).
//!
//! Each overlay is a panel plus a [`SelectableList`] (and optional detail text), so the
//! row/selection/scroll/hit idiom is not duplicated here. Footer hint rows are reserved by
//! the list and drawn via [`draw_list_footer`].

use crate::content::QuestJournalStatus;
use crate::entity::EntityId;
use crate::item::{EquipSlot, StackEquipped};
use crate::rect::Rect;
use crate::render::{FrameBuffer, Style};
use crate::ui::wrap::wrap_words;
use crate::ui::{
    chrome_inner_rect, draw_clipped_line, draw_rounded_panel, draw_selectable_list,
    draw_text_block_theme, GameUiPalette, PanelBorderEmphasis, SelectableList, UiHitTarget,
};

use super::super::{inventory_stack_display_line, overlay_layout, Game, TransferFocus};

pub(super) fn compose_inventory(game: &mut Game, fb: &mut FrameBuffer, cursor: usize) {
    let palette = GameUiPalette::DEFAULT;
    let (bags, equipment, detail) = overlay_layout::three_column_inventory(fb.width, fb.height);
    let cat = game.content.item_catalog();
    let n = game.narrative.inventory.stacks.len();

    draw_rounded_panel(fb, bags, "Inventory", PanelBorderEmphasis::Subtle, &palette);
    let inner_b = chrome_inner_rect(bags);
    let rows: Vec<String> = game
        .narrative
        .inventory
        .stacks
        .iter()
        .map(|s| inventory_stack_display_line(&cat, s))
        .collect();
    let list = SelectableList {
        inner: inner_b,
        rows: &rows,
        selected: Some(cursor),
        last_mouse: game.last_mouse_cell,
        empty_text: Some("(empty)"),
        reserved_footer_rows: 2,
    };
    draw_selectable_list(
        fb,
        &palette,
        &list,
        &mut game.ui_hits,
        UiHitTarget::InventoryStack,
    );
    draw_list_footer(fb, inner_b, &palette, "click row · u/e · Esc back");

    draw_rounded_panel(
        fb,
        equipment,
        "Equipped",
        PanelBorderEmphasis::Subtle,
        &palette,
    );
    let inner_e = chrome_inner_rect(equipment);
    let mut eq_lines: Vec<String> = Vec::new();
    for slot in EquipSlot::VARIANTS {
        let title = slot.to_string();
        let line = match game.narrative.worn_item_id_in_slot(slot) {
            None => format!("{title}: —"),
            Some(id) => format!("{title}: {}", cat.display_name(id)),
        };
        eq_lines.push(line);
    }
    eq_lines.push(String::new());
    let quiver_line = game
        .narrative
        .inventory
        .stacks
        .iter()
        .find(|s| matches!(s.equipped, Some(StackEquipped::Quiver)))
        .map_or_else(
            || "Quiver: (empty)".to_string(),
            |s| format!("Quiver: {} x{}", cat.display_name(s.id.as_str()), s.count),
        );
    eq_lines.push(quiver_line);
    draw_text_block_theme(fb, inner_e, &eq_lines, &palette);

    draw_rounded_panel(fb, detail, "Detail", PanelBorderEmphasis::Subtle, &palette);
    let inner_d = chrome_inner_rect(detail);
    let line_w = inner_d.w.max(1) as usize;
    let mut detail_lines: Vec<String> = Vec::new();
    if let Some(s) = game
        .narrative
        .inventory
        .stacks
        .get(cursor.min(n.saturating_sub(1)))
    {
        if let Some(def) = cat.get(s.id.as_str()) {
            detail_lines.push(def.name.to_string());
            detail_lines.push(cat.category_line(s.id.as_str()));
            detail_lines.push(String::new());
            detail_lines.extend(wrap_words(def.description, line_w.max(12)));
        } else {
            detail_lines.push(s.id.clone());
        }
    } else {
        detail_lines.push("(no stacks)".into());
    }
    draw_text_block_theme(fb, inner_d, &detail_lines, &palette);
}

pub(super) fn compose_journal(game: &mut Game, fb: &mut FrameBuffer, quest_cursor: usize) {
    let palette = GameUiPalette::DEFAULT;
    let (left, right) = overlay_layout::two_column_relaxed(fb.width, fb.height);

    draw_rounded_panel(fb, left, "Quests", PanelBorderEmphasis::Subtle, &palette);
    let inner_l = chrome_inner_rect(left);
    let n = game.narrative.quest_journal.len();
    let rows: Vec<String> = game
        .narrative
        .quest_journal
        .iter()
        .map(|q| format!("{} [{}]", q.title, status_label(q.status)))
        .collect();
    let list = SelectableList {
        inner: inner_l,
        rows: &rows,
        selected: Some(quest_cursor),
        last_mouse: game.last_mouse_cell,
        empty_text: Some("(no entries yet)"),
        reserved_footer_rows: 2,
    };
    draw_selectable_list(
        fb,
        &palette,
        &list,
        &mut game.ui_hits,
        UiHitTarget::JournalQuest,
    );
    draw_list_footer(fb, inner_l, &palette, "Up/Down  PgUp/PgDn  Esc back");

    draw_rounded_panel(fb, right, "Entries", PanelBorderEmphasis::Subtle, &palette);
    let inner_r = chrome_inner_rect(right);
    let line_w = inner_r.w.max(1) as usize;
    let mut detail: Vec<String> = Vec::new();
    if n == 0 {
        detail.push("Quest lines appear when you talk,".into());
        detail.push("pick up certain items, or advance".into());
        detail.push("a story beat.".into());
    } else {
        let q = &game.narrative.quest_journal[quest_cursor.min(n.saturating_sub(1))];
        detail.push(format!("{} — {}", q.title, status_label(q.status)));
        detail.push(String::new());
        let mut entries: Vec<_> = q.entries.iter().collect();
        entries.sort_by_key(|e| e.seq);
        for e in entries {
            let line = format!("[{}] {}", e.seq, e.text);
            detail.extend(wrap_words(&line, line_w.max(12)));
            detail.push(String::new());
        }
        if q.entries.is_empty() {
            detail.push("(no log lines yet)".into());
        }
    }
    draw_text_block_theme(fb, inner_r, &detail, &palette);
}

pub(super) fn compose_item_transfer(
    game: &mut Game,
    fb: &mut FrameBuffer,
    container: EntityId,
    focus: TransferFocus,
    cursor_player: usize,
    cursor_container: usize,
) {
    let palette = GameUiPalette::DEFAULT;
    let (left, right) = overlay_layout::two_column_tight(fb.width, fb.height);
    let cat = game.content.item_catalog();
    let cname = game
        .entities
        .name
        .get(container.0 as usize)
        .cloned()
        .unwrap_or_else(|| "Chest".into());
    let emphasis = |side| {
        if focus == side {
            PanelBorderEmphasis::Highlighted
        } else {
            PanelBorderEmphasis::Subtle
        }
    };
    draw_rounded_panel(fb, left, "You", emphasis(TransferFocus::Player), &palette);
    draw_rounded_panel(
        fb,
        right,
        cname.as_str(),
        emphasis(TransferFocus::Container),
        &palette,
    );

    let li = chrome_inner_rect(left);
    let player_rows: Vec<String> = game
        .narrative
        .inventory
        .stacks
        .iter()
        .map(|s| inventory_stack_display_line(&cat, s))
        .collect();
    let player_list = SelectableList {
        inner: li,
        rows: &player_rows,
        selected: (focus == TransferFocus::Player).then_some(cursor_player),
        last_mouse: game.last_mouse_cell,
        empty_text: Some("(empty)"),
        reserved_footer_rows: 2,
    };
    draw_selectable_list(
        fb,
        &palette,
        &player_list,
        &mut game.ui_hits,
        UiHitTarget::TransferPlayerStack,
    );
    draw_list_footer(fb, li, &palette, "click row move · Tab · Enter · Esc close");

    let ri = chrome_inner_rect(right);
    let cont_rows: Vec<String> = game
        .narrative
        .container_inventories
        .get(&container.0)
        .map_or(&[][..], |v| v.stacks.as_slice())
        .iter()
        .map(|s| format!("{} x{}", cat.display_name(s.id.as_str()), s.count))
        .collect();
    let cont_list = SelectableList {
        inner: ri,
        rows: &cont_rows,
        selected: (focus == TransferFocus::Container).then_some(cursor_container),
        last_mouse: game.last_mouse_cell,
        empty_text: Some("(empty)"),
        reserved_footer_rows: 2,
    };
    draw_selectable_list(
        fb,
        &palette,
        &cont_list,
        &mut game.ui_hits,
        UiHitTarget::TransferContainerStack,
    );
    draw_list_footer(fb, ri, &palette, "click row move · Tab · Enter · Esc close");
}

fn status_label(s: QuestJournalStatus) -> &'static str {
    match s {
        QuestJournalStatus::InProgress => "In progress",
        QuestJournalStatus::Failed => "Failed",
        QuestJournalStatus::Completed => "Completed",
    }
}

/// Draws the two reserved footer rows for a list panel: a separator and a hint line.
fn draw_list_footer(fb: &mut FrameBuffer, inner: Rect, palette: &GameUiPalette, hint: &str) {
    let dim = Style {
        dim: true,
        ..Default::default()
    };
    let sep_y = inner.bottom().saturating_sub(2);
    if sep_y >= inner.y && sep_y < inner.bottom() {
        draw_clipped_line(
            fb,
            inner.x,
            sep_y,
            inner.w,
            "—",
            palette.text_dim,
            palette.panel_bg,
            dim,
        );
    }
    let hint_y = inner.bottom().saturating_sub(1);
    if hint_y >= inner.y && hint_y < inner.bottom() {
        draw_clipped_line(
            fb,
            inner.x,
            hint_y,
            inner.w,
            hint,
            palette.text_dim,
            palette.panel_bg,
            dim,
        );
    }
}
