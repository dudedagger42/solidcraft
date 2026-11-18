//! #11994: a chamfer of a hole's rim where a through hole crosses a cylinder wall, the ops twin of
//! `crates/kernel/tests/chamfer_hole_rim_through_cylinder.rs`. The body is mirror-symmetric
//! (x -> -x): both rims chamfer as one shell each, with equal removed volume within the kernel's
//! curve tolerance over the chamfer's area.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic
)] // test: the bound is a product of lengths

mod common;

use common::kernel_copy;
use solidcore_ops::boolean::{boolean, BooleanOp};
use solidcore_ops::{blend_ordered, make_cylinder, Blend, BlendOptions, Body, Corners};

const DISTANCES: [f64; 4] = [0.5, 1.0, 1.5, 2.0];
/// The rim's length on the cylinder (mm).
const RIM_LENGTH: f64 = 29.78;
/// The kernel's curve tolerance (mm) that each rim's chamfer is fitted to.
const TOLERANCE: f64 = 2e-5;

/// R10 x 20 with a through hole r 4.669 along x at y 0.2, z 11.8.
fn drilled() -> Body {
    let body = make_cylinder([0.0; 3], 10.0, 20.0).unwrap();
    let tool = make_cylinder([0.0; 3], 4.669, 30.0).unwrap();
    // The tool's axis is Z; turn it onto X, then move its base to x = -15.
    tool.transform(&solidcore_ops::affine::then(
        &solidcore_ops::affine::rotation([0.0; 3], [0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2)
            .unwrap(),
        &solidcore_ops::affine::translation([-15.0, 0.2, 11.8]).unwrap(),
    )
    .unwrap())
    .unwrap();
    boolean(body, tool, BooleanOp::Subtract).unwrap().unwrap()
}

/// The closed edge of the hole's rim on the `sign` side of x = 0: one vertex, on the hole.
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
            let from_hole_axis = ((p[1] - 0.2).powi(2) + (p[2] - 11.8).powi(2)).sqrt();
            (from_hole_axis - 4.669).abs() < 1e-6 && p[0] * sign > 0.0
        })
        .unwrap()
}

#[test]
fn chamfer_hole_rim_through_cylinder_wall_both_rims_alike() {
    for d in DISTANCES {
        let mut removed = [0.0; 2];
        for (i, sign) in [1.0, -1.0].into_iter().enumerate() {
            let body = drilled();
            let before = body.volume(1e-10).unwrap();
            let shells_before = kernel_copy(&body).1.shells().len();
            let (_, order) = body.to_record().unwrap();
            let edge = rim_edge(&body, sign);
            blend_ordered(
                &body,
                &order,
                Blend::Chamfer,
                &[edge],
                &[d],
                &[],
                Corners::Filled,
                BlendOptions::default(),
            )
            .unwrap();
            assert_eq!(kernel_copy(&body).1.shells().len(), shells_before, "d {d} sign {sign}");
            removed[i] = before - body.volume(1e-10).unwrap();
        }
        let bound = 2.0 * TOLERANCE * RIM_LENGTH * d * std::f64::consts::SQRT_2;
        assert!(
            (removed[0] - removed[1]).abs() <= bound,
            "d {d}: removed {removed:?}, bound {bound:e}"
        );
    }
}
