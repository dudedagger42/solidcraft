//! #11994: a chamfer of a hole's rim where a through hole crosses a cylinder wall. The body is
//! mirror-symmetric (x -> -x), so both rims must chamfer alike. Each case runs with the drill
//! turned about Z (0 and 90 deg): turning the drill moves the hole against the cylinder's seam,
//! where turning the body would not. Every case is printed before any assertion, so one run shows
//! all the outcomes.

use solidcraft_kernel::{BoolOp, Body, boolean, chamfer, cylinder, measure};
use solidcore_record::Vec3;

const DISTANCES: [f64; 4] = [0.5, 1.0, 1.5, 2.0];
/// The rim's length on the cylinder (mm).
const RIM_LENGTH: f64 = 29.78;
/// Each rim's chamfer is fitted to the kernel's curve tolerance (2e-5 mm) over its area.
const TOLERANCE: f64 = 2e-5;
const TURNS_DEG: [(&str, f64); 2] = [("drill at 0 deg", 0.0), ("drill turned 90 deg", 90.0)];

/// The hole's drill turned `turn` rad about Z through the origin.
fn drilled(turn: f64) -> Body {
    let body = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 20.0).unwrap();
    let tool = cylinder(rotate_z(Vec3::new(-15.0, 0.2, 11.8), turn), rotate_z(Vec3::X, turn), 4.669, 30.0).unwrap();
    boolean(&body, &tool, BoolOp::Cut).unwrap().unwrap()
}

/// The rigid rotation about Z through the origin, for the drill and the pick points.
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
    let rim_pos = Vec3::new(9.998, 0.2, 7.131);
    let rim_neg = Vec3::new(-9.998, 0.2, 7.131);

    let mut cases = Vec::new();
    for (orientation, turn_deg) in TURNS_DEG {
        let turn = turn_deg.to_radians();
        let body = drilled(turn);
        let body_volume = measure(&body).unwrap().volume;
        for d in DISTANCES {
            for (rim, p) in [("x>0", rim_pos), ("x<0", rim_neg)] {
                let outcome = match chamfer(&body, &[rotate_z(p, turn)], d) {
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
    for (orientation, _) in TURNS_DEG {
        let mut prev_removed: Option<f64> = None;
        for d in DISTANCES {
            let at = |rim: &str| cases.iter().find(|c| c.orientation == orientation && c.rim == rim && c.d == d).unwrap();
            let (pos, neg) = (at("x>0"), at("x<0"));
            match (&pos.outcome, &neg.outcome) {
                (Ok((rp, sp)), Ok((rn, sn))) => {
                    if *sp != 1 || *sn != 1 {
                        failures.push(format!("{orientation} d={d}: shells x>0={sp} x<0={sn}"));
                    }
                    let bound = 2.0 * TOLERANCE * RIM_LENGTH * d * std::f64::consts::SQRT_2;
                    if (rp - rn).abs() > bound {
                        failures.push(format!("{orientation} d={d}: removed x>0={rp} x<0={rn} (bound {bound:e})"));
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
