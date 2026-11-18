//! #13519: a prism whose side leans a hair off a wall must join the wall as one solid.
//! Wall: box 8 x 20 x 20 at the origin. Prism: XY polygon (-20,5), (0,5), (-g,10), (-20,10),
//! height 10, joined by the app's Extrude path (`boolean(.., BoolOp::Union)`).
//! Exact volume 4200 - 25 g.

use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_kernel::{BoolOp, boolean, box_solid, extrude, measure};

fn wall() -> solidcraft_kernel::Body {
    box_solid(Vec3::ZERO, Vec3::new(8.0, 20.0, 20.0)).unwrap()
}

fn prism(g: f64) -> solidcraft_kernel::Body {
    let p = [Vec2::new(-20.0, 5.0), Vec2::new(0.0, 5.0), Vec2::new(-g, 10.0), Vec2::new(-20.0, 10.0)];
    let segs = vec![
        Seg2::Line { a: p[0], b: p[1] },
        Seg2::Line { a: p[1], b: p[2] },
        Seg2::Line { a: p[2], b: p[3] },
        Seg2::Line { a: p[3], b: p[0] },
    ];
    extrude(&Plane::XY, &[Region2 { outer: Loop2 { segs }, holes: vec![] }], 0.0, 10.0).unwrap().pop().unwrap()
}

fn check_join(g: f64) {
    let b = boolean(&wall(), &prism(g), BoolOp::Union)
        .unwrap_or_else(|e| panic!("g = {g:e}: boolean failed: {e:?}"))
        .unwrap_or_else(|| panic!("g = {g:e}: boolean produced nothing"));
    let m = measure(&b).unwrap();
    let want = 4200.0 - 25.0 * g;
    assert!((m.volume - want).abs() <= 5e-4, "g = {g:e}: volume {} differs from {} by more than 5e-4", m.volume, want);
    assert_eq!(m.shells, 1, "g = {g:e}: expected one shell, got {}", m.shells);
}

#[test]
fn join_with_the_side_0_off_the_wall_builds() {
    check_join(0.0);
}

#[test]
fn join_with_the_side_1e_5_off_the_wall_builds() {
    check_join(1e-5);
}

#[test]
fn join_with_the_side_3e_5_off_the_wall_builds() {
    check_join(3e-5);
}

#[test]
fn join_with_the_side_7e_5_off_the_wall_builds() {
    check_join(7e-5);
}
