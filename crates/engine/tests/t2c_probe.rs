//! PROBE (team2 fable, not for merge): where main stands on #13910, #11924, #8712 stand-ins.
use serde_json::{json, Value};
use solidcraft_engine::Session;

fn step(s: &mut Session, id: &str, p: Value) -> bool {
    let t = std::time::Instant::now();
    let r = s.execute(id, &p);
    let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
    let st = s.model.state();
    let vols: Vec<f64> = st.bodies.iter().map(|b| solidcraft_kernel::measure(&b.body).map(|m| m.volume).unwrap_or(f64::NAN)).collect();
    let valid: Vec<String> = st.bodies.iter().map(|b| format!("{:?}", b.body.validity())).collect();
    println!("T2C {id} {p} -> exec={:?} errors={errors:?} vols={vols:?} valid={valid:?} in {:.1?}", r.as_ref().map(|_| "ok").map_err(|e| e.to_string()), t.elapsed());
    r.is_ok() && errors.is_empty()
}

fn rounded_box(edges: Value, r: f64) -> Session {
    let mut s = Session::default();
    step(&mut s, "solid.box", json!({"height": 20.0, "length": 60.0, "width": 40.0}));
    step(&mut s, "solid.fillet", json!({"edges": edges, "radius": r}));
    s
}

#[test]
fn t2c_13910_shell_open_face_tangent_to_rounds() {
    let all = json!([[30, 0, 20], [60, 20, 20], [30, 40, 20], [0, 20, 20]]);
    let long = json!([[30, 0, 20], [30, 40, 20]]);
    for (name, edges, r, t) in [
        ("all-r3.8-t1", all.clone(), 3.8, 1.0),
        ("all-r3.8-t2", all.clone(), 3.8, 2.0),
        ("long-r3.8-t1", long.clone(), 3.8, 1.0),
        ("long-r3.8-t2", long.clone(), 3.8, 2.0),
        ("all-r0.8-t1", all.clone(), 0.8, 1.0),
        ("long-r0.8-t1", long.clone(), 0.8, 1.0),
    ] {
        println!("T2C ---- 13910 {name}");
        let mut s = rounded_box(edges, r);
        step(&mut s, "solid.shell", json!({"faces": [[30, 20, 20]], "thickness": t}));
    }
}

fn slot_plate() -> Session {
    let mut s = Session::default();
    step(&mut s, "solid.box", json!({"height": 3.0, "length": 30.0, "width": 20.0}));
    step(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 3], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Slot"}));
    step(&mut s, "sketch.rectangle.two_point", json!({"p0": [10.55, 8.4], "p1": [19.45, 11.6]}));
    step(&mut s, "sketch.circle.center", json!({"center": [10.55, 10.0], "radius": 1.6}));
    step(&mut s, "sketch.circle.center", json!({"center": [19.45, 10.0], "radius": 1.6}));
    step(&mut s, "sketch.finish", json!({}));
    s
}

#[test]
fn t2c_11924_slot_of_a_rectangle_and_two_circles() {
    let mid = json!({"point": [15.0, 10.0]});
    let (li, ri) = (json!({"point": [11.3, 10.0]}), json!({"point": [18.7, 10.0]}));
    let (lo, ro) = (json!({"point": [9.8, 10.0]}), json!({"point": [20.2, 10.0]}));
    for (name, profiles) in [
        ("all5", json!([mid, li, ri, lo, ro])),
        ("mid+inner", json!([mid, li, ri])),
        ("mid+li+lo", json!([mid, li, lo])),
        ("li+lo+ri+ro", json!([li, lo, ri, ro])),
        ("mid+outer", json!([mid, lo, ro])),
    ] {
        println!("T2C ---- 11924 {name}");
        let mut s = slot_plate();
        step(&mut s, "solid.extrude", json!({"sketch": "Slot", "direction": "positive", "distance": "-1.5 mm", "exact_regions": true, "operation": "cut", "profiles": profiles}));
    }
}

#[test]
fn t2c_8712_flush_pattern_copies() {
    let (c, sn) = (15f64.to_radians().cos(), 15f64.to_radians().sin());
    for (name, half, count) in [("flush-12", 1.0, 12), ("gap-12", 14.0 / 15.0, 12), ("flush-6", 1.0, 6)] {
        println!("T2C ---- 8712 {name}");
        let (c, sn) = if half == 1.0 { (c, sn) } else { (14f64.to_radians().cos(), 14f64.to_radians().sin()) };
        let mut s = Session::default();
        step(&mut s, "solid.cylinder", json!({"base": [0, 0, 0], "radius": 90, "height": 3, "body_name": "Part"}));
        step(&mut s, "solid.cylinder", json!({"base": [0, 0, -1], "radius": 70, "height": 5, "operation": "cut", "targets": ["Part"]}));
        step(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 3], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Sector"}));
        step(&mut s, "sketch.line", json!({"points": [[75.0 * c, -75.0 * sn], [85.0 * c, -85.0 * sn], [85.0 * c, 85.0 * sn], [75.0 * c, 75.0 * sn]], "closed": true}));
        step(&mut s, "sketch.finish", json!({}));
        step(&mut s, "solid.extrude", json!({"sketch": "Sector", "distance": 5, "operation": "join", "targets": ["Part"], "name": "Boss"}));
        step(&mut s, "solid.pattern.circular", json!({"features": ["Boss"], "axis": "Z", "count": count}));
    }
}
