use serde_json::json;

use super::run;
use crate::Session;

fn coil() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.coil", json!({"diameter": "40 mm", "revolutions": 5, "height": "40 mm", "section_size": "2 mm", "body_name": "C"}));
    s
}

#[test]
fn coil_split_by_plane_across_axis_793() {
    let before = run(&mut coil(), "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    for z in [10.0, 10.3, 30.0] {
        let mut s = coil();
        let r = s.execute("solid.split_body", &json!({"body": "C", "plane": {"origin": [0, 0, z], "normal": [0, 0, 1]}}));
        assert!(r.is_ok(), "z = {z}: {}", r.unwrap_err());
        let m = run(&mut s, "inspect.measure", json!({}));
        let after = m["total"]["volume_mm3"].as_f64().unwrap();
        assert_eq!(m["bodies"].as_array().map(|b| b.len()), Some(2), "z = {z}: {m}");
        assert!((after - before).abs() < 1e-3 * before, "z = {z}: {before} -> {after}");
    }
}
