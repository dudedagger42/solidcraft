use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};

use crate::*;

fn torso() -> Body {
    let p = Vec2::new;
    let pts = [p(-8., -17.), p(8., -17.), p(5., -30.), p(-5., -30.)];
    let segs = (0..4).map(|i| Seg2::Line { a: pts[i], b: pts[(i + 1) % 4] }).collect();
    crate::extrude(&Plane::XZ, &[Region2 { outer: Loop2 { segs }, holes: vec![] }], -4., 4.).unwrap().remove(0)
}

fn arm() -> Body {
    let b = box_solid(Vec3::new(6.5, -2.5, 18.), Vec3::new(10., 2.5, 28.)).unwrap();
    let c = cylinder(Vec3::new(5., 0., 26.), Vec3::X, 2., 2.).unwrap();
    boolean(&b, &c, BoolOp::Union).unwrap().unwrap()
}

const MIRROR_X: [[f64; 4]; 4] = [[-1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [0., 0., 0., 1.]];

#[test]
fn probe_13004_smb() {
    let dir = std::path::Path::new("B:/Claude/ct/opus-d-smb");
    let load = |n: &str| body_from_smlib(&solidcraft_smlib::Brep::read(&dir.join(format!("{n}.smb")), false).unwrap()).unwrap();
    let t = load("torso");
    for n in ["arm", "arm_mirrored", "cyl", "cyl_mirrored", "cyl_direct"] {
        let x = load(n);
        match boolean(&t, &x, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("smb torso+{n}: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("smb torso+{n}: none"),
            Err(e) => eprintln!("smb torso+{n}: ERR {e}"),
        }
        // Through SolidCraft's record, as an operation sends it.
        let y = Body::from_record(x.record().clone()).unwrap();
        let tt = Body::from_record(t.record().clone()).unwrap();
        match boolean(&tt, &y, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("rec torso+{n}: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("rec torso+{n}: none"),
            Err(e) => eprintln!("rec torso+{n}: ERR {e}"),
        }
    }
}

#[test]
fn probe_13004() {
    let t = torso();
    let a = arm();
    let m = transform_matrix(&a, MIRROR_X).unwrap();
    let tm = transform_matrix(&t, MIRROR_X).unwrap();
    let dir = std::path::Path::new("B:/Claude/ct/opus-d-smb");
    std::fs::create_dir_all(dir).unwrap();
    for (n, b) in [("torso", &t), ("arm", &a), ("arm_mirrored", &m), ("torso_mirrored", &tm)] {
        smlib_body(b).unwrap().write(&dir.join(format!("{n}.smb")), false).unwrap();
    }
    eprintln!("torso vol {:?} valid {:?}", measure(&t).map(|m| m.volume), validity(&t).smlib);
    eprintln!("arm vol {:?} valid {:?}", measure(&a).map(|m| m.volume), validity(&a).smlib);
    eprintln!("mirror arm vol {:?} valid {:?}", measure(&m).map(|m| m.volume), validity(&m).smlib);
    for (what, x, y) in [("torso+arm", &t, &a), ("torso+mirror", &t, &m), ("mirrortorso+arm", &tm, &a), ("arm+torso", &a, &t), ("mirror+torso", &m, &t)] {
        match boolean(x, y, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("{what}: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("{what}: none"),
            Err(e) => eprintln!("{what}: ERR {e}"),
        }
    }
    // Mirrored arm built directly.
    let b = box_solid(Vec3::new(-10., -2.5, 18.), Vec3::new(-6.5, 2.5, 28.)).unwrap();
    for (what, c) in [("cyl -X from -5", cylinder(Vec3::new(-5., 0., 26.), -Vec3::X, 2., 2.).unwrap()), ("cyl +X from -7", cylinder(Vec3::new(-7., 0., 26.), Vec3::X, 2., 2.).unwrap())] {
        let d = boolean(&b, &c, BoolOp::Union).unwrap().unwrap();
        match boolean(&t, &d, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("direct {what}: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("direct {what}: none"),
            Err(e) => eprintln!("direct {what}: ERR {e}"),
        }
        let cm = transform_matrix(&c, MIRROR_X).unwrap();
        match boolean(&t, &cm, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("torso + mirrored({what}) cyl alone: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("torso + mirrored cyl: none"),
            Err(e) => eprintln!("torso + mirrored({what}) cyl alone: ERR {e}"),
        }
    }
    let c = cylinder(Vec3::new(5., 0., 26.), Vec3::X, 2., 2.).unwrap();
    let cm = transform_matrix(&c, MIRROR_X).unwrap();
    eprintln!("cyl valid {:?} mirrored valid {:?}", validity(&c).smlib, validity(&cm).smlib);
    let direct = cylinder(Vec3::new(-7., 0., 26.), Vec3::X, 2., 2.).unwrap();
    let dir = std::path::Path::new("B:/Claude/ct/opus-d-smb");
    for (n, b) in [("cyl", &c), ("cyl_mirrored", &cm), ("cyl_direct", &direct)] {
        std::fs::write(dir.join(format!("{n}.rec.txt")), format!("{:#?}", b.record())).unwrap();
        smlib_body(b).unwrap().write(&dir.join(format!("{n}.smb")), false).unwrap();
    }
    eprintln!("cyl rec {:#?}", c.record().faces.iter().map(|f| format!("{:?}", f)).collect::<Vec<_>>());
    eprintln!("mir rec {:#?}", cm.record().faces.iter().map(|f| format!("{:?}", f)).collect::<Vec<_>>());
    for (what, x) in [("torso + cyl", &c), ("torso + mirrored cyl", &cm)] {
        match boolean(&t, x, BoolOp::Union) {
            Ok(Some(r)) => eprintln!("{what}: ok vol {:?}", measure(&r).map(|m| m.volume)),
            Ok(None) => eprintln!("{what}: none"),
            Err(e) => eprintln!("{what}: ERR {e}"),
        }
    }
}
