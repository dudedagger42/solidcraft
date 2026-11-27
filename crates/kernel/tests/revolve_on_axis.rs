//! Revolves of profiles with an edge on the axis that come back to it along a curve or a line
//! (#1853): a half slot about its centreline, a quarter disc and a bullet whose arc ends on the
//! axis, a drill point whose tip does. Their volumes are analytic, and a half slot revolved about
//! a centreline lying in a face of a plate, as a cut, takes what it should.

use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_geom::MeshGeom;
use solidcraft_kernel::{BoolOp, boolean, box_solid, measure, revolve};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const R: f64 = 3.0;

fn line(a: (f64, f64), b: (f64, f64)) -> Seg2 {
    Seg2::Line { a: Vec2::new(a.0, a.1), b: Vec2::new(b.0, b.1) }
}

/// A quarter of a circle of radius `R` about `c`, counter-clockwise from `start`.
fn quarter(c: (f64, f64), start: f64) -> Seg2 {
    Seg2::Arc { center: Vec2::new(c.0, c.1), radius: R, start, sweep: FRAC_PI_2 }
}

fn region(segs: Vec<Seg2>) -> Region2 {
    Region2 { outer: Loop2 { segs }, holes: vec![] }
}

/// Half a slot (radius `R`, centres 14 apart) cut along its centreline on the x axis.
fn half_slot() -> Region2 {
    region(vec![line((0.0, 0.0), (20.0, 0.0)), quarter((17.0, 0.0), 0.0), line((17.0, R), (3.0, R)), quarter((3.0, 0.0), FRAC_PI_2)])
}

#[test]
fn revolves_that_end_on_the_axis_along_a_curve_have_their_volume() {
    let sphere = 4.0 / 3.0 * PI * R.powi(3);
    let cylinder = PI * R * R * 14.0;
    let shapes = [
        ("half slot", half_slot(), cylinder + sphere),
        ("quarter disc", region(vec![line((-R, 0.0), (0.0, 0.0)), line((0.0, 0.0), (0.0, R)), quarter((0.0, 0.0), FRAC_PI_2)]), sphere / 2.0),
        (
            "bullet",
            region(vec![line((0.0, 0.0), (17.0, 0.0)), line((17.0, 0.0), (17.0, R)), line((17.0, R), (3.0, R)), quarter((3.0, 0.0), FRAC_PI_2)]),
            cylinder + sphere / 2.0,
        ),
        // A drill point: a line coming back to the axis (a full turn keeps truck's cone).
        (
            "drill point",
            region(vec![line((0.0, 0.0), (20.0, 0.0)), line((20.0, 0.0), (20.0, R)), line((20.0, R), (3.0, R)), line((3.0, R), (0.0, 0.0))]),
            PI * R * R * 17.0 + PI * R * R * 3.0 / 3.0,
        ),
    ];
    // The plane through the origin, and one moved and turned away from it.
    let planes = [Plane::XY, Plane { origin: Vec3::new(5.0, 7.0, 10.0), x: Vec3::Y, y: Vec3::Z }];
    for (name, r, full) in &shapes {
        for plane in &planes {
            for turn in [FRAC_PI_2, PI, -PI, TAU] {
                let body = revolve(plane, std::slice::from_ref(r), Vec2::ZERO, Vec2::new(1.0, 0.0), turn).unwrap().pop().unwrap();
                assert!(body.validity().is_empty(), "{name}, {turn}: {:?}", body.validity());
                let want = full * turn.abs() / TAU;
                let got = measure(&body).unwrap().volume;
                assert!((got - want).abs() < 1e-3 * want, "{name} turned {turn} on {:?}: {got} instead of {want}", plane.origin);
            }
        }
    }
}

#[test]
fn a_half_slot_revolved_about_a_centreline_in_a_face_cuts_it() {
    let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 20.0, 10.0)).unwrap();
    let whole = measure(&plate).unwrap().volume;
    // The half slot from x = 10 to 30 with its centreline on y = 10 of the top face; the plate
    // holds the half of the capsule below it.
    let inside = (PI * R * R * 14.0 + 4.0 / 3.0 * PI * R.powi(3)) / 2.0;
    let r = half_slot();
    // The sketch plane facing out of the plate and into it (its y running the other way).
    for flip in [1.0, -1.0] {
        let plane = Plane { origin: Vec3::new(10.0, 10.0, 10.0), x: Vec3::X, y: Vec3::Y * flip };
        for turn in [PI, -PI, TAU] {
            let tool = revolve(&plane, std::slice::from_ref(&r), Vec2::ZERO, Vec2::new(1.0, 0.0), turn).unwrap().pop().unwrap();
            // A half turn into the plate, or a full one, cuts a groove; a half turn out of it misses.
            let into = turn.abs() > PI || tool.mesh(&solidcraft_kernel::MeshTolerance::chord(0.01)).unwrap().bounds().min.z < 9.0;
            let cut = boolean(&plate, &tool, BoolOp::Cut).unwrap().unwrap();
            assert!(cut.validity().is_empty(), "flip {flip}, {turn}: {:?}", cut.validity());
            let removed = whole - measure(&cut).unwrap().volume;
            let want = if into { inside } else { 0.0 };
            assert!((removed - want).abs() < 1e-3 * inside, "flip {flip}, turned {turn}: removed {removed} instead of {want}");
        }
    }
}

