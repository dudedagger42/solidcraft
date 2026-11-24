//! Probe only (t2-ready scan 2026-11-24): #21547 and #21548 repros. Not for a PR.
use serde_json::{Value, json};
use solidcraft_engine::Session;

fn try_run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    let r = s.execute(id, &p).map_err(|e| e.to_string());
    eprintln!("PROBE {id} -> {}", match &r { Ok(v) => format!("ok {}", v.to_string().chars().take(300).collect::<String>()), Err(e) => format!("ERR {e}") });
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
        eprintln!("PROBE {tag} body {:?} comp {} transferred {} shells {} vol {vol} validity {:?}", b.name, b.component, b.transferred, b.body.shell_count(), v);
    }
}

#[test]
fn probe_21547_drafted_ellipse_top_fillet() {
    let mut s = Session::default();
    let _ = try_run(&mut s, "solid.box", json!({"length": 16, "width": 9, "height": 6, "center": [0, 0, 3], "body_name": "Part", "name": "Block"}));
    let _ = try_run(&mut s, "sketch.create", json!({"plane": "XY", "offset": 6, "name": "Outline"}));
    let _ = try_run(&mut s, "sketch.ellipse", json!({"center": [0, 0], "major": [7.5, 0], "minor_radius": 2.5, "axes": false}));
    let _ = try_run(&mut s, "sketch.finish", json!({}));
    let _ = try_run(&mut s, "solid.extrude", json!({"sketch": "Outline", "distance": 6, "direction": "negative", "taper": 40, "operation": "intersect", "targets": ["Part"], "name": "Drafted"}));
    report(&s, "after-extrude");
    for b in &s.model.state().bodies {
        let (r, _) = solidcraft_kernel::validating(solidcraft_kernel::Validation::EachStep, || solidcraft_kernel::transform(&b.body, solidcore_record::Vec3::ZERO, solidcore_record::Vec3::ZERO, solidcore_record::Vec3::Z, 0.0));
        eprintln!("PROBE after-extrude smlib-check-on-copy: {:?}", r.err().map(|e| e.to_string()));
    }
    let _ = try_run(&mut s, "model.faces", json!({}));
    let _ = try_run(&mut s, "solid.fillet", json!({"edges": [[0, 2.5, 6]], "radius": 0.2, "name": "Top"}));
    report(&s, "after-fillet");
    for r in [0.5, 1.0] {
        let mut t = Session::default();
        let _ = try_run(&mut t, "solid.box", json!({"length": 16, "width": 9, "height": 6, "center": [0, 0, 3], "body_name": "Part"}));
        let _ = try_run(&mut t, "sketch.create", json!({"plane": "XY", "offset": 6, "name": "Outline"}));
        let _ = try_run(&mut t, "sketch.ellipse", json!({"center": [0, 0], "major": [7.5, 0], "minor_radius": 2.5, "axes": false}));
        let _ = try_run(&mut t, "sketch.finish", json!({}));
        let _ = try_run(&mut t, "solid.extrude", json!({"sketch": "Outline", "distance": 6, "direction": "negative", "taper": 40, "operation": "intersect", "targets": ["Part"]}));
        eprintln!("PROBE radius {r}");
        let _ = try_run(&mut t, "solid.fillet", json!({"edges": [[0, 2.5, 6]], "radius": r}));
    }
    // No taper: is the draft the trigger?
    let mut t = Session::default();
    let _ = try_run(&mut t, "solid.box", json!({"length": 16, "width": 9, "height": 6, "center": [0, 0, 3], "body_name": "Part"}));
    let _ = try_run(&mut t, "sketch.create", json!({"plane": "XY", "offset": 6, "name": "Outline"}));
    let _ = try_run(&mut t, "sketch.ellipse", json!({"center": [0, 0], "major": [7.5, 0], "minor_radius": 2.5, "axes": false}));
    let _ = try_run(&mut t, "sketch.finish", json!({}));
    let _ = try_run(&mut t, "solid.extrude", json!({"sketch": "Outline", "distance": 6, "direction": "negative", "operation": "intersect", "targets": ["Part"]}));
    eprintln!("PROBE no-taper");
    let _ = try_run(&mut t, "solid.fillet", json!({"edges": [[0, 2.5, 6]], "radius": 0.2}));
}

