use serde_json::json;

use super::{run, volume};
use crate::Session;

/// #21347: a dome whose revolved face reaches its axis takes an emboss far from the pole.
#[test]
fn an_emboss_on_a_dome_reaching_its_axis_is_built() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Profile"}));
    run(&mut s, "sketch.spline.control_point", json!({"points": [[20, 0], [20, 20], [10, 30], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.revolve", json!({"sketch": "Profile", "axis": "y", "operation": "new", "body_name": "Dome"}));
    let before = volume(&mut s);
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, -60, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Mark"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [-3, 8], "p1": [3, 12]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.emboss", json!({"sketch": "Mark", "depth": 1, "face": [14.634538880876073, 13.56013895018432, 0.7603539285228216], "targets": "Dome"}));
    let added = volume(&mut s) - before;
    eprintln!("ADDED {added}");
    assert!((20.0..30.0).contains(&added), "{added}");
}
