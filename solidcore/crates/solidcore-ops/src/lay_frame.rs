//! A plane frame laid onto a plane or a cylinder at the point under one of its own points,
//! keeping lengths ([`solidcore::surface::lay_frame`]), and the rest of laying profiles on a face
//! ([`solidcore::surface::lay_plan`]): the path chosen before anything is built, the profiles'
//! middle and reach, the slab over the face, the cone base on a sphere, the contact where a plane
//! runs on onto a round and the band a curved face sags within. The record's profiles and planes
//! are converted by `solidcore-convert`; nothing here computes a geometric quantity.

use solidcore::surface::lay_plan as plan;
use solidcore_convert as convert;
use solidcore_record::{Loop2, Plane, Region2, Vec2, Vec3};

pub use solidcore::surface::lay_frame::{LaidFrame, LaySurface};
pub use solidcore::surface::lay_plan::{
    FaceForm, LayDirection, LayPath, LayRefusal, TouchedWall, WallTouch,
};

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

/// The point of the line through `point` along unit `direction` nearest `p`
/// ([`solidcore::surface::lay_frame::line_step`]).
#[must_use]
pub fn line_step(point: [f64; 3], direction: [f64; 3], p: [f64; 3]) -> [f64; 3] {
    solidcore::surface::lay_frame::line_step(point, direction, p)
}

/// How profiles go on a face, decided before anything is built from the face's form, the
/// direction and the stop ([`solidcore::surface::lay_plan::lay_path`]).
///
/// # Errors
/// The [`LayRefusal`] the form gives.
pub fn lay_path(
    direction: LayDirection,
    form: FaceForm,
    stop: bool,
) -> Result<LayPath, LayRefusal> {
    plan::lay_path(direction, form, stop)
}

/// The middle of the bounds of the profiles' outer loops ([`plan::profile_middle`]).
#[must_use]
pub fn profile_middle(regions: &[Region2]) -> Option<Vec2> {
    plan::profile_middle(&kernel_regions(regions)).map(convert::record_vec2)
}

/// How far the profiles' outer loops reach from `mid` ([`plan::profile_reach`]).
#[must_use]
pub fn profile_reach(regions: &[Region2], mid: Vec2) -> f64 {
    plan::profile_reach(&kernel_regions(regions), convert::vec2(mid))
}

/// Points of the profiles' outlines and holes to check a face under
/// ([`plan::outline_samples`]).
#[must_use]
pub fn outline_samples(regions: &[Region2]) -> Vec<Vec2> {
    plan::outline_samples(&kernel_regions(regions))
        .into_iter()
        .map(convert::record_vec2)
        .collect()
}

/// How far the slab over a face reaches past it for `depth` ([`plan::slab_reach`]).
#[must_use]
pub fn slab_reach(depth: f64, exact: bool) -> (f64, f64) {
    plan::slab_reach(depth, exact)
}

/// The radii of a sphere's or cylinder's offsets `reach`, inner first ([`plan::slab_radii`]).
#[must_use]
pub fn slab_radii(radius: f64, convex: bool, reach: (f64, f64)) -> (f64, f64) {
    plan::slab_radii(radius, convex, reach)
}

/// The base of the cones over profiles laid on a sphere in `laid`, reaching past the slab
/// `reach`, and the factor the profiles are scaled by onto it ([`plan::sphere_cone_base`]).
#[must_use]
pub fn sphere_cone_base(
    centre: Vec3,
    radius: f64,
    laid: &Plane,
    reach: (f64, f64),
) -> Option<(Plane, f64)> {
    let (origin, k) = plan::sphere_cone_base(arr(centre), radius, arr(laid.origin), reach)?;
    Some((
        Plane {
            origin: Vec3::from(origin),
            x: laid.x,
            y: laid.y,
        },
        k,
    ))
}

/// `lp` scaled by `k` about its frame's origin ([`plan::scaled_loop`]).
#[must_use]
pub fn scaled_loop(lp: &Loop2, k: f64) -> Loop2 {
    convert::record_loop2(&plan::scaled_loop(&convert::loop2(lp), k))
}

/// The two half-spaces (a point of each plane, the direction into it) that hold a face under
/// profiles reaching `reach` from `anchor` ([`plan::sag_band`]).
#[must_use]
pub fn sag_band(
    anchor: Vec3,
    normal: Vec3,
    reach: f64,
    stretch: f64,
    slab: (f64, f64),
    tol: f64,
) -> [(Vec3, Vec3); 2] {
    plan::sag_band(arr(anchor), arr(normal), reach, stretch, slab, tol)
        .map(|(p, n)| (Vec3::from(p), Vec3::from(n)))
}

