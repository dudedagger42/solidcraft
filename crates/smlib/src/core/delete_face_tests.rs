//! Delete Face on fillet and chamfer faces ([`super::delete_face::delete_fillet_faces`]): each
//! round or chamfer taken off gives the sharp body back, against closed forms.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use super::blend::{Corners, blend};
use super::boolean::boolean;
use super::delete_face::delete_fillet_faces;
use super::{Body, counts, make_box, make_cylinder, volume};
use crate::{Blend, BooleanOp};

const TOL: f64 = 0.01;

fn round(body: &Body, edge: [f64; 3], r: f64) -> Body {
    blend(body, Blend::Round, &[edge], &[r], TOL, &[], &[], Corners::Filled).unwrap()
}

/// A plate 40 × 30 × 5 with a boss Ø10, 8 high, on it: 6000 + 200π.
fn bossed() -> Body {
    let plate = make_box([0.0; 3], [40.0, 30.0, 5.0]).unwrap();
    let boss = make_cylinder([20.0, 15.0, 5.0], 5.0, 8.0).unwrap();
    boolean(&plate, &boss, BooleanOp::Union).unwrap().unwrap()
}

#[test]
fn deleting_a_round_on_a_box_edge_gives_the_sharp_box() {
    let cube = make_box([0.0; 3], [20.0; 3]).unwrap();
    let rounded = round(&cube, [10.0, 20.0, 20.0], 2.0);
    // On the round, half way round: the axis at y = z = 18, 2 out at 45°.
    let at = 18.0 + 2.0 * FRAC_1_SQRT_2;
    let sharp = delete_fillet_faces(&rounded, &[[10.0, at, at]], TOL).unwrap();
    assert!((volume(&sharp, 1e-9).unwrap() - 8000.0).abs() < 1e-6, "{}", volume(&sharp, 1e-9).unwrap());
    assert_eq!(counts(&sharp).unwrap(), [6, 12, 8]);
}

#[test]
fn deleting_a_boss_foot_round_or_top_round_gives_the_sharp_boss() {
    let sharp = 6000.0 + 200.0 * PI;
    // Edges are picked at +y, off the circles' seam at +x.
    // The foot round, r 1: its tube's centre circle at radius 6, height 6; the point 45° in.
    let foot = round(&bossed(), [20.0, 20.0, 5.0], 1.0);
    let at = 6.0 - FRAC_1_SQRT_2;
    let back = delete_fillet_faces(&foot, &[[20.0, 15.0 + at, at]], TOL).unwrap();
    assert!((volume(&back, 1e-9).unwrap() - sharp).abs() < 1e-6, "{}", volume(&back, 1e-9).unwrap());
    assert_eq!(counts(&back).unwrap()[0], 8);
    // The top round, r 1: its tube's centre circle at radius 4, height 12; the point 45° out.
    let top = round(&bossed(), [20.0, 20.0, 13.0], 1.0);
    let (r, z) = (4.0 + FRAC_1_SQRT_2, 12.0 + FRAC_1_SQRT_2);
    let back = delete_fillet_faces(&top, &[[20.0, 15.0 + r, z]], TOL).unwrap();
    assert!((volume(&back, 1e-9).unwrap() - sharp).abs() < 1e-6, "{}", volume(&back, 1e-9).unwrap());
    assert_eq!(counts(&back).unwrap()[0], 8);
}

fn chamfer(body: &Body, edge: [f64; 3], d: f64) -> Body {
    blend(body, Blend::Chamfer, &[edge], &[d], TOL, &[], &[], Corners::Filled).unwrap()
}

#[test]
fn deleting_a_chamfer_on_a_box_edge_gives_the_sharp_box() {
    let cube = make_box([0.0; 3], [20.0; 3]).unwrap();
    let chamfered = chamfer(&cube, [10.0, 20.0, 20.0], 2.0);
    // On the chamfer, half way across: (19, 19) in y, z.
    let sharp = delete_fillet_faces(&chamfered, &[[10.0, 19.0, 19.0]], TOL).unwrap();
    assert!((volume(&sharp, 1e-9).unwrap() - 8000.0).abs() < 1e-6, "{}", volume(&sharp, 1e-9).unwrap());
    assert_eq!(counts(&sharp).unwrap(), [6, 12, 8]);
}

/// A plate 40 × 30 × 5 with a through hole Ø6: 6000 − 45π.
fn holed() -> Body {
    let plate = make_box([0.0; 3], [40.0, 30.0, 5.0]).unwrap();
    let pin = make_cylinder([20.0, 15.0, -1.0], 3.0, 7.0).unwrap();
    boolean(&plate, &pin, BooleanOp::Subtract).unwrap().unwrap()
}

#[test]
fn deleting_a_hole_chamfer_gives_the_plain_hole() {
    let plain = 6000.0 - 45.0 * PI;
    // The hole's top edge at +y (off the circle's seam at +x), chamfered 1.
    let chamfered = chamfer(&holed(), [20.0, 18.0, 5.0], 1.0);
    // On the cone, half way down: radius 3.5, height 4.5.
    let back = delete_fillet_faces(&chamfered, &[[20.0, 18.5, 4.5]], TOL).unwrap();
    assert!((volume(&back, 1e-9).unwrap() - plain).abs() < 1e-6, "{}", volume(&back, 1e-9).unwrap());
    assert_eq!(counts(&back).unwrap()[0], counts(&holed()).unwrap()[0]);
}

#[test]
fn a_face_that_is_not_a_fillet_is_refused() {
    let cube = make_box([0.0; 3], [20.0; 3]).unwrap();
    assert!(delete_fillet_faces(&cube, &[[10.0, 10.0, 20.0]], TOL).is_err());
}
