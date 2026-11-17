//! Probe for tangency-labs/solidcraft#17218 (no fix): the design's history
//! (fixtures/probe_17218_hist.json) replayed on a session, then the issue's 2 mm fillet on edge 1
//! three ways, with the chain edges of edge 1 listed. Prints what it finds; asserts nothing.

use serde_json::{Value, json};
use solidcraft_engine::Session;

/// The pick on edge 1 (scout: cmd_edge1.json).
const EDGE1: [f64; 3] = [-11.221260709572261, -2.1248221406580754, 7.5317545219050785];
/// The tangency the chain walk uses here (5 degrees).
const ANGLE: f64 = 5.0 * std::f64::consts::PI / 180.0;

fn history() -> Value {
    serde_json::from_str(include_str!("fixtures/probe_17218_hist.json")).unwrap()
}

#[test]
fn probe_17218_edge1_three_ways() {
    let script = history();
    let cases = [
        ("as is", json!({"body": "Body1", "edges": [EDGE1], "radius": "2 mm"})),
        (
            "tangent_chain false",
            json!({"body": "Body1", "edges": [EDGE1], "radius": "2 mm", "tangent_chain": false}),
        ),
        ("1 mm", json!({"body": "Body1", "edges": [EDGE1], "radius": "1 mm"})),
    ];
    for (label, params) in cases {
        let mut s = Session::default();
        let out = s.run_script(&script);
        println!("[p17218] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        match s.execute("solid.fillet", &params) {
            Ok(v) => println!("[p17218] {label}: fillet ok: {v}"),
            Err(e) => println!("[p17218] {label}: fillet refused: {e}"),
        }
    }
}

#[test]
fn probe_17218_edge1_chain() {
    let script = history();
    let mut s = Session::default();
    let out = s.run_script(&script);
    println!("[p17218c] history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
    let ws = s.world_state().unwrap();
    let body = ws.bodies.iter().find(|b| b.name == "Body1").unwrap();
    let ops = solidcore_ops::Body::from_record(body.body.record(), true).unwrap();
    let (_, order) = ops.to_record().unwrap();
    let counts = ops.counts().unwrap();
    // the edge whose middle is nearest the pick (the pick lies on the edge)
    let mut best = (f64::INFINITY, 0usize);
    for e in 0..counts[1] {
        let m = ops.edge_middle(&order, e).unwrap();
        let d = (0..3).map(|i| (m[i] - EDGE1[i]).powi(2)).sum::<f64>().sqrt();
        if d < best.0 {
            best = (d, e);
        }
    }
    let edge = best.1;
    println!("[p17218c] pick edge: record index {edge}, middle distance to pick {:.4} mm", best.0);
    let chain = solidcore_ops::tangent_chain(&ops, Some(&order), &[edge], &[], ANGLE)
        .unwrap();
    println!("[p17218c] chain members {:?}, undecided {:?}", chain.members, chain.undecided);
    for &m in &chain.members {
        let mid = ops.edge_middle(&order, m).unwrap();
        let faces = solidcore_ops::edge_faces_along_ordered(&ops, &order, m, [1.0, 0.0, 0.0]);
        match faces {
            Ok([(f1, _), (f2, _)]) => {
                let n1 = ops.face_normal_at_edge(&order, m, f1).unwrap().value;
                let n2 = ops.face_normal_at_edge(&order, m, f2).unwrap().value;
                let dot = (0..3).map(|i| n1[i] * n2[i]).sum::<f64>().clamp(-1.0, 1.0);
                let turn = dot.acos().to_degrees();
                println!(
                    "[p17218c] edge {m}: middle [{:.3}, {:.3}, {:.3}], faces {f1} {f2}, normal angle {turn:.3} deg, dihedral {:.3} deg",
                    mid[0], mid[1], mid[2], 180.0 - turn
                );
            }
            other => println!("[p17218c] edge {m}: middle {mid:?}, faces: {:?}", other.map(|_| ()).map_err(|e| e.to_string())),
        }
    }
}
