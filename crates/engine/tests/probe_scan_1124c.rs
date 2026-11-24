//! Probe only (t2-ready scan round 3, 2026-11-24): #21581. Not for a PR.
use serde_json::{Value, json};
use solidcraft_engine::Session;

fn try_run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    let r = s.execute(id, &p).map_err(|e| e.to_string());
    eprintln!("PROBE {id} -> {}", match &r { Ok(v) => format!("ok {}", v.to_string().chars().take(200).collect::<String>()), Err(e) => format!("ERR {e}") });
    r
}

fn report(s: &Session, tag: &str) {
    for r in &s.model.results {
        if let Some(e) = &r.error {
            eprintln!("PROBE {tag} feature error: {e}");
        }
    }
    for b in &s.model.state().bodies {
        let v = b.body.validity();
        let vol = solidcraft_kernel::measure(&b.body).map(|m| m.volume).unwrap_or(f64::NAN);
        eprintln!("PROBE {tag} body {:?} shells {} faces {} vol {vol} validity {:?}", b.name, b.body.shell_count(), b.body.face_count(), v);
        let (r, _) = solidcraft_kernel::validating(solidcraft_kernel::Validation::EachStep, || solidcraft_kernel::transform(&b.body, solidcore_record::Vec3::ZERO, solidcore_record::Vec3::ZERO, solidcore_record::Vec3::Z, 0.0));
        eprintln!("PROBE {tag} body {:?} smlib-check-on-copy: {:?}", b.name, r.err().map(|e| e.to_string()));
    }
}

const PTS: &str = "[[25,0.0],[25.0,6.0],[24.9373,7.7854],[24.7495,9.0192],[24.4379,10.0702],[24.0039,10.9847],[23.4496,11.7765],[22.7765,12.4496],[21.9847,13.0039],[21.0702,13.4379],[20.0192,13.7495],[18.7854,13.9373],[17.0,14.0],[-17.0,14.0],[-18.7854,13.9373],[-20.0192,13.7495],[-21.0702,13.4379],[-21.9847,13.0039],[-22.7765,12.4496],[-23.4496,11.7765],[-24.0039,10.9847],[-24.4379,10.0702],[-24.7495,9.0192],[-24.9373,7.7854],[-25.0,6.0],[-25,0.0],[-25.0,-3.0],[-24.9686,-3.8927],[-24.8748,-4.5096],[-24.7189,-5.0351],[-24.502,-5.4923],[-24.2248,-5.8883],[-23.8883,-6.2248],[-23.4923,-6.502],[-23.0351,-6.7189],[-22.5096,-6.8748],[-21.8927,-6.9686],[-21.0,-7.0],[21.0,-7.0],[21.8927,-6.9686],[22.5096,-6.8748],[23.0351,-6.7189],[23.4923,-6.502],[23.8883,-6.2248],[24.2248,-5.8883],[24.502,-5.4923],[24.7189,-5.0351],[24.8748,-4.5096],[24.9686,-3.8927],[25.0,-3.0]]";

fn pts(midpoints: bool) -> Value {
    let p: Vec<Value> = serde_json::from_str(PTS).unwrap();
    if !midpoints { return Value::Array(p); }
    let mut out = Vec::new();
    for (i, a) in p.iter().enumerate() {
        out.push(a.clone());
        let b = &p[(i + 1) % p.len()];
        let (ax, ay, bx, by) = (a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), b[0].as_f64().unwrap(), b[1].as_f64().unwrap());
        if ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt() > 5.0 {
            out.push(json!([(ax + bx) / 2.0, (ay + by) / 2.0]));
        }
    }
    Value::Array(out)
}