#[test]
fn probe_21548_mirror_of_pattern_copy_after_deleting_siblings() {
    let dir = std::env::temp_dir().join(format!("probe21548-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("P.solidcraft");
    let mut part = Session::default();
    let _ = try_run(&mut part, "solid.box", json!({"length": 10, "width": 10, "height": 2, "body_name": "P"}));
    let _ = try_run(&mut part, "file.save", json!({"path": source}));

    let mut s = Session::default();
    let _ = try_run(&mut s, "component.insert_design", json!({"path": source}));
    let _ = try_run(&mut s, "component.insert_design", json!({"path": source, "reuse": true, "translate": [20, 0, 0]}));
    let _ = try_run(&mut s, "component.insert_design", json!({"path": source, "reuse": true, "translate": [40, 0, 0]}));
    let occ: Vec<u64> = s.doc.occurrences.iter().map(|o| o.id).collect();
    eprintln!("PROBE occurrences {occ:?}");
    report(&s, "inserted");
    let names: Vec<String> = s.model.state().bodies.iter().map(|b| b.name.clone()).collect();
    let third = names.last().cloned().unwrap_or_default();
    let _ = try_run(&mut s, "solid.pattern.rectangular", json!({"bodies": [third], "dir1": [0, 1, 0], "count1": 2, "spacing1": 20, "name": "REP"}));
    report(&s, "patterned");
    let copy = s.model.state().bodies.last().map(|b| b.name.clone()).unwrap_or_default();
    let _ = try_run(&mut s, "solid.mirror", json!({"bodies": [third, copy], "plane": {"origin": [0, 0, 10], "normal": [0, 0, 1]}, "name": "SYM"}));
    let _ = try_run(&mut s, "timeline.inspect", json!({"feature": "SYM"}));
    report(&s, "mirrored");
    let others: Vec<u64> = occ.iter().take(occ.len().saturating_sub(1)).copied().collect();
    let _ = try_run(&mut s, "selection.delete", json!({"occurrences": others, "dry_run": true}));
    let _ = try_run(&mut s, "selection.delete", json!({"occurrences": others}));
    let _ = try_run(&mut s, "timeline.compute_all", json!({}));
    report(&s, "after-delete");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn probe_21547_narrow() {
    let cases: Vec<(&str, Value, &str, [f64; 3])> = vec![
        ("circle-intersect", json!({"center": [0, 0], "radius": 2.5}), "sketch.circle", [0.0, 2.5, 6.0]),
        ("ellipse-intersect-pick-major", json!({"center": [0, 0], "major": [7.5, 0], "minor_radius": 2.5, "axes": false}), "sketch.ellipse", [7.5, 0.0, 6.0]),
    ];
    for (tag, sk, cmd, pick) in cases {
        eprintln!("PROBE case {tag}");
        let mut t = Session::default();
        let _ = try_run(&mut t, "solid.box", json!({"length": 16, "width": 9, "height": 6, "center": [0, 0, 3], "body_name": "Part"}));
        let _ = try_run(&mut t, "sketch.create", json!({"plane": "XY", "offset": 6, "name": "Outline"}));
        let _ = try_run(&mut t, cmd, sk);
        let _ = try_run(&mut t, "sketch.finish", json!({}));
        let _ = try_run(&mut t, "solid.extrude", json!({"sketch": "Outline", "distance": 6, "direction": "negative", "taper": 40, "operation": "intersect", "targets": ["Part"]}));
        let _ = try_run(&mut t, "solid.fillet", json!({"edges": [pick], "radius": 0.2}));
    }
    for taper in [5.0, 20.0, 40.0] {
        eprintln!("PROBE case ellipse-new-body taper {taper}");
        let mut t = Session::default();
        let _ = try_run(&mut t, "sketch.create", json!({"plane": "XY", "offset": 6, "name": "Outline"}));
        let _ = try_run(&mut t, "sketch.ellipse", json!({"center": [0, 0], "major": [7.5, 0], "minor_radius": 2.5, "axes": false}));
        let _ = try_run(&mut t, "sketch.finish", json!({}));
        let _ = try_run(&mut t, "solid.extrude", json!({"sketch": "Outline", "distance": 6, "direction": "negative", "taper": taper}));
        let _ = try_run(&mut t, "model.faces", json!({"body": "Body1"}));
        let _ = try_run(&mut t, "solid.fillet", json!({"edges": [[0, 2.5, 6]], "radius": 0.2}));
    }
}
