//! Shell regressions found by the robustness fuzzer (docs/robustness.md).
//!
//! The box-on-box designs below have a saddle corner: where the top box's left face meets the
//! lower box's back face, four planar faces meet with convex and concave edges in turn (the
//! top box overhangs the lower box's back face). A shell splits that corner into two corners
//! joined by a new edge.
//!
//! Lower box `[0, 25] × [0, 17.5] × [0, 35]`, top box `[7.5, 22.5] × [0, 17.5 + o] × [35, 42.5]`.
//! With the bottom face open, a shell of thickness `t` leaves this cavity (for `o < 17.5 − 2t`):
//!
//! - in the lower box: `[t, 25 − t] × [t, 17.5 − t] × [0, 35 − t]`
//! - in the top box, over the lower box: `[7.5 + t, 22.5 − t] × [t, 17.5 − t] × [35 − t, 42.5 − t]`
//! - in the overhang: `[7.5 + t, 22.5 − t] × [17.5 − t, 17.5 + o − t] × [35 + t, 42.5 − t]`
//!
//! The shell's volume is the solid's volume less those three boxes.
use serde_json::json;
use solidcraft_engine::Session;

fn run(s: &mut Session, id: &str, p: serde_json::Value) {
    if let Err(e) = s.execute(id, &p) {
        panic!("{id} {p}: {e}");
    }
}

/// The box on a box, overhanging the lower box's back face by `o`.
fn box_on_box(o: f64) -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"height": 35.0, "length": 25.0, "width": 17.5}));
    run(&mut s, "solid.box", json!({"corner": [7.5, 0.0, 35.0], "height": 7.5, "length": 15.0, "operation": "join", "width": 17.5 + o}));
    s
}

/// The shell's volume with the bottom face open (see the module comment).
fn open_bottom_shell_volume(o: f64, t: f64) -> f64 {
    let solid = 25.0 * 17.5 * 35.0 + 15.0 * (17.5 + o) * 7.5;
    let lower = (25.0 - 2.0 * t) * (17.5 - 2.0 * t) * (35.0 - t);
    let above = (15.0 - 2.0 * t) * (17.5 - 2.0 * t) * 7.5;
    let overhang = (15.0 - 2.0 * t) * o * (7.5 - 2.0 * t);
    solid - lower - above - overhang
}

/// Seed 83 (minimised): the overhang is 2.5 mm, and the shell opens the top box's left face.
/// The shell refused the saddle corner (`a corner whose faces don't meet the same way`).
#[test]
fn shell_box_on_box_overhanging_saddle_corner() {
    let mut s = box_on_box(2.5);
    run(&mut s, "solid.shell", json!({"faces": [[7.5, 17.5, 38.75]], "thickness": 1.5}));
    sound(&s);
    // The bottom closed and the top box's left face x = 7.5 open, so the cavity in the top box
    // runs out to it: [1.5, 23.5] × [1.5, 16] × [1.5, 33.5] in the lower box,
    // [7.5, 21] × [1.5, 16] × [33.5, 41] over it and [7.5, 21] × [16, 18.5] × [36.5, 41] in the
    // overhang, out of the solid's 25 × 17.5 × 35 + 15 × 20 × 7.5.
    let want = 25.0 * 17.5 * 35.0 + 15.0 * 20.0 * 7.5 - 22.0 * 14.5 * 32.0 - 13.5 * 14.5 * 7.5 - 13.5 * 2.5 * 4.5;
    let got = volume(&s);
    assert!((got - want).abs() < 1e-6 * want, "shell volume {got}, want {want}");
}

/// The same saddle corner with the bottom open: every face round the corner moves, and the
/// volume is exact.
#[test]
fn shell_saddle_corner_volume_is_exact() {
    for t in [1.0, 1.5] {
        let mut s = box_on_box(2.5);
        run(&mut s, "solid.shell", json!({"faces": [[12.5, 8.75, 0.0]], "thickness": t}));
        sound(&s);
        let (got, want) = (volume(&s), open_bottom_shell_volume(2.5, t));
        assert!((got - want).abs() < 1e-6 * want, "thickness {t}: shell volume {got}, want {want}");
    }
}

/// An overhang (1 mm) thinner than the wall (1.5 mm): the overhang's cavity is only 1 mm deep.
/// A corner landing outside the solid, or a wall left thin, would change the volume.
#[test]
fn shell_saddle_corner_with_an_overhang_thinner_than_the_wall() {
    let mut s = box_on_box(1.0);
    run(&mut s, "solid.shell", json!({"faces": [[12.5, 8.75, 0.0]], "thickness": 1.5}));
    sound(&s);
    let (got, want) = (volume(&s), open_bottom_shell_volume(1.0, 1.5));
    assert!((got - want).abs() < 1e-6 * want, "shell volume {got}, want {want}");
}

