//! Small grid and RNG helpers shared across simulation, AI, and presentation.
//!
//! Keeps distance metrics and the deterministic LCG in one place so combat, spells,
//! pathfinding heuristics, and UI brushes stay consistent.

use crate::entity::GridPos;

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

/// Squared Euclidean distance from component deltas (`i64` avoids overflow on large maps).
///
/// Compare to `radius * radius` instead of taking a square root when testing inclusion.
#[must_use]
#[inline]
pub fn euclidean_dist_sq(dx: i32, dy: i32) -> i64 {
    let dx = i64::from(dx);
    let dy = i64::from(dy);
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

/// True when the Euclidean distance from `a` to `b` is at most `radius` (inclusive).
#[must_use]
#[inline]
pub fn within_euclidean_radius(a: GridPos, b: GridPos, radius: i32) -> bool {
    let r = i64::from(radius.max(0));
    euclidean_dist_sq_between(a, b) <= r * r
}

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
    fn euclidean_radius_is_circular_not_square() {
        let origin = GridPos { x: 0, y: 0 };
        // Chebyshev 8 reaches (6, 6); Euclidean range 8 does not (~8.49 away).
        assert!(!within_euclidean_radius(origin, GridPos { x: 6, y: 6 }, 8));
        assert!(within_euclidean_radius(origin, GridPos { x: 8, y: 0 }, 8));
        assert_eq!(euclidean_dist_sq(3, 4), 25);
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
