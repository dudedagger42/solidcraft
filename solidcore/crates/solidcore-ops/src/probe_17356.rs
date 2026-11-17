//! TEMPORARY probe for tangency-labs/solidcraft#17356 (probe branch only; not for merge).
//! Classifies points against a body the way the fillet's band check does, and prints the nearest
//! face hit of a ray fire along each axis from the first point.

use crate::body::Body;
use solidcore::blend::definition_check::surface_kind_name;
use solidcore::geom::tol::{brep_zone_tol_3d, SmallTopoType};
use solidcore::query::brep::{BrepClassifier, BrepQuery, Ray};
use solidcore::topology::face::p2::surface;
use solidcore::{Kernel, KernelOptions, Point3, Vec3};
use solidcore_record::brep::Brep as Record;

/// Classifies each of `points` against the body `record`, then prints the nearest face hit of a ray
/// from the first point along +x, -x, +y, -y, +z and -z.
pub fn classify_probe(record: &Record, label: &str, points: &[[f64; 3]]) {
    let Ok((body, _order)) = Body::from_record(record, true) else {
        eprintln!("[p17356b] {label}: the record did not load as a body");
        return;
    };
    let k = Kernel::new(&KernelOptions::default());
    let mut guard = body.kernel();
    {
        let brep: &solidcore::Brep = &guard;
        let tol = brep_zone_tol_3d(&k, brep, SmallTopoType::NormalSize);
        eprintln!("[p17356b] {label}: zone tolerance {tol:e}");
        let mut classifier = BrepClassifier::of_brep(brep);
        for p in points {
            let pt = Point3::new(p[0], p[1], p[2]);
            match classifier.point_3d_classify(&k, pt, tol, true) {
                Ok(pc) => eprintln!(
                    "[p17356b] {label}: point {p:?} class={:?} object={:?}",
                    pc.point_class(),
                    pc.object()
                ),
                Err(e) => eprintln!("[p17356b] {label}: point {p:?} classify error {e:?}"),
            }
        }
    }
    let Some(first) = points.first() else { return };
    let origin = Point3::new(first[0], first[1], first[2]);
    let dirs = [
        ("+x", Vec3::new(1.0, 0.0, 0.0)),
        ("-x", Vec3::new(-1.0, 0.0, 0.0)),
        ("+y", Vec3::new(0.0, 1.0, 0.0)),
        ("-y", Vec3::new(0.0, -1.0, 0.0)),
        ("+z", Vec3::new(0.0, 0.0, 1.0)),
        ("-z", Vec3::new(0.0, 0.0, -1.0)),
    ];
    for (name, dir) in dirs {
        let ray = match Ray::new(origin, dir) {
            Ok(ray) => ray,
            Err(e) => {
                eprintln!("[p17356b] {label}: ray {name} refused {e:?}");
                continue;
            }
        };
        let query = BrepQuery::of_brep(&guard);
        match query.first_face_hit(&k, &mut guard, ray) {
            Ok(Some(hit)) => {
                let kind = match surface(&guard, hit.face) {
                    Ok(Some(s)) => surface_kind_name(s),
                    _ => "none",
                };
                let r = (hit.point.x * hit.point.x + hit.point.y * hit.point.y).sqrt();
                eprintln!(
                    "[p17356b] {label}: ray {name} from {first:?}: nearest face {:?} kind={kind} at {:?} distance={:.6} r={r:.4}",
                    hit.face, hit.point, hit.distance
                );
            }
            Ok(None) => eprintln!("[p17356b] {label}: ray {name} from {first:?}: no face hit"),
            Err(e) => eprintln!("[p17356b] {label}: ray {name} error {e:?}"),
        }
    }
}
