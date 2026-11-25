#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use solidcore::kernel::UpstreamFixes;
use solidcore_ops::affine::rotation;
use solidcore_ops::boolean::boolean;
use solidcore_ops::{blend, make_box, make_cylinder, pick, session, Blend, Body, BooleanOp, Corners};

fn dflat() -> (Body, [f64; 3]) {
    let c = make_cylinder([0.0; 3], 5.0, 30.0).unwrap();
    let cut = make_box([3.0, -20.0, -20.0], [20.0, 40.0, 80.0]).unwrap();
    cut.transform(&rotation([3.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8f64.to_radians()).unwrap())
        .unwrap();
    let cut = cut.copy().unwrap();
    let x = 5.0 * 30f64.to_radians().cos();
    (
        boolean(c, cut, BooleanOp::Subtract).unwrap().unwrap(),
        [x, 2.5, (x - 3.0) / 8f64.to_radians().tan()],
    )
}

#[test]
fn probe() {
    session::set_thread_fixes(UpstreamFixes::NAMES.iter().copied()).unwrap();
    for (kind, s) in [(Blend::Round, 4.0), (Blend::Chamfer, 2.0)] {
        let (b, p) = dflat();
        let e = pick::edges(&b, &[p], 0.05).unwrap().expect("pick");
        eprintln!("PROBE BEGIN {kind:?} {s}");
        let r = blend(&b, kind, &e, &[s], &[], Corners::Filled);
        eprintln!("PROBE RESULT {kind:?} {s}: {r:?}");
    }
}
