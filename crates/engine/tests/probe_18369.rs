//! Probe for tangency-labs/solidcraft#18369 (no fix): thicken of the 3-face HoodLower sheet
//! (fixtures/probe_18369_hist.json) at -0.5, -1, +1 and +2 mm, through `execute` so per-command
//! errors show, and the sheet's edges and face continuity. Prints what it finds; asserts nothing.

use serde_json::{Value, json};
use solidcraft_engine::Session;

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/probe_18369_hist.json")).unwrap()
}

fn thicken(thickness: &str) -> Value {
    json!({"bodies": ["HoodLower"], "thickness": thickness, "symmetric": false, "operation": "new"})
}

#[test]
fn probe_18369_thicken_sweep() {
    for (label, t) in [("-0.5", "-0.5 mm"), ("-1", "-1 mm"), ("+1", "1 mm"), ("+2", "2 mm")] {
        let mut s = Session::default();
        let out = s.run_script(&history());
        println!("[p18369] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        match s.execute("surface.thicken", &thicken(t)) {
            Ok(v) => println!("[p18369] {label}: thicken ok: {v}"),
            Err(e) => println!("[p18369] {label}: thicken refused: {e}"),
        }
    }
}

#[test]
fn probe_18369_sheet_continuity() {
    let mut s = Session::default();
    let _ = s.run_script(&history());
    let ws = s.world_state().unwrap();
    let names: Vec<String> = ws.bodies.iter().map(|b| b.name.clone()).collect();
    println!("[p18369d] bodies {names:?}");
    let body = ws
        .bodies
        .iter()
        .find(|b| b.name == "HoodLower")
        .or_else(|| ws.bodies.first())
        .unwrap();
    let (ops, order) = solidcore_ops::Body::from_record(body.body.record(), true).unwrap();
    let counts = ops.counts().unwrap();
    println!("[p18369d] {}: faces {}, edges {}", body.name, counts[0], counts[1]);
    for f in 0..counts[0] {
        match ops.face_plane_normal(&order, f) {
            Ok(Some(n)) => println!("[p18369d] face {f}: planar, normal {:?}", n.value),
            Ok(None) => println!("[p18369d] face {f}: not planar"),
            Err(e) => println!("[p18369d] face {f}: normal error {e}"),
        }
    }
    for e in 0..counts[1] {
        match solidcore_ops::edge_faces_along_ordered(&ops, &order, e, [0.0, 0.0, 1.0]) {
            Ok([(f1, _), (f2, _)]) => {
                let n1 = ops.face_normal_at_edge(&order, e, f1).unwrap().value;
                let n2 = ops.face_normal_at_edge(&order, e, f2).unwrap().value;
                let dot = (0..3).map(|i| n1[i] * n2[i]).sum::<f64>().clamp(-1.0, 1.0);
                let mid = ops.edge_middle(&order, e).unwrap();
                println!(
                    "[p18369d] edge {e}: middle [{:.3}, {:.3}, {:.3}], faces {f1} {f2}, normal angle {:.3} deg",
                    mid[0], mid[1], mid[2], dot.acos().to_degrees()
                );
            }
            other => println!("[p18369d] edge {e}: faces {:?}", other.map(|_| ()).map_err(|e| e.to_string())),
        }
    }
}
