//! The clearance a hole's tools need above an inclined face at its mouth.

use crate::math::Vec3;

/// The height above the plane across the hole's axis that the tools must reach so the mouth
/// opens fully: the lowest height at which none of eight points on the rim is still in material.
///
/// `dir` is the drilling direction (the tools run from above, along `-dir`), `spot` the hole's
/// place on the face. `contains(i, p)` says whether body `i` of `bodies` holds `p`, or `None` where
/// that is unsure. Only the bodies the drilling reaches (those holding a point just past `spot`
/// along `dir`) count. The search starts at `top` and doubles until the rim is in air, then
/// bisects to 0.1%. `None` where no answer is had (no body is drilled, the rim is unsure, or the
/// height passes `2 * size + 2 * radius`): the caller keeps its own reach.
pub fn hole_mouth_lip(
    bodies: usize,
    contains: &mut dyn FnMut(usize, Vec3) -> Option<bool>,
    spot: Vec3,
    dir: Vec3,
    radius: f64,
    top: f64,
    size: f64,
) -> Option<f64> {
    let up = dir * -1.0;
    let side = up
        .cross(if up.x.abs() < 0.9 { Vec3::X } else { Vec3::Y })
        .unitize_or_z();
    let other = up.cross(side);
    let probe = (size * 1e-5).max(1e-6);
    let deeper = spot + dir * probe;
    let drilled: Vec<usize> = (0..bodies)
        .filter(|&i| contains(i, deeper) == Some(true))
        .collect();
    if drilled.is_empty() {
        return None;
    }
    let mut in_material = |h: f64| -> Option<bool> {
        for k in 0..8 {
            let a = f64::from(k) * std::f64::consts::FRAC_PI_4;
            let p = spot + up * h + side * (radius * a.cos()) + other * (radius * a.sin());
            for &i in &drilled {
                if contains(i, p)? {
                    return Some(true);
                }
            }
        }
        Some(false)
    };
    if in_material(top) != Some(true) {
        return None;
    }
    let (mut lo, mut hi) = (top, top * 2.0);
    loop {
        if !in_material(hi)? {
            break;
        }
        lo = hi;
        hi *= 2.0;
        if hi > size * 2.0 + 2.0 * radius {
            return Some(lo);
        }
    }
    while hi - lo > 1e-3 * hi {
        let mid = 0.5 * (lo + hi);
        if in_material(mid)? {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(hi)
}

#[cfg(test)]
mod tests {
    use super::hole_mouth_lip;
    use crate::math::Vec3;

    /// A plate whose top face tilts up along +x at `tan(angle)`: material lies below
    /// `z = x * tan(angle)`. A hole drilled down from the face's origin must reach
    /// `radius * tan(angle)` above the plane through the spot, to clear the uphill rim.
    #[test]
    fn a_tilted_face_lifts_the_mouth_to_the_rim_height() {
        let (angle, radius) = (30f64.to_radians(), 3.0);
        let t = angle.tan();
        let mut contains = |_: usize, p: Vec3| Some(p.z < p.x * t);
        let lip = hole_mouth_lip(
            1,
            &mut contains,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            radius,
            0.1,
            20.0,
        )
        .expect("a lip");
        let want = radius * t;
        assert!((lip - want).abs() < 1e-3 * want, "lip {lip} vs {want}");
    }

    /// The same plate with its uphill direction 22.5° from +x, between two of the eight rim points.
    #[test]
    fn a_face_tilted_between_the_rim_points_lifts_the_mouth_to_the_rim_height() {
        let (angle, turn, radius) = (30f64.to_radians(), 22.5f64.to_radians(), 3.0);
        let t = angle.tan();
        let mut contains =
            |_: usize, p: Vec3| Some(p.z < (p.x * turn.cos() + p.y * turn.sin()) * t);
        let lip = hole_mouth_lip(
            1,
            &mut contains,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            radius,
            0.1,
            20.0,
        )
        .expect("a lip");
        let want = radius * t;
        assert!((lip - want).abs() < 1e-9 * want, "lip {lip} vs {want}");
    }

    /// A flat face: the rim is already in air at the start, so there is no lip to find.
    #[test]
    fn a_flat_face_has_no_lip() {
        let mut contains = |_: usize, p: Vec3| Some(p.z < 0.0);
        assert!(hole_mouth_lip(
            1,
            &mut contains,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            3.0,
            0.1,
            20.0
        )
        .is_none());
    }
}
