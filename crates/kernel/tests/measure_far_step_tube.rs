//! #12760: a tube far from the origin measures the same after a STEP round trip.

use solidcraft_geom::prelude::*;
use solidcraft_kernel::{BoolOp, boolean, cylinder, measure_mass, step_export, step_import};

fn tube(base: Vec3) -> solidcraft_kernel::Body {
    let end = Vec3::new(9079.496433, 447.0, 6471.008415) - Vec3::new(4805.058271, 447.0, 3740.849768);
    let (axis, len) = (end.normalize(), end.length());
    let outer = cylinder(base, axis, 21.2, len).unwrap();
    let inner = cylinder(base - axis, axis, 18.6, len + 2.0).unwrap();
    boolean(&outer, &inner, BoolOp::Cut).unwrap().unwrap()
}

#[test]
fn a_tube_far_from_the_origin_measures_the_same_after_a_step_round_trip() {
    let mut bad = Vec::new();
    for base in [Vec3::new(0.0, 447.0, 0.0), Vec3::new(4805.058271, 447.0, 3740.849768)] {
        let b = tube(base);
        let direct = measure_mass(&b).unwrap();
        let text = step_export(&[&b], "test").unwrap();
        let back = step_import(&text).unwrap();
        let again = measure_mass(&back.bodies[0].body).unwrap();
        eprintln!("base {base:?}: direct {} area {} c {:?}; imported {} area {} c {:?}", direct.volume, direct.area, direct.centroid, again.volume, again.area, again.centroid);
        if (again.volume - direct.volume).abs() > 1e-6 * direct.volume {
            bad.push(format!("base {base:?}: {} -> {}", direct.volume, again.volume));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
