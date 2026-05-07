//! Spatial smoothing of discrete fog-of-war luminance for softer color transitions.

/// Chebyshev (square) neighborhood radius, in **tiles**, for fog color smoothing.
///
/// Change this single constant to tune how far visible / explored / void colors bleed
/// across category boundaries (larger = softer edges).
pub const FOG_COLOR_SOFTEN_RADIUS_CHEBYSHEV: i32 = 1;

/// Discrete luminance for explored-but-not-visible cells (`fog_luminance_hard`); matches bake midpoint.
pub const FOG_LUMINANCE_EXPLORED: f32 = 0.5;

#[inline]
const fn chebyshev_distance(dx: i32, dy: i32) -> i32 {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax > ay {
        ax
    } else {
        ay
    }
}

/// Tent kernel weight: full weight at the center, zero outside `radius`.
#[inline]
#[allow(clippy::cast_precision_loss)] // small integer kernel weights
fn kernel_weight(radius: i32, chebyshev_d: i32) -> f32 {
    if chebyshev_d > radius || chebyshev_d < 0 {
        return 0.0;
    }
    f32::from((radius + 1 - chebyshev_d) as i16)
}

/// Discrete fog "luminance" used before spatial smoothing: unseen `0`, explored [`FOG_LUMINANCE_EXPLORED`], visible `1`.
#[inline]
pub fn fog_luminance_hard(seen: bool, visible: bool) -> f32 {
    if !seen {
        0.0
    } else if visible {
        1.0
    } else {
        FOG_LUMINANCE_EXPLORED
    }
}

/// Neighbor-weighted average of [`fog_luminance_hard`] over a Chebyshev disk of radius
/// [`FOG_COLOR_SOFTEN_RADIUS_CHEBYSHEV`].
#[must_use]
pub fn smooth_fog_luminance(
    map_width: u16,
    map_height: u16,
    explored: &[bool],
    visible: &[bool],
    wx: i32,
    wy: i32,
) -> f32 {
    let w = i32::from(map_width);
    let h = i32::from(map_height);
    let r = FOG_COLOR_SOFTEN_RADIUS_CHEBYSHEV;
    let mut acc = 0.0_f32;
    let mut wsum = 0.0_f32;
    for dy in -r..=r {
        for dx in -r..=r {
            let d = chebyshev_distance(dx, dy);
            let kw = kernel_weight(r, d);
            if kw <= 0.0 {
                continue;
            }
            let nx = wx + dx;
            let ny = wy + dy;
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let nx_u = u32::try_from(nx).expect("bounds-checked");
            let ny_u = u32::try_from(ny).expect("bounds-checked");
            let row = usize::try_from(ny_u).expect("row index fits usize");
            let col = usize::try_from(nx_u).expect("col index fits usize");
            let idx = row
                .saturating_mul(usize::from(map_width))
                .saturating_add(col);
            let seen = explored.get(idx).copied().unwrap_or(false);
            let vis = visible.get(idx).copied().unwrap_or(false);
            acc += fog_luminance_hard(seen, vis) * kw;
            wsum += kw;
        }
    }
    if wsum > 0.0 {
        acc / wsum
    } else if wx < 0 || wy < 0 || wx >= w || wy >= h {
        fog_luminance_hard(false, false)
    } else {
        let nx_u = u32::try_from(wx).expect("bounds-checked");
        let ny_u = u32::try_from(wy).expect("bounds-checked");
        let row = usize::try_from(ny_u).expect("row index fits usize");
        let col = usize::try_from(nx_u).expect("col index fits usize");
        let idx = row
            .saturating_mul(usize::from(map_width))
            .saturating_add(col);
        fog_luminance_hard(
            explored.get(idx).copied().unwrap_or(false),
            visible.get(idx).copied().unwrap_or(false),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // literal constants

    use super::*;

    #[test]
    fn hard_luminance_tri_state() {
        assert_eq!(fog_luminance_hard(false, false), 0.0);
        assert_eq!(fog_luminance_hard(true, false), FOG_LUMINANCE_EXPLORED);
        assert_eq!(fog_luminance_hard(true, true), 1.0);
    }

    #[test]
    fn smooth_uniform_visible_matches_hard() {
        let w = 5_u16;
        let h = 5_u16;
        let n = usize::from(w) * usize::from(h);
        let explored = vec![true; n];
        let visible = vec![true; n];
        let l = smooth_fog_luminance(w, h, &explored, &visible, 2, 2);
        assert!((l - 1.0).abs() < 1e-5, "got {l}");
    }

    #[test]
    fn smooth_pulls_visible_toward_explored_near_boundary() {
        let w = 7_u16;
        let h = 1_u16;
        let n = usize::from(w);
        let explored = vec![true; n];
        let mut visible = vec![false; n];
        visible[3] = true;
        // Cell 4 is explored-not-visible but sits next to visible — smoothed l < 1.
        let l = smooth_fog_luminance(w, h, &explored, &visible, 4, 0);
        assert!(l < 1.0 && l > 0.5, "expected between explored and visible, got {l}");
    }
}
