//! #12843: a capsule (cylinder R 6.75 x 60 with R 6.74 rounds on both end circles) cut from a
//! cylinder R 7.5 x 60 that it fills end to end. The cut must build one solid whose every face
//! has a solid side and a void side, with volume 10602.88 - 7945.84 = 2657.04.
//!
//! Controls: the union (10602.88) and the intersection (7945.84), both of which the cut's
//! operands must reproduce.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)] // test helpers

use solidcore::blend::fillet_executive::api::fillet_edges;
use solidcore::boolean::api_brep::{boolean_difference, boolean_intersection, boolean_union};
use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::mass::integrator::properties::{brep_volume, PropertyAttributes};
use solidcore::math::Point3;
use solidcore::primitive::api_primitives::create_cylinder;
use solidcore::query::api_queries::edge_closest_point;
use solidcore::topology::face::p2::face_side_regions;
use solidcore::topology::{edge, vertex, EdgeId};
use solidcore::{Brep, Kernel};

const TOL: f64 = 0.06;

fn origin() -> Point3 {
    Point3::new(0.0, 0.0, 0.0)
}

/// The circle edge of radius 6.75 at height `z` through `(6.75, 0, z)`. The seam line through
/// the same point also has distance 0, so a circle must start and end on the plane `z`.
fn pick_circle(k: &Kernel, b: &Brep, z: f64) -> EdgeId {
    let q = Point3::new(6.75, 0.0, z);
    b.edge_list()
        .iter()
        .copied()
        .find(|&e| {
            let Ok((Some(s), Some(t))) = edge::start_end_vertices(b, e) else {
                return false;
            };
            let on_plane = |v| {
                vertex::point(b, v)
                    .map(|p| (p.z - z).abs() < 1e-9)
                    .unwrap_or(false)
            };
            on_plane(s)
                && on_plane(t)
                && edge_closest_point(k, b, e, q)
                    .map(|c| c.closest.distance < 1e-6)
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no circle edge at z = {z}"))
}

fn volume_of(k: &Kernel, b: &mut Brep) -> f64 {
    let store = AttributeStore::default();
    let attrs = PropertyAttributes {
        store: &store,
        key_of: &|_| ObjectKey(0),
    };
    brep_volume(k, b, 1e-7, &attrs).unwrap()
}

fn capsule(k: &mut Kernel) -> Brep {
    let mut b = create_cylinder(k, origin(), 6.75, 60.0).unwrap();
    let bottom = pick_circle(k, &b, 0.0);
    let top = pick_circle(k, &b, 60.0);
    fillet_edges(k, &mut b, &[bottom, top], 6.74, 1, 1, 0.0).unwrap();
    b
}

fn sleeve(k: &mut Kernel) -> Brep {
    create_cylinder(k, origin(), 7.5, 60.0).unwrap()
}

#[test]
fn capsule_has_its_analytic_volume() {
    let mut k = Kernel::default();
    let mut b = capsule(&mut k);
    let v = volume_of(&k, &mut b);
    assert!((v - 7945.84).abs() <= TOL, "capsule volume {v}");
}

#[test]
fn union_control_is_the_sleeve_plus_capsule() {
    let mut k = Kernel::default();
    let s = sleeve(&mut k);
    let c = capsule(&mut k);
    let mut u = boolean_union(&mut k, s, c).unwrap();
    let v = volume_of(&k, &mut u);
    assert!((v - 10602.88).abs() <= TOL, "union volume {v}");
}

#[test]
fn intersection_control_is_the_capsule() {
    let mut k = Kernel::default();
    let s = sleeve(&mut k);
    let c = capsule(&mut k);
    let mut i = boolean_intersection(&mut k, s, c).unwrap();
    let v = volume_of(&k, &mut i);
    assert!((v - 7945.84).abs() <= TOL, "intersection volume {v}");
}

#[test]
fn cut_of_a_capsule_from_a_sleeve_it_fills_is_one_solid_with_a_pocket() {
    let mut k = Kernel::default();
    let s = sleeve(&mut k);
    let c = capsule(&mut k);
    let mut d = boolean_difference(&mut k, s, c).unwrap();

    // two regions: the infinite void and the one solid
    let regions: Vec<_> = d.regions().iter().map(|(id, r)| (id, r.is_void())).collect();
    assert_eq!(regions.len(), 2, "regions {regions:?}");
    let infinite = d.infinite_region().unwrap();
    for (id, void) in &regions {
        let want = i32::from(*id == infinite);
        assert_eq!(*void, want, "region {id:?} void flag");
    }

    // every face has one solid side and one void side
    let faces: Vec<_> = d.face_list().iter().copied().collect();
    assert!(!faces.is_empty());
    for f in faces {
        let [a, b] = face_side_regions(&d, f).unwrap();
        let (a, b) = (a.unwrap(), b.unwrap());
        let flags = [d.region(a).unwrap().is_void(), d.region(b).unwrap().is_void()];
        assert!(
            flags.contains(&0) && flags.contains(&1),
            "face {f:?} sides {flags:?}"
        );
    }

    let v = volume_of(&k, &mut d);
    assert!((v - 2657.04).abs() <= TOL, "difference volume {v}");
}
