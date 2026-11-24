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
        println!("PROBE freeform={freeform}: {:?}", r.err().map(|e| e.to_string()));
    }
}
