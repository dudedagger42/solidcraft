//! Probe for issue 17246 (measurements only, not a fix): the G2 corner patch of a filleted box
//! and whether its offset by -1 is certified. Prints what it finds; asserts nothing.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::float_arithmetic
)] // probe helpers

mod common;

use common::*;
use solidcore::nurbs::knots::Side;
use solidcore::nurbs::math::{CPoint, NOW};
use solidcore::nurbs::srf_basic::Surface as Nurb;
use solidcore::offset::construction::sheets::certify_regular;
use solidcore::surface::surface::SurfaceKind;
use solidcore_ops::*;

const ORIGIN: [f64; 3] = [-7.5, -7.5, 0.0];
const SIZE: [f64; 3] = [36.5, 38.5, 10.0];
const RADIUS: f64 = 4.0;
/// The shell's thickness, inward (negative), as the offset by -1 of the issue.
const THICKNESS: f64 = -1.0;
/// Samples per parameter direction are `GRID + 1` (inclusive ends, so the poles are hit).
const GRID: usize = 40;

fn corner_body(kind: Blend) -> Body {
    let b = make_box(ORIGIN, SIZE).unwrap();
    // the three top/side edges meeting at the corner (29, 31, 10): along x, along y, along z
    let edges = [
        edge_at(&b, [10.75, 31.0, 10.0]),
        edge_at(&b, [29.0, 11.75, 10.0]),
        edge_at(&b, [29.0, 31.0, 5.0]),
    ];
    blend(&b, kind, &edges, &[RADIUS], &[], Corners::Filled).unwrap();
    b
}

fn kind_name(s: &SurfaceKind) -> &'static str {
    match s {
        SurfaceKind::BSpline(_) => "BSpline",
        SurfaceKind::Plane(_) => "Plane",
        SurfaceKind::Cone(_) => "Cone",
        SurfaceKind::Torus(_) => "Torus",
        SurfaceKind::Sphere(_) => "Sphere",
        SurfaceKind::SurfOfRevolution(_) => "SurfOfRevolution",
        SurfaceKind::SurfOfExtrusion(_) => "SurfOfExtrusion",
        SurfaceKind::OffsetSurface(_) => "OffsetSurface",
        _ => "other",
    }
}

/// The Euclidean point of a control point (weighted coordinates divided by the weight).
fn euclid(c: CPoint) -> [f64; 3] {
    if c.w == NOW {
        [c.x, c.y, c.z]
    } else {
        [c.x / c.w, c.y / c.w, c.z / c.w]
    }
}

fn spread(pts: &[[f64; 3]]) -> f64 {
    let mut m = 0.0f64;
    for a in pts {
        for b in pts {
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            m = m.max(d);
        }
    }
    m
}

