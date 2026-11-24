//! Probe for #21757 (scan round 12): a second emboss on a cylinder that already has one.
use solidcraft_kernel::*;
use solidcore_record::{Loop2, Plane, Region2, Vec2, Vec3};
use solidcraft_geom::PlaneGeom;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region2 {
    Region2 { outer: Loop2::polygon(&[Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]), holes: vec![] }
}

#[test]
fn probe_21757() {
    let body = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 20.0).unwrap();
    let plane = Plane::new(Vec3::Y * 10.0, -Vec3::X, Vec3::Z).unwrap();
    let v0 = body.volume_at(1e-6).unwrap();
    let first = rect(-2.0, 12.0, 2.0, 16.0);
    let second = rect(-2.0, 3.0, 2.0, 7.0);
    for depth in [1.0, -1.0] {
        let one = emboss_cylinder(&body, &plane, std::slice::from_ref(&first), depth).unwrap();
        let v1 = one.volume_at(1e-6).unwrap();
        let rec = one.record();
        eprintln!("depth {depth}: first ok, faces {} vol {v1:.6} (d {:.6})", one.face_count(), v1 - v0);
        for (i, f) in rec.faces.iter().enumerate() {
            eprintln!("  face {i}: loops {} uses {:?}", f.loops.len(), f.loops.iter().map(|l| format!("{l:?}").matches("edge:").count()).collect::<Vec<_>>());
        }
        let alone = emboss_cylinder(&body, &plane, std::slice::from_ref(&second), depth).map(|b| b.volume_at(1e-6).unwrap() - v0);
        eprintln!("depth {depth}: second alone on fresh: {alone:?}");
        let two = emboss_cylinder(&one, &plane, std::slice::from_ref(&second), depth).map(|b| (b.volume_at(1e-6).unwrap(), b.face_count()));
        eprintln!("depth {depth}: second on embossed: {two:?} (want vol {:.6})", v1 + alone.as_ref().copied().unwrap_or(f64::NAN));
        if let Ok(b2) = emboss_cylinder(&one, &plane, std::slice::from_ref(&second), depth) {
            eprintln!("depth {depth}: two valid: {:?}", b2.validity());
            let third = rect(-2.0, 0.5, 2.0, 2.0);
            let b3 = emboss_cylinder(&b2, &plane, std::slice::from_ref(&third), depth).map(|b| (b.volume_at(1e-6).unwrap() - b2.volume_at(1e-6).unwrap(), b.validity().len()));
            eprintln!("depth {depth}: third on twice-embossed: {b3:?}");
        }
        let overlap = rect(-1.0, 10.0, 1.0, 14.0);
        let ov = emboss_cylinder(&one, &plane, std::slice::from_ref(&overlap), depth).map(|b| b.volume_at(1e-6).unwrap() - v1);
        eprintln!("depth {depth}: overlapping second: {ov:?}");
        let inferred = infer_cylinder_emboss(&one, &plane, std::slice::from_ref(&second), depth);
        eprintln!("depth {depth}: infer on embossed: {inferred:?}");
    }
}
