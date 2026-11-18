//! PROBE #11994 (throwaway): the hole drilled at angle `th` about Z, so the outer cylinder's
//! seam (angle 0) lies under the x>0 rim (th 0), far from both rims (th 90), or 6 deg off the
//! x>0 rim's top as on the reporter's body (th -35.1).

use solidcraft_kernel::{BoolOp, boolean, chamfer, cylinder, measure};
use solidcore_record::Vec3;

fn rot(p: Vec3, a: f64) -> Vec3 {
    let (c, s) = (a.cos(), a.sin());
    Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z)
}

#[test]
fn probe_seam_position() {
    for th in [90.0f64, -35.1, 0.0] {
        let a = th.to_radians();
        let body = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 20.0).unwrap();
        let tool = cylinder(rot(Vec3::new(-15.0, 0.2, 11.8), a), rot(Vec3::X, a), 4.669, 30.0).unwrap();
        let drilled = boolean(&body, &tool, BoolOp::Cut).unwrap().unwrap();
        let v0 = measure(&drilled).unwrap().volume;
        let m = measure(&drilled).unwrap();
        eprintln!("SEAM th={th} faces {} edges {} vertices {} volume {v0}", m.faces, m.edges, m.vertices);
        for d in [0.5, 1.0, 1.5, 2.0] {
            for (rim, p) in [("x>0", Vec3::new(9.998, 0.2, 7.131)), ("x<0", Vec3::new(-9.998, 0.2, 7.131))] {
                let out = match chamfer(&drilled, &[rot(p, a)], d) {
                    Ok(b) => format!("ok removed {:.9}", v0 - measure(&b).unwrap().volume),
                    Err(e) => format!("ERR {e}"),
                };
                eprintln!("SEAM th={th} d={d} {rim}: {out}");
            }
        }
    }
}