/// Of the points `hits` on lines along `normal` through `p`, the one nearest `p`, a tie going
/// behind it ([`plan::nearest_along`]).
#[must_use]
pub fn nearest_along(p: Vec3, normal: Vec3, hits: &[Option<Vec3>]) -> Option<usize> {
    let hits: Vec<_> = hits.iter().map(|h| h.map(arr)).collect();
    plan::nearest_along(arr(p), arr(normal), &hits)
}

/// The point of the axis through `origin` along `axis` at `z` ([`plan::on_axis`]).
#[must_use]
pub fn on_axis(origin: Vec3, axis: Vec3, z: f64) -> Vec3 {
    Vec3::from(plan::on_axis(arr(origin), arr(axis), z))
}

/// Where profiles laid in `laid`, touching `wall` along one of its straight edges, run on onto
/// it: the frame rolled to the middle of the wall face and a point there
/// ([`plan::wall_contact`]). `None` unless a profile runs past the line of contact.
#[must_use]
pub fn wall_contact(
    laid: &Plane,
    wall: &TouchedWall,
    regions: &[Region2],
    tol: f64,
) -> Option<(Plane, Vec3)> {
    let points = plan::outline_samples(&kernel_regions(regions));
    let c = plan::wall_contact(
        (arr(laid.origin), arr(laid.x), arr(laid.y)),
        wall,
        &points,
        tol,
    )?;
    Some((
        Plane {
            origin: Vec3::from(c.origin),
            x: Vec3::from(c.x),
            y: Vec3::from(c.y),
        },
        Vec3::from(c.on),
    ))
}

/// The whole wall a wrap from `laid`, touching `wall`, is built on ([`plan::wall_seat`]): its
/// base, its length and a bore's outer radius.
#[must_use]
pub fn wall_seat(
    laid: &Plane,
    regions: &[Region2],
    wall: &TouchedWall,
    convex: bool,
    depth: f64,
) -> Option<(Plane, f64, f64)> {
    let bounds = plan::profile_bounds(&kernel_regions(regions))?;
    let s = plan::wall_seat(
        (arr(laid.origin), arr(laid.x), arr(laid.y)),
        bounds,
        wall,
        convex,
        depth,
    )?;
    Some((
        Plane {
            origin: Vec3::from(s.origin),
            x: Vec3::from(s.x),
            y: Vec3::from(s.y),
        },
        s.length,
        s.outer,
    ))
}

fn kernel_regions(regions: &[Region2]) -> Vec<solidcore::prism::profile::Region2> {
    regions.iter().map(convert::region2).collect()
}

fn arr(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_run_onto_two_rounds_is_refused_by_name_and_a_sphere_cone_base_is_a_plane() {
        let e = lay_path(LayDirection::Face, FaceForm::Plane { walls: 2 }, false).unwrap_err();
        assert_eq!(e, LayRefusal::SeveralWalls);
        assert!(e.message().contains("more than one round"));
        let laid = Plane {
            origin: Vec3::new(3.0, 10.0, 0.0),
            x: Vec3::new(-1.0, 0.0, 0.0),
            y: Vec3::new(0.0, 0.0, 1.0),
        };
        let (base, k) =
            sphere_cone_base(Vec3::new(0.0, 0.0, 0.0), 10.0, &laid, (-0.2, 1.0)).unwrap();
        assert!((k - 1.2).abs() < 1e-12);
        assert!(
            (base.origin.x - 3.6).abs() < 1e-12
                && (base.origin.y - 12.0).abs() < 1e-12
                && base.origin.z.abs() < 1e-12
        );
        assert_eq!(base.x, laid.x);
        let square = Region2 {
            outer: Loop2::polygon(&[
                Vec2::new(-1.0, -1.0),
                Vec2::new(3.0, -1.0),
                Vec2::new(3.0, 1.0),
                Vec2::new(-1.0, 1.0),
            ]),
            holes: vec![],
        };
        let mid = profile_middle(std::slice::from_ref(&square)).unwrap();
        assert_eq!(mid, Vec2::new(1.0, 0.0));
        assert!((profile_reach(std::slice::from_ref(&square), mid) - 5f64.sqrt()).abs() < 1e-12);
    }
}
