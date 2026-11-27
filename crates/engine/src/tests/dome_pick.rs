use serde_json::json;

use super::run;
use crate::Session;

/// A point of the revolved dome's face, its profile's cubic Bezier at `t` turned `turn` about the
/// axis: exactly on the face.
fn on_dome(t: f64, turn: f64) -> [f64; 3] {
    let (a, b) = (1.0 - t, t);
    let w = [a * a * a, 3.0 * a * a * b, 3.0 * a * b * b, b * b * b];
    let (r, z) = [[20.0, 0.0], [20.0, 20.0], [10.0, 30.0], [0.0, 30.0]].iter().zip(w).fold((0.0, 0.0), |(r, z), (p, w)| (r + w * p[0], z + w * p[1]));
    [r * turn.cos(), r * turn.sin(), z]
}

/// A point of the dome's curved face where its profile lies (y = 0) is a face pick for an emboss.
#[test]
fn a_point_on_a_revolved_dome_is_on_its_face() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 40], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Mark"}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 30}));
    run(&mut s, "sketch.finish", json!({}));
    let at = on_dome(0.65, 0.0);
    let r = s.execute("solid.emboss", &json!({"sketch": "Mark", "depth": 0.5, "face": at, "targets": "Dome"}));
    if let Err(e) = r {
        assert!(!e.to_string().contains("lies on no face"), "{e}");
    }
}