#[test]
fn probe_22813_notch_on_cone_seam() {
    use solidcraft_kernel::cylinder;
    let section = [Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(0.5, 1000.0), Vec2::new(0.0, 1000.0)];
    let plane = Plane { origin: Vec3::ZERO, x: Vec3::X, y: Vec3::Z };
    let r = Region2 { outer: Loop2::polygon(&section), holes: vec![] };
    let cone = revolve(&plane, &[r], Vec2::ZERO, Vec2::Y, TAU).unwrap().pop().unwrap();
    let mut out = String::new();
    for d in [-1e-3, -2e-5, -1e-5, -5e-6, -1e-6, 0.0, 1e-5, 99.0] {
        let a = if d == 99.0 { FRAC_PI_2 } else { (0.06 + d) / 0.5 };
        let base = Vec3::new(0.5 * a.cos(), 0.5 * a.sin(), 999.5);
        let notch = cylinder(base, Vec3::Z, 0.06, 0.5).unwrap();
        let got = boolean(&cone, &notch, BoolOp::Cut).map(|b| b.map(|b| measure(&b).unwrap().volume));
        out += &format!("d {d:e}: {got:?}\n");
    }
    panic!("PROBE\n{out}");
}

#[test]
fn probe_22813_paths() {
    use solidcraft_kernel::cylinder;
    use solidcraft_smlib::sweeps::{Curve, DraftCorner};
    use solidcraft_smlib::{Brep, BooleanOp};
    let section = [Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(0.5, 1000.0), Vec2::new(0.0, 1000.0)];
    let plane = Plane { origin: Vec3::ZERO, x: Vec3::X, y: Vec3::Z };
    let r = Region2 { outer: Loop2::polygon(&section), holes: vec![] };
    let s3 = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.5, 0.0, 1000.0], [0.0, 0.0, 1000.0]];
    let profile: Vec<Curve> = (0..4).map(|i| Curve::Line { start: s3[i], end: s3[(i + 1) % 4] }).collect();
    let mut out = String::new();
    for d in [-1e-5, -5e-6, -2e-5] {
        let a: f64 = (0.06 + d) / 0.5;
        let base = Vec3::new(0.5 * a.cos(), 0.5 * a.sin(), 999.5);
        let cone = revolve(&plane, &[r.clone()], Vec2::ZERO, Vec2::Y, TAU).unwrap().pop().unwrap();
        let notch = cylinder(base, Vec3::Z, 0.06, 0.5).unwrap();
        let ok = |x: Result<Option<Brep>, solidcraft_smlib::Error>| x.map(|b| b.map(|b| b.volume(1e-9).unwrap())).map_err(|e| e.to_string()[..e.to_string().len().min(90)].to_string());
        let kept = ok(cone.smlib().unwrap().boolean_tagged(&notch.smlib().unwrap(), BooleanOp::Subtract));
        let rec_a = Brep::from_record(cone.record(), true).unwrap();
        let rec_b = Brep::from_record(notch.record(), true).unwrap();
        let rec = ok(rec_a.boolean(&rec_b, BooleanOp::Subtract));
        let rec_t = ok(rec_a.boolean_tagged(&rec_b, BooleanOp::Subtract));
        for (zdir, start) in [(-1.0, 0.6131), (1.0, 0.0)] {
            let nc = Brep::revolve(&profile, [0.0; 3], [0.0, 0.0, zdir], TAU, true).unwrap();
            let circle = [Curve::Arc { center: [base.x, base.y, 999.5], axis: [0.0, 0.0, 1.0], reference: [1.0, 0.0, 0.0], radius: 0.06, start, end: start + TAU }];
            let nn = Brep::extrude(&circle, [0.0, 0.0, 1.0], 0.5, 0.0, DraftCorner::Curvature, true).unwrap();
            out += &format!("d {d:e} native z{zdir}: plain {:?} tagged {:?}\n", ok(nc.boolean(&nn, BooleanOp::Subtract)), ok(nc.boolean_tagged(&nn, BooleanOp::Subtract)));
        }
        out += &format!("d {d:e}: kept-smlib tagged {kept:?}\nd {d:e}: from_record plain {rec:?} tagged {rec_t:?}\n");
    }
    panic!("PROBE\n{out}");
}
