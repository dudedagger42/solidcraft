//! Probe for #21734 (scan round 11): classify points on a hollow tube's inner faces.

use solidcore_record::Vec3;
use solidcraft_kernel::*;

#[test]
fn probe_21734() {
    let outer = box_solid(Vec3::new(1000.0, 360.0, 752.0), Vec3::new(1060.0, 385.0, 990.0)).unwrap();
    let inner = box_solid(Vec3::new(1002.5, 362.5, 740.0), Vec3::new(1057.5, 382.5, 1000.0)).unwrap();
    let tube = boolean(&outer, &inner, BoolOp::Cut).unwrap().unwrap();
    eprintln!("faces={} tol={}", tube.face_count(), tube.record().tolerance);
    let cases = [
        ("inner x face", Vec3::new(1002.5, 370.5, 800.5)),
        ("inner x face far", Vec3::new(1057.5, 370.5, 900.5)),
        ("inner y face", Vec3::new(1010.5, 362.5, 800.5)),
        ("inner y face far", Vec3::new(1010.5, 382.5, 800.5)),
        ("inner edge", Vec3::new(1002.5, 362.5, 800.5)),
        ("inner edge far", Vec3::new(1057.5, 382.5, 800.5)),
        ("outer x face", Vec3::new(1000.0, 370.5, 800.5)),
        ("end face", Vec3::new(1001.5, 370.5, 752.0)),
        ("end face inner edge", Vec3::new(1002.5, 370.5, 752.0)),
        ("material", Vec3::new(1001.5, 370.5, 800.5)),
        ("air", Vec3::new(1003.5, 370.5, 800.5)),
    ];
    for (what, p) in cases {
        let one = tube.classify_points_with_normals(&[p], None);
        let class = tube.classify_point(p);
        eprintln!("{what} {p:?}: classify_point={class:?} with_normals={one:?}");
    }
    // A 1 mm grid offset by 0.5 across one cross-section, batch of 256 like the report.
    let mut pts = Vec::new();
    for i in 0..61 {
        for j in 0..26 {
            pts.push(Vec3::new(1000.5 + f64::from(i), 360.5 + f64::from(j), 800.5));
        }
    }
    let mut bad = 0;
    for chunk in pts.chunks(256) {
        if let Err(e) = tube.classify_points_with_normals(chunk, None) {
            bad += 1;
            for p in chunk {
                if let Err(e1) = tube.classify_points_with_normals(&[*p], None) {
                    eprintln!("grid point {p:?} alone fails: {e1}");
                }
            }
            eprintln!("batch fails: {e}");
        }
    }
    eprintln!("grid batches {} failing {bad}", pts.len().div_ceil(256));
}