fn volume(s: &Session) -> f64 {
    s.model.state().bodies.iter().map(|b| solidcraft_kernel::measure(&b.body).map(|m| m.volume).unwrap_or(f64::NAN)).sum()
}

fn sound(s: &Session) {
    let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let st = s.model.state();
    assert_eq!(st.bodies.len(), 1, "one body");
    for b in &st.bodies {
        assert_eq!(b.body.shell_count(), 1, "one shell");
        assert!(b.body.validity().is_empty(), "{:?}", b.body.validity());
    }
}

/// A shell that opens no face (rig shell/4009): a closed hollow body, the void sealed inside. A
/// 40 × 30 × 12.5 box with a 2.5 wall inside leaves a 35 × 25 × 7.5 void; grown 2.5 outside, the
/// walls are a 45 × 35 × 17.5 box less the original.
#[test]
fn a_shell_that_opens_no_face_seals_its_void() {
    for (direction, want, shells) in [("inside", 15000.0 - 35.0 * 25.0 * 7.5, 2), ("outside", 45.0 * 35.0 * 17.5 - 15000.0, 2)] {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40.0, "width": 30.0, "height": 12.5}));
        run(&mut s, "solid.shell", json!({"faces": [], "thickness": 2.5, "direction": direction}));
        let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
        assert!(errors.is_empty(), "{direction}: {errors:?}");
        let st = s.model.state();
        assert_eq!(st.bodies.len(), 1, "{direction}: one body");
        assert_eq!(st.bodies[0].body.shell_count(), shells, "{direction}: an outer and an inner shell");
        let got = volume(&s);
        assert!((got - want).abs() < 1e-6 * want, "{direction}: volume {got}, want {want}");
    }
}

/// #17158: a box with a round pocket cut into its top through two of its sides, and the two
/// edges where the pocket's wall meets those sides rounded, so each round is tangent to the
/// pocket's wall. An outside shell moves the round and the wall out together: they stay
/// tangent along a line and meet the pocket's floor there. The shell was refused (`how far its
/// neighbours extend to meet it has no certified bound`); it builds one closed sound body.
#[test]
fn an_outside_shell_of_a_round_tangent_to_a_pocket_wall_builds() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": 10}));
    run(&mut s, "sketch.circle.center", json!({"center": [35, 5], "radius": 10}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 4, "direction": "negative", "operation": "cut"}));
    // where the pocket's wall (radius 10 about (35, 5)) leaves the sides x = 40 and y = 0
    run(&mut s, "solid.fillet", json!({"edges": [[40.0, 13.660254037844387, 8.0], [26.339745962155614, 0.0, 8.0]], "radius": 2}));
    run(&mut s, "solid.shell", json!({"faces": [[10, 10, 0]], "thickness": 1.5, "direction": "outside"}));
    sound(&s);
}

