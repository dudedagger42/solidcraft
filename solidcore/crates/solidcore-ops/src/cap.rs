//! Whether a point or a planar face lies on a plane ([`is_point_on_plane`],
//! [`is_face_on_plane`], #16864): the kernel's [`solidcore::geom::plane_height`] on records'
//! numbers. Ops converts. All-in-one 0.42 port: only #16864's part of this file.

use solidcore::geom::plane_height::{is_face_on_plane_at, is_point_at};
use solidcore::geom::vector3d::Vec3;

fn vec(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// Whether `point` lies within `tol` of the plane `normal · x = height` (`normal` unit; the
/// kernel's [`is_point_at`]).
#[must_use]
pub fn is_point_on_plane(point: [f64; 3], normal: [f64; 3], height: f64, tol: f64) -> bool {
    is_point_at(vec(point), vec(normal), height, tol)
}

/// Whether a planar face with unit normal `face_normal` and boundary corners `corners` lies on
/// the plane `normal · x = height` within `tol`: facing the same way, every corner on it (the
/// kernel's [`is_face_on_plane_at`]).
#[must_use]
pub fn is_face_on_plane(
    face_normal: [f64; 3],
    corners: &[[f64; 3]],
    normal: [f64; 3],
    height: f64,
    tol: f64,
) -> bool {
    let corners: Vec<Vec3> = corners.iter().map(|p| vec(*p)).collect();
    is_face_on_plane_at(vec(face_normal), &corners, vec(normal), height, tol)
}
