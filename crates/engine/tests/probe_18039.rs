//! Probe for tangency-labs/solidcraft#18039 (no fix): the design's history (fixtures/probe_18039_hist.json,
//! the nine commands before the cut) replayed on a session, then the 360 degree cut Revolve2
//! (fixtures/probe_18039_revolve.json) at 360, 359.9 and 359.99 degrees. Prints what it finds;
//! asserts nothing.

use serde_json::{Value, json};
use solidcraft_engine::Session;

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/probe_18039_hist.json")).unwrap()
}

fn revolve_at(angle: &str) -> Value {
    let mut v: Value = serde_json::from_str(include_str!("fixtures/probe_18039_revolve.json")).unwrap();
    v["commands"][0]["params"]["feature"]["angle"] = json!(angle);
    v
}

#[test]
fn probe_18039_revolve_angles() {
    for (label, angle) in [("360", "360 deg"), ("359.9", "359.9deg"), ("359.99", "359.99deg")] {
        let mut s = Session::default();
        let out = s.run_script(&history());
        println!("[p18039] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        let cmd = revolve_at(angle);
        let params = &cmd["commands"][0]["params"];
        match s.execute("timeline.add", params) {
            Ok(v) => println!("[p18039] {label}: revolve ok: {v}"),
            Err(e) => println!("[p18039] {label}: revolve refused: {e}"),
        }
    }
}
