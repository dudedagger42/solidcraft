//! Probe for tangency-labs/solidcraft#17356 (no fix): the design's history (fixtures/probe_17356_hist.json,
//! its command script) replayed on a session, then the 2 mm fillet on the outer top edge through
//! (-41.713, -53.931, 100). The issue's refusal: CutsThePart.

use serde_json::{Value, json};
use solidcraft_engine::Session;

#[test]
fn probe_outer_fillet_after_inner() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut s = Session::default();
    let out = s.run_script(&script);
    println!("history: {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
    let r = s.execute("solid.fillet", &json!({"body": "Body1", "edges": [[-41.71346095159383, -53.93070352992657, 100.0]], "radius": "2 mm"}));
    match r {
        Ok(v) => println!("fillet ok: {v}"),
        Err(e) => println!("fillet refused: {e}"),
    }
}
