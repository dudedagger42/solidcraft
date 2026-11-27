//! The part of a face an emboss offsets (#21347): a freeform face's offset is certified piece by
//! piece and never at a pole, so an emboss offsets only the part of its face under the profiles.

/// The outline, on the sketch plane, of the box an emboss cuts its face down to: the bounds of
/// the profiles' `points`, widened by `reach`, the farthest any point of the emboss's slab lies
/// from the face. Counter-clockwise from the lowest corner; `None` for no points, a point or a
/// reach that is not finite, or a negative reach.
#[must_use]
pub fn patch_outline(points: &[[f64; 2]], reach: f64) -> Option<[[f64; 2]; 4]> {
    if !(reach.is_finite() && reach >= 0.0) || points.iter().flatten().any(|c| !c.is_finite()) {
        return None;
    }
    let (first, rest) = points.split_first()?;
    let (mut lo, mut hi) = (*first, *first);
    for p in rest {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let (lo, hi) = (
        [lo[0] - reach, lo[1] - reach],
        [hi[0] + reach, hi[1] + reach],
    );
    Some([lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]])
}

#[cfg(test)]
mod tests {
    use super::patch_outline;

    #[test]
    fn the_outline_is_the_points_bounds_widened_by_the_reach() {
        let got = patch_outline(&[[-3.0, 8.0], [3.0, 12.0], [0.0, 10.0]], 1.5);
        assert_eq!(
            got,
            Some([[-4.5, 6.5], [4.5, 6.5], [4.5, 13.5], [-4.5, 13.5]])
        );
    }

    #[test]
    fn no_points_or_a_bad_reach_have_no_outline() {
        assert_eq!(patch_outline(&[], 1.0), None);
        assert_eq!(patch_outline(&[[0.0, 0.0]], -1.0), None);
        assert_eq!(patch_outline(&[[0.0, f64::NAN]], 1.0), None);
    }
}
