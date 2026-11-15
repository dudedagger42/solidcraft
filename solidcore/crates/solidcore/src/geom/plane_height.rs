//! Whether a point, or a planar face by its corners, lies on a plane at a given height along
//! a unit direction (not upstream, #16864): Delete Face extending a neighbour's plane across a
//! gap. All-in-one 0.42 port: only #16864's part of this file (0.42 lacks #16713's
//! `is_square_plane_at`).

use crate::geom::vector3d::Vec3;

/// How far from the same direction two unit normals may be for their planes to face the same
/// way: one less their dot product.
const SAME_WAY: f64 = 1.0e-9;

/// Whether `point` lies within `tol` of the plane at `height` along the unit direction `normal`
/// from the world origin. A value that is not finite is on no plane: `false`.
#[must_use]
pub fn is_point_at(point: Vec3, normal: Vec3, height: f64, tol: f64) -> bool {
    (point.dot(normal) - height).abs() <= tol
}

/// Whether a planar face with unit normal `face_normal`, whose boundary has the corners
/// `corners`, lies on the plane at `height` along the unit direction `normal` from the world
/// origin, within `tol`: the normals point the same way (their dot within `1e-9` of one) and
/// every corner is on the plane ([`is_point_at`]). No corners, or a value that is not finite:
/// `false`.
#[must_use]
pub fn is_face_on_plane_at(
    face_normal: Vec3,
    corners: &[Vec3],
    normal: Vec3,
    height: f64,
    tol: f64,
) -> bool {
    face_normal.dot(normal) > 1.0 - SAME_WAY
        && !corners.is_empty()
        && corners.iter().all(|p| is_point_at(*p, normal, height, tol))
}
