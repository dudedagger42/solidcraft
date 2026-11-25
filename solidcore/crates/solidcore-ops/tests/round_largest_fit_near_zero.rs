//! Rounds on an open arc of a cylinder, refused as too large (SolidCraft #21938): the refusal
//! named about 1e-12 mm, the ball touching both faces where the arc's side edge meets them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use solidcore::kernel::UpstreamFixes;
use solidcore_ops::affine::rotation;
use solidcore_ops::boolean::boolean;
use solidcore_ops::{
    blend, make_box, make_cylinder, pick, session, Blend, Body, BooleanOp, Corners, Error,
};

type Part = fn() -> (Body, [f64; 3]);

fn cut(a: Body, lo: [f64; 3], hi: [f64; 3], about: Option<([f64; 3], [f64; 3], f64)>) -> Body {
    let tool = make_box(lo, [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]).unwrap();
    if let Some((point, axis, degrees)) = about {
        tool.transform(&rotation(point, axis, degrees.to_radians()).unwrap())
            .unwrap();
    }
    boolean(a, tool.copy().unwrap(), BooleanOp::Subtract)
        .unwrap()
        .unwrap()
}

/// A shaft of radius 5 with a flat on the plane through x = 3 tilted 8 degrees from its axis.
fn tilted_flat() -> (Body, [f64; 3]) {
    let shaft = make_cylinder([0.0; 3], 5.0, 30.0).unwrap();
    let tilt = ([3.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8.0);
    let x = 5.0 * 30f64.to_radians().cos();
    (
        cut(shaft, [3.0, -20.0, -20.0], [23.0, 20.0, 60.0], Some(tilt)),
        [x, 2.5, (x - 3.0) / 8f64.to_radians().tan()],
    )
}

/// A boss of radius 3, its top cut 30 degrees from square and a slot cut through one side.
fn slotted_boss() -> (Body, [f64; 3]) {
    let boss = make_cylinder([0.0; 3], 3.0, 20.0).unwrap();
    let tilt = ([0.0, 0.0, 8.0], [1.0, 0.0, 0.0], 30.0);
    let sloped = cut(boss, [-20.0, -20.0, 8.0], [20.0, 20.0, 40.0], Some(tilt));
    (
        cut(sloped, [1.5, -10.0, 4.0], [2.0, 10.0, 40.0], None),
        [-3.0, 0.0, 8.0],
    )
}

#[test]
fn a_round_refused_as_too_large_names_no_radius_below_one_that_builds() {
    session::set_thread_fixes(UpstreamFixes::NAMES.iter().copied()).unwrap();
    let cases: [(Part, f64, f64); 3] = [
        (tilted_flat, 3.0, 4.0),
        (tilted_flat, 3.0, 5.05),
        (slotted_boss, 1.0, 2.0),
    ];
    for (part, builds, refused) in cases {
        let (body, at) = part();
        let edge = pick::edges(&body, &[at], 0.05).unwrap().expect("pick");
        blend(&body, Blend::Round, &edge, &[builds], &[], Corners::Filled).unwrap();
        let (body, _) = part();
        let edge = pick::edges(&body, &[at], 0.05).unwrap().expect("pick");
        match blend(&body, Blend::Round, &edge, &[refused], &[], Corners::Filled) {
            Err(Error::NoRoom) => {}
            Err(Error::TooLarge { size, .. }) => assert!(
                size >= builds,
                "a round of {refused} is refused naming {size} where a round of {builds} builds"
            ),
            other => panic!("a round of {refused}: {other:?}"),
        }
    }
}
