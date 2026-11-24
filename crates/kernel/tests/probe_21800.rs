//! Probe for #21800 (scan, not for PR): chamfer the four creases of a square-to-circle loft.
use solidcore_record::{Loop2, Plane, Vec2, Vec3};
use solidcraft_kernel::{chamfer, fillet, loft, measure};

fn body() -> solidcraft_kernel::Body {
    let sq = Loop2::polygon(&[Vec2::new(-20.0, -20.0), Vec2::new(20.0, -20.0), Vec2::new(20.0, 20.0), Vec2::new(-20.0, 20.0)]);
    loft(&[(Plane::XY, sq), (Plane { origin: Vec3::new(0.0, 0.0, 40.0), ..Plane::XY }, Loop2::circle(Vec2::ZERO, 10.0))]).unwrap()
}

fn creases(t: f64) -> Vec<Vec3> {
    let c = 10.0 / 2f64.sqrt();
    [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .iter()
        .map(|&(sx, sy)| Vec3::new(sx * (20.0 + (c - 20.0) * t), sy * (20.0 + (c - 20.0) * t), 40.0 * t))
        .collect()
}

#[test]
fn probe_21800() {
    let b = body();
    let v0 = measure(&b).unwrap().volume;
    eprintln!("P21800 loft: faces {} valid {:?} vol {v0}", b.face_count(), b.validity());
    for d in [0.5, 1.0, 2.0] {
        match chamfer(&b, &creases(0.5), d) {
            Ok(c) => eprintln!("P21800 chamfer4 d {d}: OK faces {} valid {:?} removed {}", c.face_count(), c.validity(), v0 - measure(&c).unwrap().volume),
            Err(e) => eprintln!("P21800 chamfer4 d {d}: ERR {e}"),
        }
        match chamfer(&b, &creases(0.5)[..1], d) {
            Ok(c) => eprintln!("P21800 chamfer1 d {d}: OK faces {} valid {:?} removed {}", c.face_count(), c.validity(), v0 - measure(&c).unwrap().volume),
            Err(e) => eprintln!("P21800 chamfer1 d {d}: ERR {e}"),
        }
    }
    match fillet(&b, &creases(0.5), 1.0) {
        Ok(c) => eprintln!("P21800 fillet4 r 1: OK faces {} valid {:?}", c.face_count(), c.validity()),
        Err(e) => eprintln!("P21800 fillet4 r 1: ERR {e}"),
    }
}
