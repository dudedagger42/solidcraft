use serde_json::json;

use super::run;
use crate::Session;

#[test]
fn emboss_on_a_dome_that_reaches_the_axis_21347() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    let before = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, -60, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Mark"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [-3, 8], "p1": [3, 12]}));
    run(&mut s, "sketch.finish", json!({}));
    let r = s.execute("solid.emboss", &json!({"sketch": "Mark", "depth": 1, "face": [14.634538880876073, 13.56013895018432, 0.7603539285228216], "targets": "Dome"}));
    assert!(r.is_ok(), "emboss refused: {}", r.unwrap_err());
    let after = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    assert!(after > before + 10.0, "{before} -> {after}");
}

fn dome(last: [f64; 2], tail: &[[f64; 2]]) -> solidcraft_kernel::Body {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], last]}));
    let mut pts = vec![last];
    pts.extend_from_slice(tail);
    run(&mut s, "sketch.line", json!({"points": pts}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    s.model.state().body("Dome").map(|b| b.body.clone()).unwrap()
}

#[test]
fn diag_cropped_dome_offset_21347() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    let r = run(&mut s, "solid.split_body", json!({"body": "Dome", "plane": {"origin": [0, 0, 25], "normal": [0, 0, 1]}}));
    eprintln!("DIAG split: {r}");
    let st = s.model.state();
    for mb in st.bodies.iter() {
        let b = &mb.body;
        for f in 0..b.face_count() {
            if solidcraft_kernel::face_surface(b, f).is_some() { continue; }
            for d in [-0.2, 0.2] {
                solidcraft_kernel::failure::clear();
                let o = solidcraft_kernel::offset_surface(b, &[f], d, 1e-4);
                eprintln!("DIAG body {} bbox {:?} face {f} offset {d}: {:?} last={:?}", mb.name, b.exact_bounds().ok(), o.as_ref().map(|x| x.face_count()), solidcraft_kernel::failure::last());
            }
        }
    }
    panic!("diag");
}

#[test]
fn diag_dome_offsets_21347() {
    for (name, b) in [("axis", dome([0.0, 30.0], &[[0.0, 0.0], [20.0, 0.0]])), ("short", dome([5.0, 30.0], &[[0.0, 30.0], [0.0, 0.0], [20.0, 0.0]]))] {
        for f in 0..b.face_count() {
            let kind = solidcraft_kernel::face_surface(&b, f);
            eprintln!("DIAG {name} face {f}: {kind:?}");
            for d in [-0.2, 0.2, -1.0] {
                solidcraft_kernel::failure::clear();
                let r = solidcraft_kernel::offset_surface(&b, &[f], d, 1e-4);
                eprintln!("DIAG {name} face {f} offset {d}: {:?} last={:?}", r.as_ref().map(|s| s.face_count()), solidcraft_kernel::failure::last());
            }
        }
        eprintln!("DIAG {name} record: {:?}", b.record().faces.len());
    }
    panic!("diag");
}
