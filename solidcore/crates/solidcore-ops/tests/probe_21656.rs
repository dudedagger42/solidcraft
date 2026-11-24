//! Probe for #21656 (scan round 10, not for merge).
use solidcore_ops::blend::{blend, Blend, Corners};
use solidcore_ops::body::make_box;
use solidcore_ops::hlr::{self, Options};
use solidcore_record::brep::Surface;

#[test]
fn probe_21656() {
    let b = make_box([0.0; 3], [10.0; 3]).unwrap();
    let all: Vec<usize> = (0..12).collect();
    blend(&b, Blend::Round, &all, &[2.0; 12], &[], Corners::Filled).unwrap();
    let (rec, _) = b.to_record().unwrap();
    let mut kinds = std::collections::BTreeMap::new();
    for f in &rec.faces {
        let k = match &f.surface {
            Surface::Plane { .. } => "Plane",
            Surface::Cone { .. } => "Cone",
            Surface::Cylinder { .. } => "Cylinder",
            Surface::Sphere { .. } => "Sphere",
            Surface::BallPatch { .. } => "BallPatch",
            Surface::Blend { .. } => "Blend",
            Surface::RationalSpline { .. } => "RationalSpline",
            Surface::Spline { .. } => "Spline",
            _ => "other",
        };
        *kinds.entry((k, f.nurbs.is_some())).or_insert(0) += 1;
    }
    println!("PROBE faces {} kinds {:?}", rec.faces.len(), kinds);
    for freeform in [false, true] {
        let opts = Options { freeform, ..Options::default() };
        let r = hlr::prepare(&rec, &opts);
        match r {
            Err(e) => println!("PROBE freeform={freeform}: err {e}"),
            Ok(p) => {
                println!("PROBE freeform={freeform}: ok surfaces {:?} freeform_faces {:?}", p.report.surfaces, p.report.freeform_faces);
                let v = |x: f64, y: f64, z: f64| solidcore_record::Vec3 { x, y, z };
                let view = hlr::View::new(v(-1.0, -2.0, -3.0), v(0.0, 0.0, 1.0)).unwrap();
                let pl = hlr::Placement { body: 0, rotation: [v(1.,0.,0.), v(0.,1.,0.), v(0.,0.,1.)], translation: v(0.,0.,0.) };
                let d = hlr::hidden_lines(&[p], &[pl], &view, &opts, &|| false).unwrap();
                let mut c = std::collections::BTreeMap::new();
                for l in &d.lines {
                    let kind = format!("{:?}", l.source).split([' ', '{', '(']).next().unwrap().to_string();
                    for pc in &l.pieces {
                        *c.entry((kind.clone(), format!("{:?}", pc.visibility), pc.covered)).or_insert(0) += 1;
                    }
                }
                println!("PROBE   lines {} unsupported {} pieces {:?}", d.lines.len(), d.unsupported.len(), c);
            }
        }
    }
}
