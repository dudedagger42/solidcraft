//! Scratch probe (not for upstream): replays brep-conformance 1.0.0 failure groups on main with
//! the calls the suite's solidcore adapter makes. Prints, never asserts.

use solidcore::boolean::api_brep::{boolean_difference, boolean_intersection, boolean_union};
use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::geom::axis2_placement::Axis2Placement;
use solidcore::intersect::point_class::PointClassType;
use solidcore::mass::integrator::properties::{
    PropertyAttributes, brep_area, brep_mass_properties, brep_volume,
};
use solidcore::offset::offset_executive::p2::{
    OffsetOptions, ShellOptions, offset_brep_simple, shell_brep_simple,
};
use solidcore::primitive::api_primitives as api;
use solidcore::query::api_queries::{face_classify_uv, get_faces, surface_closest_point};
use solidcore::query::brep::BrepClassifier;
use solidcore::topology::brep::validation::{CheckLevel, check_brep};
use solidcore::{Brep, FaceId, Kernel, KernelOptions, Point3, Tolerances, Vec3};

fn adapter_kernel() -> Kernel {
    let zone = 1e-7;
    Kernel::new(&KernelOptions {
        tolerances: Tolerances {
            zone_tol_3d: zone,
            xsect_tol_3d: 2.0 * zone,
            approx_tol_3d: zone / 2.0,
            ..Tolerances::default()
        },
        ..KernelOptions::default()
    })
}

fn default_kernel() -> Kernel {
    Kernel::new(&KernelOptions::default())
}

fn kernels() -> Vec<(&'static str, Kernel)> {
    vec![("adapter", adapter_kernel()), ("default", default_kernel())]
}

fn o() -> Point3 {
    Point3::new(0.0, 0.0, 0.0)
}

fn moved(k: &mut Kernel, mut b: Brep, at: [f64; 3]) -> Brep {
    let p = Axis2Placement::new(
        Point3::new(at[0], at[1], at[2]),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );
    solidcore::topology::brep::p5::transform(k, &mut b, &p, None, false).unwrap();
    b
}

fn check(k: &mut Kernel, b: &mut Brep) -> String {
    match check_brep(k, b, CheckLevel::Level2) {
        Ok(c) => {
            let l = c.lines();
            if l.is_empty() {
                "check OK".into()
            } else {
                let head: Vec<String> = l.iter().take(3).map(|s| s.chars().take(160).collect()).collect();
                format!("check {} issues: {}", l.len(), head.join(" | "))
            }
        }
        Err(e) => format!("check Err: {e}"),
    }
}

fn measures(k: &Kernel, b: &mut Brep) -> String {
    if b.face_list().is_empty() {
        return "empty (no faces)".into();
    }
    let store = AttributeStore::new();
    let key_of = |_| ObjectKey(0);
    let attrs = PropertyAttributes {
        store: &store,
        key_of: &key_of,
    };
    let v = brep_volume(k, b, 1e-7, &attrs).map_err(|e| e.to_string());
    let a = brep_area(k, b, 1e-7).map_err(|e| e.to_string());
    let m = brep_mass_properties(k, b, 1e-7, 1.0, o(), &attrs)
        .map(|m| format!("centroid {:?}", m.centroid))
        .map_err(|e| e.to_string());
    format!(
        "faces {} vol {:?} area {:?} mass {:?}",
        b.face_list().len(),
        v,
        a,
        m
    )
}

fn report(tag: &str, k: &mut Kernel, r: solidcore::Result<Brep>) {
    match r {
        Ok(mut b) => {
            let m = measures(k, &mut b);
            let c = check(k, &mut b);
            eprintln!("PROBE {tag}: Ok; {m}; {c}");
        }
        Err(e) => eprintln!("PROBE {tag}: Err {e}"),
    }
}

fn faces_at(k: &Kernel, b: &mut Brep, p: Point3) -> Option<FaceId> {
    let mut best: Option<(f64, FaceId)> = None;
    for f in get_faces(b).ok()? {
        let Some(surface) = b.face(f).ok()?.surface.clone() else {
            continue;
        };
        let Ok(near) = surface_closest_point(k, &surface, p) else {
            continue;
        };
        let Some(&uv) = near.uvs.first() else {
            continue;
        };
        let d = near.closest.distance;
        let held = matches!(
            face_classify_uv(k, b, f, uv),
            Ok(PointClassType::Face | PointClassType::Edge | PointClassType::Vertex)
        );
        if held && d <= 1e-6 && best.is_none_or(|(least, _)| d < least) {
            best = Some((d, f));
        }
    }
    best.map(|(_, f)| f)
}

