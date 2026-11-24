//! Probe for #21721 (scan round 11): Offset Face on the outer wall of a thin ring.

use solidcraft_kernel::*;
use solidcore_record::Vec3;
use std::f64::consts::PI;

fn ring(h: f64) -> Body {
    let outer = cylinder(Vec3::ZERO, Vec3::Z, 50.0, h).unwrap();
    let bore = cylinder(Vec3::new(0.0, 0.0, -1.0), Vec3::Z, 25.0, h + 2.0).unwrap();
    boolean(&outer, &bore, BoolOp::Cut).unwrap().unwrap()
}

fn vol(b: &Body) -> f64 {
    b.volume_at(1e-6).unwrap()
}

#[test]
fn probe_21721() {
    for h in [0.1, 0.2, 0.5, 1.0, 5.0] {
        let b = ring(h);
        let outer_at = Vec3::new(50.0, 0.0, h / 2.0);
        let inner_at = Vec3::new(-25.0, 0.0, h / 2.0);
        let picked = b.face_at_point(outer_at).unwrap();
        let d = 0.45;
        let right_outer = PI * ((50.0 + d) * (50.0 + d) - 25.0 * 25.0) * h;
        let wrong_bottom = PI * (50.0 * 50.0 - 25.0 * 25.0) * (h + d);
        let r = offset_faces(&b, &[outer_at], d).map(|r| vol(&r));
        eprintln!("h={h} faces={} outer pick face={picked:?} size={} vol0={:.4} outer->{r:?} want {right_outer:.4} (bottom-moved {wrong_bottom:.4})", b.face_count(), b.size(), vol(&b));
        let right_inner = PI * (50.0 * 50.0 - (25.0 - d) * (25.0 - d)) * h;
        let r = offset_faces(&b, &[inner_at], d).map(|r| vol(&r));
        eprintln!("h={h} inner->{r:?} want {right_inner:.4}");
        let right_both = PI * ((50.0 + d) * (50.0 + d) - (25.0 - d) * (25.0 - d)) * h;
        let r = offset_faces(&b, &[outer_at, inner_at], d).map(|r| vol(&r));
        eprintln!("h={h} both->{r:?} want {right_both:.4}");
    }
}
