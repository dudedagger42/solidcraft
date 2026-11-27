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

/// The half of a cylinder about `axis` a line along `normal` crosses once, towards `towards`
/// ([`solidcore::surface::lay_frame::crossing_side`]).
#[must_use]
pub fn crossing_side(axis: [f64; 3], normal: [f64; 3], towards: [f64; 3]) -> Option<[f64; 3]> {
    solidcore::surface::lay_frame::crossing_side(axis, normal, towards)
}

/// Whether two cylinders (axis point, unit axis, radius, convex) are one surface within `tol`
/// ([`solidcore::surface::lay_frame::same_cylinder`]).
#[must_use]
pub fn same_cylinder(
    a: ([f64; 3], [f64; 3], f64, bool),
    b: ([f64; 3], [f64; 3], f64, bool),
    tol: f64,
) -> bool {
    solidcore::surface::lay_frame::same_cylinder(a, b, tol)
}

/// The unit `direction` turned towards `towards` ([`solidcore::surface::lay_frame::toward`]).
#[must_use]
pub fn toward(direction: [f64; 3], towards: [f64; 3]) -> Option<[f64; 3]> {
    solidcore::surface::lay_frame::toward(direction, towards)
}

/// A frame (`x`, `y`) laid so its point `uv` lands on `at`, turned square to `face_normal`
/// ([`solidcore::surface::lay_frame::lay_frame_at`]).
#[must_use]
pub fn lay_frame_at(
    x: [f64; 3],
    y: [f64; 3],
    uv: (f64, f64),
    at: [f64; 3],
    face_normal: [f64; 3],
) -> Option<LaidFrame> {
    solidcore::surface::lay_frame::lay_frame_at(x, y, uv, at, face_normal)
}

/// The most a length shrinks laid on a sphere by the closest point, out to `reach`
/// ([`solidcore::surface::lay_frame::central_shrink`]).
#[must_use]
pub fn central_shrink(reach: f64, radius: f64) -> f64 {
    solidcore::surface::lay_frame::central_shrink(reach, radius)
}

/// The reach within which a length shrinks at most `shrink`
/// ([`solidcore::surface::lay_frame::central_reach`]).
#[must_use]
pub fn central_reach(radius: f64, shrink: f64) -> f64 {
    solidcore::surface::lay_frame::central_reach(radius, shrink)
}

/// The most a length stretches projected along `normal` onto a surface with these `normals`
/// ([`solidcore::surface::lay_frame::projected_stretch`]).
#[must_use]
pub fn projected_stretch(normal: [f64; 3], normals: &[[f64; 3]]) -> f64 {
    solidcore::surface::lay_frame::projected_stretch(normal, normals)
}
