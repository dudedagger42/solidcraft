//! Regressions for revolves whose 360 degree seam runs beside a near-miss (#18039): a 360 degree
//! cut about the Z axis, after the design's history, must not be refused by the seam check.

use serde_json::Value;
use solidcraft_engine::Session;

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/issue_18039_hist.json")).unwrap()
}

#[test]
fn revolve_360_cut_after_history_has_no_seam_error() {
    let mut s = Session::default();
    s.run_script(&history()).unwrap();
    let cmd: Value =
        serde_json::from_str(include_str!("fixtures/issue_18039_revolve.json")).unwrap();
    let v = s.execute("timeline.add", &cmd["commands"][0]["params"]).unwrap();
    assert!(v["error"].is_null(), "360 degree revolve refused: {v}");
}
