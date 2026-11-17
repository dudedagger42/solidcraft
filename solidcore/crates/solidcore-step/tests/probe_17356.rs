//! Probe for tangency-labs/solidcraft#17356 (no fix): the 2 mm fillet on the outer top edge
//! through (-41.713, -53.931, 100) of the reporter's body, after the inner round (Fillet2),
//! is refused as CutsThePart with a largest radius of 0.241 mm. Body: the design's history
//! run by solidcraft-cli, exported as STEP (fixtures/probe_17356_body.step).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)] // probe

use solidcore::blend::fillet_executive::api::fillet_edges;
use solidcore::kernel::{Kernel, KernelOptions};
use solidcore::math::Point3;
use solidcore::query::api_queries::{edge_closest_point, get_edges};
use solidcore::topology::{EdgeId, edge};

const OUTER_EDGE: [f64; 3] = [-41.713_460_951_593_83, -53.930_703_529_926_57, 100.0];

#[test]
fn probe_outer_fillet_after_inner() {
    let text = include_str!("fixtures/probe_17356_body.step");
    let ex = solidcore_step::p21::parse(text).unwrap();
    let read = solidcore_step::topology::read_solids(&ex);
    println!("read: {} solids, {} failures", read.solids.len(), read.failures.len());
    for f in &read.failures {
        println!("failure: {:?}", f.message);
    }
    let solid = read.solids.into_iter().next().unwrap();
    let mut k = Kernel::new(&KernelOptions::default());
    let mut body = solidcore_convert::brep::to_kernel_body(&mut k, &solid.brep, false).unwrap();
    let faces = solidcore::query::api_queries::get_faces(&body).unwrap();
    println!("body faces: {}", faces.len());

    let q = Point3::new(OUTER_EDGE[0], OUTER_EDGE[1], OUTER_EDGE[2]);
    let mut best: (f64, Option<EdgeId>) = (f64::INFINITY, None);
    for e in get_edges(&body).unwrap() {
        if let Ok(c) = edge_closest_point(&k, &body, e, q) {
            if c.closest.distance < best.0 {
                best = (c.closest.distance, Some(e));
            }
        }
    }
    let e = best.1.unwrap();
    println!("picked edge {:?} at distance {}", e, best.0);
    println!("edge faces: {}", edge::faces(&body, e, None).unwrap().len());

    let r = fillet_edges(&mut k, &mut body, &[e], 2.0, 1, 1, 1.0);
    println!("fillet result: {:?}", r.map(|_| "ok"));
}
