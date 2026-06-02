//! Interactive level editor: paint, spawns, atmosphere, save/hot-reload.
//!
//! Hot-reload: when the level `.ron` **or** the referenced `terrain_pack` file changes on disk, the
//! editor reloads when there are no unsaved edits; otherwise it prompts (Y discard + reload,
//! N/Esc keep). Reload only applies after RON parse and `validate_level` succeed; failures keep
//! the previous level and show an error.

mod atmosphere;
mod bootstrap;
mod brush_memory;
mod compose;
mod dialogs;
mod disk;
mod input_key;
mod input_mouse;
mod paint;
mod sidebar;
mod spawns;
mod step;
mod types;
mod viewport;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use tui_game_core::content::ContentPack;
use tui_game_core::input::MouseCell;
use tui_game_core::level::LevelFile;
use tui_game_core::ui::UiHitState;
use tui_game_core::world::{FogBakedTrio, MapGrid, TileId};

use disk::FileFingerprint;

pub use types::{Dialog, Mode, PaintLayer};

/// Fixed width for the right-hand palette / help column.
pub(crate) const EDITOR_SIDEBAR_WIDTH: u16 = 64;

pub(crate) const MAX_BRUSH_SIZE: u8 = 16;

pub struct Editor {
    path: PathBuf,
    level: LevelFile,
    content: ContentPack,
    current_tile: tui_game_core::world::TileId,
    /// Last non–prop-clear terrain brush (restored when returning to paint / ground layer).
    last_terrain_tile_id: TileId,
    /// Index into `content.entity_blueprints` when placing spawns.
    spawn_blueprint_idx: usize,
    /// Last chosen blueprint index (restored when returning to place mode).
    last_entity_blueprint_idx: usize,
    mode: Mode,
    status: String,
    dialog: Option<Dialog>,
    /// Last framebuffer size (updated in `compose`).
    viewport_w: u16,
    viewport_h: u16,
    /// Chebyshev brush radius in cells (0 = single cell).
    brush_radius: u8,
    /// Prop layer only: `0` or `100` = dense; `1..99` = each cell gets brush with probability p/100, else prop cleared.
    brush_sparse_pct: u8,
    /// Level cells already given a sparse roll during the current LMB paint stroke.
    sparse_paint_drag_seen: HashSet<(i32, i32)>,
    /// Ground vs prop overlay when [`Mode::PaintTiles`].
    paint_layer: PaintLayer,
    /// Shift–left drag rectangle: anchor corner until mouse up.
    rect_drag_start: Option<(i32, i32)>,
    last_paint_cell: Option<(i32, i32)>,
    /// Clickable regions from the last `compose` (sidebar rows + modal picker rows), picked on
    /// the next frame's mouse input. Shares the game shell's [`UiHitState`].
    ui_hits: UiHitState,
    /// Map cell under the mouse (level coords), when over the map and no modal dialog.
    hover_map_cell: Option<(i32, i32)>,
    /// Top-left level coordinate of the visible map window.
    view_origin_x: i32,
    view_origin_y: i32,
    last_mouse_cell: Option<MouseCell>,
    viewport_edge_scroll_cooldown: u16,
    /// Last `to_map` + `rebuild_display_cache` for compose and atmosphere.
    level_map: Option<MapGrid>,
    /// Baked fog colors per cell (parallel to tiles; rebuilt with tile display).
    atmosphere_bake: Vec<FogBakedTrio>,
    map_visual_seed: u64,
    surface_tick: u64,
    /// True after any edit not yet written with Ctrl+S / save dialog.
    dirty: bool,
    /// Last known on-disk fingerprint for the level `.ron`.
    last_level_disk_fingerprint: FileFingerprint,
    /// Fingerprint for the resolved `terrain_pack` file when non-empty.
    last_pack_disk_fingerprint: FileFingerprint,
    /// Rate-limit disk reads for hot-reload.
    last_hot_reload_poll: Instant,
}
