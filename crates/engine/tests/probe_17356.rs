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

/// PROBE #17356-b (temporary): classify points on the body after the history, with and without
/// Fillet2 (the last command), before any outer fillet.
#[test]
fn probe_17356b_classify_before_outer_fillet() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut without = script.clone();
    let dropped = without["commands"].as_array_mut().and_then(|c| c.pop());
    println!("[p17356b] dropped for the without run: {:?}", dropped.as_ref().map(|d| d["params"]["feature"]["name"].clone()));
    for (label, sc) in [("with Fillet2", &script), ("without Fillet2", &without)] {
        let mut s = Session::default();
        let out = s.run_script(sc);
        println!("[p17356b] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        let ws = match s.world_state() {
            Ok(ws) => ws,
            Err(e) => {
                println!("[p17356b] {label}: no world state {e}");
                continue;
            }
        };
        let Some(body) = ws.bodies.iter().find(|b| b.name == "Body1") else {
            println!("[p17356b] {label}: no Body1");
            continue;
        };
        // M: the section's middle from 17356-a (j=15); the other points move it 0.2 mm toward the axis,
        // then 0.2 mm down, and a plainly inside point at r = 60, same angle, z = 50.
        let (mx, my, mz) = (-41.95180172423388_f64, -52.769215120747496_f64, 99.32524009643143_f64);
        let r = (mx * mx + my * my).sqrt();
        let (ux, uy) = (-mx / r, -my / r);
        let points = [
            [mx, my, mz],
            [mx + 0.2 * ux, my + 0.2 * uy, mz],
            [mx + 0.2 * ux, my + 0.2 * uy, mz - 0.2],
            [60.0 * mx / r, 60.0 * my / r, 50.0],
        ];
        solidcore_ops::probe_17356::classify_probe(body.body.record(), label, &points);
    }
}

/// PROBE #17356-c (temporary): on the body with Fillet2, the shell ray solve from M (classify fires
/// are printed by the -b probe), the surface-level roots of the line with Fillet2's rounds, and the
/// first face hit from a point before the bore's entry.
#[test]
fn probe_17356c_surface_roots() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut s = Session::default();
    let out = s.run_script(&script);
    println!("[p17356c] history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
    let ws = s.world_state().unwrap();
    let body = ws.bodies.iter().find(|b| b.name == "Body1").unwrap();
    let (mx, my, mz) = (-41.95180172423388_f64, -52.769215120747496_f64, 99.32524009643143_f64);
    let r = (mx * mx + my * my).sqrt();
    let (ux, uy) = (-mx / r, -my / r);
    let points = [[mx, my, mz], [mx + 0.2 * ux, my + 0.2 * uy, mz]];
    solidcore_ops::probe_17356::classify_probe(body.body.record(), "with Fillet2", &points);
    solidcore_ops::probe_17356c::surface_probe(body.body.record(), "with Fillet2");
}

/// PROBE #17356-d (temporary): with Fillet2, classify M only (the ray solve of each fire is printed
/// by the ray_fire_with_bbox_and_solver probe), then the faces near the round and their distance to
/// the entry point (-12.187, M_Y, M_Z).
#[test]
fn probe_17356d_m_and_round_faces() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut s = Session::default();
    let out = s.run_script(&script);
    println!("[p17356d] history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
    let ws = s.world_state().unwrap();
    let body = ws.bodies.iter().find(|b| b.name == "Body1").unwrap();
    let m = [-41.95180172423388_f64, -52.769215120747496_f64, 99.32524009643143_f64];
    solidcore_ops::probe_17356::classify_probe(body.body.record(), "with Fillet2 M only", &[m]);
    let entry = [-12.186985155_f64, -52.769215120747496_f64, 99.32524009643143_f64];
    solidcore_ops::probe_17356d::face_list_probe(body.body.record(), "with Fillet2", entry);
}

/// PROBE #17356-e (temporary): the sides of the hit at H (with Fillet2, FaceId 34) and of the control
/// hit (without Fillet2, FaceId 20): normals, faceuse orientation, dot with +x, and the regions on
/// each side (classify along +x; a ray fire along +z). The sector decisions print with [p17356e].
#[test]
fn probe_17356e_sides_of_the_hit() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut without = script.clone();
    let _ = without["commands"].as_array_mut().and_then(|c| c.pop());
    for (label, sc, target, face) in [
        ("with Fillet2", &script, [12.166435715262917_f64, -52.769215120747496_f64, 99.32524009643144_f64], "FaceId(34)"),
        ("without Fillet2", &without, [4.4652078144584335_f64, -52.76921512074749_f64, 99.32524009643143_f64], "FaceId(20)"),
    ] {
        let mut s = Session::default();
        let out = s.run_script(sc);
        println!("[p17356e] {label}: history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
        let ws = s.world_state().unwrap();
        let body = ws.bodies.iter().find(|b| b.name == "Body1").unwrap();
        solidcore_ops::probe_17356e::side_probe(body.body.record(), label, target, face);
    }
}

/// PROBE #17356-f (temporary): with Fillet2, walk from M to H-0.3 along x (region by the +z fire),
/// bisect the first region change to X0, find the face of 30..38 at X0, and solve the long segment
/// M-H and the short segment X0-1..X0+1 through that face's surface.
#[test]
fn probe_17356f_walk_and_roots() {
    let script: Value = serde_json::from_str(include_str!("fixtures/probe_17356_hist.json")).unwrap();
    let mut s = Session::default();
    let out = s.run_script(&script);
    println!("[p17356f] history {:?}", out.as_ref().map(|v| v.len()).map_err(|e| e.to_string()));
    let ws = s.world_state().unwrap();
    let body = ws.bodies.iter().find(|b| b.name == "Body1").unwrap();
    let m = [-41.95180172423388_f64, -52.769215120747496_f64, 99.32524009643143_f64];
    let h_minus = [11.866435715262917_f64, -52.769215120747496_f64, 99.32524009643144_f64];
    solidcore_ops::probe_17356f::walk_probe(body.body.record(), "with Fillet2", m, h_minus, -52.769215120747496, 99.32524009643144);
}
