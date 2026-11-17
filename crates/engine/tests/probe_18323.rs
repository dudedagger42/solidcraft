//! Probe for tangency-labs/solidcraft#18323 (no fix): the design's history
//! (fixtures/probe_18323_hist.json) replayed on a session, then the shell of Body1 at the
//! bottom face at 2.094, 2.125 and 2.6 mm, tangent chain on. Prints what it finds; asserts
//! nothing.

use serde_json::{Value, json};
use solidcraft_engine::Session;

/// The bottom face of the body (scout: runs/chk_2.094.json).
const FACE: [f64; 3] = [-7.505902267054012, -21.963322302444283, 1.1368683772161603e-13];

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/probe_18323_hist.json")).unwrap()
}

#[test]
fn probe_18323_shell_thickness() {
    let script = history();
    for t in ["2.094 mm", "2.125 mm", "2.6 mm"] {
        let mut s = Session::default();
        let out = s.run_script(&script);
        println!("[p18323] {t}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        let params = json!({"body": "Body1", "faces": [FACE], "tangent_chain": true, "thickness": t});
        match s.execute("solid.shell", &params) {
            Ok(v) => println!("[p18323] {t}: shell ok: {v}"),
            Err(e) => println!("[p18323] {t}: shell refused: {e}"),
        }
    }
}
