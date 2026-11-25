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

/// #13910's own attachment (shell_tangent_rim_repro.json), replayed step by step, then the
/// shell at 2 mm and 1 mm with the tangent chain off and on.
#[test]
fn t2c_13910_attachment_replay() {
    let script: Value = serde_json::from_str(include_str!("support/t2c_13910_repro.json")).unwrap();
    let commands = script["commands"].as_array().unwrap();
    let (build, shell) = commands.split_at(commands.len() - 1);
    for (t, chain) in [(2.0, false), (1.0, false), (2.0, true)] {
        println!("T2C ---- 13910 attachment t={t} chain={chain}");
        let mut s = Session::default();
        for c in build {
            let id = c["command"].as_str().unwrap();
            let r = s.execute(id, &c["params"]);
            let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
            println!("T2C   {id} {} -> {:?} errors={errors:?}", c["params"]["feature"]["name"], r.as_ref().map(|_| "ok").map_err(|e| e.to_string()));
        }
        {
            let st = s.model.state();
            for b in &st.bodies {
                let m = solidcraft_kernel::measure(&b.body).unwrap();
                println!("T2C   body {} volume {} valid {:?}", b.name, m.volume, b.body.validity());
            }
        }
        let mut p = shell[0]["params"].clone();
        p["thickness"] = json!(t);
        p["tangent_chain"] = json!(chain);
        step(&mut s, "solid.shell", p);
        let st = s.model.state();
        for b in &st.bodies {
            println!("T2C   after: body {} faces {} shells {}", b.name, b.body.face_count(), b.body.shell_count());
        }
    }
}

/// #14082's design (the reporter's attachment), as saved and with d1 changed.
#[test]
fn t2c_14082_offset_face_replay() {
    let bytes = include_bytes!("support/t2c_14082.sc");
    for d1 in ["65", "58", "57.6", "50"] {
        println!("T2C ---- 14082 d1={d1}");
        let doc = solidcraft_io::read_design(bytes).unwrap();
        let mut s = Session::opened(doc, None);
        let r = s.execute("parameters.change", &json!({"name": "d1", "expression": d1}));
        println!("T2C   change -> {:?}", r.as_ref().map(|_| "ok").map_err(|e| e.to_string()));
        println!("T2C   results: {}", s.model.results.len());
        for r in &s.model.results {
            if let Some(e) = &r.error {
                println!("T2C   feature {} error: {e}", r.name);
            }
        }
        let st = s.model.state();
        for b in &st.bodies {
            println!("T2C   body {} volume {:?} faces {} valid {:?}", b.name, solidcraft_kernel::measure(&b.body).map(|m| m.volume).ok(), b.body.face_count(), b.body.validity());
        }
    }
}

/// #12805's design (the reporter's attachment) and the report's cut.
#[test]
fn t2c_12805_cut_into_imported_body() {
    let doc = solidcraft_io::read_design(include_bytes!("support/t2c_12805.sc")).unwrap();
    let mut s = Session::opened(doc, None);
    println!("T2C ---- 12805 opened: results {}", s.model.results.len());
    for r in &s.model.results {
        if let Some(e) = &r.error {
            println!("T2C   feature {} error: {e}", r.name);
        }
    }
    {
        let st = s.model.state();
        for b in &st.bodies {
            println!("T2C   body {} volume {:?} faces {} valid {:?}", b.name, solidcraft_kernel::measure(&b.body).map(|m| m.volume).ok(), b.body.face_count(), b.body.validity());
        }
    }
    let p = json!({"direction": "positive", "distance": "-14 mm", "exact_regions": true, "operation": "cut", "profiles": [5], "sketch": 36, "targets": ["Microspora_X_Tier"]});
    let t = std::time::Instant::now();
    let r = s.execute("solid.extrude", &p);
    println!("T2C   cut -> {:?} in {:.1?}", r.as_ref().map(|_| "ok").map_err(|e| e.to_string()), t.elapsed());
    for r in &s.model.results {
        if let Some(e) = &r.error {
            println!("T2C   feature {} error: {e}", r.name);
        }
    }
    let st = s.model.state();
    for b in &st.bodies {
        println!("T2C   after: body {} volume {:?} faces {} valid {:?}", b.name, solidcraft_kernel::measure(&b.body).map(|m| m.volume).ok(), b.body.face_count(), b.body.validity());
    }
}