/// #17210: a disc with two circles swept down slanting lines onto it from either side, both
/// ending on one circle on the disc's top (oblique cylinders, mirror images, tangent to each
/// other where their crossing curve meets the disc). Shelled with the bottom open, the two
/// walls' offsets meet on that circle's offset and on a curve through it, which crosses the
/// offset top below the circle: the edge between the walls runs past the tangent point to
/// its vertex there. The shell left a gap (`a gap in its offset's loops cannot be bridged`).
/// The join itself is the kernel's `intersection_pieces_through_a_tangent_point_are_one_curve`.
#[test]
fn a_shell_of_two_sweeps_from_one_circle_meets_past_their_tangent_point() {
    let mut s = Session::default();
    let feature = |s: &mut Session, f: serde_json::Value| run(s, "timeline.add", json!({ "feature": f }));
    feature(
        &mut s,
        json!({"id": 1, "name": "Sketch1", "type": "sketch", "plane": {"type": "origin", "name": "XY"}, "sketch": {
        "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}, {"id": "c1.center", "pos": [0.0, 0.0], "fixed": false}],
        "curves": [{"id": "c1", "type": "circle", "c": 1, "r": 30.0, "construction": false}], "constraints": [], "counters": {"c": 1}}}),
    );
    feature(
        &mut s,
        json!({"id": 2, "name": "Extrude1", "type": "extrude", "sketch": 1, "body_names": ["Body1"], "operation": "new_body",
        "profiles": {"type": "curves", "loops": [["c1"]]}, "extent": {"distance": "8", "convention": "total_distance", "direction": "positive"}}),
    );
    feature(
        &mut s,
        json!({"id": 3, "name": "Sketch2", "type": "sketch", "plane": {"type": "flipped", "base": {"type": "origin", "name": "XZ"}}, "sketch": {
        "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}, {"id": "l1.start", "pos": [0.0, 8.0], "fixed": false},
            {"id": "l1.end", "pos": [40.0, 40.0], "fixed": false}, {"id": "l2.end", "pos": [-40.0, 40.0], "fixed": false}],
        "curves": [{"id": "l1", "type": "line", "a": 1, "b": 2, "construction": false}, {"id": "l2", "type": "line", "a": 1, "b": 3, "construction": false}],
        "constraints": [], "counters": {"l": 2}}}),
    );
    feature(
        &mut s,
        json!({"id": 4, "name": "Plane1", "type": "construction_plane", "plane": {"type": "through_point", "base": {"type": "origin", "name": "XY"},
        "point": {"type": "sketch_point", "sketch": 3, "point": "l1.end"}}}),
    );
    feature(
        &mut s,
        json!({"id": 5, "name": "Sketch3", "type": "sketch", "plane": {"type": "construction", "feature": 4}, "sketch": {
        "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}, {"id": "c1.center", "pos": [40.0, 0.0], "fixed": false}, {"id": "c2.center", "pos": [-40.0, 0.0], "fixed": false}],
        "curves": [{"id": "c1", "type": "circle", "c": 1, "r": 20.0, "construction": false}, {"id": "c2", "type": "circle", "c": 2, "r": 20.0, "construction": false}],
        "constraints": [], "counters": {"c": 2}}}),
    );
    feature(
        &mut s,
        json!({"id": 6, "name": "Sweep1", "type": "sweep", "sketch": 5, "profiles": {"type": "curves", "loops": [["c2"]]}, "path_sketch": 3, "path": ["l2"], "operation": "join"}),
    );
    feature(
        &mut s,
        json!({"id": 7, "name": "Sweep2", "type": "sweep", "sketch": 5, "profiles": {"type": "curves", "loops": [["c1"]]}, "path_sketch": 3, "path": ["l1"], "operation": "join"}),
    );
    run(&mut s, "solid.shell", json!({"body": "Body1", "faces": [[10.0, -10.0, 0.0]], "thickness": "1.5 mm", "tangent_chain": true}));
    sound(&s);
}

/// A box with a curved tunnel lofted through it (from a circle on its side, through a tilted
/// circle, to a circle on its bottom), shelled 2 mm with its top open. The shell builds but
/// SMLib's check refuses it ("Face Near missing Seam": an edge close to the seam of the tunnel's
/// wall) ([T-202](https://tickets.solidcraft.io/t/202), solidcore#3318). It must give a valid
/// body that weighs less than the solid.
#[test]
fn shell_of_a_box_with_a_lofted_tunnel() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 80, "width": 50, "height": 30, "corner": [-30, -25, 0], "body_name": "Body1"}));
    run(&mut s, "sketch.create", json!({"plane": {"origin": [50, 0, 0], "x_dir": [0, 1, 0], "y_dir": [0, 0, 1]}, "name": "A"}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 15], "radius": 8}));
    run(&mut s, "sketch.finish", json!({}));
    let tilt = std::f64::consts::FRAC_1_SQRT_2;
    run(&mut s, "sketch.create", json!({"plane": {"origin": [25, 0, 18], "x_dir": [0, 1, 0], "y_dir": [tilt, 0, tilt]}, "name": "B"}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 9}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "C"}));
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 10}));
    run(&mut s, "sketch.finish", json!({}));
    let sections = json!([{"sketch": "A", "profiles": [0]}, {"sketch": "B", "profiles": [0]}, {"sketch": "C", "profiles": [0]}]);
    run(&mut s, "solid.loft", json!({"sections": sections, "operation": "cut", "targets": ["Body1"]}));
    let solid = volume(&s);
    run(&mut s, "solid.shell", json!({"body": "Body1", "faces": [[-20, 20, 30]], "thickness": "2 mm"}));
    let state = s.model.state();
    for b in &state.bodies {
        assert!(b.body.validity().is_empty(), "{:?}", b.body.validity());
    }
    let shelled = volume(&s);
    assert!(shelled > 0.0 && shelled < solid, "{shelled} vs {solid}");
}

