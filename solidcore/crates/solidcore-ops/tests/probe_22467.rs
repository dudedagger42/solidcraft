//! Probe for #22467 (not for merge).
#![allow(clippy::unwrap_used)]

mod common;

use common::edge_at;
use solidcore_ops::boolean::boolean;
use solidcore_ops::{blend, make_box, make_cylinder, Blend, BooleanOp, Corners};

fn cut_at(x0: f64) {
    let b = make_box([x0, 0.0, 0.0], [60.0, 40.0, 20.0]).unwrap();
    let pts = [
        [30.0, 0.0, 20.0], [30.0, 40.0, 20.0], [0.0, 20.0, 20.0], [60.0, 20.0, 20.0],
        [30.0, 0.0, 0.0], [30.0, 40.0, 0.0], [0.0, 20.0, 0.0], [60.0, 20.0, 0.0],
        [0.0, 0.0, 10.0], [60.0, 0.0, 10.0], [0.0, 40.0, 10.0], [60.0, 40.0, 10.0],
    ];
    let edges: Vec<usize> = pts.iter().map(|p| edge_at(&b, [p[0] + x0, p[1], p[2]])).collect();
    blend(&b, Blend::Round, &edges, &[4.0], &[], Corners::Filled).unwrap();
    let c = make_cylinder([x0 + 15.0, 20.0, -5.0], 2.5, 30.0).unwrap();
    eprintln!("PROBE22467 start x0={x0}");
    let t = std::time::Instant::now();
    let r = boolean(b, c, BooleanOp::Subtract).unwrap().unwrap();
    eprintln!(
        "PROBE22467 x0={x0} cut_ms={:.1} vol={}",
        t.elapsed().as_secs_f64() * 1e3,
        r.volume(1e-8).unwrap_or(f64::NAN)
    );
}

#[test]
fn probe_22467_cut() {
    common::all_fixes();
    cut_at(0.0);
    cut_at(2000.0);
}
