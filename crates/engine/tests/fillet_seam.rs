//! #17218: the issue's design (fixtures/fillet_17218_hist.json) replayed on a session, then a
//! 2 mm fillet on edge 1, a seam-crossing edge of a revolved cone. The fillet must build as one
//! closed body.

use serde_json::{Value, json};
use solidcraft_engine::Session;

/// The pick on edge 1 of the issue's design.
const EDGE1: [f64; 3] = [-11.221260709572261, -2.1248221406580754, 7.5317545219050785];

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/fillet_17218_hist.json")).unwrap()
}

#[test]
fn a_fillet_across_a_periodic_seam_builds() {
    let mut s = Session::default();
    s.run_script(&history()).expect("history");
    let params = json!({"body": "Body1", "edges": [EDGE1], "radius": "2 mm"});
    if let Err(e) = s.execute("solid.fillet", &params) {
        panic!("fillet refused: {e}");
    }
    let ws = s.world_state().expect("world state");
    let bodies: Vec<_> = ws.bodies.iter().filter(|b| b.name == "Body1").collect();
    assert_eq!(bodies.len(), 1, "one body named Body1");
}
