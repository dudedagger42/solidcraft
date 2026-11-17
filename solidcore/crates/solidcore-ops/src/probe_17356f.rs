//! TEMPORARY probe for tangency-labs/solidcraft#17356-f (probe branch only; not for merge).
//! Walks the line from M to H-0.3 for the region changes (+z ray fire), bisects the first change
//! to X0, projects X0 onto faces 30..38, and solves the segments through the nearest face's surface.

use crate::body::Body;
use solidcore::geom::tol::{brep_zone_tol_3d, SmallTopoType};
use solidcore::math::Point2;
use solidcore::query::brep::{evaluate_surface_normal, BrepQuery};
use solidcore::surface::surface::Surface;
use solidcore::topology::brep::p1::faces;
use solidcore::topology::face::p2::surface;
use solidcore::topology::FaceId;
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
        let a = eval(s, k, u + hu, v)?;
        let b = eval(s, k, u - hu, v)?;
        let su = [(a[0] - b[0]) / (2.0 * hu), (a[1] - b[1]) / (2.0 * hu), (a[2] - b[2]) / (2.0 * hu)];
        let a = eval(s, k, u, v + hv)?;
        let b = eval(s, k, u, v - hv)?;
        let sv = [(a[0] - b[0]) / (2.0 * hv), (a[1] - b[1]) / (2.0 * hv), (a[2] - b[2]) / (2.0 * hv)];
        let (a11, a12, a22) = (dot(su, su), dot(su, sv), dot(sv, sv));
        let (b1, b2) = (-dot(su, f), -dot(sv, f));
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 {
            break;
        }
        u = (u + (b1 * a22 - a12 * b2) / det).clamp(lo.x, hi.x);
        v = (v + (a11 * b2 - a12 * b1) / det).clamp(lo.y, hi.y);
    }
    let q = eval(s, k, u, v)?;
    Some((u, v, dot(sub(q, p), sub(q, p)).sqrt()))
}

