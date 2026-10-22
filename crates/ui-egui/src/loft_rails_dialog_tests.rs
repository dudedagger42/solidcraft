//! The Loft and Surface Loft dialogs' guide rails (#201): rails picked in the dialog reach the
//! commands with their sketches and the fit, and editing the loft brings them back and keeps them.
use crate::dialogs::Kind;
use crate::dialogs_surface::Surf;
use crate::scenario::Harness;
use serde_json::json;
use solidcraft_engine::Sel;
use solidcraft_engine::doc::{FeatureKind, RailFit, SketchId, SweepRail};

/// Circles `Base` (z = 0) and `Top` (z = 40) and in `R` on XZ a rail `out` from (10, 0, 0) out to
/// x = 15 halfway and back to (10, 0, 40); the circles' and the rail's sketch ids.
fn setup(h: &mut Harness) -> (u64, u64, u64) {
    for (name, z) in [("Base", 0), ("Top", 40)] {
        h.app.run("sketch.create", json!({"name": name, "plane": {"origin": [0, 0, z], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}})).unwrap();
        h.app.run("sketch.circle.center", json!({"center": [0, 0], "radius": 10})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
    }
    h.app.run("sketch.create", json!({"plane": "XZ", "name": "R"})).unwrap();
    h.app.run("sketch.arc.three_point", json!({"start": [10, 0], "end": [10, -40], "through": [15, -20], "id": "out"})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    let id = |h: &Harness, n: &str| h.app.session.doc.find_feature(n).unwrap().id;
    (id(h, "Base"), id(h, "Top"), id(h, "R"))
}

fn ok(h: &mut Harness) {
    h.frames(2);
    h.step(&json!({"widget":"OK"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?} {:?}", h.app.dialog.as_ref().map(|d| &d.error), h.app.session.log);
}

#[test]
fn a_rail_picked_in_the_loft_dialog_is_kept_through_an_edit() {
    let mut h = Harness::new();
    let (base, top, rail_sketch) = setup(&mut h);
    h.step(&json!({"call":"ui.start","params":{"command":"solid.loft"}})).unwrap();
    h.frames(3);
    assert!(h.widget("Rail").is_some() && h.widget("Rail Fit").is_some());
    let d = h.app.dialog.as_mut().unwrap();
    d.inputs[0].items = vec![Sel::Profile { sketch: base, index: 0 }, Sel::Profile { sketch: top, index: 0 }];
    d.inputs[1].items = vec![Sel::SketchCurve { id: "out".into() }];
    let Kind::Loft { rail_fit, .. } = &mut d.kind else { panic!("{:?}", d.kind) };
    *rail_fit = 1;
    ok(&mut h);
    let name = h.app.session.doc.features.last().unwrap().name.clone();
    let rails = |h: &Harness| match &h.app.session.doc.find_feature(&name).unwrap().kind {
        FeatureKind::Loft { rails, rail_fit, .. } => (rails.clone(), *rail_fit),
        k => panic!("{k:?}"),
    };
    let (stored, fit) = rails(&h);
    assert_eq!(stored, vec![SweepRail { sketch: SketchId(rail_sketch), curves: vec!["out".into()] }]);
    assert_eq!(fit, RailFit::Stretch);
    h.step(&json!({"call":"ui.editFeature","params":{"feature":name}})).unwrap();
    h.frames(8);
    let d = h.app.dialog.as_ref().unwrap();
    assert_eq!(d.inputs[1].items, vec![Sel::SketchCurve { id: "out".into() }]);
    assert!(matches!(d.kind, Kind::Loft { rail_fit: 1, .. }), "{:?}", d.kind);
    ok(&mut h);
    assert_eq!(rails(&h), (stored, fit));
}

#[test]
fn a_rail_picked_in_the_surface_loft_dialog_is_kept_through_an_edit() {
    let mut h = Harness::new();
    let (base, top, rail_sketch) = setup(&mut h);
    h.step(&json!({"call":"ui.start","params":{"command":"surface.loft"}})).unwrap();
    h.frames(3);
    assert!(h.widget("Rail").is_some() && h.widget("Rail Fit").is_some());
    let d = h.app.dialog.as_mut().unwrap();
    d.inputs[0].items = vec![Sel::Profile { sketch: base, index: 0 }, Sel::Profile { sketch: top, index: 0 }];
    d.inputs[1].items = vec![Sel::SketchCurve { id: "out".into() }];
    ok(&mut h);
    let name = h.app.session.doc.features.last().unwrap().name.clone();
    let rails = |h: &Harness| match &h.app.session.doc.find_feature(&name).unwrap().kind {
        FeatureKind::SurfaceLoft { rails, rail_fit, .. } => (rails.clone(), *rail_fit),
        k => panic!("{k:?}"),
    };
    assert_eq!(rails(&h), (vec![SweepRail { sketch: SketchId(rail_sketch), curves: vec!["out".into()] }], RailFit::Scale));
    h.step(&json!({"call":"ui.editFeature","params":{"feature":name}})).unwrap();
    h.frames(8);
    let d = h.app.dialog.as_ref().unwrap();
    assert!(matches!(d.kind, Kind::Surface(Surf::Loft { rail_fit: 0 })), "{:?}", d.kind);
    assert_eq!(d.inputs[1].items, vec![Sel::SketchCurve { id: "out".into() }]);
    ok(&mut h);
    assert_eq!(rails(&h).0.len(), 1);
}

/// #4579: Surface Loft sections clicked in two sketches whose curve ids repeat (both have `l1`
/// and `l2`). A picked curve kept only its id, and the dialog took each one for the last sketch
/// with a curve of that id, so both sections fell in the second sketch and OK stayed off.
#[test]
fn surface_loft_sections_keep_the_sketch_they_were_picked_in() {
    let mut h = Harness::new();
    for (name, z) in [("Low", 0), ("High", 30)] {
        h.app.run("sketch.create", json!({"name": name, "plane": {"origin": [0, 0, z], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}})).unwrap();
        h.app.run("sketch.line", json!({"points": [[0, 0], [20, 0]]})).unwrap();
        h.app.run("sketch.line", json!({"points": [[0, 10], [20, 10]]})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
    }
    let id = |h: &Harness, n: &str| h.app.session.doc.find_feature(n).unwrap().id;
    let (low, high) = (id(&h, "Low"), id(&h, "High"));
    h.step(&json!({"call":"ui.view","params":{"view":"iso"}})).unwrap();
    h.step(&json!({"call":"ui.view","params":{"view":"fit"}})).unwrap();
    h.step(&json!({"call":"ui.start","params":{"command":"surface.loft"}})).unwrap();
    h.frames(3);
    // Low's `l2` (y = 10), then High's `l1` (y = 0).
    h.step(&json!({"click":{"world":[10, 10, 0]}})).unwrap();
    h.step(&json!({"click":{"world":[10, 0, 30]}})).unwrap();
    h.frames(2);
    let d = h.app.dialog.as_ref().unwrap();
    assert_eq!(
        d.inputs[0].items,
        vec![Sel::ExternalSketchCurve { sketch: low, id: "l2".into() }, Sel::ExternalSketchCurve { sketch: high, id: "l1".into() }]
    );
    ok(&mut h);
    let name = h.app.session.doc.features.last().unwrap().name.clone();
    let sections = |h: &Harness| match &h.app.session.doc.find_feature(&name).unwrap().kind {
        FeatureKind::SurfaceLoft { sections, .. } => sections.iter().map(|s| (s.sketch.0, s.curves.clone())).collect::<Vec<_>>(),
        k => panic!("{k:?}"),
    };
    assert_eq!(sections(&h), vec![(low, vec!["l2".to_string()]), (high, vec!["l1".to_string()])]);
    // Editing it brings the sections back in their sketches, and OK keeps them.
    h.step(&json!({"call":"ui.editFeature","params":{"feature":name}})).unwrap();
    h.frames(8);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items.len(), 2);
    ok(&mut h);
    assert_eq!(sections(&h), vec![(low, vec!["l2".to_string()]), (high, vec!["l1".to_string()])]);
}

/// #4465: Surface Extrude of a curve clicked in the first of two sketches whose curve ids repeat
/// was made from the last sketch with a curve of that id.
#[test]
fn surface_extrude_uses_the_sketch_its_curve_was_picked_in() {
    let mut h = Harness::new();
    for (name, z) in [("Low", 0), ("High", 30)] {
        h.app.run("sketch.create", json!({"name": name, "plane": {"origin": [0, 0, z], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}})).unwrap();
        h.app.run("sketch.line", json!({"points": [[0, 0], [20, 0]]})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
    }
    let low = h.app.session.doc.find_feature("Low").unwrap().id;
    h.step(&json!({"call":"ui.view","params":{"view":"iso"}})).unwrap();
    h.step(&json!({"call":"ui.view","params":{"view":"fit"}})).unwrap();
    h.step(&json!({"call":"ui.start","params":{"command":"surface.extrude"}})).unwrap();
    h.frames(3);
    h.step(&json!({"click":{"world":[10, 0, 0]}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items, vec![Sel::ExternalSketchCurve { sketch: low, id: "l1".into() }]);
    ok(&mut h);
    let name = h.app.session.doc.features.last().unwrap().name.clone();
    match &h.app.session.doc.find_feature(&name).unwrap().kind {
        FeatureKind::SurfaceExtrude { sketch, curves, .. } => assert_eq!((sketch.0, curves.clone()), (low, vec!["l1".to_string()])),
        k => panic!("{k:?}"),
    }
    // Edit Feature brings the curve back in its sketch.
    h.step(&json!({"call":"ui.editFeature","params":{"feature":name}})).unwrap();
    h.frames(8);
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items, vec![Sel::ExternalSketchCurve { sketch: low, id: "l1".into() }]);
}
