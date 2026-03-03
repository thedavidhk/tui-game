//! Viewport origin for tile maps: center on the player (or follow) plus optional pan,
//! with clamping so the view never leaves the map.

use crate::entity::GridPos;
use crate::rect::Rect;

/// Screen-cell margin from the world panel border that triggers edge scrolling.
pub const EDGE_MARGIN_CELLS: i32 = 2;

/// Ticks between edge-scroll origin steps while the pointer stays in the margin.
/// `0` allows one tile of pan per game tick (~60 Hz in `tui_game`).
pub const EDGE_SCROLL_COOLDOWN_TICKS: u16 = 0;

#[must_use]
pub fn map_larger_than_view(map_w: u16, map_h: u16, view_w: u16, view_h: u16) -> bool {
    map_w as i32 > view_w as i32 || map_h as i32 > view_h as i32
}

#[must_use]
pub fn clamp_origin(ox: i32, oy: i32, map_w: u16, map_h: u16, view_w: u16, view_h: u16) -> (i32, i32) {
    let mw = map_w as i32;
    let mh = map_h as i32;
    let vw = view_w as i32;
    let vh = view_h as i32;
    let max_ox = (mw - vw).max(0);
    let max_oy = (mh - vh).max(0);
    (ox.clamp(0, max_ox), oy.clamp(0, max_oy))
}

/// Top-left world tile for the view: player-centered plus pan offset, then clamped.
#[must_use]
pub fn world_view_origin(
    player: GridPos,
    pan: (i32, i32),
    map_w: u16,
    map_h: u16,
    view_w: u16,
    view_h: u16,
) -> (i32, i32) {
    let vw = view_w as i32;
    let vh = view_h as i32;
    let cx = player.x - vw / 2 + pan.0;
    let cy = player.y - vh / 2 + pan.1;
    clamp_origin(cx, cy, map_w, map_h, view_w, view_h)
}

/// Pan delta in world tiles (applied to view origin): positive X scrolls the map east
/// (origin moves right; content appears to shift left).
#[must_use]
pub fn edge_scroll_pan_delta(local_x: i32, local_y: i32, view_w: u16, view_h: u16) -> (i32, i32) {
    let vw = view_w as i32;
    let vh = view_h as i32;
    let m = EDGE_MARGIN_CELLS.max(1);
    let mut dx = 0;
    let mut dy = 0;
    if local_x < m {
        dx -= 1;
    } else if local_x >= vw - m {
        dx += 1;
    }
    if local_y < m {
        dy -= 1;
    } else if local_y >= vh - m {
        dy += 1;
    }
    (dx, dy)
}

#[must_use]
pub fn screen_cell_to_world(
    cell: crate::input::MouseCell,
    world_screen: Rect,
    origin: (i32, i32),
    map_w: u16,
    map_h: u16,
) -> Option<GridPos> {
    if !world_screen.contains(cell.x, cell.y) {
        return None;
    }
    let ox = origin.0;
    let oy = origin.1;
    let lx = i32::from(cell.x.saturating_sub(world_screen.x));
    let ly = i32::from(cell.y.saturating_sub(world_screen.y));
    let wx = ox + lx;
    let wy = oy + ly;
    if wx < 0 || wy < 0 || wx >= map_w as i32 || wy >= map_h as i32 {
        return None;
    }
    Some(GridPos { x: wx, y: wy })
}
