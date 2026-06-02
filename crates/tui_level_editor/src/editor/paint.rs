//! Ground / prop painting, sparse brush, and rectangle fill.

use tui_game_core::ui::{for_each_in_brush, for_each_in_rect};
use tui_game_core::world::{mix64, TileId, EMPTY_PROP_ID};

use super::{Editor, Mode, PaintLayer};

impl Editor {
    pub fn set_ground_clamped(&mut self, tx: i32, ty: i32, tile: TileId) {
        let w = self.level.width as i32;
        let h = self.level.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            return;
        }
        let i = ty as usize * self.level.width as usize + tx as usize;
        if i < self.level.tiles.len() {
            self.level.tiles[i] = tile;
            self.mark_dirty();
        }
    }

    pub fn set_prop_clamped(&mut self, tx: i32, ty: i32, tile: TileId) {
        let w = self.level.width as i32;
        let h = self.level.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            return;
        }
        self.ensure_level_props_len();
        let i = ty as usize * self.level.width as usize + tx as usize;
        if i < self.level.props.len() {
            self.level.props[i] = tile;
            self.mark_dirty();
        }
    }

    /// Prop layer: with probability `brush_sparse_pct/100` set brush tile, else clear prop ([`EMPTY_PROP_ID`]).
    pub fn apply_sparse_paint_to_cells(
        &mut self,
        label: &str,
        cells: Vec<(i32, i32)>,
        drag_session_dedupe: bool,
    ) {
        if cells.is_empty() {
            return;
        }
        let brush_tid = self.current_tile;
        let p_base = (self.brush_sparse_pct as f32 / 100.0).clamp(0.0, 1.0);
        let mut seed = mix64(
            self.map_visual_seed
                ^ self.surface_tick.wrapping_mul(0x9E3779B185EBCA87)
                ^ mix64(cells.len() as u64),
        );
        for (tx, ty) in cells {
            if drag_session_dedupe && !self.sparse_paint_drag_seen.insert((tx, ty)) {
                continue;
            }
            seed = mix64(seed ^ (tx as u64).rotate_left(3) ^ (ty as u64).rotate_left(19));
            let roll = ((seed >> 16) & 0xFFFFFF) as f32 / 16_777_215.0_f32;
            if roll < p_base {
                self.set_prop_clamped(tx, ty, brush_tid);
            } else {
                self.set_prop_clamped(tx, ty, EMPTY_PROP_ID);
            }
        }
        self.status = format!(
            "{label}Prop sparse {}% (p→brush, else clear){}",
            self.brush_sparse_pct,
            if drag_session_dedupe {
                " · drag: each cell once"
            } else {
                ""
            }
        );
    }

    pub fn apply_sparse_paint_brush(&mut self, cx: i32, cy: i32, drag_session_dedupe: bool) {
        let mut cells = Vec::new();
        for_each_in_brush(cx, cy, self.brush_radius, |tx, ty| {
            if tx >= 0
                && ty >= 0
                && (tx as u16) < self.level.width
                && (ty as u16) < self.level.height
            {
                cells.push((tx, ty));
            }
        });
        self.apply_sparse_paint_to_cells(
            &format!("Paint @({cx},{cy}) r{}. ", self.brush_radius),
            cells,
            drag_session_dedupe,
        );
    }

    pub fn apply_paint_brush(&mut self, cx: i32, cy: i32, drag_session_dedupe: bool) {
        match self.paint_layer {
            PaintLayer::Ground => {
                let t = self.current_tile;
                for_each_in_brush(cx, cy, self.brush_radius, |tx, ty| {
                    self.set_ground_clamped(tx, ty, t);
                });
                self.mark_dirty();
            }
            PaintLayer::Prop => {
                if self.brush_sparse_pct == 0 || self.brush_sparse_pct >= 100 {
                    let t = self.current_tile;
                    for_each_in_brush(cx, cy, self.brush_radius, |tx, ty| {
                        self.set_prop_clamped(tx, ty, t);
                    });
                    self.mark_dirty();
                } else {
                    self.apply_sparse_paint_brush(cx, cy, drag_session_dedupe);
                }
            }
        }
        self.rebuild_tile_display_full();
    }

    pub fn set_status_after_dense_paint(&mut self, tx: i32, ty: i32, drag: bool) {
        if self.paint_layer == PaintLayer::Prop
            && self.brush_sparse_pct > 0
            && self.brush_sparse_pct < 100
        {
            return;
        }
        self.status = if drag {
            format!("Paint drag ({tx},{ty}).")
        } else {
            format!("Paint at ({tx},{ty}) r{}.", self.brush_radius)
        };
    }

    pub fn fill_rect_tiles(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        if self.mode != Mode::PaintTiles {
            return;
        }
        match self.paint_layer {
            PaintLayer::Ground => {
                let t = self.current_tile;
                for_each_in_rect(x0, y0, x1, y1, |tx, ty| {
                    self.set_ground_clamped(tx, ty, t);
                });
                self.mark_dirty();
                self.status = format!("Filled ground ({x0},{y0})—({x1},{y1}).");
            }
            PaintLayer::Prop => {
                if self.brush_sparse_pct == 0 || self.brush_sparse_pct >= 100 {
                    let t = self.current_tile;
                    for_each_in_rect(x0, y0, x1, y1, |tx, ty| {
                        self.set_prop_clamped(tx, ty, t);
                    });
                    self.mark_dirty();
                    self.status = format!("Filled props ({x0},{y0})—({x1},{y1}).");
                } else {
                    let mut cells = Vec::new();
                    for_each_in_rect(x0, y0, x1, y1, |tx, ty| {
                        if tx >= 0
                            && ty >= 0
                            && (tx as u16) < self.level.width
                            && (ty as u16) < self.level.height
                        {
                            cells.push((tx, ty));
                        }
                    });
                    self.apply_sparse_paint_to_cells(
                        &format!("Rect fill props ({x0},{y0})—({x1},{y1}). "),
                        cells,
                        false,
                    );
                }
            }
        }
        self.rebuild_tile_display_full();
    }
}