fn section(s: &mut Session, y: f64, name: &str, mid: bool) {
    let _ = try_run(s, "sketch.create", json!({"plane": {"origin": [0, y, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": name}));
    let _ = try_run(s, "sketch.spline.fit_point", json!({"points": pts(mid), "periodic": true}));
    let _ = try_run(s, "sketch.finish", json!({}));
}

fn cut(s: &mut Session, tag: &str) {
    let _ = try_run(s, "solid.box", json!({"length": "15 mm", "width": "1.7 mm", "height": "20 mm", "corner": [-35, 0, -20], "operation": "new", "body_name": "w"}));
    let _ = try_run(s, "solid.combine", json!({"target": "innen", "tools": ["w"], "operation": "cut", "keep_tools": false}));
    report(s, tag);
}

fn loft_case(tag: &str, mid: bool) {
    eprintln!("PROBE ===== {tag}");
    let mut s = Session::default();
    section(&mut s, -30.0, "s0", mid);
    section(&mut s, 30.0, "s1", mid);
    let _ = try_run(&mut s, "solid.loft", json!({"sections": [{"sketch": "s0"}, {"sketch": "s1"}], "operation": "new", "name": "raum", "body_name": "innen"}));
    report(&s, &format!("{tag}-after-loft"));
    cut(&mut s, &format!("{tag}-after-cut"));
}

#[test]
fn probe_21581_report() { loft_case("report", false); }

#[test]
fn probe_21581_midpoints() { loft_case("midpoints", true); }

#[test]
fn probe_21581_extrude_same_profile() {
    eprintln!("PROBE ===== extrude");
    let mut s = Session::default();
    section(&mut s, -30.0, "s0", false);
    let _ = try_run(&mut s, "solid.extrude", json!({"sketch": "s0", "distance": 60, "direction": "negative", "operation": "new", "body_name": "innen"}));
    report(&s, "extrude-after");
    cut(&mut s, "extrude-after-cut");
}

fn loft_body(s: &mut Session, ys: &[f64]) {
    let mut secs = Vec::new();
    for (i, y) in ys.iter().enumerate() {
        let n = format!("s{i}");
        section(s, *y, &n, false);
        secs.push(json!({"sketch": n}));
    }
    let _ = try_run(s, "solid.loft", json!({"sections": secs, "operation": "new", "body_name": "innen"}));
}

fn box_cut(tag: &str, ys: &[f64], corner: [f64; 3], size: [f64; 3]) {
    eprintln!("PROBE ===== {tag} corner {corner:?} size {size:?}");
    let mut s = Session::default();
    loft_body(&mut s, ys);
    let _ = try_run(&mut s, "solid.box", json!({"length": size[0], "width": size[1], "height": size[2], "corner": corner, "operation": "new", "body_name": "w"}));
    let r = try_run(&mut s, "solid.combine", json!({"target": "innen", "tools": ["w"], "operation": "cut", "keep_tools": false}));
    eprintln!("PROBE RESULT {tag}: {}", if r.is_ok() { "ok" } else { "REFUSED" });
}

#[test]
fn probe_21581_variants() {
    let two = [-30.0, 30.0];
    box_cut("v-report", &two, [-35.0, 0.0, -20.0], [15.0, 1.7, 20.0]);
    // side wall only (x=-25 band, z -5..-1): no bottom crossing
    box_cut("v-side-only", &two, [-35.0, 0.0, -5.0], [15.0, 1.7, 4.0]);
    // bottom only (z=-7 span, x -10..10)
    box_cut("v-bottom-only", &two, [-10.0, 0.0, -20.0], [20.0, 1.7, 15.0]);
    // top straight span only
    box_cut("v-top-only", &two, [-10.0, 0.0, 10.0], [20.0, 1.7, 10.0]);
    // right side mirror of the report
    box_cut("v-mirror-x", &two, [20.0, 0.0, -20.0], [15.0, 1.7, 20.0]);
    // other y positions
    box_cut("v-y-10", &two, [-35.0, 10.0, -20.0], [15.0, 1.7, 20.0]);
    box_cut("v-y-m20", &two, [-35.0, -20.0, -20.0], [15.0, 1.7, 20.0]);
    // three sections (degree 2 skin)
    box_cut("v-3sec", &[-30.0, 0.5, 30.0], [-35.0, 0.0, -20.0], [15.0, 1.7, 20.0]);
    // corner only (lower-left rounded corner, not the long spans)
    box_cut("v-corner", &two, [-35.0, 0.0, -20.0], [11.0, 1.7, 17.0]);
}
