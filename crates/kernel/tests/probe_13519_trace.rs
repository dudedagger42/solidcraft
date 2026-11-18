//! Throwaway probe for #13519 (not for merge): shells, faces per shell and vertices near the
//! contact of wall + prism leaning g off the wall, over a sweep of g.

use solidcore_record::brep::Shell;
use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_kernel::{BoolOp, boolean, box_solid, extrude, measure};

fn prism(g: f64) -> solidcraft_kernel::Body {
    let p = [Vec2::new(-20.0, 5.0), Vec2::new(0.0, 5.0), Vec2::new(-g, 10.0), Vec2::new(-20.0, 10.0)];
    let segs = (0..4).map(|i| Seg2::Line { a: p[i], b: p[(i + 1) % 4] }).collect();
    extrude(&Plane::XY, &[Region2 { outer: Loop2 { segs }, holes: vec![] }], 0.0, 10.0).unwrap().pop().unwrap()
}

#[test]
fn probe_sweep() {
    let wall = box_solid(Vec3::ZERO, Vec3::new(8.0, 20.0, 20.0)).unwrap();
    println!("wall tol {:e}", wall.record().tolerance);
    for g in sweep() {
        println!("PROBE ===== g={g:e}");
        let pr = prism(g);
        match boolean(&wall, &pr, BoolOp::Union) {
            Err(e) => println!("PROBE g={g:e}: ERR {e:?}"),
            Ok(None) => println!("PROBE g={g:e}: NONE"),
            Ok(Some(b)) => {
                let m = measure(&b).unwrap();
                let r = b.record();
                let per_shell: Vec<usize> = r
                    .shells
                    .iter()
                    .map(|s| match s {
                        Shell::Faces { faces, .. } => faces.len(),
                        _ => 0,
                    })
                    .collect();
                let near: Vec<String> = r
                    .vertices
                    .iter()
                    .filter(|v| v.point[0].abs() < 1e-2)
                    .map(|v| format!("({:.3e},{:.4},{:.1} t{:.1e})", v.point[0], v.point[1], v.point[2], v.tolerance))
                    .collect();
                println!(
                    "PROBE g={g:e}: vol {:.7} want {:.7} dv {:.2e} shells {} regions {:?} faces/shell {:?} F{} E{} V{} tol {:e}",
                    m.volume,
                    4200.0 - 25.0 * g,
                    m.volume - (4200.0 - 25.0 * g),
                    m.shells,
                    r.regions.iter().map(|x| (x.shells.clone(), x.void)).collect::<Vec<_>>(),
                    per_shell,
                    r.faces.len(),
                    r.edges.len(),
                    r.vertices.len(),
                    r.tolerance
                );
                println!("PROBE   near x=0: {}", near.join(" "));
            }
        }
    }
}

fn sweep() -> Vec<f64> {
    vec![0.0, 1e-5, 1.5e-5, 1.9e-5, 2.1e-5, 2.5e-5, 3e-5, 5e-5, 7e-5, 1e-4, 1e-3, 1e-2]
}
