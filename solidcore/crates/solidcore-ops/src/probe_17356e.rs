//! TEMPORARY probe for tangency-labs/solidcraft#17356-e (probe branch only; not for merge).
//! At a hit point on a face: the face's uv, raw surface normal, faceuse orientation, the effective
//! outward normal and its dot with +x; and the region of the points on either side of the hit, by
//! classify (along +x) and by a ray fire along +z.

use crate::body::Body;
use solidcore::geom::tol::{brep_zone_tol_3d, SmallTopoType};
use solidcore::math::Point2;
use solidcore::query::brep::{evaluate_surface_normal, BrepClassifier, BrepQuery};
use solidcore::surface::surface::Surface;
use solidcore::topology::brep::p1::faces;
use solidcore::topology::face::p2::surface;
use solidcore::{Kernel, KernelOptions, Point3, Vec3};
use solidcore_record::brep::Brep as Record;

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn eval(s: &Surface, k: &Kernel, u: f64, v: f64) -> Option<P3> {
    s.evaluate_point(k, Point2::new(u, v)).ok().map(|q| [q.x, q.y, q.z])
}

/// The (u, v) of `s` nearest `p`: a grid search, then Gauss-Newton on |S(u,v) - p|.
fn project(s: &Surface, k: &Kernel, p: P3, lo: Point2, hi: Point2) -> Option<(f64, f64, f64)> {
    let n = 160usize;
    let mut best: Option<(f64, f64, f64)> = None;
    for i in 0..=n {
        for j in 0..=n {
            let u = lo.x + (hi.x - lo.x) * (i as f64) / (n as f64);
            let v = lo.y + (hi.y - lo.y) * (j as f64) / (n as f64);
            if let Some(q) = eval(s, k, u, v) {
                let d = dot(sub(q, p), sub(q, p)).sqrt();
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, u, v));
                }
            }
        }
    }
    let (_, mut u, mut v) = best?;
    let hu = 1e-7 * (hi.x - lo.x).max(1e-9);
    let hv = 1e-7 * (hi.y - lo.y).max(1e-9);
    for _ in 0..60 {
        let q = eval(s, k, u, v)?;
        let f = sub(q, p);
        let su = {
            let a = eval(s, k, u + hu, v)?;
            let b = eval(s, k, u - hu, v)?;
            [(a[0] - b[0]) / (2.0 * hu), (a[1] - b[1]) / (2.0 * hu), (a[2] - b[2]) / (2.0 * hu)]
        };
        let sv = {
            let a = eval(s, k, u, v + hv)?;
            let b = eval(s, k, u, v - hv)?;
            [(a[0] - b[0]) / (2.0 * hv), (a[1] - b[1]) / (2.0 * hv), (a[2] - b[2]) / (2.0 * hv)]
        };
        let (a11, a12, a22) = (dot(su, su), dot(su, sv), dot(sv, sv));
        let (b1, b2) = (-dot(su, f), -dot(sv, f));
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 {
            break;
        }
        let du = (b1 * a22 - a12 * b2) / det;
        let dv = (a11 * b2 - a12 * b1) / det;
        u = (u + du).clamp(lo.x, hi.x);
        v = (v + dv).clamp(lo.y, hi.y);
    }
    let q = eval(s, k, u, v)?;
    Some((u, v, dot(sub(q, p), sub(q, p)).sqrt()))
}

/// Normal, orientation and sides at `target` on the face named `face_name` (a FaceId debug name).
pub fn side_probe(record: &Record, label: &str, target: P3, face_name: &str) {
    let Ok((body, _order)) = Body::from_record(record, true) else {
        eprintln!("[p17356e] {label}: the record did not load as a body");
        return;
    };
    let k = Kernel::new(&KernelOptions::default());
    let mut guard = body.kernel();
    let all = match faces(&guard, None) {
        Ok(all) => all,
        Err(e) => {
            eprintln!("[p17356e] {label}: faces failed {e:?}");
            return;
        }
    };
    let Some(f) = all.iter().copied().find(|f| format!("{f:?}") == face_name) else {
        eprintln!("[p17356e] {label}: no {face_name}");
        return;
    };
    let orient = guard
        .face(f)
        .ok()
        .and_then(|fc| fc.faceuse)
        .and_then(|fu| guard.faceuse(fu).ok())
        .map(|fu| fu.orientation);
    // the face's surface: every use of it ends before the sides are classified below
    {
        let Ok(Some(s)) = surface(&guard, f) else {
            eprintln!("[p17356e] {label}: {face_name} has no surface");
            return;
        };
        let dom = match s.natural_uv_domain_with_nurb(&k) {
            Ok(dom) => dom,
            Err(e) => {
                eprintln!("[p17356e] {label}: domain failed {e:?}");
                return;
            }
        };
        let (lo, hi): (Point2, Point2) = (dom.min(), dom.max());
        let Some((u, v, dist)) = project(s, &k, target, lo, hi) else {
            eprintln!("[p17356e] {label}: {face_name} projection failed");
            return;
        };
        let uv = Point2::new(u, v);
        let on = eval(s, &k, u, v).unwrap_or(target);
        let raw = match evaluate_surface_normal(&k, Some(s), uv) {
            Ok(n) => [n.x, n.y, n.z],
            Err(e) => {
                eprintln!("[p17356e] {label}: normal failed {e:?}");
                return;
            }
        };
        let flip = matches!(orient, Some(ref o) if format!("{o:?}").contains("Opposite"));
        let eff = if flip { [-raw[0], -raw[1], -raw[2]] } else { raw };
        let xdir = [1.0, 0.0, 0.0];
        eprintln!(
            "[p17356e] {label}: {face_name} uv=({u:.9}, {v:.9}) |S-target|={dist:.3e} surface point {on:?} domain u [{:.6},{:.6}] v [{:.6},{:.6}]",
            lo.x, hi.x, lo.y, hi.y
        );
        eprintln!(
            "[p17356e] {label}: {face_name} raw surface normal {raw:?} faceuse orientation {orient:?} effective outward {eff:?} dot(+x) raw {:.6} effective {:.6}",
            dot(raw, xdir),
            dot(eff, xdir)
        );
    }
    // the two sides of the target along x, classified two ways
    for sign in [-1.0_f64, 1.0] {
        let q = [target[0] + sign * 0.3, target[1], target[2]];
        let tol = {
            let brep: &solidcore::Brep = &guard;
            brep_zone_tol_3d(&k, brep, SmallTopoType::NormalSize)
        };
        let by_x = {
            let brep: &solidcore::Brep = &guard;
            let mut classifier = BrepClassifier::of_brep(brep);
            match classifier.point_3d_classify(&k, Point3::new(q[0], q[1], q[2]), tol, true) {
                Ok(pc) => format!("classify along +x: {:?} {:?}", pc.point_class(), pc.object()),
                Err(e) => format!("classify error {e:?}"),
            }
        };
        let query = BrepQuery::of_brep(&guard);
        let by_z = match query.probe_fire_region(&k, &mut guard, Point3::new(q[0], q[1], q[2]), Vec3::new(0.0, 0.0, 1.0), tol) {
            Ok(region) => format!("ray fire along +z: region {region:?}"),
            Err(e) => format!("ray fire along +z error {e:?}"),
        };
        eprintln!(
            "[p17356e] {label}: side at x={:+.1} from target ({:.6}, {:.6}, {:.6}): {by_x}; {by_z}",
            sign * 0.3,
            q[0],
            q[1],
            q[2]
        );
    }
}
