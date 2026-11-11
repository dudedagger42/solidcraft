//! Extrudes of a pinched profile (an outer loop touching a hole at one point, #9216) against the
//! same profile widened outward by δ. Regions A and B are pinched; C is widened by δ = 1e-4 mm.

use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2};
use solidcraft_kernel::{extrude, measure};
use std::f64::consts::PI;

const DELTA: f64 = 1e-4;

fn line(a: (f64, f64), b: (f64, f64)) -> Seg2 {
    Seg2::Line { a: Vec2::new(a.0, a.1), b: Vec2::new(b.0, b.1) }
}

/// A circle of radius `r` about `c`, split into two arcs at its bottom point `c - (0, r)` and its top.
fn split_circle(c: (f64, f64), r: f64) -> Loop2 {
    Loop2 {
        segs: vec![
            Seg2::Arc { center: Vec2::new(c.0, c.1), radius: r, start: -PI / 2.0, sweep: PI },
            Seg2::Arc { center: Vec2::new(c.0, c.1), radius: r, start: PI / 2.0, sweep: PI },
        ],
    }
}

/// Region A (`joint` = 0) or B (`joint` = 1e-9): a 10×10 square whose bottom edge is two lines
/// meeting at (5, joint), with a hole of radius 2.5 at (5, 2.5 + joint) whose split points are at
/// its bottom and top, so the hole touches the outline at the joint.
fn pinched(joint: f64) -> Region2 {
    let outer = Loop2 {
        segs: vec![
            line((0.0, 0.0), (5.0, joint)),
            line((5.0, joint), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 10.0), (0.0, 0.0)),
        ],
    };
    Region2 { outer, holes: vec![split_circle((5.0, 2.5 + joint), 2.5)] }
}

/// Region C: the square and the hole each moved outward and inward by δ, so the neck is 2δ wide.
fn widened() -> Region2 {
    let d = DELTA;
    let outer = Loop2::polygon(&[Vec2::new(-d, -d), Vec2::new(10.0 + d, -d), Vec2::new(10.0 + d, 10.0 + d), Vec2::new(-d, 10.0 + d)]);
    Region2 { outer, holes: vec![Loop2::circle(Vec2::new(5.0, 2.5), 2.5 - d)] }
}

fn report(name: &str, r: &Region2) {
    match extrude(&Plane::XY, std::slice::from_ref(r), 0.0, 10.0) {
        Ok(bodies) => {
            eprintln!("[pinch] {name}: Ok, {} bodies", bodies.len());
            for (i, b) in bodies.iter().enumerate() {
                let v = measure(b).map(|m| m.volume);
                eprintln!("[pinch] {name}: body {i}: validity = {:?}, volume = {:?}", b.validity(), v);
            }
        }
        Err(e) => eprintln!("[pinch] {name}: Err: {e:?}"),
    }
}

#[test]
fn pinch_probe_print_only() {
    report("A (pinched exact)", &pinched(0.0));
    report("B (pinched 1e-9)", &pinched(1e-9));
    report("C (widened 1e-4)", &widened());
    let _ = PI;
}
