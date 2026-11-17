//! Probe for tangency-labs/solidcraft#17356 (no fix): the 2 mm fillet on the outer top edge
//! through (-41.713, -53.931, 100) of the reporter's body, after the inner round (Fillet2),
//! is refused as CutsThePart with a largest radius of 0.241 mm. The body is the design's
//! history run by solidcraft-cli, exported as STEP (probe_17356_body.step).

use solidcore_record::Vec3;
use solidcraft_kernel::{fillet, measure, step_import};

#[test]
fn probe_outer_fillet_after_inner() {
    let text = include_str!("probe_17356_body.step");
    let read = step_import(text).unwrap();
    println!("import: {} bodies, warnings {:?}", read.bodies.len(), read.warnings);
    let body = &read.bodies[0].body;
    let m = measure(body).unwrap();
    println!("body: faces {} volume {}", m.faces, m.volume);
    let edge = Vec3::new(-41.713_460_951_593_83, -53.930_703_529_926_57, 100.0);
    let r = fillet(body, &[edge], 2.0);
    match &r {
        Ok(b) => println!("fillet 2 mm: ok, faces {}", measure(b).unwrap().faces),
        Err(e) => println!("fillet 2 mm: refused: {e}"),
    }
}
