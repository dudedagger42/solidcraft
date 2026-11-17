//! A shell whose wall folds over a G2 fillet corner names the largest wall that works (#17246):
//! the corner patch of a 4 mm G2 round on a box, shelled 1 mm inward, folds at 0.9674 mm.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)] // test helpers

mod common;

use common::*;
use solidcore_ops::*;

const ORIGIN: [f64; 3] = [-7.5, -7.5, 0.0];
const SIZE: [f64; 3] = [36.5, 38.5, 10.0];
const RADIUS: f64 = 4.0;
/// The smallest radius of curvature on the corner's displaced side, which the wall folds past.
const LARGEST_WALL: f64 = 0.967_412_193_608_327_8;

fn corner_body() -> Body {
    let b = make_box(ORIGIN, SIZE).unwrap();
    // the three edges meeting at the corner (29, 31, 10)
    let edges = [
        edge_at(&b, [10.75, 31.0, 10.0]),
        edge_at(&b, [29.0, 11.75, 10.0]),
        edge_at(&b, [29.0, 31.0, 5.0]),
    ];
    blend(
        &b,
        Blend::Curvature,
        &edges,
        &[RADIUS],
        &[],
        Corners::Filled,
    )
    .unwrap();
    b
}

#[test]
fn shell_of_g2_corner_names_largest_wall() {
    let mut b = corner_body();
    let (_, order) = b.to_record().unwrap();
    let counts = b.counts().unwrap();
    let mut top: Option<(usize, f64)> = None;
    for i in 0..counts[0] {
        let c = b.face_centroid(&order, i, 1e-8).unwrap();
        if c.centroid[2] >= 9.999 && top.is_none_or(|(_, a)| c.area > a) {
            top = Some((i, c.area));
        }
    }
    let (top, _) = top.expect("a top face at z = 10");
    match shell(&b, &order, &[top], -1.0) {
        Err(Error::WallFolds { largest: Some(l) }) => {
            assert!(
                (l - LARGEST_WALL).abs() < 1e-3,
                "largest wall {l}, expected {LARGEST_WALL}"
            );
        }
        other => panic!("expected WallFolds naming the largest wall, got {other:?}"),
    }
    let _ = &mut b;
}
