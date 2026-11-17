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
    sound(&s);
    let (got, want) = (volume(&s), 8854.3089);
    assert!((got - want).abs() < 1e-6 * want, "volume {got}, want {want}");
}

fn volume(s: &Session) -> f64 {
    s.model.state().bodies.iter().map(|b| solidcraft_kernel::measure(&b.body).map(|m| m.volume).unwrap_or(f64::NAN)).sum()
}

fn sound(s: &Session) {
    let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let st = s.model.state();
    assert_eq!(st.bodies.len(), 1, "one body");
    for b in &st.bodies {
        assert_eq!(b.body.shell_count(), 1, "one shell");
        assert!(b.body.validity().is_empty(), "{:?}", b.body.validity());
    }
}
