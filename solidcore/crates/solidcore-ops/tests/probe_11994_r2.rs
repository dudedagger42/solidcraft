//! PROBE (throwaway) #11994 round 2: the edges of the x>0 rim chamfer at d 1.0 / 1.5.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, unused)]

mod common;

use common::{all_fixes, kernel_copy};
use solidcore::nurbs::b_spline_surface_n_lib::p2::EvaluateFlags;
use solidcore::topology::edgeuse;
use solidcore::Point2;
use solidcore_ops::boolean::{boolean, BooleanOp};
use solidcore_ops::check::CheckLevel;
use solidcore_ops::{blend_ordered, make_cylinder, Blend, BlendOptions, Body, Corners};

fn drilled() -> Body {
    let body = make_cylinder([0.0; 3], 10.0, 20.0).unwrap();
    let tool = make_cylinder([0.0; 3], 4.669, 30.0).unwrap();
    tool.transform(&solidcore_ops::affine::then(
        &solidcore_ops::affine::rotation([0.0; 3], [0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2).unwrap(),
        &solidcore_ops::affine::translation([-15.0, 0.2, 11.8]).unwrap(),
    )
    .unwrap())
    .unwrap();
    boolean(body, tool, BooleanOp::Subtract).unwrap().unwrap()
}

fn rim_edge(body: &Body, sign: f64) -> usize {
    let (record, _) = body.to_record().unwrap();
    record
        .edges
        .iter()
        .position(|e| {
            let [s, t] = e.vertices;
            if s != t {
                return false;
            }
            let p = record.vertices[s as usize].point;
            let r = ((p[1] - 0.2).powi(2) + (p[2] - 11.8).powi(2)).sqrt();
            (r - 4.669).abs() < 1e-6 && p[0] * sign > 0.0
        })
        .unwrap()
}

fn kind(s: &solidcore::surface::surface::Surface) -> String {
    format!("{:?}", s.kind()).chars().take(14).collect()
}

fn dump(body: &Body, tag: &str) {
    let (k, b) = kernel_copy(body);
    eprintln!("PROBE {tag} brep tol {:e}", b.tolerance());
    for (fi, &f) in b.face_list().iter().enumerate() {
        let fd = b.face(f).unwrap();
        let s = fd.surface.as_ref().map(kind).unwrap_or_default();
        eprintln!("PROBE {tag} face {fi} {f:?} {s} dom {:?} tol {:e}", fd.uv_domain, fd.tolerance);
    }
    for (ei, &e) in b.edge_list().iter().enumerate() {
        let ed = b.edge(e).unwrap();
        let iv = ed.interval;
        let c = ed.curve.as_ref().unwrap();
        let vp: Vec<_> = ed.vertices.iter().map(|&v| { let vd = b.vertex(v).unwrap(); (v, vd.point, vd.tolerance) }).collect();
        let c0 = c.evaluate(&k, iv.min(), 0, true, false).unwrap()[0];
        let c1 = c.evaluate(&k, iv.max(), 0, true, false).unwrap()[0];
        eprintln!(
            "PROBE {tag} edge {ei} {e:?} tol {:e} iv ({:.6},{:.6}) c[{:.5?}->{:.5?}] verts {vp:.5?}",
            ed.tolerance, iv.min(), iv.max(), c0, c1
        );
        for &eu in &ed.edgeuses {
            let eud = b.edgeuse(eu).unwrap();
            let f = edgeuse::p1::face(&b, eu).unwrap().unwrap();
            let fd = b.face(f).unwrap();
            let surf = fd.surface.as_ref().unwrap();
            let Some(uv) = eud.uv_trim_curve.as_ref() else {
                eprintln!("PROBE {tag}   eu {eu:?} face {f:?} no uv");
                continue;
            };
            let nat = uv.natural_interval();
            let mut maxgap: f64 = 0.0;
            let mut at = 0.0;
            let mut uvs = Vec::new();
            for i in 0..=64 {
                let t = iv.min() + (iv.max() - iv.min()) * i as f64 / 64.0;
                let p = uv.evaluate(&k, t, 0, true, false).unwrap()[0];
                let s3 = surf.evaluate(&k, Point2::new(p.x, p.y), 0, 0, EvaluateFlags::default()).unwrap()[0][0];
                let q = c.evaluate(&k, t, 0, true, false).unwrap()[0];
                let g = (s3 - q).length();
                if g > maxgap { maxgap = g; at = t; }
                if i % 16 == 0 { uvs.push((p.x, p.y)); }
            }
            eprintln!(
                "PROBE {tag}   eu {eu:?} face {f:?} {:?} uvnat ({:.6},{:.6}) uv@0,1/4..1 {uvs:.5?} max|S(uv)-C| {maxgap:e} at t {at:.6} gapcache {:?}",
                eud.orientation, nat.min(), nat.max(), eud.max_edge_face_gap
            );
        }
    }
}

#[test]
fn probe_11994_r2() {
    all_fixes();
    for d in [1.0, 1.5, 2.0] {
        let body = drilled();
        let (_, order) = body.to_record().unwrap();
        let edge = rim_edge(&body, 1.0);
        let r = blend_ordered(&body, &order, Blend::Chamfer, &[edge], &[d], &[], Corners::Filled, BlendOptions::default());
        eprintln!("PROBE d {d} blend {:?}", r.as_ref().err());
        if r.is_err() {
            continue;
        }
        let check = body.check(CheckLevel::All);
        eprintln!("PROBE d {d} check {}", match &check { Ok(c) => c.to_string(), Err(e) => format!("{e:?}") });
        dump(&body, &format!("d{d}"));
    }
}
