//! Initial load, default level template, resize, and palette helpers.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use tui_game_core::game_content;
use tui_game_core::level::{
    derive_visual_seed, level_from_ron, materialize_tile_defs_from_pack, AtmosphereRecipe,
    EntitySpawn, LevelFile, PlayerSpawn,
};
use tui_game_core::ui::SearchListPicker;
use tui_game_core::world::{TileDef, EMPTY_PROP_ID};
use tui_game_core::EntityBlueprint;

use super::disk::FileFingerprint;
use super::{Dialog, Editor, Mode, PaintLayer};

impl Editor {
    pub fn default_level() -> LevelFile {
        let tile_defs = game_content::embedded_demo_level().tile_defs;
        let floor_tile = tile_defs
            .iter()
            .find(|d| !d.blocks_movement)
            .map_or(0, |d| d.idx);
        let wall_tile = tile_defs
            .iter()
            .find(|d| d.blocks_movement)
            .map_or(floor_tile, |d| d.idx);
        let w = 24u16;
        let h = 16u16;
        let n = (w as usize) * (h as usize);
        let tiles = vec![floor_tile; n];
        let mut props = vec![EMPTY_PROP_ID; n];
        for x in 0..w {
            props[x as usize] = wall_tile;
            props[(h as usize - 1) * w as usize + x as usize] = wall_tile;
        }
        for y in 0..h {
            props[y as usize * w as usize] = wall_tile;
            props[y as usize * w as usize + (w as usize - 1)] = wall_tile;
        }
        LevelFile {
            schema_version: LevelFile::SCHEMA,
            name: "untitled".into(),
            width: w,
            height: h,
            tiles,
            props,
            terrain_pack: String::new(),
            terrain_palette: Vec::new(),
            tile_defs,
            spawns: vec![EntitySpawn {
                kind: "guide".into(),
                x: 10,
                y: 8,
                glyph_override: None,
                name_override: None,
                fg_override: None,
            }],
            player_spawn: Some(PlayerSpawn { x: 12, y: 8 }),
            visual_seed: None,
            default_atmosphere: AtmosphereRecipe::default(),
            atmosphere_zones: Vec::new(),
        }
    }

