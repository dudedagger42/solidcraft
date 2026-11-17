//! TEMPORARY probe for tangency-labs/solidcraft#17356-d (probe branch only; not for merge).
//! Lists the faces whose sampled extent reaches z > 97.5 and r within 50..58 of the origin (the
//! Fillet2 round region), and whether a point lies on each (point_on_face, tolerance 0.01).

use crate::body::Body;
use solidcore::blend::definition_check::surface_kind_name;
use solidcore::math::Point2;
use solidcore::topology::brep::p1::faces;
use solidcore::topology::face::p2::surface;
use solidcore::{Kernel, KernelOptions};
use solidcore_record::brep::Brep as Record;

/// The faces near the round, and the distance from `p` to each face (point_on_face, 0.01).
pub fn face_list_probe(record: &Record, label: &str, p: [f64; 3]) {
    let Ok((body, order)) = Body::from_record(record, true) else {
        eprintln!("[p17356d] {label}: the record did not load as a body");
        return;
    };
    let k = Kernel::new(&KernelOptions::default());
    let guard = body.kernel();
    let all = match faces(&guard, None) {
        Ok(all) => all,
        Err(e) => {
            eprintln!("[p17356d] {label}: faces failed {e:?}");
            return;
        }
    };
    for (pos, f) in all.iter().enumerate() {
        let Ok(Some(s)) = surface(&guard, *f) else { continue };
        let dom = match s.natural_uv_domain_with_nurb(&k) {
            Ok(dom) => dom,
            Err(_) => continue,
        };
        let (lo, hi): (Point2, Point2) = (dom.min(), dom.max());
        // sampled extent over the natural domain (untrimmed: a superset of the face)
        let n = 24usize;
        let (mut zmin, mut zmax, mut rmin, mut rmax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        let mut near_round = false;
        for i in 0..=n {
            for j in 0..=n {
                let u = lo.x + (hi.x - lo.x) * (i as f64) / (n as f64);
                let v = lo.y + (hi.y - lo.y) * (j as f64) / (n as f64);
                if let Ok(q) = s.evaluate_point(&k, Point2::new(u, v)) {
                    let r = (q.x * q.x + q.y * q.y).sqrt();
                    zmin = zmin.min(q.z);
                    zmax = zmax.max(q.z);
                    rmin = rmin.min(r);
                    rmax = rmax.max(r);
                    if q.z > 97.5 && (50.0..=58.0).contains(&r) {
                        near_round = true;
                    }
                }
            }
        }
        if !near_round {
            continue;
        }
        // a second copy: `guard` holds the first body's lock
        let on = match Body::from_record(record, true) {
            Ok((b2, o2)) => b2.point_on_face(&o2, pos, p, 0.01),
            Err(_) => Err(crate::error::Error::Argument),
        };
        let on = match on {
            Ok(Some(q)) => {
                let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
                format!("on face: nearest ({:.6}, {:.6}, {:.6}) distance {d:.6}", q[0], q[1], q[2])
            }
            Ok(None) => "not on face within 0.01".to_string(),
            Err(e) => format!("point_on_face error {e:?}"),
        };
        eprintln!(
            "[p17356d] {label}: {f:?} position {pos} kind={} sampled bbox x/y/z approx r [{rmin:.3}, {rmax:.3}] z [{zmin:.3}, {zmax:.3}]; {on}",
            surface_kind_name(s)
        );
    }
}
