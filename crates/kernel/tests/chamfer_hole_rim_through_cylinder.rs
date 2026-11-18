//! #11994: a chamfer of a hole's rim where a through hole crosses a cylinder wall. The body is
//! mirror-symmetric (x -> -x), so both rims must chamfer alike. Each case runs on the body as
//! built (seam at angle 0) and rotated about Z by 35.1 deg (the reporter's seam position).
//! Every case is printed before any assertion, so one run shows all the outcomes.

use solidcraft_kernel::{BoolOp, Body, boolean, chamfer, cylinder, measure, transform};
use solidcore_record::Vec3;

const DISTANCES: [f64; 4] = [0.5, 1.0, 1.5, 2.0];
const SEAM_DEG: f64 = 35.1;

fn drilled() -> Body {
    let body = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 20.0).unwrap();
    let tool = cylinder(Vec3::new(-15.0, 0.2, 11.8), Vec3::X, 4.669, 30.0).unwrap();
    boolean(&body, &tool, BoolOp::Cut).unwrap().unwrap()
}

/// The same rigid rotation `transform` applies (about Z through the origin), for the pick points.
fn rotate_z(p: Vec3, angle: f64) -> Vec3 {
    let (c, s) = (angle.cos(), angle.sin());
    Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z)
}

#[derive(Debug)]
struct Case {
    orientation: &'static str,
    rim: &'static str,
    d: f64,
    outcome: Result<(f64, usize), String>,
}

#[test]
fn chamfer_hole_rim_through_cylinder_wall_both_rims_alike() {
    let base = drilled();
    let base_volume = measure(&base).unwrap().volume;
    let angle = SEAM_DEG.to_radians();
    let turned = transform(&base, Vec3::ZERO, Vec3::ZERO, Vec3::Z, angle).unwrap();
    let rim_pos = Vec3::new(9.998, 0.2, 7.131);
    let rim_neg = Vec3::new(-9.998, 0.2, 7.131);

    let mut cases = Vec::new();
    for (orientation, body, rot) in [("as built", &base, false), ("rotated 35.1 deg", &turned, true)] {
        let pick = |p: Vec3| if rot { rotate_z(p, angle) } else { p };
        let body_volume = if rot { measure(body).unwrap().volume } else { base_volume };
        for d in DISTANCES {
            for (rim, p) in [("x>0", rim_pos), ("x<0", rim_neg)] {
                let outcome = match chamfer(body, &[pick(p)], d) {
                    Ok(made) => {
                        let removed = body_volume - measure(&made).unwrap().volume;
                        Ok((removed, made.shell_count()))
                    }
                    Err(e) => Err(format!("{e}")),
                };
                let case = Case { orientation, rim, d, outcome };
                eprintln!("CASE {case:?}");
                cases.push(case);
            }
        }
    }

    let mut failures = Vec::new();
    for orientation in ["as built", "rotated 35.1 deg"] {
        let mut prev_removed: Option<f64> = None;
        for d in DISTANCES {
            let at = |rim: &str| cases.iter().find(|c| c.orientation == orientation && c.rim == rim && c.d == d).unwrap();
            let (pos, neg) = (at("x>0"), at("x<0"));
            match (&pos.outcome, &neg.outcome) {
                (Ok((rp, sp)), Ok((rn, sn))) => {
                    if *sp != 1 || *sn != 1 {
                        failures.push(format!("{orientation} d={d}: shells x>0={sp} x<0={sn}"));
                    }
                    let rel = (rp - rn).abs() / rp.abs().max(1e-12);
                    if rel > 1e-6 {
                        failures.push(format!("{orientation} d={d}: removed x>0={rp} x<0={rn} (rel {rel:e})"));
                    }
                    if let Some(prev) = prev_removed {
                        if *rp <= prev {
                            failures.push(format!("{orientation} d={d}: removed {rp} does not exceed previous {prev}"));
                        }
                    }
                    prev_removed = Some(*rp);
                }
                _ => {
                    failures.push(format!("{orientation} d={d}: x>0 {:?} / x<0 {:?}", pos.outcome, neg.outcome));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} failing checks:\n{}", failures.len(), failures.join("\n"));
}