#[test]
fn probe_offset() {
    for (name, mut k) in kernels() {
        let mut b = api::create_box(&mut k, o(), 10.0, 8.0, 6.0).unwrap();
        let opt = OffsetOptions {
            distance: 1.0,
            extend_convex_edges: false,
            merge_results: true,
        };
        let r = offset_brep_simple(&mut k, &mut b, opt);
        report(&format!("[{name}] offset box +1"), &mut k, r);
        let mut s = api::create_sphere(&mut k, o(), 5.0).unwrap();
        let r = offset_brep_simple(&mut k, &mut s, opt);
        report(&format!("[{name}] offset sphere +1"), &mut k, r);
        let mut c = api::create_cylinder(&mut k, o(), 3.0, 6.0).unwrap();
        let r = offset_brep_simple(&mut k, &mut c, opt);
        report(&format!("[{name}] offset cylinder +1"), &mut k, r);
    }
}

#[test]
fn probe_shell() {
    for (name, mut k) in kernels() {
        let mut b = api::create_box(&mut k, o(), 10.0, 8.0, 6.0).unwrap();
        let Some(top) = faces_at(&k, &mut b, Point3::new(5.0, 4.0, 6.0)) else {
            eprintln!("PROBE [{name}] shell box: no top face");
            continue;
        };
        let opt = ShellOptions {
            offset: OffsetOptions {
                distance: -1.0,
                extend_convex_edges: false,
                merge_results: true,
            },
            create_offset_solid: false,
            merge_sheet_side_walls: false,
        };
        let r = shell_brep_simple(&mut k, &b, opt, &[top]);
        report(&format!("[{name}] shell box open top t1"), &mut k, r);
        // As thick as the part (B18 expects a refusal).
        let opt2 = ShellOptions {
            offset: OffsetOptions {
                distance: -3.0,
                ..opt.offset
            },
            ..opt
        };
        let r = shell_brep_simple(&mut k, &b, opt2, &[top]);
        report(&format!("[{name}] shell box t3 (half of 6 high? as thick as part in z)"), &mut k, r);
        let opt3 = ShellOptions {
            offset: OffsetOptions {
                distance: -6.0,
                ..opt.offset
            },
            ..opt
        };
        let r = shell_brep_simple(&mut k, &b, opt3, &[top]);
        report(&format!("[{name}] shell box t6 (thicker than the part)"), &mut k, r);
    }
}

#[test]
fn probe_rejections() {
    let mut k = adapter_kernel();
    { let r = api::create_box(&mut k, o(), 0.0, 2.0, 2.0); report("box size 0", &mut k, r); }
    { let r = api::create_box(&mut k, o(), -3.0, 2.0, 2.0); report("box size -3", &mut k, r); }
    { let r = api::create_cylinder(&mut k, o(), 0.0, 2.0); report("cylinder r 0", &mut k, r); }
    { let r = api::create_cylinder(&mut k, o(), -2.0, 2.0); report("cylinder r -2", &mut k, r); }
    { let r = api::create_cylinder(&mut k, o(), 2.0, 0.0); report("cylinder h 0", &mut k, r); }
    { let r = api::create_sphere(&mut k, o(), 0.0); report("sphere r 0", &mut k, r); }
    { let r = api::create_sphere(&mut k, o(), -3.0); report("sphere r -3", &mut k, r); }
    { let r = api::create_torus(&mut k, o(), 1.0, 2.0); report("torus major 1 minor 2", &mut k, r); }
    { let r = api::create_torus(&mut k, o(), 2.0, 2.0); report("torus major 2 minor 2", &mut k, r); }
    { let r = api::create_cone(&mut k, o(), 0.0, 0.0, 3.0); report("cone 0 0", &mut k, r); }
}

#[test]
fn probe_touching() {
    for (name, mut k) in kernels() {
        // Boxes face to face: the intersection should be empty.
        let a = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap();
        let b0 = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap(); let b = moved(&mut k, b0, [10.0, 2.0, 3.0]);
        let r = boolean_intersection(&mut k, a, b);
        report(&format!("[{name}] boxes face-to-face a∩b"), &mut k, r);
        let a = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap();
        let b0 = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap(); let b = moved(&mut k, b0, [10.0, 2.0, 3.0]);
        let r = boolean_union(&mut k, a, b);
        report(&format!("[{name}] boxes face-to-face a∪b"), &mut k, r);
        // Boxes along an edge (line contact).
        let a = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap();
        let b0 = api::create_box(&mut k, o(), 10.0, 10.0, 10.0).unwrap(); let b = moved(&mut k, b0, [10.0, 10.0, 2.0]);
        let r = boolean_union(&mut k, a, b);
        report(&format!("[{name}] boxes along a line a∪b"), &mut k, r);
        // Sphere inscribed in a cylinder round its equator.
        let cyl = api::create_cylinder(&mut k, o(), 2.0, 8.0).unwrap();
        let s = api::create_sphere(&mut k, Point3::new(0.0, 0.0, 4.0), 2.0).unwrap();
        let r = boolean_difference(&mut k, cyl, s);
        report(&format!("[{name}] cylinder ∖ inscribed sphere"), &mut k, r);
        // Spheres touching at a point.
        let s1 = api::create_sphere(&mut k, o(), 2.0).unwrap();
        let s2 = api::create_sphere(&mut k, Point3::new(3.0, 0.0, 0.0), 1.0).unwrap();
        let r = boolean_union(&mut k, s1, s2);
        report(&format!("[{name}] spheres at a point a∪b"), &mut k, r);
    }
}

