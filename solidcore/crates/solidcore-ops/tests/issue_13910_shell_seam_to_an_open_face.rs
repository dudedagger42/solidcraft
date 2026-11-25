//! tangency-labs/solidcraft#13910: a 100 x 40 x 14 block, its vertical edges rounded 10 at
//! one end and 15 at the other, its top and bottom edge chains rounded 5, sliced flat at
//! z = 12.8 (1.2 below its top) and shelled 1 inward, open at the slice.
//!
//! Round the corners the 5 round is a surface of revolution, along the sides a cylinder. The
//! two are tangent along an arc, and the slice cuts both at an angle. The open face does not
//! move, so the offsets of the two rounds reach it past the end of their seam's offset: the
//! seam's own arc, continued, is the piece missing from both loops. The shell was refused
//! there (a loop gap not bridged): the chord from the seam's end to the vertex is not on a
//! curved face, and no face's plane holds both.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)] // test helpers

mod common;

use solidcore_ops::boolean::boolean;
use solidcore_ops::*;

use common::*;

/// The rounded block with everything above z = 12.8 cut away.
fn sliced_block() -> Body {
    let b = make_box([0.0; 3], [100.0, 40.0, 14.0]).unwrap();
    let round = |b: &Body, at: &[[f64; 3]], r: f64| {
        let edges: Vec<usize> = at.iter().map(|&p| edge_at(b, p)).collect();
        blend(b, Blend::Round, &edges, &[r], &[], Corners::Filled).unwrap();
    };
    round(&b, &[[100.0, 0.0, 7.0], [100.0, 40.0, 7.0]], 10.0);
    round(&b, &[[0.0, 40.0, 7.0], [0.0, 0.0, 7.0]], 15.0);
    // the top and bottom chains: four lines and four arcs each
    let (near, far) = (10.0 - 50.0_f64.sqrt(), 15.0 - 112.5_f64.sqrt());
    let mut chain = Vec::new();
    for z in [14.0, 0.0] {
        chain.extend([
            [50.0, 0.0, z],
            [50.0, 40.0, z],
            [100.0, 20.0, z],
            [0.0, 20.0, z],
            [100.0 - near, near, z],
            [100.0 - near, 40.0 - near, z],
            [far, far, z],
            [far, 40.0 - far, z],
        ]);
    }
    round(&b, &chain, 5.0);
    let above = make_box([-5.0, -5.0, 12.8], [110.0, 50.0, 5.0]).unwrap();
    boolean(b, above, BooleanOp::Subtract).unwrap().unwrap()
}

#[test]
fn a_sliced_rounded_block_shells_open_at_the_slice() {
    all_fixes();
    let b = sliced_block();
    let solid = volume(&b);
    let (_, o) = b.to_record().unwrap();
    let open: Vec<usize> = (0..o.faces.len())
        .filter(|&f| {
            let face = b.face_centroid(&o, f, 1e-8).unwrap();
            (face.centroid[2] - 12.8).abs() < 1e-9
        })
        .collect();
    assert_eq!(open.len(), 1, "the sliced face");
    shell(&b, &o, &open, -1.0).unwrap();
    assert!(b.check(CheckLevel::All).unwrap().is_valid());
    let shelled = volume(&b);
    assert!(
        shelled > 0.0 && shelled < solid,
        "{shelled} vs the solid's {solid}"
    );
    // as SolidCraft builds the same part and shell
    assert!(rel(shelled, 6545.793098208915) < 1e-6, "{shelled}");
}
