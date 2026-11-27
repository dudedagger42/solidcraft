//! A plane frame laid onto a plane or a cylinder at the point under one of its own points,
//! keeping lengths ([`solidcore::surface::lay_frame`]).

pub use solidcore::surface::lay_frame::{LaidFrame, LaySurface};

/// The frame (`origin`, unit `x`, unit `y`) laid onto `surface` at the point under the frame
/// point `uv` along its normal, the nearer `near` of a cylinder's two
/// ([`solidcore::surface::lay_frame::lay_frame`]).
#[must_use]
pub fn lay_frame(
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    uv: (f64, f64),
    surface: LaySurface,
    near: [f64; 3],
) -> Option<LaidFrame> {
    solidcore::surface::lay_frame::lay_frame(origin, x, y, uv, surface, near)
}

/// The frame (`origin`, `x`, `y`), tangent to the cylinder about `axis` through `axis_origin` of
/// `radius`, rolled round it by `angle` without slipping
/// ([`solidcore::surface::lay_frame::roll_frame`]).
#[must_use]
pub fn roll_frame(
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    axis_origin: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    angle: f64,
) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    solidcore::surface::lay_frame::roll_frame(origin, x, y, axis_origin, axis, radius, angle)
}
