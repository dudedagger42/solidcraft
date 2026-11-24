//! Probe for #21864 (scan, not for PR): Offset Face fills a split-in pocket depending on the
//! pocket outline's direction.

use serde_json::{Value, json};
use solidcraft_engine::Session;

type P = [f64; 2];

fn elements() -> Vec<(bool, Vec<P>)> {
    vec![
        (true, vec![[-8.44878, 378.5], [-3.799174, 378.5]]),
        (false, vec![[-3.799174, 378.5], [-2.442599, 378.499905], [-1.254548, 377.587947], [-0.902724, 376.277972]]),
        (false, vec![[-0.902724, 376.277972], [-0.550956, 374.96748], [-1.122935, 373.583376], [-2.297185, 372.903494]]),
        (false, vec![[-2.297185, 372.903494], [-3.848691, 372.005326], [-5.399984, 371.106791], [-6.950631, 370.207141]]),
        (false, vec![[-6.950631, 370.207141], [-7.875272, 369.670732], [-9.015685, 369.668575], [-9.940929, 370.204788]]),
        (false, vec![[-9.940929, 370.204788], [-10.867204, 370.741687], [-11.433462, 371.734105], [-11.433499, 372.803648]]),
        (true, vec![[-11.433499, 372.803648], [-11.433499, 375.5]]),
        (false, vec![[-11.433499, 375.5], [-11.433488, 376.294081], [-11.120363, 377.057985], [-10.56026, 377.62132]]),
        (false, vec![[-10.56026, 377.62132], [-10.001249, 378.183525], [-9.241693, 378.499988], [-8.44878, 378.5]]),
    ]
}

fn script(els: &[(bool, Vec<P>)]) -> Value {
    let mut c = vec![
        json!({"command": "sketch.create", "params": {"plane": "XY", "name": "Skin"}}),
        json!({"command": "sketch.circle.center", "params": {"center": [0, 0], "diameter": 198.0}}),
        json!({"command": "sketch.circle.center", "params": {"center": [0, 0], "diameter": 202.0}}),
        json!({"command": "sketch.finish", "params": {}}),
        json!({"command": "solid.extrude", "params": {"distance": 400.0, "body_name": "Tube"}}),
        json!({"command": "solid.split_face", "params": {"faces": [[0, 99, 10], [0, -99, 10]], "plane": {"origin": [0, 0, 0], "normal": [0, 1, 0]}}}),
        json!({"command": "sketch.create", "params": {"plane": {"origin": [0.0, 0.0, 0.0], "x_dir": [-0.382683, 0.92388, 0.0], "y_dir": [0, 0, 1]}, "name": "Pocket"}}),
    ];
    for (line, pts) in els {
        if *line {
            c.push(json!({"command": "sketch.line", "params": {"points": pts}}));
        } else {
            c.push(json!({"command": "sketch.spline.control_point", "params": {"points": pts}}));
        }
    }
    c.push(json!({"command": "sketch.finish", "params": {}}));
    c.push(json!({"command": "solid.split_face", "params": {"faces": [[91.464074, 37.88566, 10.0]], "sketch": "Pocket", "loops": true}}));
    c.push(json!({"command": "solid.offset_face", "params": {"faces": [[91.464074, 37.88566, 10.0], [91.464074, -37.88566, 10.0]], "distance": 4.0}}));
    json!({ "commands": c })
}

fn run(name: &str, els: &[(bool, Vec<P>)]) {
    eprintln!("P21864 ===== {name}");
    let mut s = Session::default();
    s.run_script(&script(els)).unwrap();
    let b = s.model.state().body("Tube").unwrap();
    let m = solidcraft_kernel::measure(&b.body).unwrap();
    eprintln!("P21864 RESULT {name}: volume {:.3} faces {}", m.volume, b.body.record().faces.len());
}

#[test]
fn probe_21864() {
    let bad = elements();
    run("bad", &bad);
    let rev: Vec<(bool, Vec<P>)> = bad.iter().rev().map(|(l, p)| (*l, p.iter().rev().copied().collect())).collect();
    run("reversed", &rev);
    // Each spline element alone reversed in place (order kept), to see which one matters.
    for k in 0..bad.len() {
        if bad[k].0 {
            continue;
        }
        let mut one = bad.clone();
        one[k].1.reverse();
        run(&format!("flip-{k}"), &one);
    }
}
