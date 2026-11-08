//! #12001: a fillet on the valley edge of a V-groove cut across a bore. The valley edge runs from
//! the bore's wall to the part's side; the round on one half fails with SMLib status 1001 while
//! the other half builds.

use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_kernel::{BoolOp, Body, boolean, extrude, fillet, tangent_chain};
use std::f64::consts::TAU;

const AXIS: Vec2 = Vec2 { x: -7.2, y: 0.0 };
const TOP: f64 = 5.8;
const BORE: f64 = 0.762;
/// The groove's half width at the top, as wide as the bore.
const HALF: f64 = 0.762;

fn polygon(points: &[Vec2]) -> Region2 {
    let segs = (0..points.len())
        .map(|i| Seg2::Line { a: points[i], b: points[(i + 1) % points.len()] })
        .collect();
    Region2 { outer: Loop2 { segs }, holes: vec![] }
}

/// A 6.75 mm square block round the axis, a bore through it (its circle starting at
/// `bore_start` radians) and a 90° V-groove along x across the top, through the axis.
fn grooved(bore_start: f64) -> Body {
    let (x0, x1, y0, y1) = (AXIS.x - 3.375, AXIS.x + 3.375, -3.375, 3.375);
    let block = polygon(&[Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]);
    let block = extrude(&Plane::XY, &[block], 0.0, TOP).unwrap().pop().unwrap();
    let bore = Region2 {
        outer: Loop2 { segs: vec![Seg2::Arc { center: AXIS, radius: BORE, start: bore_start, sweep: TAU }] },
        holes: vec![],
    };
    let bore = extrude(&Plane::XY, &[bore], 0.0, TOP).unwrap().pop().unwrap();
    let bored = boolean(&block, &bore, BoolOp::Cut).unwrap().unwrap();
    // YZ: sketch x = +Y, y = +Z; extruded both ways along X
    let v = polygon(&[Vec2::new(-HALF, TOP), Vec2::new(0.0, TOP - HALF), Vec2::new(HALF, TOP)]);
    let v = extrude(&Plane::YZ, &[v], -30.0, 30.0).unwrap().pop().unwrap();
    boolean(&bored, &v, BoolOp::Cut).unwrap().unwrap()
}

fn fillet_valley(body: &Body, x: f64) -> Result<Body, String> {
    let edges = tangent_chain(body, &[Vec3::new(x, 0.0, TOP - HALF)]).map_err(|e| e.to_string())?;
    fillet(body, &edges, 0.2).map_err(|e| e.to_string())
}

#[test]
fn a_fillet_on_either_half_of_a_groove_valley_across_a_bore_builds() {
    for bore_start in [0.0, 0.25 * TAU, 0.5 * TAU] {
        let body = grooved(bore_start);
        for x in [AXIS.x + 1.5, AXIS.x - 1.5] {
            let made = fillet_valley(&body, x);
            eprintln!("bore start {bore_start:.3} valley x {x}: {:?}", made.as_ref().map(|_| "ok"));
        }
    }
    for bore_start in [0.0, 0.25 * TAU, 0.5 * TAU] {
        let body = grooved(bore_start);
        for x in [AXIS.x + 1.5, AXIS.x - 1.5] {
            let made = fillet_valley(&body, x).unwrap_or_else(|e| panic!("bore start {bore_start}, valley at x {x}: {e}"));
            assert!(made.validity().is_empty(), "{:?}", made.validity());
        }
    }
}
