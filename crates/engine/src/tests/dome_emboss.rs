use serde_json::json;
use solidcore_record::Vec3;
use solidcraft_kernel::PointClass;

use super::{run, volume};
use crate::Session;

fn dome() -> Session {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    s
}

/// A rectangle 6 x 4 on the plane `plane`, embossed 1 mm onto `body` where it projects: what it
/// adds to the volume, or the refusal.
fn emboss_mark(s: &mut Session, plane: serde_json::Value, p0: [f64; 2], p1: [f64; 2], body: &str) -> Result<f64, String> {
    let before = volume(s);
    run(s, "sketch.create", json!({"plane": plane, "name": "Mark"}));
    run(s, "sketch.rectangle.two_point", json!({"p0": p0, "p1": p1}));
    run(s, "sketch.finish", json!({}));
    s.execute("solid.emboss", &json!({"sketch": "Mark", "depth": 1, "targets": body})).map_err(|e| e.to_string())?;
    Ok(volume(s) - before)
}

/// #21347: a dome whose revolved face reaches its axis takes an emboss far from the pole, on the
/// side picked.
#[test]
fn an_emboss_on_a_dome_reaching_its_axis_is_built() {
    let mut s = dome();
    let before = volume(&mut s);
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, -60, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Mark"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [-3, 8], "p1": [3, 12]}));
    run(&mut s, "sketch.finish", json!({}));
    run(
        &mut s,
        "solid.emboss",
        json!({"sketch": "Mark", "depth": 1, "face": [14.634538880876073, 13.56013895018432, 0.7603539285228216], "targets": "Dome"}),
    );
    let added = volume(&mut s) - before;
    assert!((added - 24.594).abs() < 0.05, "{added}");
    // The dome's radius at z = 10 is 19.05: half the raise out on the picked side, none on the other.
    let points = [Vec3::new(0.0, 19.55, 10.0), Vec3::new(0.0, -19.55, 10.0)];
    let classes = s.model.state().body("Dome").unwrap().body.classify_points(&points).unwrap();
    assert_eq!(classes, [PointClass::Inside, PointClass::Outside]);
}

/// Over the pole, the dome's emboss is refused by name.
#[test]
fn an_emboss_over_a_dome_s_pole_is_refused_by_name() {
    let mut s = dome();
    let top = json!({"origin": [0, 0, 40], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]});
    let refused = emboss_mark(&mut s, top, [-3.0, -2.0], [3.0, 2.0], "Dome").unwrap_err();
    assert!(refused.contains("the profiles cover a pole of the selected face"), "{refused}");
}

/// A loft between two spline rings has a freeform side: the emboss on it is cut down to the part
/// under the profile, and adds what it added when the whole side was offset.
#[test]
fn an_emboss_on_a_lofted_side_is_built() {
    let mut s = Session::default();
    let ring = |r: f64| (0..8).map(|i| f64::from(i) * std::f64::consts::FRAC_PI_4).map(|t| [r * t.cos(), 0.75 * r * t.sin()]).collect::<Vec<_>>();
    for (name, x, r) in [("Low", -30, 20.0), ("High", 30, 15.0)] {
        run(&mut s, "sketch.create", json!({"plane": {"origin": [x, 0, 0], "x_dir": [0, 1, 0], "y_dir": [0, 0, 1]}, "name": name}));
        run(&mut s, "sketch.spline.fit_point", json!({"points": ring(r), "periodic": true}));
        run(&mut s, "sketch.finish", json!({}));
    }
    run(&mut s, "solid.loft", json!({"sections": [{"sketch": "Low"}, {"sketch": "High"}], "body_name": "Loft", "operation": "new"}));
    let top = json!({"origin": [0, 0, 40], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]});
    let t = std::time::Instant::now();
    let added = emboss_mark(&mut s, top, [-3.0, -2.0], [3.0, 2.0], "Loft").unwrap();
    eprintln!("LOFT added {added} in {:?}", t.elapsed());
}

/// A circle swept along a spline has a freeform wall: the emboss on it is built.
#[test]
fn an_emboss_on_a_swept_wall_is_built() {
    let mut s = Session::default();
    let profile = run(&mut s, "sketch.create", json!({"plane": "XY"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 4}));
    run(&mut s, "sketch.finish", json!({}));
    let path =
        run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}}))["sketch"].as_u64().unwrap();
    let wire = run(&mut s, "sketch.spline.control_point", json!({"points": [[0, 0], [0, 20], [15, 35], [40, 40]]}))["curves"][0]
        .as_str()
        .unwrap()
        .to_string();
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.sweep", json!({"sketch": profile, "path_sketch": path, "path": [wire], "operation": "new", "body_name": "Pipe"}));
    let front = json!({"origin": [0, -40, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]});
    let t = std::time::Instant::now();
    let added = emboss_mark(&mut s, front, [-1.5, 5.0], [1.5, 9.0], "Pipe").unwrap();
    eprintln!("SWEEP added {added} in {:?}", t.elapsed());
}
