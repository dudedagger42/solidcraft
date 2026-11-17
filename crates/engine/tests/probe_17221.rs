//! Probe for tangency-labs/solidcraft#17221 (no fix): the design's history (fixtures/probe_17221_hist.json,
//! the scout's hist.json) replayed on a session, then the 2 mm fillet on the edge at (23.93, -7.23, 12.5),
//! with the default tangent chain and with tangent_chain false.

use serde_json::{Value, json};
use solidcraft_engine::Session;

#[test]
fn probe_17221_edge2_at_2mm() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17221_hist.json")).unwrap();
    for (label, params) in [
        ("as is", json!({"body": "Body1", "edges": [[23.930747057609892, -7.230698087075147, 12.5]], "radius": "2 mm"})),
        ("tangent_chain false", json!({"body": "Body1", "edges": [[23.930747057609892, -7.230698087075147, 12.5]], "radius": "2 mm", "tangent_chain": false})),
    ] {
        let mut s = Session::default();
        let out = s.run_script(&script);
        println!("[p17221] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        match s.execute("solid.fillet", &params) {
            Ok(v) => println!("[p17221] {label}: fillet ok: {v}"),
            Err(e) => println!("[p17221] {label}: fillet refused: {e}"),
        }
    }
}