/// The walk, X0, the faces 30..38 near X0, and the roots of the two segments through the nearest.
pub fn walk_probe(record: &Record, label: &str, m: P3, h_minus: P3, y: f64, z: f64) {
    let Ok((body, _order)) = Body::from_record(record, true) else {
        eprintln!("[p17356f] {label}: the record did not load as a body");
        return;
    };
    let k = Kernel::new(&KernelOptions::default());
    let mut guard = body.kernel();
    let tol = {
        let brep: &solidcore::Brep = &guard;
        brep_zone_tol_3d(&k, brep, SmallTopoType::NormalSize)
    };
    let query = BrepQuery::of_brep(&guard);
    let up = Vec3::new(0.0, 0.0, 1.0);

    // 1. walk along x in 0.25 steps, the region by the +z fire; print where it changes
    let x_start = m[0];
    let steps = ((h_minus[0] - x_start) / 0.25).floor() as usize;
    let mut prev_x = x_start;
    let mut prev_r = query.probe_fire_region(&k, &mut guard, Point3::new(x_start, y, z), up, tol);
    eprintln!("[p17356f] {label}: start x={x_start:.9} region {prev_r:?}");
    let mut first_change: Option<(f64, f64)> = None;
    for i in 1..=steps {
        let x = x_start + 0.25 * i as f64;
        let r = query.probe_fire_region(&k, &mut guard, Point3::new(x, y, z), up, tol);
        if format!("{r:?}") != format!("{prev_r:?}") {
            eprintln!("[p17356f] {label}: region change between x={prev_x:.6} ({prev_r:?}) and x={x:.6} ({r:?})");
            if first_change.is_none() {
                first_change = Some((prev_x, x));
            }
        }
        prev_x = x;
        prev_r = r;
    }
    let Some((mut lo_x, mut hi_x)) = first_change else {
        eprintln!("[p17356f] {label}: no region change on the walk");
        return;
    };
    // 1b. bisect the first change to 1e-6: X0 is the first point of the new region
    let lo_r = query.probe_fire_region(&k, &mut guard, Point3::new(lo_x, y, z), up, tol);
    while hi_x - lo_x > 1e-6 {
        let mid = 0.5 * (lo_x + hi_x);
        let r = query.probe_fire_region(&k, &mut guard, Point3::new(mid, y, z), up, tol);
        if format!("{r:?}") == format!("{lo_r:?}") {
            lo_x = mid;
        } else {
            hi_x = mid;
        }
    }
    let x0 = hi_x;
    let r_at_x0 = query.probe_fire_region(&k, &mut guard, Point3::new(x0, y, z), up, tol);
    eprintln!("[p17356f] {label}: X0 = {x0:.9} (region {lo_r:?} before, {r_at_x0:?} at X0)");
    let x0p = [x0, y, z];

    // 2. faces 30..38: distance from X0, uv, the effective normal dotted with +x
    let mut near: Vec<(FaceId, f64)> = Vec::new();
    {
        let all = match faces(&guard, None) {
            Ok(all) => all,
            Err(e) => {
                eprintln!("[p17356f] {label}: faces failed {e:?}");
                return;
            }
        };
        for f in all.iter() {
            let name = format!("{f:?}");
            let num = name
                .strip_prefix("FaceId(")
                .and_then(|s| s.strip_suffix(')'))
                .and_then(|s| s.parse::<usize>().ok());
            let Some(num) = num else { continue };
            if !(30..=38).contains(&num) {
                continue;
            }
            let Ok(Some(s)) = surface(&guard, *f) else { continue };
            let Ok(dom) = s.natural_uv_domain_with_nurb(&k) else { continue };
            let (lo, hi): (Point2, Point2) = (dom.min(), dom.max());
            let Some((u, v, dist)) = project(s, &k, x0p, lo, hi) else { continue };
            let orient = guard
                .face(*f)
                .ok()
                .and_then(|fc| fc.faceuse)
                .and_then(|fu| guard.faceuse(fu).ok())
                .map(|fu| format!("{:?}", fu.orientation));
            let raw = match evaluate_surface_normal(&k, Some(s), Point2::new(u, v)) {
                Ok(n) => [n.x, n.y, n.z],
                Err(_) => [0.0; 3],
            };
            let flip = orient.as_deref().is_some_and(|o| o.contains("Opposite"));
            let eff = if flip { [-raw[0], -raw[1], -raw[2]] } else { raw };
            eprintln!(
                "[p17356f] {label}: {name} distance to X0 {dist:.3e} uv=({u:.9}, {v:.9}) orientation {orient:?} effective normal {eff:?} dot(+x) {:.6}",
                eff[0]
            );
            near.push((*f, dist));
        }
    }
    let Some(&(face, dist)) = near.iter().min_by(|a, b| a.1.total_cmp(&b.1)) else {
        eprintln!("[p17356f] {label}: no face of 30..38 in the body");
        return;
    };
    eprintln!("[p17356f] {label}: nearest face of 30..38 to X0 is {face:?} at {dist:.3e}");

    // 3. the roots of the long segment M to H and of the short segment X0-1 to X0+1 through `face`
    let long_end = [h_minus[0] + 0.3, y, z];
    let segments = [
        ("long M-H", m, long_end),
        ("short X0+-1", [x0 - 1.0, y, z], [x0 + 1.0, y, z]),
    ];
    for (name, p0, p1) in segments {
        let p0 = Point3::new(p0[0], p0[1], p0[2]);
        let p1 = Point3::new(p1[0], p1[1], p1[2]);
        match query.probe_segment_face_roots(&k, &mut guard, face, p0, p1, tol) {
            Ok(roots) => {
                eprintln!("[p17356f] {label}: {name} roots on {face:?}: {} root(s)", roots.len());
                for (p, t, u, v) in roots.iter() {
                    eprintln!(
                        "[p17356f] {label}:   root x={:.9} y={:.9} z={:.9} segment t={t:.9} uv=({u:.9}, {v:.9}) x-X0={:+.3e}",
                        p.x,
                        p.y,
                        p.z,
                        p.x - x0
                    );
                }
                let kept = roots.iter().any(|(p, _, _, _)| (p.x - x0).abs() < 1e-4);
                eprintln!("[p17356f] {label}: {name} keeps the X0 root within 1e-4: {kept}");
            }
            Err(e) => eprintln!("[p17356f] {label}: {name} solve failed {e:?}"),
        }
    }
}
