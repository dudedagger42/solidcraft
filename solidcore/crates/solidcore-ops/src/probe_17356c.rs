//! TEMPORARY probe for tangency-labs/solidcraft#17356-c (probe branch only; not for merge).
//! Surface-level roots of the line from M along +x with Fillet2's rounds (FaceId 34 and 35), by
//! their revolution profile, each root's trim test (`point_on_face`), and the first face hit from
//! a point before the bore's entry.

use crate::body::Body;
use solidcore::blend::definition_check::surface_kind_name;
use solidcore::geom::tol::{brep_zone_tol_3d, SmallTopoType};
use solidcore::math::Point2;
use solidcore::query::brep::{BrepQuery, Ray};
use solidcore::topology::brep::p1::faces;
use solidcore::topology::face::p2::surface;
use solidcore::{Kernel, KernelOptions, Point3, Vec3};
use solidcore_record::brep::Brep as Record;

const M_Y: f64 = -52.769215120747496;
const M_Z: f64 = 99.32524009643143;

fn angle(x: f64, y: f64) -> f64 {
    y.atan2(x)
}

/// Surface-level roots of the line (t, M_Y, M_Z) for the faces 34 and 35, by their revolution:
/// the profile's (z, r) about the z axis through the origin, checked at two u first.
pub fn surface_probe(record: &Record, label: &str) {
    let Ok((body, order)) = Body::from_record(record, true) else {
        eprintln!("[p17356c] {label}: the record did not load as a body");
        return;
    };
    let k = Kernel::new(&KernelOptions::default());
    let mut guard = body.kernel();
    let all = match faces(&guard, None) {
        Ok(all) => all,
        Err(e) => {
            eprintln!("[p17356c] {label}: faces failed {e:?}");
            return;
        }
    };
    for (pos, f) in all.iter().enumerate() {
        let name = format!("{f:?}");
        if name != "FaceId(34)" && name != "FaceId(35)" {
            continue;
        }
        let s = match surface(&guard, *f) {
            Ok(Some(s)) => s,
            _ => {
                eprintln!("[p17356c] {label}: {name} has no surface");
                continue;
            }
        };
        let dom = match s.natural_uv_domain_with_nurb(&k) {
            Ok(dom) => dom,
            Err(e) => {
                eprintln!("[p17356c] {label}: {name} domain failed {e:?}");
                continue;
            }
        };
        let (lo, hi): (Point2, Point2) = (dom.min(), dom.max());
        eprintln!(
            "[p17356c] {label}: {name} position {pos} kind={} u [{:.9}, {:.9}] v [{:.9}, {:.9}]",
            surface_kind_name(s),
            lo.x,
            hi.x,
            lo.y,
            hi.y
        );
        let dbg = format!("{s:?}");
        eprintln!("[p17356c] {label}: {name} surface data (first 2500 chars): {}", dbg.chars().take(2500).collect::<String>());
        let eval = |u: f64, v: f64| s.evaluate_point(&k, Point2::new(u, v));
        let u1 = lo.x + 0.25 * (hi.x - lo.x);
        let u2 = lo.x + 0.7 * (hi.x - lo.x);
        let mut worst_axis = 0.0_f64;
        for i in 1..6 {
            let v = lo.y + (hi.y - lo.y) * (i as f64) / 6.0;
            if let (Ok(a), Ok(b)) = (eval(u1, v), eval(u2, v)) {
                let ra = (a.x * a.x + a.y * a.y).sqrt();
                let rb = (b.x * b.x + b.y * b.y).sqrt();
                worst_axis = worst_axis.max((a.z - b.z).abs()).max((ra - rb).abs());
            }
        }
        eprintln!("[p17356c] {label}: {name} revolution check about z through origin: max deviation between u1 and u2 = {worst_axis:.3e}");
        // Profile scan at u midway: (z, r) against the height M_Z.
        let u0 = lo.x + 0.5 * (hi.x - lo.x);
        let prof = |v: f64| eval(u0, v).ok().map(|p| (p.z, (p.x * p.x + p.y * p.y).sqrt()));
        let n = 4000usize;
        let mut prev: Option<(f64, f64)> = None;
        for i in 0..=n {
            let v = lo.y + (hi.y - lo.y) * (i as f64) / (n as f64);
            let Some((z, _)) = prof(v) else { continue };
            let dz = z - M_Z;
            if let Some((pv, pdz)) = prev {
                if pdz * dz < 0.0 {
                    let (mut a, mut b, mut fa) = (pv, v, pdz);
                    for _ in 0..80 {
                        let m = 0.5 * (a + b);
                        let Some((zm, _)) = prof(m) else { break };
                        let fm = zm - M_Z;
                        if fm.signum() == fa.signum() {
                            a = m;
                            fa = fm;
                        } else {
                            b = m;
                        }
                    }
                    let vr = 0.5 * (a + b);
                    if let Some((zr, rr)) = prof(vr) {
                        eprintln!("[p17356c] {label}: {name} profile root v={vr:.9} z={zr:.9} r={rr:.9}");
                        if rr > M_Y.abs() {
                            let half = (rr * rr - M_Y * M_Y).sqrt();
                            for t in [-half, half] {
                                let want = angle(t, M_Y);
                                // the u whose surface point at v=vr has the ray point's angle about z
                                let mut best: Option<(f64, f64, Point3)> = None;
                                for j in 0..=2000usize {
                                    let u = lo.x + (hi.x - lo.x) * (j as f64) / 2000.0;
                                    if let Ok(p) = eval(u, vr) {
                                        let mut d = (angle(p.x, p.y) - want).abs();
                                        if d > std::f64::consts::PI {
                                            d = 2.0 * std::f64::consts::PI - d;
                                        }
                                        if best.is_none_or(|(bd, _, _)| d < bd) {
                                            best = Some((d, u, p));
                                        }
                                    }
                                }
                                if let Some((d, u, p)) = best {
                                    // a second copy: `guard` holds the first body's lock
                                    let face_ok = match Body::from_record(record, true) {
                                        Ok((b2, o2)) => b2.point_on_face(&o2, pos, [p.x, p.y, p.z], 1e-5),
                                        Err(_) => Err(crate::error::Error::Argument),
                                    };
                                    eprintln!(
                                        "[p17356c] {label}: {name} root t={t:.9} (x of the ray point) surface point ({:.9}, {:.9}, {:.9}) uv=({u:.9}, {vr:.9}) angle gap={d:.3e} point_on_face(position {pos}, 1e-5)={face_ok:?}",
                                        p.x, p.y, p.z
                                    );
                                }
                            }
                        }
                    }
                }
            }
            prev = Some((v, dz));
        }
    }
    // First face hit from a point before the bore's entry, along +x.
    let origin = Point3::new(-20.0, M_Y, M_Z);
    match Ray::new(origin, Vec3::new(1.0, 0.0, 0.0)) {
        Ok(ray) => {
            let query = BrepQuery::of_brep(&guard);
            let tol = brep_zone_tol_3d(&k, &guard, SmallTopoType::NormalSize);
            match query.first_face_hit(&k, &mut guard, ray) {
                Ok(Some(hit)) => eprintln!(
                    "[p17356c] {label}: first_face_hit from (-20, M_Y, M_Z) along +x: face {:?} at {:?} distance {:.9} (zone tol {tol:e})",
                    hit.face, hit.point, hit.distance
                ),
                Ok(None) => eprintln!("[p17356c] {label}: first_face_hit from (-20, M_Y, M_Z) along +x: no hit"),
                Err(e) => eprintln!("[p17356c] {label}: first_face_hit error {e:?}"),
            }
        }
        Err(e) => eprintln!("[p17356c] {label}: ray refused {e:?}"),
    }
}
