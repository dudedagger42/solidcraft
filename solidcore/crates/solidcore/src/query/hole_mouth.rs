//! (not upstream) How high a hole's entry face stands over its rim: the clearance the hole's
//! tools need above the plane across the axis at the entry point.
//!
//! A plane with outward unit normal `n`, entered along the unit axis `d` at a point of the axis,
//! reaches `r |n x d| / |n . d|` above the plane across the axis within `r` of the axis, at the
//! rim point straight uphill. A convex face (a cylinder, cone or sphere with its material on the
//! axis' or centre's side) lies on or below its tangent plane at the entry point, so that plane's
//! height bounds it. No other face is decided here.

use crate::error::{EntityRef, Result, SolidError};
use crate::geom::tol::{brep_zone_tol_3d, SmallTopoType};
use crate::intersect::point_class::PointClassType;
use crate::kernel::Kernel;
use crate::mass::integrator::properties::region_material;
use crate::math::{Point3, Vec3};
use crate::nurbs::b_spline_surface_n_lib::p2::OrientType;
use crate::nurbs::math::MODEL_TOL;
use crate::query::brep::{self as qbrep, BrepClassifier};
use crate::query::face::p1::point_on_face_at;
use crate::query::normal::{certify, Sense};
use crate::surface::surface::{Surface, SurfaceKind};
use crate::topology::face::p2::{self as fp2, face_side_regions};
use crate::topology::{edge, vertex, Brep, FaceId, RegionId};

const OP: &str = "query::hole_mouth";

/// The height a plane with outward unit `normal` (proved within `normal_bound`) reaches above
/// the plane across the axis `dir`, within `radius` of the axis, measured from the point where
/// the axis enters it. `None` for a plane the axis does not enter: one facing along `dir`, or
/// parallel to it within `normal_bound` plus the kernel's model tolerance ([`MODEL_TOL`]), the
/// tilt below which the kernel takes two directions for the same.
///
/// # Errors
///
/// [`SolidError::InvalidInput`] for a `dir` of no length, a negative `radius` or a value that
/// is not finite.
pub fn plane_lip(normal: Vec3, normal_bound: f64, dir: Vec3, radius: f64) -> Result<Option<f64>> {
    let d = dir.unitize()?;
    let into = -normal.dot(d);
    let across = normal.cross(d).length();
    if !(into.is_finite() && across.is_finite() && normal_bound.is_finite() && radius >= 0.0) {
        return Err(SolidError::invalid_input(
            OP,
            "the normal, its bound or the radius is not usable",
        ));
    }
    if into <= normal_bound + MODEL_TOL {
        return Ok(None);
    }
    let lip = radius * across / into;
    if !lip.is_finite() {
        return Err(SolidError::invalid_input(OP, "the radius is not finite"));
    }
    Ok(Some(lip))
}

/// The height the faces of `b` that hold `spot` reach above the plane across the axis `dir`
/// through `spot`, within `radius` of the axis: the greatest [`plane_lip`] of the faces the axis
/// enters there (one face, or those of the edge or vertex at `spot`). The faces are the ones the
/// body's own classification of `spot` names, at the body's zone tolerance. Exact for a planar
/// face; for a convex cylinder, cone or sphere the height of its tangent plane at `spot`, an
/// upper bound. A sheet face is entered from either side. `None` where `spot` is not on the
/// body's boundary, or the axis enters none of the faces there.
///
/// # Errors
///
/// [`SolidError::Unsupported`] for an entered face that is neither planar nor convex as above;
/// [`SolidError::InvalidInput`] as [`plane_lip`], and for a face without its two uses, without
/// a surface, or that `spot` does not drop onto; the classification's, the normal's and its
/// certification's errors; [`SolidError::UnsureRegion`] for a side the kernel is unsure is
/// material.
pub fn hole_mouth_lip(
    k: &Kernel,
    b: &mut Brep,
    spot: Point3,
    dir: Vec3,
    radius: f64,
) -> Result<Option<f64>> {
    let tol = brep_zone_tol_3d(k, b, SmallTopoType::NormalSize);
    let at = BrepClassifier::of_brep(b).point_3d_classify(k, spot, tol, true)?;
    let faces = match (at.point_class(), at.object()) {
        (PointClassType::Face, Some(EntityRef::Face(f))) => vec![f],
        (PointClassType::Edge, Some(EntityRef::Edge(e))) => edge::faces(b, e, None)?,
        (PointClassType::Vertex, Some(EntityRef::Vertex(v))) => vertex::faces(b, v)?,
        _ => return Ok(None),
    };
    let mut highest = None;
    for f in faces {
        if let Some(lip) = face_lip(k, b, f, spot, tol, dir, radius)? {
            highest = Some(highest.map_or(lip, |h: f64| h.max(lip)));
        }
    }
    Ok(highest)
}

