//! Entity spawn placement, erasing, and player start marker.

use tui_game_core::level::{EntitySpawn, PlayerSpawn};
use tui_game_core::render::Color;
use tui_game_core::ui::{cell_in_axis_rect, cell_in_brush};

use super::{Editor, Mode};

impl Editor {
    pub fn spawn_glyph(&self, spawn: &EntitySpawn) -> char {
        spawn.glyph_override.unwrap_or_else(|| {
            self.content
                .blueprint(spawn.kind.as_str())
                .map_or('?', |bp| bp.default_glyph)
        })
    }

    pub fn spawn_fg(&self, spawn: &EntitySpawn) -> Color {
        spawn.fg_override.unwrap_or_else(|| {
            self.content
                .blueprint(spawn.kind.as_str())
                .map_or(Color::rgb(255, 160, 80), |bp| bp.default_fg.to_render_color())
        })
    }

    pub fn cell_has_spawn(&self, tx: i32, ty: i32) -> bool {
        self.level.spawns.iter().any(|s| s.x == tx && s.y == ty)
    }

    pub fn remove_spawns_in_brush(&mut self, cx: i32, cy: i32) -> usize {
        let r = self.brush_radius;
        let before = self.level.spawns.len();
        self.level
            .spawns
            .retain(|s| !cell_in_brush(s.x, s.y, cx, cy, r));
        let removed = before.saturating_sub(self.level.spawns.len());
        if removed > 0 {
            self.mark_dirty();
        }
        removed
    }

    pub fn remove_spawns_in_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) -> usize {
        let before = self.level.spawns.len();
        self.level
            .spawns
            .retain(|s| !cell_in_axis_rect(s.x, s.y, x0, y0, x1, y1));
        let removed = before.saturating_sub(self.level.spawns.len());
        if removed > 0 {
            self.mark_dirty();
        }
        removed
    }

    pub fn cycle_mode(&mut self) {
        let next = match self.mode {
            Mode::PaintTiles => Mode::PlaceSpawns,
            Mode::PlaceSpawns => Mode::EraseSpawns,
            Mode::EraseSpawns => Mode::SetPlayerSpawn,
            Mode::SetPlayerSpawn => Mode::AtmosphereZones,
            Mode::AtmosphereZones => Mode::PaintTiles,
        };
        self.set_mode(next);
        self.status = format!("Mode: {:?}", self.mode);
    }

    pub fn set_player_spawn_at(&mut self, tx: i32, ty: i32) {
        let w = self.level.width as i32;
        let h = self.level.height as i32;
        let x = tx.clamp(0, w - 1);
        let y = ty.clamp(0, h - 1);
        self.level.player_spawn = Some(PlayerSpawn { x, y });
        self.mark_dirty();
        self.status = format!("Player spawn set to ({x},{y}).");
        self.ensure_world_cell_visible(x, y);
    }

    pub fn clear_player_spawn(&mut self) {
        self.level.player_spawn = None;
        self.mark_dirty();
        self.status = "Player spawn cleared (game will use map center).".into();
    }

    pub fn place_spawn_at(&mut self, tx: i32, ty: i32) {
        let Some(bp) = self.current_spawn_blueprint() else {
            self.status = "No entity blueprints in content pack.".into();
            return;
        };
        let x = tx.clamp(0, self.level.width as i32 - 1);
        let y = ty.clamp(0, self.level.height as i32 - 1);
        self.level.spawns.push(EntitySpawn {
            kind: bp.kind.to_string(),
            x,
            y,
            glyph_override: None,
            name_override: None,
            fg_override: None,
        });
        self.mark_dirty();
        self.status = format!("Spawn {} at ({}, {}).", bp.kind, x, y);
        self.ensure_world_cell_visible(x, y);
        self.touch_entity_memory_from_current_idx();
    }
}