fn arr(v: solidcore::Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn describe(nurb: &Nurb) -> (f64, f64, f64, f64) {
    let (p, q) = nurb.degrees();
    println!("  degrees (p, q) = ({p}, {q})");
    println!("  u knots = {:?}", nurb.u_knots());
    println!("  v knots = {:?}", nurb.v_knots());
    let cpts = nurb.cpts();
    let rows = cpts.len();
    let cols = cpts[0].len();
    println!("  control net: {rows} x {cols} (first index = u, second = v, as cpts() returns it)");
    let mut wmin = f64::INFINITY;
    let mut wmax = f64::NEG_INFINITY;
    for row in &cpts {
        for c in row {
            wmin = wmin.min(c.w);
            wmax = wmax.max(c.w);
        }
    }
    println!("  weights in [{wmin}, {wmax}]");
    let last_r = rows - 1;
    let last_c = cols - 1;
    let line = |name: &str, pts: Vec<[f64; 3]>| {
        let s = spread(&pts);
        println!("  boundary {name}: max point spread {s:e}, collapsed: {}", s <= 1e-9);
    };
    line("row u=first", cpts[0].iter().map(|&c| euclid(c)).collect());
    line("row u=last", cpts[last_r].iter().map(|&c| euclid(c)).collect());
    line("col v=first", cpts.iter().map(|r| euclid(r[0])).collect());
    line("col v=last", cpts.iter().map(|r| euclid(r[last_c])).collect());
    (
        *nurb.u_knots().first().unwrap(),
        *nurb.u_knots().last().unwrap(),
        *nurb.v_knots().first().unwrap(),
        *nurb.v_knots().last().unwrap(),
    )
}

fn analyse(k: &solidcore::Kernel, nurb: &Nurb, label: &str) {
    println!("--- surface {label}");
    let (u0, u1, v0, v1) = describe(nurb);
    println!("  domain u [{u0}, {u1}], v [{v0}, {v1}]");

    // certify_regular at d = -1, whole domain and each quarter
    println!("  certify_regular(d = {THICKNESS}, whole) = {:?}", certify_regular(k, nurb, THICKNESS, None));
    let (um, vm) = (0.5 * (u0 + u1), 0.5 * (v0 + v1));
    let quarters = [
        ("u low, v low", [u0, um, v0, vm]),
        ("u high, v low", [um, u1, v0, vm]),
        ("u low, v high", [u0, um, vm, v1]),
        ("u high, v high", [um, u1, vm, v1]),
    ];
    for (name, uv) in quarters {
        println!(
            "  certify_regular(d = {THICKNESS}, quarter {name}) = {:?}",
            certify_regular(k, nurb, THICKNESS, Some(uv))
        );
    }

    // principal curvatures and |Su x Sv| on a (GRID+1)^2 inclusive grid
    let mut kmax = (0.0f64, f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    let mut nmin = (f64::INFINITY, f64::NAN, f64::NAN);
    let mut nan_count = 0usize;
    let mut tiny_count = 0usize;
    for i in 0..=GRID {
        for j in 0..=GRID {
            let u = u0 + (u1 - u0) * i as f64 / GRID as f64;
            let v = v0 + (v1 - v0) * j as f64 / GRID as f64;
            let Ok(sd) = nurb.derivs(u, v, Side::Left, Side::Left, false, 2, 2) else {
                nan_count += 1;
                continue;
            };
            let su = arr(sd[1][0]);
            let sv = arr(sd[0][1]);
            let suu = arr(sd[2][0]);
            let suv = arr(sd[1][1]);
            let svv = arr(sd[0][2]);
            let nv = cross(su, sv);
            let nn = dot(nv, nv).sqrt();
            if nn < nmin.0 {
                nmin = (nn, u, v);
            }
            if nn <= 1e-9 {
                tiny_count += 1;
                continue;
            }
            let nh = [nv[0] / nn, nv[1] / nn, nv[2] / nn];
            let e = dot(su, su);
            let f = dot(su, sv);
            let g = dot(sv, sv);
            let l = dot(suu, nh);
            let m = dot(suv, nh);
            let n2 = dot(svv, nh);
            let det = e * g - f * f;
            let gauss = (l * n2 - m * m) / det;
            let mean = (e * n2 + g * l - 2.0 * f * m) / (2.0 * det);
            let disc = (mean * mean - gauss).max(0.0).sqrt();
            let (k1, k2) = (mean + disc, mean - disc);
            let mag = k1.abs().max(k2.abs());
            if mag > kmax.0 {
                kmax = (mag, u, v, k1, k2);
            }
        }
    }
    println!("  samples failing derivs: {nan_count}; samples with |Su x Sv| <= 1e-9: {tiny_count}");
    println!(
        "  min |Su x Sv| = {:e} at (u, v) = ({}, {})",
        nmin.0, nmin.1, nmin.2
    );
    println!(
        "  max |principal curvature| = {:e} at (u, v) = ({}, {}), k1 = {:e}, k2 = {:e}",
        kmax.0, kmax.1, kmax.2, kmax.3, kmax.4
    );
    let _ = (um, vm);
    let _ = (u0, u1, v0, v1);
}

#[test]
fn probe_17246_corner_patch() {
    for (label, kind) in [("g2", Blend::Curvature), ("rolling", Blend::Round)] {
        println!("=== corner: {label} ===");
        let mut b = corner_body(kind);
        let (k, brep) = kernel_copy(&b);
        for f in brep.face_list() {
            let face = brep.face(*f).unwrap();
            let Some(surface) = face.surface.as_ref() else {
                println!("face {f:?}: no surface");
                continue;
            };
            let s = surface.kind();
            println!("face {f:?}: {}", kind_name(s));
            if let SurfaceKind::BSpline(bs) = s {
                match bs.gw_nurb() {
                    Some(nurb) => analyse(&k, nurb, &format!("{label} {f:?}")),
                    None => println!("  BSpline without a NURB"),
                }
            }
        }

        // the shell as the issue runs it: the top face (largest face at z = 10) opened
        let (_, order) = b.to_record().unwrap();
        let counts = b.counts().unwrap();
        let mut top: Option<(usize, f64)> = None;
        for i in 0..counts[0] {
            let c = b.face_centroid(&order, i, 1e-8).unwrap();
            if c.centroid[2] >= 9.999 && top.is_none_or(|(_, a)| c.area > a) {
                top = Some((i, c.area));
            }
        }
        let (top, area) = top.expect("a top face at z = 10");
        println!("top face index {top}, area {area}");
        let shelled = shell(&b, &order, &[top], THICKNESS);
        println!("shell(open top, thickness {THICKNESS}) = {shelled:?}");
        let _ = &mut b;
    }
}
