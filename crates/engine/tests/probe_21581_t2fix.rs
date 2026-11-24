//! Probe only (t2-ready scan round 3, 2026-11-24): #21581. Not for a PR.
use serde_json::{Value, json};
use solidcraft_engine::Session;

fn try_run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    let r = s.execute(id, &p).map_err(|e| e.to_string());
    eprintln!("PROBE {id} -> {}", match &r { Ok(v) => format!("ok {}", v.to_string().chars().take(200).collect::<String>()), Err(e) => format!("ERR {e}") });
    r
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

fn mults(knots: &[f64]) -> Vec<(f64, usize)> {
    let mut out: Vec<(f64, usize)> = Vec::new();
    for &k in knots {
        match out.last_mut() {
            Some(l) if l.0 == k => l.1 += 1,
            _ => out.push((k, 1)),
        }
    }
    out
}

fn dump(s: &Session, tag: &str) {
    use solidcore_record::brep as r;
    for b in &s.model.state().bodies {
        let n = b.body.record();
        eprintln!("PROBE {tag} body {:?} faces {} edges {} tol {}", b.name, n.faces.len(), n.edges.len(), n.tolerance);
        for (fi, f) in n.faces.iter().enumerate() {
            let surf = f.nurbs.as_ref().unwrap_or(&f.surface);
            let kind: String = match surf {
                r::Surface::Spline { u_knots, v_knots, points } => format!("Spline pts {}x{} u {:?} v {:?}", points.len(), points.first().map_or(0, Vec::len), mults(u_knots), mults(v_knots)),
                r::Surface::RationalSpline { u_knots, v_knots, homogeneous } => format!("RationalSpline pts {}x{} u {:?} v {:?}", homogeneous.len(), homogeneous.first().map_or(0, Vec::len), mults(u_knots), mults(v_knots)),
                other => format!("{other:?}").chars().take(200).collect(),
            };
            eprintln!("PROBE {tag} face {fi} domain {:?} surf {kind}", f.domain);
            for l in &f.loops {
                let r::Loop::Edges { edges } = l else { continue };
                for oe in edges {
                    let e = &n.edges[oe.edge as usize];
                    let (pts, knots): (Vec<[f64; 2]>, &Vec<f64>) = match &oe.pcurve {
                        r::Curve2::Spline { knots, points } => (points.clone(), knots),
                        r::Curve2::RationalSpline { knots, homogeneous } => (homogeneous.iter().map(|h| [h[0] / h[2], h[1] / h[2]]).collect(), knots),
                    };
                    let rep: Vec<usize> = pts.windows(2).enumerate().filter(|(_, w)| (w[0][0] - w[1][0]).abs() < 1e-6 && (w[0][1] - w[1][1]).abs() < 1e-6).map(|(k, _)| k).collect();
                    let v: Vec<[f64; 3]> = e.vertices.iter().map(|&v| n.vertices[v as usize].point).collect();
                    let c3: String = format!("{:?}", e.curve).chars().take(160).collect();
                    eprintln!("PROBE {tag}  face {fi} edge {} fwd {} iv {:?} verts {v:?} pcurve nk {} pts {} repeated {rep:?} c3 {c3}", oe.edge, oe.forward, e.interval, knots.len(), pts.len());
                    if !rep.is_empty() {
                        eprintln!("PROBE {tag}   BAD knots {:?}", mults(knots));
                        eprintln!("PROBE {tag}   BAD pts {pts:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn probe_21581_dump_pcurves() {
    let mut s = Session::default();
    s.set_validation(solidcraft_kernel::Validation::Off);
    section(&mut s, -30.0, "s0", false);
    section(&mut s, 30.0, "s1", false);
    let _ = try_run(&mut s, "solid.loft", json!({"sections": [{"sketch": "s0"}, {"sketch": "s1"}], "operation": "new", "name": "raum", "body_name": "innen"}));
    dump(&s, "loft");
    let _ = try_run(&mut s, "solid.box", json!({"length": "15 mm", "width": "1.7 mm", "height": "20 mm", "corner": [-35, 0, -20], "operation": "new", "body_name": "w"}));
    let _ = try_run(&mut s, "solid.combine", json!({"target": "innen", "tools": ["w"], "operation": "cut", "keep_tools": false}));
    dump(&s, "cut");
}
