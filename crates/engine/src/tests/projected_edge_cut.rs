//! #458: a cut whose profile is a projected top edge closed by a fit spline leaves a sliver edge.

use serde_json::json;

use super::run;
use crate::Session;

#[test]
fn cut_from_a_projected_edge_and_spline_leaves_no_sliver_edge() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"corner": [0, 0, 0], "length": 20, "width": 10, "height": 20, "body_name": "Part"}));
    run(&mut s, "sketch.create", json!({"plane": {"face": [10, 5, 20]}, "name": "S"}));
    run(&mut s, "sketch.project", json!({"refs": [{"edge": [10, 0, 20]}]}));
    run(&mut s, "sketch.spline.fit_point", json!({"points": [[0, 0], [6, 4], [14, 4], [20, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(
        &mut s,
        "solid.extrude",
        json!({"sketch": "S", "distance": "-11.5 mm", "direction": "positive", "operation": "cut", "profiles": [{"point": [10, 1]}]}),
    );
    let edges = run(&mut s, "model.edges", json!({"body": "Part"}));
    let shortest = edges["edges"]
        .as_array()
        .unwrap()
        .iter()
        .min_by(|a, b| a["length"].as_f64().unwrap().total_cmp(&b["length"].as_f64().unwrap()))
        .unwrap()
        .clone();
    assert!(shortest["length"].as_f64().unwrap() > 1e-3, "sliver edge: {shortest}");
}