#[test]
fn probe_classify_boundary() {
    for (name, k) in kernels() {
        let mut kk = k;
        let b = api::create_box(&mut kk, o(), 10.0, 8.0, 6.0).unwrap();
        let mut c = BrepClassifier::of_brep(&b);
        for p in [
            Point3::new(5.0, 4.0, 6.0),
            Point3::new(10.0, 4.0, 3.0),
            Point3::new(10.0, 8.0, 3.0),
            Point3::new(10.0, 8.0, 6.0),
            Point3::new(5.0, 4.0, 3.0),
        ] {
            for bnd in [false, true] {
                let r = c.point_3d_classify(&kk, p, 1e-7, bnd);
                match r {
                    Ok(pc) => eprintln!("PROBE [{name}] classify boundary={bnd} {p:?}: {:?}", pc.point_class()),
                    Err(e) => eprintln!("PROBE [{name}] classify boundary={bnd} {p:?}: Err {e}"),
                }
            }
        }
        let s = api::create_sphere(&mut kk, o(), 5.0).unwrap();
        let mut c = BrepClassifier::of_brep(&s);
        for p in [Point3::new(5.0, 0.0, 0.0), Point3::new(0.0, 0.0, 5.0), Point3::new(0.0, 5.0, 0.0)] {
            match c.point_3d_classify(&kk, p, 1e-7, false) {
                Ok(pc) => eprintln!("PROBE [{name}] sphere classify {p:?}: {:?}", pc.point_class()),
                Err(e) => eprintln!("PROBE [{name}] sphere classify {p:?}: Err {e}"),
            }
        }
    }
}

fn edges_at(k: &Kernel, b: &Brep, pts: &[Point3]) -> Option<Vec<solidcore::EdgeId>> {
    let all = solidcore::topology::brep::p1::edges(b, None).ok()?;
    pts.iter()
        .map(|&p| {
            all.iter()
                .filter_map(|&e| {
                    let c = solidcore::query::api_queries::edge_closest_point(k, b, e, p).ok()?;
                    Some((c.closest.distance, e))
                })
                .filter(|(d, _)| *d <= 1e-6)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, e)| e)
        })
        .collect()
}

/// fillets/top-loop-after-corner-fillets and top-loop-bigger-than-corner-rounds: a box's four
/// side edges rounded at `rc`, then its whole top loop at `rt`.
#[test]
fn probe_fillet_top_loop() {
    use solidcore::blend::fillet_executive::api::fillet_edges;
    for (name, mut k) in kernels() {
        for (rc, rt) in [(1.5, 0.5), (1.5, 1.0), (1.0, 1.5), (1.0, 2.0)] {
            let (w, d, h) = (10.0, 8.0, 6.0);
            let mut b = api::create_box(&mut k, o(), w, d, h).unwrap();
            let sides: Vec<Point3> = [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d)]
                .iter()
                .map(|&(x, y)| Point3::new(x, y, h / 2.0))
                .collect();
            let ids = edges_at(&k, &b, &sides).unwrap();
            if let Err(e) = fillet_edges(&mut k, &mut b, &ids, rc, 1, 1, 1.0) {
                eprintln!("PROBE [{name}] corner rounds rc {rc}: Err {e}");
                continue;
            }
            let s = std::f64::consts::FRAC_1_SQRT_2;
            let mut top = vec![
                Point3::new(w / 2.0, 0.0, h),
                Point3::new(w, d / 2.0, h),
                Point3::new(w / 2.0, d, h),
                Point3::new(0.0, d / 2.0, h),
            ];
            for (cx, cy, sx, sy) in [(rc, rc, -1.0, -1.0), (w - rc, rc, 1.0, -1.0), (w - rc, d - rc, 1.0, 1.0), (rc, d - rc, -1.0, 1.0)] {
                top.push(Point3::new(cx + sx * rc * s, cy + sy * rc * s, h));
            }
            let Some(ids) = edges_at(&k, &b, &top) else {
                eprintln!("PROBE [{name}] top loop rc {rc}: edges not found");
                continue;
            };
            let r = fillet_edges(&mut k, &mut b, &ids, rt, 1, 1, 1.0).map(|()| b);
            report(&format!("[{name}] top loop rt {rt} after corners rc {rc}"), &mut k, r);
        }
    }
}
