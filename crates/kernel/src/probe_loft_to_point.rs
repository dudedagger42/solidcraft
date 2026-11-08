//! PROBE (uncommitted): solidcraft#11216. Does the skin loft take a degenerate section (every
//! curve collapsed to the apex), giving a cone from a curved section to a point?
use solidcore_record::Vec3;
use solidcraft_smlib::sweeps::Curve;
use solidcraft_smlib::Brep;
use std::f64::consts::TAU;

fn apex_curve(apex: [f64; 3], degree: usize) -> Curve {
    // A degree-`degree` NURBS of degree + 1 control points, all at the apex, clamped knots.
    let n = degree + 1;
    let points = vec![[apex[0], apex[1], apex[2], 1.0]; n];
    let mut knots = vec![0.0; n];
    knots.extend(vec![1.0; n]);
    Curve::Nurbs { degree, points, knots }
}

#[test]
fn probe_loft_curved_section_to_a_point() {
    let apex = [0.0, 0.0, 30.0];
    let circle = Curve::Arc { center: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], reference: [1.0, 0.0, 0.0], radius: 20.0, start: 0.0, end: TAU };
    for degree in [1usize, 2, 3] {
        let r = Brep::loft(&[vec![circle.clone()], vec![apex_curve(apex, degree)]], 1, true);
        match &r {
            Ok(b) => eprintln!("PROBE circle -> apex (apex curve degree {degree}): ok faces {} volume {:.3} (cone 12566.371)", b.counts().unwrap()[0], b.volume(1e-7).unwrap_or(f64::NAN)),
            Err(e) => eprintln!("PROBE circle -> apex (apex curve degree {degree}): ERR {e:?}"),
        }
    }
    // A rounded square: two lines and two arcs, each paired with its own apex curve.
    let a = 10.0;
    let r = 5.0;
    let sec = vec![
        Curve::Line { start: [-a + r, -a, 0.0], end: [a - r, -a, 0.0] },
        Curve::Arc { center: [a - r, -a + r, 0.0], axis: [0.0, 0.0, 1.0], reference: [0.0, -1.0, 0.0], radius: r, start: 0.0, end: TAU / 4.0 },
        Curve::Line { start: [a, -a + r, 0.0], end: [a, a - r, 0.0] },
        Curve::Arc { center: [a - r, a - r, 0.0], axis: [0.0, 0.0, 1.0], reference: [1.0, 0.0, 0.0], radius: r, start: 0.0, end: TAU / 4.0 },
        Curve::Line { start: [a - r, a, 0.0], end: [-a + r, a, 0.0] },
        Curve::Arc { center: [-a + r, a - r, 0.0], axis: [0.0, 0.0, 1.0], reference: [0.0, 1.0, 0.0], radius: r, start: 0.0, end: TAU / 4.0 },
        Curve::Line { start: [-a, a - r, 0.0], end: [-a, -a + r, 0.0] },
        Curve::Arc { center: [-a + r, -a + r, 0.0], axis: [0.0, 0.0, 1.0], reference: [-1.0, 0.0, 0.0], radius: r, start: 0.0, end: TAU / 4.0 },
    ];
    let top: Vec<Curve> = sec.iter().map(|_| apex_curve(apex, 1)).collect();
    let r = Brep::loft(&[sec, top], 1, true);
    // The pyramid over the rounded square: area (400 - (4 - pi) 25) x 30 / 3.
    let area = 400.0 - (4.0 - std::f64::consts::PI) * 25.0;
    match &r {
        Ok(b) => eprintln!("PROBE rounded square -> apex: ok faces {} volume {:.3} (expected {:.3})", b.counts().unwrap()[0], b.volume(1e-7).unwrap_or(f64::NAN), area * 30.0 / 3.0),
        Err(e) => eprintln!("PROBE rounded square -> apex: ERR {e:?}"),
    }
    let _ = Vec3::ZERO;
}
