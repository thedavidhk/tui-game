//! Map panel layout, view panning, and cursor clamping.

use tui_game_core::rect::Rect;
use tui_game_core::ui::viewport_scroll::{edge_scroll_pan_delta, EDGE_SCROLL_COOLDOWN_TICKS};

use super::Editor;
use super::EDITOR_SIDEBAR_WIDTH;

impl Editor {
    pub fn sidebar_screen_width(&self) -> u16 {
        EDITOR_SIDEBAR_WIDTH
            .min(self.viewport_w.saturating_sub(4))
            .max(10)
    }

    pub fn map_area_rect(&self) -> Rect {
        let sw = self.sidebar_screen_width();
        let mw = self.viewport_w.saturating_sub(sw).max(1);
        Rect::new(0, 0, mw, self.viewport_h)
    }

    pub fn sidebar_rect(&self) -> Rect {
        let map = self.map_area_rect();
        let sw = self.viewport_w.saturating_sub(map.w).max(1);
        Rect::new(map.right(), map.y, sw, map.h)
    }

    pub fn clamp_editor_view(&mut self) {
        let map = self.map_area_rect();
        let vw = map.w as i32;
        let vh = map.h as i32;
        let mw = self.level.width as i32;
        let mh = self.level.height as i32;
        let max_ox = (mw - vw).max(0);
        let max_oy = (mh - vh).max(0);
        self.view_origin_x = self.view_origin_x.clamp(0, max_ox);
        self.view_origin_y = self.view_origin_y.clamp(0, max_oy);
    }

    pub fn editor_map_needs_scroll(&self) -> bool {
        let map = self.map_area_rect();
        self.level.width as i32 > map.w as i32 || self.level.height as i32 > map.h as i32
    }

    /// Pan the map view so world cell `(cx, cy)` is visible (e.g. after placing something at the pointer).
    pub fn ensure_world_cell_visible(&mut self, cx: i32, cy: i32) {
        let map = self.map_area_rect();
        let vw = map.w as i32;
        let vh = map.h as i32;
        if cx < self.view_origin_x {
            self.view_origin_x = cx;
        }
        if cy < self.view_origin_y {
            self.view_origin_y = cy;
        }
        if cx >= self.view_origin_x + vw {
            self.view_origin_x = cx - vw + 1;
        }
        if cy >= self.view_origin_y + vh {
            self.view_origin_y = cy - vh + 1;
        }
        self.clamp_editor_view();
    }

    pub fn idle_viewport_tick(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        let map = self.map_area_rect();
        let Some(cell) = self.last_mouse_cell else {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        };
        if !map.contains(cell.x, cell.y) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if !self.editor_map_needs_scroll() {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        let lx = i32::from(cell.x.saturating_sub(map.x));
        let ly = i32::from(cell.y.saturating_sub(map.y));
        let (pdx, pdy) = edge_scroll_pan_delta(lx, ly, map.w, map.h);
        if (pdx, pdy) == (0, 0) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if self.viewport_edge_scroll_cooldown > 0 {
            self.viewport_edge_scroll_cooldown =
                self.viewport_edge_scroll_cooldown.saturating_sub(1);
            return;
        }
        self.view_origin_x += pdx;
        self.view_origin_y += pdy;
        self.clamp_editor_view();
        self.viewport_edge_scroll_cooldown = EDGE_SCROLL_COOLDOWN_TICKS;
    }
}
