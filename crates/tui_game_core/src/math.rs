//! Small grid and RNG helpers shared across simulation, AI, and presentation.
//!
//! Keeps distance metrics and the deterministic LCG in one place so combat, spells,
//! pathfinding heuristics, and UI brushes stay consistent.
//!
//! ## Terminal tile aspect (`TILE_Y_PER_X`)
//!
//! TUI cells are typically ~twice as tall as they are wide. Set [`TILE_Y_PER_X`] to `2` so
//! Euclidean ranges, FoW, movement AP, path costs, and step pacing match on-screen proportions.
//! Set to **`1`** to disable (square tiles / future sprite renderer) — no other code changes.

use crate::entity::GridPos;

// ── Tile aspect (single knob) ─────────────────────────────────────────────────

/// Vertical grid steps count as this many units per horizontal step (`1` = disabled).
pub const TILE_Y_PER_X: i32 = 2;

/// Whether [`TILE_Y_PER_X`] is correcting for non-square terminal cells.
#[must_use]
#[inline]
pub const fn tile_aspect_enabled() -> bool {
    TILE_Y_PER_X > 1
}

#[inline]
fn scale_dy_i64(dy: i32) -> i64 {
    i64::from(dy) * i64::from(TILE_Y_PER_X)
}

/// Integer square root for `u64` (used for aspect-aware step costs).
#[must_use]
pub fn isqrt_u64(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ── Distances ─────────────────────────────────────────────────────────────────

/// Chebyshev (square / L∞) distance from component deltas.
#[must_use]
#[inline]
pub const fn chebyshev_dist(dx: i32, dy: i32) -> i32 {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax > ay { ax } else { ay }
}

/// Chebyshev distance between two grid cells (includes diagonals at cost 1).
#[must_use]
#[inline]
pub fn chebyshev(a: GridPos, b: GridPos) -> i32 {
    chebyshev_dist(a.x - b.x, a.y - b.y)
}

/// Manhattan (L1) distance between two grid cells.
#[must_use]
#[inline]
pub fn manhattan(a: GridPos, b: GridPos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Squared Euclidean distance from component deltas, with Y scaled by [`TILE_Y_PER_X`].
///
/// Compare to `radius * radius` instead of taking a square root when testing inclusion.
#[must_use]
#[inline]
pub fn euclidean_dist_sq(dx: i32, dy: i32) -> i64 {
    let dx = i64::from(dx);
    let dy = scale_dy_i64(dy);
    dx * dx + dy * dy
}

/// Squared Euclidean distance between two grid cells.
#[must_use]
#[inline]
pub fn euclidean_dist_sq_between(a: GridPos, b: GridPos) -> i64 {
    euclidean_dist_sq(a.x - b.x, a.y - b.y)
}

/// Squared Euclidean distance between two `(x, y)` keys (pathfinding, etc.).
#[must_use]
#[inline]
pub fn euclidean_dist_sq_coords(a: (i32, i32), b: (i32, i32)) -> i64 {
    euclidean_dist_sq(a.0 - b.0, a.1 - b.1)
}

/// True when the aspect-corrected Euclidean distance from `a` to `b` is at most `radius`.
#[must_use]
#[inline]
pub fn within_euclidean_radius(a: GridPos, b: GridPos, radius: i32) -> bool {
    let r = i64::from(radius.max(0));
    euclidean_dist_sq_between(a, b) <= r * r
}

// ── Movement costs (one grid step) ────────────────────────────────────────────

/// AP / path cost for a single step `(dx, dy)` in `{-1,0,1}`, scaled by `orthogonal_base`.
///
/// With `TILE_Y_PER_X = 1`: orthogonal = base, diagonal ≈ `1.4 × base` (classic 10 / 14).
/// With `TILE_Y_PER_X = 2`: vertical steps cost `2 × base`; diagonal derived from scaled distance.
#[must_use]
pub fn grid_step_cost_units(dx: i32, dy: i32, orthogonal_base: u16) -> Option<u16> {
    let adx = dx.abs();
    let ady = dy.abs();
    if adx == 0 && ady == 0 {
        return None;
    }
    if adx > 1 || ady > 1 {
        return None;
    }
    let dist_sq = u64::try_from(euclidean_dist_sq(dx, dy)).ok()?;
    let base = u64::from(orthogonal_base);
    let cost_sq = dist_sq.saturating_mul(base).saturating_mul(base);
    let cost = isqrt_u64(cost_sq);
    u16::try_from(cost).ok()
}

/// Multiplier for step pacing when `TILE_Y_PER_X > 1` (1 = horizontal, 2 = vertical, etc.).
#[must_use]
pub fn grid_step_pace_multiplier(dx: i32, dy: i32) -> u16 {
    grid_step_cost_units(dx, dy, 1).unwrap_or(1)
}

// ── RNG ───────────────────────────────────────────────────────────────────────

/// Advance the game-wide deterministic LCG and return the high 32 bits.
///
/// Same stream is used for combat rolls, spell damage variance, and NPC roam picks.
pub fn lcg_next_u32(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    (*seed >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chebyshev_dist_matches_grid_helper() {
        let a = GridPos { x: 0, y: 0 };
        let b = GridPos { x: 3, y: 4 };
        assert_eq!(chebyshev(a, b), chebyshev_dist(3, 4));
        assert_eq!(chebyshev_dist(3, 4), 4);
    }

    #[test]
    fn manhattan_diagonal_is_two() {
        let a = GridPos { x: 0, y: 0 };
        let b = GridPos { x: 1, y: 1 };
        assert_eq!(manhattan(a, b), 2);
    }

    #[test]
    fn euclidean_radius_respects_tile_aspect() {
        let origin = GridPos { x: 0, y: 0 };
        if tile_aspect_enabled() {
            // Tall ellipse on screen: fewer raw Y tiles within range 8.
            assert!(!within_euclidean_radius(origin, GridPos { x: 6, y: 6 }, 8));
            assert!(within_euclidean_radius(origin, GridPos { x: 8, y: 0 }, 8));
            assert!(!within_euclidean_radius(origin, GridPos { x: 0, y: 5 }, 8));
            assert!(within_euclidean_radius(origin, GridPos { x: 0, y: 4 }, 8));
        } else {
            assert!(!within_euclidean_radius(origin, GridPos { x: 6, y: 6 }, 8));
            assert!(within_euclidean_radius(origin, GridPos { x: 8, y: 0 }, 8));
        }
    }

    #[test]
    fn grid_step_cost_matches_classic_when_aspect_disabled() {
        if tile_aspect_enabled() {
            assert_eq!(grid_step_cost_units(1, 0, 10), Some(10));
            assert_eq!(grid_step_cost_units(0, 1, 10), Some(20));
            assert_eq!(grid_step_cost_units(1, 1, 10), Some(22));
        } else {
            assert_eq!(grid_step_cost_units(1, 0, 10), Some(10));
            assert_eq!(grid_step_cost_units(0, 1, 10), Some(10));
            assert_eq!(grid_step_cost_units(1, 1, 10), Some(14));
        }
    }

    #[test]
    fn lcg_is_deterministic() {
        let mut s = 42_u64;
        let a = lcg_next_u32(&mut s);
        let mut s2 = 42_u64;
        let b = lcg_next_u32(&mut s2);
        assert_eq!(a, b);
        assert_ne!(a, lcg_next_u32(&mut s));
    }
}
