//! The clearance a hole's tools need above an inclined face at its mouth.

/// The height above the plane across the hole's axis that the tools must reach so the mouth
/// opens fully: the lowest height at which none of eight points on the rim (radius `radius`) is
/// still in material. `dir` is the drilling direction (the tools run from above, along `-dir`
/// into the part) and `spot` the hole's position on the face. `contains(i, p)` says whether body
/// `i` (of `bodies`) holds the point `p`: `Some(true)`/`Some(false)`, or `None` where the
/// classification is unsure. Only bodies the drilling reaches (those holding a point just past
/// `spot` along `dir`) count. `top` is where the search starts (doubled until the rim is in air,
/// then bisected to 0.1%); `None` where no answer can be had (no body is drilled, the rim is
/// unsure, or the height grows past `2 * size + 2 * radius`), and the caller keeps its own reach.
#[allow(clippy::too_many_arguments)]
pub fn hole_mouth_lip(
    bodies: usize,
    contains: &mut dyn FnMut(usize, [f64; 3]) -> Option<bool>,
    spot: [f64; 3],
    dir: [f64; 3],
    radius: f64,
    top: f64,
    size: f64,
) -> Option<f64> {
    let up = [-dir[0], -dir[1], -dir[2]];
    let side = normalized(cross(up, if up[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] })).unwrap_or([0.0, 0.0, 1.0]);
    let other = cross(up, side);
    let probe = (size * 1e-5).max(1e-6);
    let deeper = at(spot, dir, probe);
    let drilled: Vec<usize> = (0..bodies).filter(|&i| contains(i, deeper) == Some(true)).collect();
    if drilled.is_empty() {
        return None;
    }
    let mut in_material = |h: f64| -> Option<bool> {
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            let rim = at(at(spot, up, h), side, radius * a.cos());
            let p = at(rim, other, radius * a.sin());
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
        match in_material(hi)? {
            false => break,
            true => {
                lo = hi;
                hi *= 2.0;
                if hi > size * 2.0 + 2.0 * radius {
                    return Some(lo);
                }
            }
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

/// `p` moved `t` along `d`.
fn at(p: [f64; 3], d: [f64; 3], t: f64) -> [f64; 3] {
    [p[0] + d[0] * t, p[1] + d[1] * t, p[2] + d[2] * t]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalized(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (len > 0.0 && len.is_finite()).then(|| [v[0] / len, v[1] / len, v[2] / len])
}

#[cfg(test)]
mod tests {
    use super::hole_mouth_lip;

    /// A plate whose top face tilts up along +x at `tan(angle)`: material lies below
    /// `z = x * tan(angle)`. A hole drilled down from the origin of that face (`dir` = -z)
    /// must reach `radius * tan(angle)` above the face's plane through the spot, to clear the
    /// uphill rim.
    #[test]
    fn a_tilted_face_lifts_the_mouth_to_the_rim_height() {
        let (angle, radius) = (30f64.to_radians(), 3.0);
        let t = angle.tan();
        let mut contains = |_: usize, p: [f64; 3]| Some(p[2] < p[0] * t);
        let lip = hole_mouth_lip(1, &mut contains, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], radius, 0.1, 20.0).expect("a lip");
        let want = radius * t;
        assert!((lip - want).abs() < 1e-3 * want, "lip {lip} vs {want}");
    }

    /// A flat face: the rim is already in air at the start, so there is no lip to find.
    #[test]
    fn a_flat_face_has_no_lip() {
        let mut contains = |_: usize, p: [f64; 3]| Some(p[2] < 0.0);
        assert!(hole_mouth_lip(1, &mut contains, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], 3.0, 0.1, 20.0).is_none());
    }
}
