//! tangency-labs/solidcraft#13910: a 100 x 40 x 14 block, its vertical edges rounded 10 at
//! one end and 15 at the other, its top and bottom edge chains rounded 5, sliced flat at
//! z = 12.8 (1.2 below its top) and shelled 1 inward, open at the slice. The body is the one
//! SolidCraft hands the shell, as it wrote it (`issue_13910_sliced_rounded_block.smb`): built
//! here step by step it does not reach the gap.
//!
//! Round the corners the 5 round is a surface of revolution, along the sides a cylinder. The
//! two are tangent along an arc, and the slice cuts both at an angle. The open face does not
//! move, so the offsets of the two rounds reach it past the end of their seam's offset: the
//! seam's own arc, continued, is the piece missing from both loops. The shell was refused
//! there (a loop gap not bridged): the chord from the seam's end to the vertex is not on a
//! curved face, and no face's plane holds both.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)] // test helpers

mod common;

use solidcore_ops::*;

use common::*;

const BLOCK: &[u8] =
    include_bytes!("../../solidcore/tests/data/issue_13910_sliced_rounded_block.smb");

#[test]
fn a_sliced_rounded_block_shells_open_at_the_slice() {
    all_fixes();
    let b = Body::from_smb(BLOCK.to_vec(), false).unwrap();
    let solid = volume(&b);
    assert!(rel(solid, 47_545.579_142_638_24) < 1e-9, "{solid}");
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
    // as SolidCraft builds the same shell
    let shelled = volume(&b);
    assert!(rel(shelled, 6_545.793_098_208_915) < 1e-6, "{shelled}");
}
