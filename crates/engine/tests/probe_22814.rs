use serde_json::json;
use solidcraft_engine::Session;

fn notch(d: f64) -> Result<f64, String> {
    let mut s = Session::default();
    let a = (0.06 + d) / 0.5;
    s.execute("solid.cone", &json!({"radius": 3, "top_radius": 0.5, "height": 1000, "base": [0, 0, 0], "operation": "new", "body_name": "Part"})).map_err(|e| e.to_string())?;
    s.execute("solid.fillet", &json!({"edges": [[0, -0.5, 1000]], "radius": 0.05})).map_err(|e| e.to_string())?;
    s.execute("solid.cylinder", &json!({"radius": 0.06, "height": 0.5, "base": [0.5 * a.cos(), 0.5 * a.sin(), 999.5], "axis": "Z", "operation": "cut", "targets": ["Part"]})).map_err(|e| e.to_string())?;
    Ok(s.model.state().body("Part").map(|b| solidcraft_kernel::measure(&b.body).unwrap().volume).unwrap_or(0.0))
}

#[test]
fn probe_22814() {
    let mut bad = vec![];
    for d in [2e-5, -1e-5] {
        let r = notch(d);
        println!("PROBE d={d:e}: {r:?}");
        if r.is_err() { bad.push(d); }
    }
    assert!(bad.is_empty(), "refused at {bad:?}");
}
