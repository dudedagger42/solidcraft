//! The clearance a hole's tools need above an inclined face, on plain numbers
//! ([`solidcore::query::hole_mouth`]).

use solidcore::geom::vector3d::Vec3;
use solidcore::query::hole_mouth as hm;

fn vec(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// The height the tools must reach above a hole's mouth, or `None` to keep the caller's reach.
pub fn hole_mouth_lip(
    bodies: usize,
    contains: &mut dyn FnMut(usize, [f64; 3]) -> Option<bool>,
    spot: [f64; 3],
    dir: [f64; 3],
    radius: f64,
    top: f64,
    size: f64,
) -> Option<f64> {
    hm::hole_mouth_lip(
        bodies,
        &mut |i, p: Vec3| contains(i, [p.x, p.y, p.z]),
        vec(spot),
        vec(dir),
        radius,
        top,
        size,
    )
}
