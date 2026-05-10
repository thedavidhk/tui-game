//! Remember last terrain / entity brush when switching modes so picks stay valid.

use tui_game_core::world::{TileDef, TileId, EMPTY_PROP_ID};

use super::{Dialog, Editor, Mode, PaintLayer};

impl Editor {
    /// Call once after constructing the editor from a level.
    pub fn init_brush_memory_defaults(&mut self) {
        self.sync_brush_memory_from_level();
    }

    /// After hot-reload or external level replace, keep remembered ids consistent with the new tables.
    pub fn sync_brush_memory_from_level(&mut self) {
        if self.level.tile_defs.iter().any(|d| d.id == self.current_tile)
            && self.current_tile != EMPTY_PROP_ID
        {
            self.last_terrain_tile_id = self.current_tile;
        } else if let Some(d) = self.level.tile_defs.first() {
            self.last_terrain_tile_id = d.id;
        } else {
            self.last_terrain_tile_id = 0;
        }

        let n = self.content.entity_blueprints.len();
        if n == 0 {
            self.last_entity_blueprint_idx = 0;
            self.spawn_blueprint_idx = 0;
        } else {
            self.spawn_blueprint_idx = self.spawn_blueprint_idx.min(n - 1);
            self.last_entity_blueprint_idx = self.spawn_blueprint_idx;
        }

        self.ensure_valid_terrain_brush();
        self.ensure_valid_entity_brush();
    }

    /// Switch mode while remembering the prior mode’s brush and restoring the new mode’s last brush.
    pub fn set_mode(&mut self, next: Mode) {
        let prev = self.mode;
        self.remember_brush_for_mode(prev);
        self.mode = next;
        self.restore_brush_for_mode(next);
    }

    fn remember_brush_for_mode(&mut self, mode: Mode) {
        match mode {
            Mode::PaintTiles => {
                if self.current_tile != EMPTY_PROP_ID && self.current_tile_def().is_some() {
                    self.last_terrain_tile_id = self.current_tile;
                }
            }
            Mode::PlaceSpawns => {
                if self.spawn_blueprint_idx < self.content.entity_blueprints.len() {
                    self.last_entity_blueprint_idx = self.spawn_blueprint_idx;
                }
            }
            Mode::EraseSpawns | Mode::SetPlayerSpawn | Mode::AtmosphereZones => {}
        }
    }

    fn restore_brush_for_mode(&mut self, mode: Mode) {
        match mode {
            Mode::PaintTiles => self.ensure_valid_terrain_brush(),
            Mode::PlaceSpawns => self.ensure_valid_entity_brush(),
            Mode::EraseSpawns | Mode::SetPlayerSpawn | Mode::AtmosphereZones => {}
        }
    }

    /// Persist the current entity blueprint index when in range.
    pub fn touch_entity_memory_from_current_idx(&mut self) {
        if self.spawn_blueprint_idx < self.content.entity_blueprints.len() {
            self.last_entity_blueprint_idx = self.spawn_blueprint_idx;
        }
    }

    pub fn ensure_valid_terrain_brush(&mut self) {
        if self.current_tile == EMPTY_PROP_ID {
            if self.paint_layer == PaintLayer::Ground {
                self.current_tile = self.fallback_terrain_tile_id();
            }
            return;
        }
        if self.current_tile_def().is_none() {
            self.current_tile = self.fallback_terrain_tile_id();
        }
    }

    fn fallback_terrain_tile_id(&mut self) -> TileId {
        if self
            .level
            .tile_defs
            .iter()
            .any(|d| d.id == self.last_terrain_tile_id)
        {
            return self.last_terrain_tile_id;
        }
        if let Some(d) = self.level.tile_defs.first() {
            self.last_terrain_tile_id = d.id;
            return d.id;
        }
        0
    }

    pub fn ensure_valid_entity_brush(&mut self) {
        let n = self.content.entity_blueprints.len();
        if n == 0 {
            self.spawn_blueprint_idx = 0;
            return;
        }
        let i = self.last_entity_blueprint_idx.min(n - 1);
        self.spawn_blueprint_idx = i;
    }

    /// Terrain row for the sidebar preview (`None` when only prop-clear is active).
    pub fn preview_terrain_def(&self) -> Option<&TileDef> {
        if self.current_tile == EMPTY_PROP_ID {
            return None;
        }
        self.current_tile_def()
    }

    /// Entity row for the sidebar preview.
    pub fn preview_entity_blueprint(&self) -> Option<&'static tui_game_core::EntityBlueprint> {
        self.current_spawn_blueprint()
    }

    /// Full-screen dimmed modals that hide the map for hit-testing / hover.
    pub(crate) fn dialog_covers_map(&self) -> bool {
        matches!(
            self.dialog,
            Some(Dialog::SavePath { .. } | Dialog::HotReloadUnsaved)
        )
    }

    /// Apply a terrain def pick from the searchable list (keyboard or mouse).
    pub fn apply_terrain_pick_by_def_index(&mut self, def_index: usize) {
        let Some(d) = self.level.tile_defs.get(def_index) else {
            return;
        };
        self.current_tile = d.id;
        self.last_terrain_tile_id = d.id;
        self.status = format!("Terrain brush: {} (def {})", d.name, def_index);
        if self.mode != Mode::PaintTiles {
            self.set_mode(Mode::PaintTiles);
        }
    }

    /// Apply an entity blueprint pick from the searchable list.
    pub fn apply_entity_pick_by_index(&mut self, blueprint_index: usize) {
        if blueprint_index >= self.content.entity_blueprints.len() {
            return;
        }
        self.spawn_blueprint_idx = blueprint_index;
        self.last_entity_blueprint_idx = blueprint_index;
        let bp = &self.content.entity_blueprints[blueprint_index];
        self.status = format!("Place entity: {}", bp.kind);
        if self.mode != Mode::PlaceSpawns {
            self.set_mode(Mode::PlaceSpawns);
        }
    }
}
