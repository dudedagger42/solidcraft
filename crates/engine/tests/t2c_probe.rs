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

/// #13910's own attachment (shell_tangent_rim_repro.json), replayed step by step, then the
/// shell at 2 mm and 1 mm with the tangent chain off and on.
#[test]
fn t2c_13910_attachment_replay() {
    let script: Value = serde_json::from_str(include_str!("support/t2c_13910_repro.json")).unwrap();
    let commands = script["commands"].as_array().unwrap();
    let (build, shell) = commands.split_at(commands.len() - 1);
    for (t, chain) in [(2.0, false), (1.0, false), (2.0, true), (1.0, true), (0.5, false)] {
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
        let built = step(&mut s, "solid.shell", p);
        println!("T2C   RESULT t={t} chain={chain} built={built}");
        let st = s.model.state();
        for b in &st.bodies {
            println!("T2C   after: body {} faces {} shells {}", b.name, b.body.face_count(), b.body.shell_count());
        }
    }
}

