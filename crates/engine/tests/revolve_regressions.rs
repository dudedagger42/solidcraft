//! Revolve regressions: designs whose revolve was refused or built wrong.

use serde_json::Value;
use solidcraft_engine::Session;

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/issue_18039_hist.json")).unwrap()
}

/// A 360 degree cut about the Z axis, after the design's history, is not refused by the seam
/// check: its seam runs beside a near miss where the seam leaves a face (#18039).
#[test]
fn revolve_360_cut_after_history_has_no_seam_error() {
    let mut s = Session::default();
    s.run_script(&history()).unwrap();
    let cmd: Value = serde_json::from_str(include_str!("fixtures/issue_18039_revolve.json")).unwrap();
    let v = s.execute("timeline.add", &cmd["commands"][0]["params"]).unwrap();
    assert!(v["error"].is_null(), "360 degree revolve refused: {v}");
}
