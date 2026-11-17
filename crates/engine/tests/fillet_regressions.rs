//! Fillet regressions: a blend the kernel refused on a design whose fillets fit (#17356).
//!
//! The design (fixtures/fillet_17356_hist.json) ends with a 2 mm round on its inner top edges.
//! Its outer top edges then take a 2 mm round (fixtures/fillet_17356_issue.json), which the
//! kernel refused as "the blend would cut the part in pieces", although the other order builds.
use serde_json::Value;
use solidcraft_engine::Session;

#[test]
fn outer_top_fillet_after_the_inner_round_builds_one_closed_body() {
    let history: Value = serde_json::from_str(include_str!("fixtures/fillet_17356_hist.json")).unwrap();
    let issue: Value = serde_json::from_str(include_str!("fixtures/fillet_17356_issue.json")).unwrap();
    let mut s = Session::default();
    s.run_script(&history).unwrap();
    let cmd = &issue["commands"][0];
    let r = s.execute(cmd["command"].as_str().unwrap(), &cmd["params"]);
    assert!(r.is_ok(), "fillet refused: {}", r.err().map(|e| e.to_string()).unwrap_or_default());
    let ws = s.world_state().unwrap();
    assert_eq!(ws.bodies.len(), 1, "one body after the fillet");
}
