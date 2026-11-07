use crate::scenario::Harness;
use serde_json::json;
use solidcraft_engine::geom::prelude::*;
use solidcraft_engine::sketch::{ConstraintKind, CurveKind};

/// A sketch holding one line, `a`, from (0, 0) to (20, 0): its midpoint is (10, 0).
fn sketch() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
    h.app.run("sketch.line", json!({"points":[[0,0],[20,0]],"ids":["a"],"infer":false})).unwrap();
    h.app.run("ui.camera", json!({"target":[10,10,0],"distance":150})).ok();
    h.settle();
    h
}

/// The line the Line tool drew from `clicks` (two clicks), with its start and end points.
fn drawn(h: &mut Harness, clicks: [[f64; 2]; 2]) -> (usize, usize, usize) {
    h.step(&json!({"start":"sketch.line"})).unwrap();
    for q in clicks {
        h.step(&json!({"click":{"sketch":q}})).unwrap();
        h.settle();
    }
    let sid = h.app.session.active_sketch.unwrap();
    let state = h.app.session.world_state().unwrap();
    let sketch = &state.sketch(sid).unwrap().sketch;
    assert_eq!(sketch.curves.len(), 2, "the second line is drawn: {:?} {:?}", h.app.status, h.app.tool);
    let CurveKind::Line { a, b } = sketch.curves[1].kind else { panic!("a line") };
    (sketch.curve_index(sketch.curves[0].id.as_str()).unwrap(), a, b)
}

fn held_to_midpoint(h: &Harness, p: usize, line: usize) -> bool {
    let sid = h.app.session.active_sketch.unwrap();
    let state = h.app.session.world_state().unwrap();
    let sketch = &state.sketch(sid).unwrap().sketch;
    sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Midpoint { p: q, l } if q == p && l == line))
}

/// Gabe, 2026-11-07: finishing a line on another line's midpoint works.
#[test]
fn a_line_finishes_on_another_lines_midpoint() {
    let mut h = sketch();
    let (a, start, end) = drawn(&mut h, [[10., 15.], [10., 0.]]);
    let sid = h.app.session.active_sketch.unwrap();
    let state = h.app.session.world_state().unwrap();
    let sketch = &state.sketch(sid).unwrap().sketch;
    let (s, e) = (sketch.point(start).unwrap(), sketch.point(end).unwrap());
    assert!(s.dist(solidcore_record::Vec2::new(10., 15.)) < 1e-4 && e.dist(solidcore_record::Vec2::new(10., 0.)) < 1e-4, "{s:?} {e:?}");
    assert!(held_to_midpoint(&h, end, a), "{:?}", sketch.constraints);
}

/// ... and starting one there must too: the first click on a midpoint begins the line.
#[test]
fn a_line_starts_on_another_lines_midpoint() {
    let mut h = sketch();
    let (a, start, end) = drawn(&mut h, [[10., 0.], [10., 15.]]);
    let sid = h.app.session.active_sketch.unwrap();
    let state = h.app.session.world_state().unwrap();
    let sketch = &state.sketch(sid).unwrap().sketch;
    let (s, e) = (sketch.point(start).unwrap(), sketch.point(end).unwrap());
    assert!(s.dist(solidcore_record::Vec2::new(10., 0.)) < 1e-4 && e.dist(solidcore_record::Vec2::new(10., 15.)) < 1e-4, "{s:?} {e:?}");
    assert!(held_to_midpoint(&h, start, a), "{:?}", sketch.constraints);
}

/// Starting a Line on the midpoint of a rectangle's side: each case is the rectangle alone, or
/// with another line drawn first.
#[test]
fn a_line_starts_on_a_rectangle_sides_midpoint() {
    for first_line in [false, true] {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,20]})).unwrap();
        h.app.run("ui.camera", json!({"target":[20,10,0],"distance":150})).ok();
        h.settle();
        let sid = h.app.session.active_sketch.unwrap();
        let before = h.app.session.world_state().unwrap().sketch(sid).unwrap().sketch.curves.len();
        let mut expect = before;
        if first_line {
            h.step(&json!({"start":"sketch.line"})).unwrap();
            for q in [[60., 5.], [60., 25.]] {
                h.step(&json!({"click":{"sketch":q}})).unwrap();
                h.settle();
            }
            h.step(&json!({"key":"Escape"})).ok();
            h.settle();
            expect += 1;
        }
        h.step(&json!({"start":"sketch.line"})).unwrap();
        h.step(&json!({"click":{"sketch":[20., 0.]}})).unwrap();
        h.settle();
        let started = h.app.tool.as_ref().map(|t| t.pts.clone());
        assert!(started.as_ref().is_some_and(|p| p.len() == 1), "first_line {first_line}: the click on the midpoint starts the line: {started:?} {:?}", h.app.status);
        h.step(&json!({"click":{"sketch":[20., -15.]}})).unwrap();
        h.settle();
        let state = h.app.session.world_state().unwrap();
        let sketch = &state.sketch(sid).unwrap().sketch;
        assert_eq!(sketch.curves.len(), expect + 1, "first_line {first_line}: the line is drawn: {:?} {:?}", h.app.status, h.app.tool);
        let CurveKind::Line { a, .. } = sketch.curves.last().unwrap().kind else { panic!("a line") };
        let start = sketch.point(a).unwrap();
        assert!(start.dist(solidcore_record::Vec2::new(20., 0.)) < 1e-4, "{start:?}");
        assert!(sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Midpoint { p, .. } if p == a)), "{:?}", sketch.constraints);
    }
}
