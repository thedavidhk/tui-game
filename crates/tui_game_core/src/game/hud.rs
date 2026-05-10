//! Player-facing HUD copy: status column, command footers, and debug-only lines for the F1 overlay.
//!
//! Raw simulation labels (internal quest phases, tile ids, etc.) stay out of the default HUD and
//! are surfaced through [`debug_overlay_lines`] when [`super::Game::debug_overlay`] is enabled.

use crate::combat::CombatState;
use crate::game::services::hover;
use crate::game::{Game, GameMode};

/// Short mode label for the status column (`docs/ui_design.md` §4).
#[must_use]
pub fn mode_heading(game: &Game) -> &'static str {
    match game.modes.current() {
        Some(GameMode::Exploration) => "Explore",
        Some(GameMode::Combat(_)) => "Combat",
        Some(GameMode::Dialogue { .. }) => "Dialogue",
        Some(GameMode::Inventory { .. }) => "Inventory",
        Some(GameMode::Journal { .. }) => "Journal",
        Some(GameMode::ItemTransfer { .. }) => "Transfer",
        Some(GameMode::MainMenu { .. }) => "Menu",
        Some(GameMode::GameOver) => "Game over",
        None => "—",
    }
}

/// One-line command footer for the log strip (`docs/ui_design.md` §11).
#[must_use]
pub fn command_footer(game: &Game) -> &'static str {
    match game.modes.current() {
        Some(GameMode::Combat(_)) => {
            "LMB attack/move   RMB march   Space wait   f flee   Esc cancel   F1 debug"
        }
        Some(GameMode::Inventory { .. }) => {
            "u use   e equip   Up/Down browse   Esc back   F1 debug"
        }
        Some(GameMode::Journal { .. }) => {
            "Up/Down browse   PgUp/PgDn scroll   Esc back   F1 debug"
        }
        Some(GameMode::ItemTransfer { .. }) => {
            "Enter move stack   Tab side   Esc close   F1 debug"
        }
        Some(GameMode::Dialogue { .. }) => "Enter confirm   Esc leave   F1 debug",
        Some(GameMode::MainMenu { .. }) => "Enter confirm   Esc quit   F1 debug",
        Some(GameMode::GameOver) => "Enter / Esc — main menu",
        _ => "LMB interact   RMB move   I inventory   J journal   F1 debug",
    }
}

/// Primary status lines for exploration (no internal debug strings).
#[must_use]
pub fn exploration_status_lines(game: &Game) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("Mode     {}", mode_heading(game)));
    if let Some(pid) = game.player_id() {
        if let Some(s) = game.entities.stats(pid) {
            lines.push(format!("Health   {}/{}", s.hp, s.max_hp));
            lines.push(format!("Speed    {}", s.speed));
        }
    }
    lines.push(String::new());
    lines.push("Quest".into());
    lines.extend(player_quest_summary(game));
    lines.push(String::new());
    lines.push("Here".into());
    lines.extend(hover::exploration_here_lines(game));
    lines
}

/// Combat-focused status for the right column (`docs/ui_design.md` §8).
#[must_use]
pub fn combat_status_lines(game: &Game, state: &CombatState) -> Vec<String> {
    let mut lines = Vec::new();
    if matches!(
        state.profile.ruleset,
        crate::combat::CombatRuleset::NonLethalSpar | crate::combat::CombatRuleset::NonLethalBrawl
    ) {
        lines.push("Combat (training)".into());
    } else {
        lines.push("Combat".into());
    }
    lines.push(String::new());
    let actor = state.current_actor();
    let who = actor
        .and_then(|id| game.entities.name.get(id.0 as usize).cloned())
        .unwrap_or_else(|| "—".into());
    lines.push(format!("Turn     {who}"));
    if let Some(pid) = game.player_id() {
        if let Some(s) = game.entities.stats(pid) {
            lines.push(format!("Health   {}/{}", s.hp, s.max_hp));
        }
        if actor == Some(pid) {
            lines.push(format!("AP       {}", state.current_ap().unwrap_or(0)));
        } else {
            lines.push("AP       (enemy)".into());
        }
    }
    lines.push(String::new());
    lines.push("Target".into());
    lines.extend(hover::combat_target_lines(game, state));
    lines
}

fn player_quest_summary(game: &Game) -> Vec<String> {
    let mut v = game
        .content
        .runtime_hooks
        .hud_quest_status_lines(&game.narrative);
    if v.is_empty() {
        v.push("(no active objective)".into());
    }
    v
}

/// Lines appended to the F1 debug overlay (encoding stats, internal quest phase, verbose hover).
#[must_use]
pub fn debug_overlay_lines(game: &Game, dirty_cells_prev: usize) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Demo quest phase (internal): {:?}",
            game.narrative.quests
        ),
        format!(
            "viewport {}×{} · map {}×{}",
            game.viewport_w, game.viewport_h, game.map.width, game.map.height
        ),
        format!("dirty_cells(prev) ~{dirty_cells_prev}"),
    ];
    match game.modes.current() {
        Some(GameMode::Exploration) => {
            lines.extend(hover::exploration_hover_debug_lines(game));
        }
        Some(GameMode::Combat(c)) => {
            lines.extend(hover::combat_hover_debug_lines(game, c));
        }
        _ => {}
    }
    if let Some(p) = game.last_perf.as_ref() {
        lines.push(format!("encode_us {}", p.encode_nanos / 1000));
    } else {
        lines.push("encode_us —".into());
    }
    lines
}
