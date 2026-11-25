//! tangency-labs/solidcraft#13910: a block with its four vertical edges rounded r10 and its
//! top edges rounded r5, sliced flat at z = 12.8 (1.2 below its top), shelled 1 mm inward
//! open at the slice.
//!
//! Round the corners, the r5 round is a surface of revolution; along the sides it is a
//! cylinder. The two are tangent along an arc, and the slice cuts both at an angle. The open
//! face does not move, so the offsets of the two rounds reach it short of where the offset of
//! their seam ends: the seam's own arc, continued, is the piece missing from both loops. The
//! shell was refused there (`LoopGapUncertified`): the chord between the seam's end and the
//! vertex is not on a curved face, and no face's plane holds both.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)] // test helpers

#[path = "support/offset_fixture.rs"]
mod offset_fixture;

use offset_fixture::{api_shell, planar_faces_at, report, volume};
use solidcore::blend::fillet_executive::api::fillet_edges;
use solidcore::boolean::api_brep::boolean;
use solidcore::geom::a_object::BooleanOperation;
use solidcore::math::Point3;
use solidcore::offset::offset_executive::p2::{OffsetOptions, ShellOptions};
use solidcore::primitive::api_primitives::create_box;
use solidcore::topology::EdgeId;
use solidcore::{Brep, Kernel};

/// The edges of `b` whose two ends satisfy `keep`.
fn edges_where(k: &Kernel, b: &Brep, keep: impl Fn(Point3, Point3) -> bool) -> Vec<EdgeId> {
    b.edges()
        .iter()
        .filter(|(_, e)| {
            let c = e.curve.as_ref().unwrap();
            let p = c.evaluate_point(k, e.interval.min()).unwrap();
            let q = c.evaluate_point(k, e.interval.max()).unwrap();
            keep(p, q)
        })
        .map(|(id, _)| id)
        .collect()
}

/// The 60 x 40 x 14 block, its vertical edges rounded r10, then its top edges r5, with
/// everything above z = 12.8 cut away.
fn part(k: &mut Kernel) -> Brep {
    let mut b = create_box(k, Point3::new(0.0, 0.0, 0.0), 60.0, 40.0, 14.0).unwrap();
    let vertical = edges_where(k, &b, |p, q| (p.z - q.z).abs() > 1.0);
    assert_eq!(vertical.len(), 4, "the four vertical edges");
    fillet_edges(k, &mut b, &vertical, 10.0, 1, 1, 1.0).unwrap();
    b.refresh_cached_topology();
    let top = edges_where(k, &b, |p, q| {
        (p.z - 14.0).abs() < 1e-9 && (q.z - 14.0).abs() < 1e-9
    });
    assert_eq!(top.len(), 8, "the top face's four lines and four arcs");
    fillet_edges(k, &mut b, &top, 5.0, 1, 1, 1.0).unwrap();
    b.refresh_cached_topology();
    let above = create_box(k, Point3::new(-5.0, -5.0, 12.8), 70.0, 50.0, 5.0).unwrap();
    let mut sliced = boolean(k, b, above, BooleanOperation::Difference)
        .unwrap()
        .result;
    sliced.refresh_cached_topology();
    sliced
}

#[test]
fn issue_13910_a_sliced_rounded_block_shells_open_at_the_slice() {
    let mut k = Kernel::default();
    let mut b = part(&mut k);
    let solid = volume(&k, &mut b).unwrap();
    let open = planar_faces_at(&k, &b, 2, 12.8);
    assert_eq!(open.len(), 1, "the sliced face");
    // as SolidCraft's shell calls it
    let options = ShellOptions {
        offset: OffsetOptions {
            distance: -1.0,
            extend_convex_edges: true,
            merge_results: true,
        },
        create_offset_solid: true,
        merge_sheet_side_walls: true,
    };
    let mut shelled = api_shell(&mut k, &mut b, options, &open).unwrap();
    let got = volume(&k, &mut shelled).unwrap();
    assert!(got > 0.0 && got < solid, "{got} vs the solid's {solid}");
    assert!(report(&k, "issue_13910 sliced block", Ok(shelled), None, 1e-6));
}