    pub fn load_or_new(path: &PathBuf) -> Self {
        let (level, status) = if path.exists() {
            match fs::read_to_string(path) {
                Ok(s) => match level_from_ron(&s) {
                    Ok(mut l) => {
                        let st = format!("Loaded {}", path.display());
                        if let Err(e) = materialize_tile_defs_from_pack(&mut l, path.parent()) {
                            (
                                Self::default_level(),
                                format!("Terrain pack: {e}; new level"),
                            )
                        } else {
                            (l, st)
                        }
                    }
                    Err(e) => (
                        Self::default_level(),
                        format!("Parse error: {e}; new level"),
                    ),
                },
                Err(e) => (Self::default_level(), format!("Read error: {e}; new level")),
            }
        } else {
            (
                Self::default_level(),
                format!("New level ({} missing)", path.display()),
            )
        };
        let content = game_content::content_pack();
        let _ = content.validate();
        let mut status = status;
        if let Err(e) = content.validate_level(&level) {
            status.push_str(&format!(" | Check: {e}"));
        }
        let spawn_blueprint_idx = 0;
        let map_visual_seed = level
            .visual_seed
            .unwrap_or_else(|| derive_visual_seed(&level));
        let last_level_disk_fingerprint =
            FileFingerprint::from_path(path).unwrap_or(FileFingerprint::MISSING);
        let last_pack_disk_fingerprint = Editor::pack_fingerprint_for_path(path, &level);
        let mut ed = Self {
            path: path.clone(),
            level,
            content,
            current_tile: 0,
            last_terrain_tile_id: 0,
            spawn_blueprint_idx,
            last_entity_blueprint_idx: spawn_blueprint_idx,
            mode: Mode::PaintTiles,
            status,
            dialog: None,
            viewport_w: 80,
            viewport_h: 24,
            brush_radius: 0,
            brush_sparse_pct: 0,
            sparse_paint_drag_seen: HashSet::new(),
            paint_layer: PaintLayer::Ground,
            rect_drag_start: None,
            last_paint_cell: None,
            sidebar_hits: Vec::new(),
            hover_map_cell: None,
            view_origin_x: 0,
            view_origin_y: 0,
            last_mouse_cell: None,
            viewport_edge_scroll_cooldown: 0,
            level_map: None,
            atmosphere_bake: Vec::new(),
            map_visual_seed,
            surface_tick: 0,
            dirty: false,
            last_level_disk_fingerprint,
            last_pack_disk_fingerprint,
            last_hot_reload_poll: Instant::now(),
        };
        ed.rebuild_tile_display_full();
        ed.init_brush_memory_defaults();
        ed
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn apply_reloaded_level(&mut self, new_level: LevelFile) {
        self.level = new_level;
        self.map_visual_seed = self
            .level
            .visual_seed
            .unwrap_or_else(|| derive_visual_seed(&self.level));
        self.dirty = false;
        self.refresh_disk_fingerprint();
        if self.level.tile_defs.is_empty() {
            self.current_tile = 0;
        } else if !self
            .level
            .tile_defs
            .iter()
            .any(|d| d.idx == self.current_tile)
        {
            self.current_tile = self.level.tile_defs[0].idx;
        }
        self.sync_brush_memory_from_level();
        self.clamp_editor_view();
        self.rebuild_tile_display_full();
    }

    pub fn rebuild_tile_display_full(&mut self) {
        let Ok(mut m) = self.level.to_map() else {
            self.level_map = None;
            self.atmosphere_bake.clear();
            return;
        };
        m.rebuild_display_cache(self.map_visual_seed);
        tui_game_core::world::rebuild_atmosphere_bake(&m, &mut self.atmosphere_bake);
        self.level_map = Some(m);
    }

    pub fn ensure_level_props_len(&mut self) {
        let n = (self.level.width as usize) * (self.level.height as usize);
        if self.level.props.len() != n {
            self.level.props.resize(n, EMPTY_PROP_ID);
        }
    }

    /// Rows for [`SearchListPicker`]: `(tile_defs_index, display_line, search_blob)`.
    pub fn terrain_picker_entries(&self) -> Vec<(usize, String, String)> {
        self.level
            .tile_defs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let line = format!("{:>3}  {}  {}", i, d.glyph, d.description());
                let solid_txt = if d.solid() { "solid" } else { "open" };
                let hay = format!(
                    "{i} {} {} {} {} {} {}",
                    d.terrain_id,
                    d.name,
                    d.description,
                    d.glyph,
                    d.idx,
                    solid_txt
                );
                (i, line, hay)
            })
            .collect()
    }

    /// Rows for entity blueprint picker: `(blueprint_index, line, search_blob)`.
    pub fn entity_picker_entries(&self) -> Vec<(usize, String, String)> {
        self.content
            .entity_blueprints
            .iter()
            .enumerate()
            .map(|(i, bp)| {
                let line = format!("{}  {}  {}", bp.kind, bp.default_glyph, bp.display_name);
                let hay = format!(
                    "{} {} {} {} {}",
                    i, bp.kind, bp.display_name, bp.description, bp.default_glyph
                );
                (i, line, hay)
            })
            .collect()
    }

    pub fn open_terrain_picker(&mut self) {
        let mut picker = SearchListPicker::new(12, 48);
        picker.set_entries(self.terrain_picker_entries());
        self.dialog = Some(Dialog::PickTerrain { picker });
        self.status = "Pick terrain (type to filter).".into();
    }

    pub fn open_entity_picker(&mut self) {
        let mut picker = SearchListPicker::new(12, 48);
        picker.set_entries(self.entity_picker_entries());
        self.dialog = Some(Dialog::PickEntity { picker });
        self.status = "Pick entity to place (type to filter).".into();
    }

    pub fn current_tile_def(&self) -> Option<&TileDef> {
        self.level
            .tile_defs
            .iter()
            .find(|d| d.idx == self.current_tile)
    }

    pub fn current_spawn_blueprint(&self) -> Option<&'static EntityBlueprint> {
        self.content.entity_blueprints.get(self.spawn_blueprint_idx)
    }

    pub fn should_quit(&self) -> bool {
        self.status == "QUIT"
    }
}
