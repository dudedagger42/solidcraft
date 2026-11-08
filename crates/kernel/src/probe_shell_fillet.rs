//! PROBE (uncommitted): solidcraft#8287. A loft circle -> square, the square end's edges
//! filleted, then shelled: rc.3 rejects the shell (SM_ERR_INVALID_INPUT); unfilleted it shells.
use solidcore_record::{Loop2, Plane, Vec2, Vec3};

fn sections() -> [(Plane, Loop2); 2] {
    let top = Plane { origin: Vec3::new(0.0, 0.0, 40.0), x: Vec3::X, y: Vec3::Y };
    let h = 15.0;
    [
        (Plane::XY, Loop2::circle(Vec2::ZERO, 20.0)),
        (top, Loop2::polygon(&[Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)])),
    ]
}

fn say(label: &str, r: &crate::Result<crate::Body>) {
    match r {
        Ok(b) => eprintln!("PROBE {label}: ok faces {} volume {:.3}", b.face_count(), crate::measure(b).map(|m| m.volume).unwrap_or(f64::NAN)),
        Err(e) => eprintln!("PROBE {label}: ERR {e}"),
    }
}


/// Each face's record surface: kind, and for splines the degrees and the interior knot
/// multiplicities in u and v (a multiplicity >= degree is a C0 joint, a crease unless proved smooth).
fn dump(label: &str, b: &crate::Body) {
    use solidcore_record::brep::Surface;
    fn mults(knots: &[f64], n_points: usize) -> (usize, Vec<usize>) {
        let degree = knots.len().saturating_sub(n_points + 1);
        let (lo, hi) = (knots[degree], knots[knots.len() - 1 - degree]);
        let mut out = Vec::new();
        let mut i = 0;
        while i < knots.len() {
            let k = knots[i];
            let mut m = 1;
            while i + m < knots.len() && knots[i + m] == k {
                m += 1;
            }
            if k > lo && k < hi {
                out.push(m);
            }
            i += m;
        }
        (degree, out)
    }
    for (i, f) in b.record().faces.iter().enumerate() {
        let desc = match &f.surface {
            Surface::Spline { u_knots, v_knots, points } => {
                let (du, mu) = mults(u_knots, points.len());
                let (dv, mv) = mults(v_knots, points.first().map_or(0, Vec::len));
                format!("Spline deg ({du},{dv}) interior mults u {mu:?} v {mv:?}")
            }
            Surface::RationalSpline { u_knots, v_knots, homogeneous } => {
                let (du, mu) = mults(u_knots, homogeneous.len());
                let (dv, mv) = mults(v_knots, homogeneous.first().map_or(0, Vec::len));
                format!("RationalSpline deg ({du},{dv}) interior mults u {mu:?} v {mv:?}")
            }
            other => {
                let d = format!("{other:?}");
                d[..d.find('{').unwrap_or(d.len().min(30))].trim().to_string()
            }
        };
        eprintln!("PROBE {label} face {i}: {desc}");
    }
}

#[test]
fn probe_shell_after_fillet_on_a_loft() {
    let loft = crate::loft(&sections()).unwrap();
    eprintln!("PROBE loft: faces {} volume {:.3}", loft.face_count(), crate::measure(&loft).unwrap().volume);
    dump("loft", &loft);
    say("shell of the plain loft, open at the circle", &crate::shell(&loft, &[Vec3::new(0.0, 0.0, 0.0)], 2.0));
    let one = crate::fillet(&loft, &[Vec3::new(15.0, 0.0, 40.0)], 3.0);
    say("fillet of one square edge", &one);
    if let Ok(f) = &one {
        dump("one fillet", f);
        say("shell after one fillet, open at the circle", &crate::shell(f, &[Vec3::new(0.0, 0.0, 0.0)], 2.0));
        say("shell after one fillet, open at the square", &crate::shell(f, &[Vec3::new(0.0, 0.0, 40.0)], 2.0));
    }
    let four = crate::fillet(&loft, &[Vec3::new(15.0, 0.0, 40.0), Vec3::new(0.0, 15.0, 40.0), Vec3::new(-15.0, 0.0, 40.0), Vec3::new(0.0, -15.0, 40.0)], 3.0);
    say("fillet of the four square edges", &four);
    if let Ok(f) = &four {
        say("shell after four fillets, open at the circle", &crate::shell(f, &[Vec3::new(0.0, 0.0, 0.0)], 2.0));
    }
    // The control from the CLI session: a filleted box shells.
    let bx = crate::box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 20.0)).unwrap();
    let bf = crate::fillet(&bx, &[Vec3::new(20.0, 0.0, 20.0)], 3.0).unwrap();
    say("shell of a filleted box", &crate::shell(&bf, &[Vec3::new(20.0, 15.0, 0.0)], 2.0));
}
