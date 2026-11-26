//! Old open bug issues, replayed with the reporter's own commands (or the closest engine
//! command) to see whether upstream main still fails them. One test per issue, named by number.

use serde_json::{Value, json};
use std::f64::consts::PI;

use crate::Session;

fn replay(script: &str) -> Session {
    let mut s = Session::default();
    let script: Value = serde_json::from_str(script).unwrap();
    s.run_script(&script).unwrap();
    s
}

fn total(s: &Session) -> f64 {
    s.model.state().bodies.iter().map(|b| solidcraft_kernel::measure(&b.body).unwrap().volume).sum()
}

fn feature_errors(s: &Session) -> Vec<(String, String)> {
    s.model.results.iter().filter_map(|r| r.error.clone().map(|e| (r.name.clone(), e))).collect()
}

/// #114: a circle the same size as a cylinder's top, joined on top of it with no taper.
#[test]
fn issue_114_extrude_join_on_curved_body_with_zero_taper() {
    let mut s = Session::default();
    s.execute("solid.cylinder", &json!({"radius": 10, "height": 10, "base": [0, 0, 0], "axis": "Z", "body_name": "Base"})).unwrap();
    s.execute("sketch.create", &json!({"plane": "XY", "offset": 10})).unwrap();
    s.execute("sketch.circle.center", &json!({"center": [0, 0], "radius": 10})).unwrap();
    s.execute("sketch.finish", &json!({})).unwrap();
    s.execute("solid.extrude", &json!({"distance": 5, "operation": "join", "profiles": [0]})).unwrap();
    let want = PI * 100.0 * 15.0;
    let v = total(&s);
    assert!((v - want).abs() < 1e-3 * want, "{v} vs {want}");
}

/// #500: a radius 20 cylinder cut out of a coaxial radius 40 disc spanning x -1..1 lost 2.6%.
#[test]
fn issue_500_coaxial_cylinder_cut_from_thin_disc_keeps_its_volume() {
    let mut s = Session::default();
    s.execute("solid.cylinder", &json!({"radius": 40, "height": 2, "base": [-1, 0, 0], "axis": "X", "body_name": "Disc"})).unwrap();
    s.execute(
        "solid.cylinder",
        &json!({"radius": 20, "height": 6, "base": [-3, 0, 0], "axis": "X", "operation": "cut", "targets": ["Disc"]}),
    )
    .unwrap();
    let want = PI * (40.0 * 40.0 - 20.0 * 20.0) * 2.0;
    let v = total(&s);
    assert!((v - want).abs() < 1e-3 * want, "{v} vs {want}");
}

/// #501: one edge flange over all four edges of a base flange, with a different angle on each pair.
#[test]
fn issue_501_edge_flange_with_different_angles_closes_its_corners() {
    let mut s = replay(
        &json!({"commands": [
            {"command": "sketch.create", "params": {"plane": "XY", "name": "Base"}},
            {"command": "sketch.rectangle.two_point", "params": {"p0": [-45, -35], "p1": [45, 35]}},
            {"command": "sketch.finish", "params": {}},
            {"command": "sheet.flange", "params": {"sketch": "Base"}},
            {"command": "sheet.flange", "params": {
                "edges": [[45, 0, 0], [-45, 0, 0], {"point": [0, 35, 0], "angle": "60 deg"}, {"point": [0, -35, 0], "angle": "60 deg"}],
                "height": 18,
                "angle": "75 deg"
            }}
        ]})
        .to_string(),
    );
    let errors = feature_errors(&s);
    assert!(errors.is_empty(), "{errors:?}");
    for b in &s.model.state().bodies {
        assert!(b.body.validity().is_empty(), "{:?}", b.body.validity());
    }
}

/// #559: a 1 mm through-all hole straight down into the 2 mm rim of a shelled box. The rim is the
/// wall at y 23..25 and the hole is at y 24, so it removes pi * 0.25 * 30 from 19992.
#[test]
fn issue_559_through_all_hole_into_shelled_box_rim() {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"corner": [-35, -25, 0], "length": 70, "width": 50, "height": 30})).unwrap();
    s.execute("solid.shell", &json!({"faces": [[0, 0, 30]], "thickness": 2})).unwrap();
    s.execute("solid.hole", &json!({"position": [0, 24, 30], "diameter": 1})).unwrap();
    let want = 70.0 * 50.0 * 30.0 - 66.0 * 46.0 * 28.0 - PI * 0.25 * 30.0;
    let v = total(&s);
    assert!((v - want).abs() < 0.5, "{v} vs {want}");
}

/// #562: pulling a floor down 1 mm when its neighbours are rounds (vertical corner fillets).
/// The floor grows by its footprint, 2400 less four corners of 9 * (1 - pi / 4) * 4 each.
#[test]
fn issue_562_press_pull_floor_beside_rounds() {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"corner": [0, 0, 0], "length": 60, "width": 40, "height": 20, "body_name": "Case"})).unwrap();
    s.execute(
        "solid.fillet",
        &json!({"body": "Case", "radius": 3, "edges": [[0, 0, 10], [60, 0, 10], [0, 40, 10], [60, 40, 10]]}),
    )
    .unwrap();
    let before = total(&s);
    s.execute("solid.press_pull", &json!({"face": [30, 20, 0], "distance": 1})).unwrap();
    let grew = total(&s) - before;
    let want = 2400.0 - 4.0 * 9.0 * (1.0 - PI / 4.0);
    assert!((grew - want).abs() < 1.0, "grew {grew} vs {want}");
}

/// #793: a coil split by a plane square to its axis ("boolean Intersect: no result").
#[test]
fn issue_793_coil_split_by_a_plane_across_its_axis() {
    let mut s = Session::default();
    s.execute(
        "solid.coil",
        &json!({"diameter": "40 mm", "revolutions": 5, "height": "40 mm", "section_size": "2 mm", "body_name": "C"}),
    )
    .unwrap();
    let before = total(&s);
    s.execute("solid.split_body", &json!({"body": "C", "plane": {"origin": [0, 0, 10], "normal": [0, 0, 1]}})).unwrap();
    let after = total(&s);
    assert!((after - before).abs() < 0.01 * before, "{before} -> {after}");
}

/// #1669: a window and a lofted pleat in a case shelled 3.5 mm, mirrored to the far end.
#[test]
fn issue_1669_mirrored_pleat_joins_into_a_thick_case_wall() {
    let s = replay(include_str!("../../../examples/regressions/battery_case_pleats.json"));
    let errors = feature_errors(&s);
    assert!(errors.is_empty(), "{errors:?}");
}