/// [`hole_mouth_lip`] of the one face `f`.
fn face_lip(
    k: &Kernel,
    b: &mut Brep,
    f: FaceId,
    spot: Point3,
    tol: f64,
    dir: Vec3,
    radius: f64,
) -> Result<Option<f64>> {
    let (Some(x), Some(y)) = fp2::faceuses(b, f)? else {
        return Err(SolidError::invalid_input(OP, "a face without its two uses"));
    };
    let material = |r: Option<RegionId>| r.map_or(Ok(false), |r| region_material(b, r));
    let [rx, ry] = face_side_regions(b, f)?;
    // the use that faces out of the material; a sheet has two, a partition none
    let (out, sheet) = match (material(rx)?, material(ry)?) {
        (true, true) => return Ok(None),
        (true, false) => (y, false),
        (false, behind) => (x, !behind),
    };
    let (sign, sense) = match b.faceuse(out)?.orientation {
        OrientType::Same => (1.0, Sense::Surface),
        OrientType::Opposite => (-1.0, Sense::Reversed),
        OrientType::Unknown => {
            return Err(SolidError::Unsupported {
                operation: OP,
                reason: "a faceuse whose orientation is unknown has no proved sense",
            })
        }
    };
    let Some((uv, on)) = point_on_face_at(k, b, f, spot, tol)? else {
        return Err(SolidError::invalid_input(
            OP,
            "the entry point does not drop onto its face",
        ));
    };
    let face_tol = fp2::tolerance(b, f)?;
    let surface =
        fp2::surface(b, f)?.ok_or_else(|| SolidError::invalid_input(OP, "face without surface"))?;
    let n = qbrep::evaluate_surface_normal(k, Some(surface), uv)?.unitize()?;
    let n = certify(k, surface, uv, n * sign, sense)?;
    let lip = match plane_lip(n.value, n.bound, dir, radius)? {
        None if sheet => plane_lip(-n.value, n.bound, dir, radius)?,
        lip => lip,
    };
    if lip.is_none() || surface.is_planar(k, face_tol)?.is_planar || convex(surface, on, n.value) {
        return Ok(lip);
    }
    Err(SolidError::Unsupported {
        operation: OP,
        reason:
            "a hole entering a face that is neither planar nor a convex cylinder, cone or sphere",
    })
}

/// Whether `surface` is a cylinder, cone or sphere whose `outward` normal at its point `on`
/// points away from its axis or centre: its material is convex, on or below every tangent plane.
fn convex(surface: &Surface, on: Point3, outward: Vec3) -> bool {
    match surface.kind() {
        SurfaceKind::Cone(c) => {
            let (o, a) = (c.rev.position.origin(), c.rev.position.z_axis());
            let v = Vec3::between(o, on);
            (v - a * v.dot(a)).dot(outward) > 0.0
        }
        SurfaceKind::Sphere(s) => Vec3::between(s.base.position.origin(), on).dot(outward) > 0.0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::plane_lip;
    use crate::math::Vec3;

    /// The outward normal of a face rising at `angle` towards the direction `turn` from +x.
    fn tilted(angle: f64, turn: f64) -> Vec3 {
        Vec3::new(
            -angle.sin() * turn.cos(),
            -angle.sin() * turn.sin(),
            angle.cos(),
        )
    }

    #[test]
    fn a_tilted_face_lifts_the_mouth_to_the_rim_height() {
        let (angle, radius) = (30f64.to_radians(), 3.0);
        let lip = plane_lip(tilted(angle, 0.0), 0.0, Vec3::new(0.0, 0.0, -1.0), radius)
            .unwrap()
            .expect("a lip");
        let want = radius * angle.tan();
        assert!((lip - want).abs() < 1e-9 * want, "lip {lip} vs {want}");
    }

    /// The same face with its uphill direction 22.5° from +x, between two of the eight rim
    /// points the lip was once probed at.
    #[test]
    fn a_face_tilted_between_the_rim_points_lifts_the_mouth_to_the_rim_height() {
        let (angle, turn, radius) = (30f64.to_radians(), 22.5f64.to_radians(), 3.0);
        let lip = plane_lip(tilted(angle, turn), 0.0, Vec3::new(0.0, 0.0, -2.0), radius)
            .unwrap()
            .expect("a lip");
        let want = radius * angle.tan();
        assert!((lip - want).abs() < 1e-9 * want, "lip {lip} vs {want}");
    }

    #[test]
    fn a_flat_face_has_no_lip_and_a_face_not_entered_has_none() {
        let down = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(plane_lip(Vec3::Z, 0.0, down, 3.0).unwrap(), Some(0.0));
        assert_eq!(plane_lip(Vec3::Z, 0.0, Vec3::Z, 3.0).unwrap(), None);
        assert_eq!(plane_lip(Vec3::X, 0.0, down, 3.0).unwrap(), None);
        assert!(plane_lip(Vec3::Z, 0.0, Vec3::ZERO, 3.0).is_err());
        assert!(plane_lip(Vec3::Z, 0.0, down, -1.0).is_err());
    }
}
