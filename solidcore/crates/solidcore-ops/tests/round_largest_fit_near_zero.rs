#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use solidcore::kernel::UpstreamFixes;
use solidcore_ops::affine::rotation;
use solidcore_ops::boolean::boolean;
use solidcore_ops::{blend, make_box, make_cone, make_cylinder, pick, session, Blend, Body, BooleanOp, Corners};

fn rot(b: &Body, p: [f64; 3], axis: [f64; 3], deg: f64) -> Body {
    b.transform(&rotation(p, axis, deg.to_radians()).unwrap()).unwrap();
    b.copy().unwrap()
}
fn sub(a: Body, b: Body) -> Body {
    boolean(a, b, BooleanOp::Subtract).unwrap().unwrap()
}
fn bx(lo: [f64; 3], hi: [f64; 3]) -> Body {
    make_box(lo, [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]).unwrap()
}
fn try_blend(name: &str, b: &Body, p: [f64; 3], kind: Blend, size: f64) {
    let e = pick::edges(b, &[p], 0.05).unwrap().expect("pick");
    eprintln!("PROBE BEGIN {name} {kind:?} {size}");
    let r = blend(b, kind, &e, &[size], &[], Corners::Filled);
    match r {
        Ok(_) => eprintln!("PROBE RESULT {name} {kind:?} {size}: ok"),
        Err(err) => eprintln!("PROBE RESULT {name} {kind:?} {size}: ERR {err} || {err:?}"),
    }
}
fn dflat() -> (Body, [f64; 3]) {
    let c = make_cylinder([0.0; 3], 5.0, 30.0).unwrap();
    let cut = rot(&bx([3.0, -20.0, -20.0], [23.0, 20.0, 60.0]), [3.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8.0);
    let x = 5.0 * 30f64.to_radians().cos();
    (sub(c, cut), [x, 2.5, (x - 3.0) / 8f64.to_radians().tan()])
}
fn boss_top(s0: f64, s1: f64) -> (Body, [f64; 3]) {
    let c = make_cylinder([0.0; 3], 3.0, 20.0).unwrap();
    let top = rot(&bx([-20.0, -20.0, 8.0], [20.0, 20.0, 40.0]), [0.0, 0.0, 8.0], [1.0, 0.0, 0.0], 30.0);
    (sub(sub(c, top), bx([s0, -10.0, 4.0], [s1, 10.0, 40.0])), [-3.0, 0.0, 8.0])
}
fn cone_top() -> (Body, [f64; 3]) {
    let zc = 9.0;
    let c = make_cone([0.0; 3], 6.0, 1.0, 10.0).unwrap();
    let top = rot(&bx([-20.0, -20.0, zc], [20.0, 20.0, 40.0]), [0.0, 0.0, zc], [1.0, 0.0, 0.0], 25.0);
    (sub(sub(c, top), bx([0.5, -10.0, zc - 3.0], [1.0, 10.0, 40.0])), [-(6.0 - 0.5 * zc), 0.0, zc])
}

#[test]
fn probe() {
    session::set_thread_fixes(UpstreamFixes::NAMES.iter().copied()).unwrap();
    for s in [3.9, 3.97, 4.0, 5.05] {
        let (b, p) = dflat();
        try_blend("dflat", &b, p, Blend::Round, s);
    }
    let (b, p) = dflat();
    try_blend("dflat", &b, p, Blend::Chamfer, 2.0);
    for s in [2.0, 3.05] {
        let (b, p) = boss_top(1.5, 2.0);
        try_blend("boss15", &b, p, Blend::Round, s);
    }
    let (b, p) = boss_top(2.0, 2.6);
    try_blend("boss20", &b, p, Blend::Round, 2.0);
    let (b, p) = boss_top(2.0, 2.6);
    try_blend("boss20", &b, p, Blend::Chamfer, 1.0);
    let (b, p) = cone_top();
    try_blend("cone", &b, p, Blend::Round, 2.0);
}