/// #14376: a 30 × 30 × 12 box with four R5 lobes, 6 tall, on its corners (one at (16.5, 15) so
/// the lobe's cavity joins the box's). Four walls of the lobed body are the reporter's case:
/// the shell of the bare body is refused "not supported yet: SMLib shell for this geometry",
/// and the shell of the body with its four top arcs rounded (R2.5) fails "SMLib failed
/// (status 1001)". The top face z = 12 is opened; the shell is 2 mm.
fn box_with_four_corner_lobes() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 30.0, "width": 30.0, "height": 12.0, "corner": [-15.0, -15.0, 0.0]}));
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    for c in [[-15.0, 15.0], [16.5, 15.0], [15.0, -15.0], [-15.0, -15.0]] {
        run(&mut s, "sketch.circle.center", json!({"center": c, "radius": 5.0}));
    }
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 6.0, "operation": "join"}));
    s
}

/// The lobes' outside area: three quarter discs and the (16.5, 15) disc less its 12.249 mm2
/// inside the square. The volume before the shell is 10800 + 6 * this.
fn lobes_outside_area() -> f64 {
    3.0 * (3.0 / 4.0) * 25.0 * std::f64::consts::PI + (25.0 * std::f64::consts::PI - 12.249)
}

fn bbox(s: &Session) -> [f64; 6] {
    let b = &s.model.state().bodies[0];
    let m = solidcraft_kernel::measure(&b.body).expect("measure");
    [m.bbox.min.x, m.bbox.min.y, m.bbox.min.z, m.bbox.max.x, m.bbox.max.y, m.bbox.max.z]
}

/// Shells the lobed body 2 mm, top face open, and checks the result is one sound closed hollow
/// body with the outside unchanged and the cavity removed.
fn shell_the_lobed_body(s: &mut Session, before: f64, lower: f64) {
    let bbox0 = bbox(s);
    let r = s.execute("solid.shell", &json!({"body": "Body1", "faces": [[0.0, 0.0, 12.0]], "thickness": "2 mm", "tangent_chain": true}));
    if let Err(e) = &r {
        panic!("solid.shell refused: {e}");
    }
    sound(s);
    let bbox1 = bbox(s);
    for (a, b) in bbox0.iter().zip(bbox1.iter()) {
        assert!((a - b).abs() < 1e-5, "bbox changed: {bbox0:?} -> {bbox1:?}");
    }
    let got = volume(s);
    assert!(got > lower && got < before, "shell volume {got}, want between {lower} and {before}");
}

/// The reporter's "not supported yet: SMLib shell for this geometry" case (no fillet).
#[test]
fn four_corner_lobes_shell_builds() {
    let mut s = box_with_four_corner_lobes();
    let before = volume(&s);
    let want = 10800.0 + 6.0 * lobes_outside_area();
    assert!((before - want).abs() < 1e-6 * want, "before-shell volume {before}, want {want}");
    shell_the_lobed_body(&mut s, before, 2.0 * (900.0 + lobes_outside_area()));
}

/// The reporter's "SMLib failed (status 1001)" case: the same body with the four top arcs
/// rounded R2.5 first. Picks are the arcs' midpoints. Precondition: the filleted body weighs
/// 12,143.69 mm3 (the face count is not checked: the extrude splits the right lobes at theta 0).
#[test]
#[ignore = "fillet: two tangent halves meeting at a seam vertex are refused (separate fillet defect)"]
fn four_rounded_corner_lobes_shell_builds() {
    let mut s = box_with_four_corner_lobes();
    run(
        &mut s,
        "solid.fillet",
        json!({"edges": [[-18.5355, 18.5355, 6.0], [-18.5355, -18.5355, 6.0], [15.0, -20.0, 6.0], [18.5355, -11.4645, 6.0], [16.5, 20.0, 6.0], [20.0355, 11.4645, 6.0]], "radius": 2.5, "tangent_chain": false}),
    );
    let before = volume(&s);
    assert!((before - 12143.69).abs() < 0.5, "PRECONDITION: filleted body has {before} mm3, want 12143.69");
    shell_the_lobed_body(&mut s, before, 2.0 * (900.0 + lobes_outside_area()));
}

/// #14376: the reporter's box with only the two left lobes (no off-centre lobe), 2 mm open top.
/// Its footprint is the 30 x 30 square plus two three-quarter discs of R5 outside it.
#[test]
fn two_left_lobes_shell_builds() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 30.0, "width": 30.0, "height": 12.0, "corner": [-15.0, -15.0, 0.0]}));
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    for c in [[-15.0, 15.0], [-15.0, -15.0]] {
        run(&mut s, "sketch.circle.center", json!({"center": c, "radius": 5.0}));
    }
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 6.0, "operation": "join"}));
    let before = volume(&s);
    let outside = 2.0 * (3.0 / 4.0) * 25.0 * std::f64::consts::PI;
    let want = 10800.0 + 6.0 * outside;
    assert!((before - want).abs() < 1e-6 * want, "before-shell volume {before}, want {want}");
    shell_the_lobed_body(&mut s, before, 2.0 * 900.0);
}
