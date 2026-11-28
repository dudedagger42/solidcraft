//! The recipe of an emboss raised along a face's normal (Normal to face), stopped at the edge of
//! its target face (Stop at face edge) or put on several faces (#21423).
//!
//! [`route`] decides how an emboss onto one face is built from the face's kind, the direction and
//! the stop, before anything is built; each route then takes what it needs from the calls here
//! and refuses with an [`EmbossRefusal`] that names the reason when it does not apply. Every
//! quantity is the kernel's ([`solidcore::surface::emboss_direction`],
//! [`solidcore::surface::lay_frame`], [`solidcore::surface::cylinder_wall`]); this picks the
//! samples, compares the kernel's numbers with the tolerances and reports.

use solidcore::surface::emboss_direction as k;
use solidcore_convert::{loop2, record_loop2};
use solidcore_record::{Plane, Region2, Vec2, Vec3};

use crate::cylinder_wall::{wall_frame, WallFrame};
use crate::lay_frame::{
    central_reach, central_shrink, crossing_side, lay_frame, lay_frame_at, line_step,
    projected_stretch, roll_frame, toward, LaySurface,
};

pub use solidcore::surface::emboss_direction::{EmbossRefusal, SIZE_TOLERANCE};

/// The most target faces one emboss takes.
pub const MAX_FACES: usize = 64;

/// Which way an emboss raises its profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Along the sketch's normal, the profiles keeping their size as projected.
    #[default]
    Sketch,
    /// Along the face's normal, the profiles keeping their size on the face.
    Face,
}

/// The surface of an emboss's target face, as the emboss reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FaceKind {
    /// The plane `normal · p = offset`, `normal` the face's outward unit normal.
    Plane {
        /// The outward unit normal.
        normal: Vec3,
        /// Its offset along the normal.
        offset: f64,
    },
    /// A cylinder wall: outside (`convex`) or a bore.
    Cylinder {
        /// A point of the axis.
        origin: Vec3,
        /// The axis's unit direction.
        axis: Vec3,
        /// The radius.
        radius: f64,
        /// Whether its outward normal points away from the axis.
        convex: bool,
    },
    /// A sphere: outside (`convex`) or a hollow.
    Sphere {
        /// Its centre.
        centre: Vec3,
        /// The radius.
        radius: f64,
        /// Whether its outward normal points away from the centre.
        convex: bool,
    },
    /// A cone, torus or freeform face.
    Other,
}

/// How an emboss onto one face is built, decided by [`route`] before anything is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Normal to sketch without the stop: the emboss as it has always been built.
    AsBefore,
    /// Normal to sketch, stopped at a cylinder wall's edge: the wrap where the sketch touches the
    /// wall, else the prisms along the sketch's normal on the side of the wall picked.
    SketchToWallEdge,
    /// Normal to sketch, stopped at a sphere's edge: the prisms on the side of it picked.
    SketchToSphereEdge,
    /// Normal to sketch, stopped at the edge of any other face: the prisms along its normal.
    SketchToFaceEdge,
    /// Normal to face on a plane: projected from the frame laid on it, or carried on onto the
    /// one round tangent to it the profiles run onto ([`onto_wall`]).
    FaceOnPlane,
    /// Normal to face on a cylinder wall: the wrap from the frame laid on it.
    FaceOnWall,
    /// Normal to face on a sphere: the cones from its centre.
    FaceOnSphere,
    /// Normal to face on a cone, torus or freeform face: projected along the face's normal under
    /// the middle of the profiles, within [`SIZE_TOLERANCE`].
    FaceOnCurved,
}

/// How an emboss onto a face of `kind` is built, raised `direction`, stopped at the face's edge
/// when `stop`.
#[must_use]
pub fn route(direction: Direction, stop: bool, kind: &FaceKind) -> Route {
    match (direction, stop, kind) {
        (Direction::Sketch, false, _) => Route::AsBefore,
        (Direction::Sketch, true, FaceKind::Cylinder { .. }) => Route::SketchToWallEdge,
        (Direction::Sketch, true, FaceKind::Sphere { .. }) => Route::SketchToSphereEdge,
        (Direction::Sketch, true, _) => Route::SketchToFaceEdge,
        (Direction::Face, _, FaceKind::Plane { .. }) => Route::FaceOnPlane,
        (Direction::Face, _, FaceKind::Cylinder { .. }) => Route::FaceOnWall,
        (Direction::Face, _, FaceKind::Sphere { .. }) => Route::FaceOnSphere,
        (Direction::Face, _, FaceKind::Other) => Route::FaceOnCurved,
    }
}

fn arr(v: Vec3) -> [f64; 3] {
    v.into()
}

fn vec(a: [f64; 3]) -> Vec3 {
    Vec3::from(a)
}

/// Points along the profiles' outer loops to bound them by.
fn outline_points(regions: &[Region2]) -> Vec<[f64; 2]> {
    regions
        .iter()
        .flat_map(|r| k::loop_points(&loop2(&r.outer), 256))
        .collect()
}

/// A depth and a number of profiles an emboss takes: finite, not zero, at most 1e6, and 1 to
/// 4096 profiles.
///
/// # Errors
/// [`EmbossRefusal::DepthOrProfiles`] otherwise.
pub fn checked(depth: f64, profiles: usize) -> Result<(), EmbossRefusal> {
    let depth_ok = depth.is_finite() && depth.abs() >= 1e-6 && depth.abs() <= 1e6;
    if depth_ok && (1..=4096).contains(&profiles) {
        Ok(())
    } else {
        Err(EmbossRefusal::DepthOrProfiles)
    }
}

/// The middle of the bounds of the profiles' outlines.
///
/// # Errors
/// [`EmbossRefusal::DepthOrProfiles`] for profiles with no points.
pub fn middle(regions: &[Region2]) -> Result<Vec2, EmbossRefusal> {
    let [u, v] =
        k::bounds_middle(&outline_points(regions)).ok_or(EmbossRefusal::DepthOrProfiles)?;
    Ok(Vec2::new(u, v))
}

/// How far the profiles' outlines reach from `mid`.
#[must_use]
pub fn reach(regions: &[Region2], mid: Vec2) -> f64 {
    k::reach(&outline_points(regions), [mid.x, mid.y])
}

/// The world point of the sketch point `uv` of `plane`.
#[must_use]
pub fn sketch_point(plane: &Plane, uv: Vec2) -> Vec3 {
    vec(k::frame_point(
        arr(plane.origin),
        arr(plane.x),
        arr(plane.y),
        [uv.x, uv.y],
    ))
}

/// The unit normal of `plane`.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] for axes that are parallel or not finite.
pub fn sketch_normal(plane: &Plane) -> Result<Vec3, EmbossRefusal> {
    k::frame_normal(arr(plane.x), arr(plane.y))
        .map(vec)
        .ok_or(EmbossRefusal::BadSketchFrame)
}

/// The surface of a face of `kind` that a frame is laid on; `None` for a cone, torus or
/// freeform face.
fn lay_surface(kind: &FaceKind) -> Option<LaySurface> {
    Some(match *kind {
        FaceKind::Plane { normal, offset } => LaySurface::Plane {
            point: k::along([0.0; 3], arr(normal), offset),
            normal: arr(normal),
        },
        FaceKind::Cylinder {
            origin,
            axis,
            radius,
            ..
        } => LaySurface::Cylinder {
            origin: arr(origin),
            axis: arr(axis),
            radius,
        },
        FaceKind::Sphere { centre, radius, .. } => LaySurface::Sphere {
            centre: arr(centre),
            radius,
        },
        FaceKind::Other => return None,
    })
}

/// The sketch's frame laid on a face, keeping lengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laid {
    /// The laid frame.
    pub plane: Plane,
    /// The point of the face's surface under the middle of the profiles.
    pub at: Vec3,
}

/// Where the line through the sketch point `mid` along the sketch's normal meets the surface of
/// a plane, cylinder or sphere face of `kind` (of two, the one nearer `near`); `None` where it
/// misses it or the face is of another kind.
#[must_use]
pub fn point_under(kind: &FaceKind, plane: &Plane, mid: Vec2, near: Vec3) -> Option<Vec3> {
    let f = lay_frame(
        arr(plane.origin),
        arr(plane.x),
        arr(plane.y),
        (mid.x, mid.y),
        lay_surface(kind)?,
        arr(near),
    )?;
    Some(vec(f.at))
}

/// Normal to face on a plane, cylinder or sphere: the sketch's frame laid on the face at the
/// point under the middle of the profiles `mid` (of two, the one nearer the pick `near`).
///
/// # Errors
/// [`EmbossRefusal::MiddleOffFace`] where the line along the sketch's normal misses the face's
/// surface (and for a face of another kind); [`EmbossRefusal::FacesAwayFromWall`] or
/// [`EmbossRefusal::FacesAwayFromSphere`] where the sketch faces the other side of an outside
/// wall or sphere.
pub fn lay(kind: &FaceKind, plane: &Plane, mid: Vec2, near: Vec3) -> Result<Laid, EmbossRefusal> {
    let surface = lay_surface(kind).ok_or(EmbossRefusal::MiddleOffFace)?;
    let f = lay_frame(
        arr(plane.origin),
        arr(plane.x),
        arr(plane.y),
        (mid.x, mid.y),
        surface,
        arr(near),
    )
    .ok_or(EmbossRefusal::MiddleOffFace)?;
    match kind {
        FaceKind::Cylinder { convex: true, .. } if !f.facing => {
            return Err(EmbossRefusal::FacesAwayFromWall)
        }
        FaceKind::Sphere { convex: true, .. } if !f.facing => {
            return Err(EmbossRefusal::FacesAwayFromSphere)
        }
        _ => {}
    }
    Ok(Laid {
        plane: Plane {
            origin: vec(f.origin),
            x: vec(f.x),
            y: vec(f.y),
        },
        at: vec(f.at),
    })
}

/// The line along the sketch's normal through the middle of the profiles `mid`: its point there,
/// its unit direction, and the point of it nearest the pick `pick`, from which its nearest
/// meeting with the face is taken.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] for a sketch frame with no normal.
pub fn line_under(
    plane: &Plane,
    mid: Vec2,
    pick: Vec3,
) -> Result<(Vec3, Vec3, Vec3), EmbossRefusal> {
    let m = sketch_point(plane, mid);
    let n = sketch_normal(plane)?;
    Ok((m, n, vec(line_step(arr(m), arr(n), arr(pick)))))
}

/// Normal to face on a cone, torus or freeform face: the sketch's frame laid so the middle of the
/// profiles `mid` lands on `at`, square to the face's unit `normal` there.
///
/// # Errors
/// [`EmbossRefusal::FacesAwayFromFace`] where the sketch faces the face's other side;
/// [`EmbossRefusal::BadSketchFrame`] for a frame that cannot be laid.
pub fn lay_on_normal(
    plane: &Plane,
    mid: Vec2,
    at: Vec3,
    normal: Vec3,
) -> Result<Plane, EmbossRefusal> {
    let f = lay_frame_at(
        arr(plane.x),
        arr(plane.y),
        (mid.x, mid.y),
        arr(at),
        arr(normal),
    )
    .ok_or(EmbossRefusal::BadSketchFrame)?;
    if !f.facing {
        return Err(EmbossRefusal::FacesAwayFromFace);
    }
    Ok(Plane {
        origin: vec(f.origin),
        x: vec(f.x),
        y: vec(f.y),
    })
}

/// Points of the profiles' outlines and holes to check a face under, in `plane`: every
/// segment's ends and points between, about 400 in all.
#[must_use]
pub fn outline_samples(plane: &Plane, regions: &[Region2]) -> Vec<Vec3> {
    let loops: Vec<_> = regions
        .iter()
        .flat_map(|r| std::iter::once(&r.outer).chain(&r.holes))
        .map(loop2)
        .collect();
    let segs: Vec<_> = loops.iter().flat_map(|l| &l.segs).collect();
    let each = (400 / segs.len().max(1)).clamp(1, 4);
    let stride = segs.len() / 400 + 1;
    segs.iter()
        .step_by(stride)
        .flat_map(|s| k::segment_points(s, each))
        .map(|[u, v]| sketch_point(plane, Vec2::new(u, v)))
        .collect()
}

/// Normal to face on a cone, torus or freeform face: how much a length under the profiles
/// stretches, projected along the face's unit `normal` under their middle onto a face whose unit
/// normals under their outlines are `normals`.
///
/// # Errors
/// [`EmbossRefusal::CurvesTooMuch`] past [`SIZE_TOLERANCE`].
pub fn curved_stretch(normal: Vec3, normals: &[Vec3]) -> Result<f64, EmbossRefusal> {
    let normals: Vec<[f64; 3]> = normals.iter().copied().map(arr).collect();
    let stretch = projected_stretch(arr(normal), &normals);
    if stretch > SIZE_TOLERANCE {
        return Err(EmbossRefusal::CurvesTooMuch);
    }
    Ok(stretch)
}

/// Normal to face, stopped, on a cone, torus or freeform face: the two half spaces (a point on
/// each plane and the side kept) that hold the face under the profiles, either side of the
/// tangent plane at `anchor` (unit `normal`) by the face's sag under them and the slab's reach.
#[must_use]
pub fn curved_sides(
    anchor: Vec3,
    normal: Vec3,
    regions: &[Region2],
    mid: Vec2,
    (stretch, depth, tol): (f64, f64, f64),
) -> [(Vec3, Vec3); 2] {
    let band = k::sag_band(
        reach(regions, mid),
        stretch,
        k::slab_reach(depth, false),
        tol,
    );
    k::band_sides(arr(anchor), arr(normal), band).map(|(p, side)| (vec(p), vec(side)))
}

/// How far the slab an emboss of `depth` keeps over a face reaches past it, below and above
/// ([`k::slab_reach`]).
#[must_use]
pub fn slab_reach(depth: f64, exact: bool) -> (f64, f64) {
    k::slab_reach(depth, exact)
}

/// How thick a slab reaching `reach` below and above a face is.
#[must_use]
pub fn slab_thickness(reach: (f64, f64)) -> f64 {
    k::padded(reach, 0.0).1
}

/// The inner and outer radii of a cylinder's or sphere's offsets `reach`
/// ([`k::offset_radii`]).
#[must_use]
pub fn offset_radii(radius: f64, reach: (f64, f64), convex: bool) -> (f64, f64) {
    k::offset_radii(radius, reach, convex)
}

/// What Normal to face on a sphere builds from: the shell between the offsets, and each
/// profile's cone from the centre over the base.
#[derive(Clone, Debug, PartialEq)]
pub struct SphereCones {
    /// The plane the cones' bases lie in, past the outer offset.
    pub base: Plane,
    /// The profiles scaled onto [`SphereCones::base`].
    pub profiles: Vec<Region2>,
    /// The shell's inner and outer radii.
    pub radii: (f64, f64),
    /// The slab's reach below and above the face.
    pub slab: (f64, f64),
}

/// Normal to face on a sphere about `centre` of `radius` (`convex` outside), the frame `laid` on
/// it under the middle of the profiles `mid`.
///
/// # Errors
/// [`EmbossRefusal::DepthReachesCentre`] for a depth reaching the centre (within `tol`);
/// [`EmbossRefusal::ShrinksOnSphere`] for profiles reaching so far that a length shrinks by more
/// than [`SIZE_TOLERANCE`]; [`EmbossRefusal::BadSketchFrame`] where the base cannot be placed.
#[allow(clippy::too_many_arguments)]
pub fn sphere_cones(
    (centre, radius, convex): (Vec3, f64, bool),
    laid: &Plane,
    regions: &[Region2],
    mid: Vec2,
    depth: f64,
    tol: f64,
) -> Result<SphereCones, EmbossRefusal> {
    let slab = k::slab_reach(depth, false);
    let radii = k::offset_radii(radius, slab, convex);
    if radii.0 <= tol {
        return Err(EmbossRefusal::DepthReachesCentre);
    }
    // Sizes are measured on the face; the cap and floor scale with their radius, as a wrap's do
    // on a cylinder.
    let far = reach(regions, mid);
    if central_shrink(far, radius) > SIZE_TOLERANCE {
        return Err(EmbossRefusal::ShrinksOnSphere {
            most: central_reach(radius, SIZE_TOLERANCE),
            reach: far,
        });
    }
    let (origin, scale) = k::cone_base(arr(centre), radius, slab, arr(laid.origin))
        .ok_or(EmbossRefusal::BadSketchFrame)?;
    let scaled = |l| record_loop2(&k::scaled_loop(&loop2(l), scale));
    Ok(SphereCones {
        base: Plane {
            origin: vec(origin),
            x: laid.x,
            y: laid.y,
        },
        profiles: regions
            .iter()
            .map(|r| Region2 {
                outer: scaled(&r.outer),
                holes: r.holes.iter().map(scaled).collect(),
            })
            .collect(),
        radii,
        slab,
    })
}

/// Whether a profile's piece over the face (`kept`, a volume) is the whole of it on the face's
/// surface (`full`), to the volumes' rounding.
#[must_use]
pub fn kept_whole(kept: f64, full: f64) -> bool {
    k::same_volume(kept, full)
}

/// A box past a body bounded by `min` and `max` on the side of the plane through `o` that `side`
/// points to: its base square (the frame and the square's half width) and its height, the same.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] for a `side` of no length.
pub fn half_space(
    min: Vec3,
    max: Vec3,
    o: Vec3,
    side: Vec3,
) -> Result<(Plane, f64), EmbossRefusal> {
    let reach = k::past_reach(arr(min), arr(max), arr(o));
    let (x, y) = k::square_axes(arr(side)).ok_or(EmbossRefusal::BadSketchFrame)?;
    Ok((
        Plane {
            origin: o,
            x: vec(x),
            y: vec(y),
        },
        reach,
    ))
}

/// Normal to sketch, stopped, on a cylinder wall about `axis` through `o`: the half of the wall
/// the pick `pick` is on, which a line along the sketch's normal crosses once.
///
/// # Errors
/// [`EmbossRefusal::NormalAlongAxis`] where the sketch's normal runs along the axis.
pub fn wall_half(o: Vec3, axis: Vec3, normal: Vec3, pick: Vec3) -> Result<Vec3, EmbossRefusal> {
    crossing_side(arr(axis), arr(normal), k::from_point(arr(o), arr(pick)))
        .map(vec)
        .ok_or(EmbossRefusal::NormalAlongAxis)
}

/// Normal to sketch, stopped, on a sphere about `centre`: the half of it the pick `pick` is on.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] for a normal of no length.
pub fn sphere_half(centre: Vec3, normal: Vec3, pick: Vec3) -> Result<Vec3, EmbossRefusal> {
    toward(arr(normal), k::from_point(arr(centre), arr(pick)))
        .map(vec)
        .ok_or(EmbossRefusal::BadSketchFrame)
}

/// Where `plane` touches the wall (`o`, unit `axis`, `radius`; `convex` for an outside wall,
/// which needs the plane's normal pointing away from the axis), within `tol`: the frame of the
/// line of contact. `None` where it does not touch it.
#[must_use]
pub fn touches(
    plane: &Plane,
    (o, axis, radius, convex): (Vec3, Vec3, f64, bool),
    tol: f64,
) -> Option<WallFrame> {
    let normal = k::frame_normal(arr(plane.x), arr(plane.y))?;
    wall_frame(
        normal,
        k::from_point(arr(o), arr(plane.origin)),
        arr(axis),
        radius,
        convex,
        tol,
    )
}

/// A round beside a plane face, tangent to it, that an emboss can carry on onto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentWall {
    /// A point of its axis.
    pub origin: Vec3,
    /// Its axis's unit direction.
    pub axis: Vec3,
    /// Its radius.
    pub radius: f64,
    /// Where the plane touches it ([`touches`]).
    pub frame: WallFrame,
    /// The angles it spans from the line of contact, towards the frame's tangent.
    pub angles: (f64, f64),
    /// Its heights along its axis, from `origin`.
    pub heights: (f64, f64),
}

/// Normal to face on a plane, with the frame `laid` on it: which of the outside `walls` tangent
/// to the plane the profiles run onto past their lines of contact (by more than `tol`). `None`
/// when they run onto none, so they are projected on the plane.
///
/// # Errors
/// [`EmbossRefusal::OntoSeveralWalls`] when they run onto more than one.
pub fn onto_wall(
    laid: &Plane,
    regions: &[Region2],
    walls: &[TangentWall],
    tol: f64,
) -> Result<Option<usize>, EmbossRefusal> {
    let points: Vec<[f64; 3]> = regions
        .iter()
        .flat_map(|r| std::iter::once(&r.outer).chain(&r.holes))
        .flat_map(|l| k::loop_points(&loop2(l), 256))
        .map(|[u, v]| arr(sketch_point(laid, Vec2::new(u, v))))
        .collect();
    let onto: Vec<usize> = walls
        .iter()
        .enumerate()
        .filter(|(_, w)| {
            let Some(side) = k::wall_side(w.angles, w.radius, tol) else {
                return false;
            };
            let contact = k::wall_point(
                arr(w.origin),
                arr(w.axis),
                w.radius,
                (w.frame.radial, w.frame.tangent),
                0.0,
                0.0,
            );
            k::past_contact(&points, contact, w.frame.tangent, side, tol)
        })
        .map(|(i, _)| i)
        .collect();
    match onto.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        more => Err(EmbossRefusal::OntoSeveralWalls { count: more.len() }),
    }
}

/// Whether `wall` starts at its line of contact with the plane (within `tol`), so it is the round
/// beside the plane rather than a wall the plane only touches.
#[must_use]
pub fn beside(wall: &TangentWall, tol: f64) -> bool {
    k::starts_at_contact(wall.angles, wall.radius, tol)
}

/// The wrap of a profile running off a plane onto the tangent `wall`: the plane touches the wall
/// at its edge, where the wrap does not start, so the frame `laid` is rolled round the wall to
/// the middle of its face. Returns a pick on the wall there and the rolled frame, which lays
/// every profile point where the plane does.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] where the frame cannot be rolled.
pub fn rolled_onto(laid: &Plane, wall: &TangentWall) -> Result<(Vec3, Plane), EmbossRefusal> {
    let turn = k::middle_of(wall.angles);
    let (origin, x, y) = roll_frame(
        arr(laid.origin),
        arr(laid.x),
        arr(laid.y),
        arr(wall.origin),
        arr(wall.axis),
        wall.radius,
        turn,
    )
    .ok_or(EmbossRefusal::BadSketchFrame)?;
    let on = k::wall_point(
        arr(wall.origin),
        arr(wall.axis),
        wall.radius,
        (wall.frame.radial, wall.frame.tangent),
        turn,
        k::middle_of(wall.heights),
    );
    Ok((
        vec(on),
        Plane {
            origin: vec(origin),
            x: vec(x),
            y: vec(y),
        },
    ))
}

/// The whole wall a stopped wrap is built on before it is trimmed: the frame of its base disc
/// (centred on the axis, its x towards the line `frame` of contact, so the wall's seam lies
/// opposite where the wrap starts) and its length along the axis, reaching past the profiles'
/// heights by the depth and 1 more.
///
/// # Errors
/// [`EmbossRefusal::DepthOrProfiles`] for profiles with no points.
pub fn whole_wall(
    plane: &Plane,
    regions: &[Region2],
    (o, axis, convex): (Vec3, Vec3, bool),
    frame: &WallFrame,
    depth: f64,
) -> Result<(Plane, f64), EmbossRefusal> {
    let points: Vec<[f64; 3]> = outline_points(regions)
        .into_iter()
        .map(|[u, v]| arr(sketch_point(plane, Vec2::new(u, v))))
        .collect();
    let span = k::heights(&points, arr(o), arr(axis)).ok_or(EmbossRefusal::DepthOrProfiles)?;
    let (z_lo, length) = k::padded(span, k::depth_margin(depth));
    let y = if convex {
        frame.tangent
    } else {
        k::reversed(frame.tangent)
    };
    Ok((
        Plane {
            origin: vec(k::along(arr(o), arr(axis), z_lo)),
            x: vec(frame.radial),
            y: vec(y),
        },
        length,
    ))
}

/// The radius a bore's whole wall is built out to for a stopped wrap: past the depth by the
/// depth and 1 more.
#[must_use]
pub fn bore_reach(radius: f64, depth: f64) -> f64 {
    k::bore_reach(radius, depth)
}

/// A whole-turn wall face (`angles` round its axis) spanning `heights` along it from `o`: the
/// base and length of the ring between its offsets. `None` where it does not go the whole way
/// round.
#[must_use]
pub fn whole_turn_ring(
    o: Vec3,
    axis: Vec3,
    angles: (f64, f64),
    heights: (f64, f64),
) -> Option<(Vec3, f64)> {
    if !k::whole_turn(angles) {
        return None;
    }
    let (z_lo, length) = k::padded(heights, 0.0);
    Some((vec(k::along(arr(o), arr(axis), z_lo)), length))
}

/// The heights along `plane`'s normal, from its origin, that prisms of an emboss `depth` deep run
/// between to pass right through a body bounded by `min` and `max`: its corners' lowest and
/// highest, each past by the depth and 1 more.
///
/// # Errors
/// [`EmbossRefusal::BadSketchFrame`] for a sketch frame with no normal.
pub fn through_body(
    min: Vec3,
    max: Vec3,
    plane: &Plane,
    depth: f64,
) -> Result<(f64, f64), EmbossRefusal> {
    let normal = sketch_normal(plane)?;
    let corners: Vec<[f64; 3]> = [min.x, max.x]
        .into_iter()
        .flat_map(|x| {
            [min.y, max.y]
                .into_iter()
                .flat_map(move |y| [min.z, max.z].map(|z| [x, y, z]))
        })
        .collect();
    let span = k::heights(&corners, arr(plane.origin), arr(normal))
        .ok_or(EmbossRefusal::BadSketchFrame)?;
    Ok(k::widened(span, k::depth_margin(depth)))
}

/// Two unit axes square to each other and to `axis`, to measure angles round it by.
#[must_use]
pub fn axes_about(axis: Vec3) -> Option<(Vec3, Vec3)> {
    k::square_axes(arr(axis)).map(|(x, y)| (vec(x), vec(y)))
}

/// The distance between two points.
#[must_use]
pub fn distance(a: Vec3, b: Vec3) -> f64 {
    k::distance(arr(a), arr(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcore_record::Loop2;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region2 {
        Region2 {
            outer: Loop2::polygon(&[
                Vec2::new(x0, y0),
                Vec2::new(x1, y0),
                Vec2::new(x1, y1),
                Vec2::new(x0, y1),
            ]),
            holes: vec![],
        }
    }

    const POST: FaceKind = FaceKind::Cylinder {
        origin: Vec3::ZERO,
        axis: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        radius: 10.0,
        convex: true,
    };

    #[test]
    fn the_route_is_decided_by_the_face_kind_the_direction_and_the_stop() {
        let plane = FaceKind::Plane {
            normal: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            offset: 0.0,
        };
        let ball = FaceKind::Sphere {
            centre: Vec3::ZERO,
            radius: 1.0,
            convex: true,
        };
        assert_eq!(route(Direction::Sketch, false, &POST), Route::AsBefore);
        assert_eq!(
            route(Direction::Sketch, true, &POST),
            Route::SketchToWallEdge
        );
        assert_eq!(
            route(Direction::Sketch, true, &ball),
            Route::SketchToSphereEdge
        );
        assert_eq!(
            route(Direction::Sketch, true, &plane),
            Route::SketchToFaceEdge
        );
        assert_eq!(route(Direction::Face, true, &plane), Route::FaceOnPlane);
        assert_eq!(route(Direction::Face, false, &POST), Route::FaceOnWall);
        assert_eq!(route(Direction::Face, false, &ball), Route::FaceOnSphere);
        assert_eq!(
            route(Direction::Face, false, &FaceKind::Other),
            Route::FaceOnCurved
        );
    }

    #[test]
    fn a_sketch_facing_into_an_outside_wall_is_refused_by_name() {
        // The plane y = 30 seen from -y: its normal points at the axis.
        let inward = Plane {
            origin: Vec3 {
                x: 0.0,
                y: 30.0,
                z: 0.0,
            },
            x: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            y: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        let near = Vec3 {
            x: 0.0,
            y: 10.0,
            z: 5.0,
        };
        assert_eq!(
            lay(&POST, &inward, Vec2::new(0.0, 5.0), near),
            Err(EmbossRefusal::FacesAwayFromWall)
        );
        let outward = Plane {
            x: Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            ..inward
        };
        let laid = lay(&POST, &outward, Vec2::new(0.0, 5.0), near).unwrap();
        assert!(distance(laid.at, near) < 1e-12);
        assert_eq!(
            lay(&FaceKind::Other, &outward, Vec2::new(0.0, 5.0), near),
            Err(EmbossRefusal::MiddleOffFace)
        );
    }

    #[test]
    fn a_profile_running_past_the_line_of_contact_runs_onto_that_wall_only() {
        // The plane y = 10 seen from +y touches the post along x = 0; a half wall runs from there
        // round towards -x.
        let laid = Plane {
            origin: Vec3 {
                x: 0.0,
                y: 10.0,
                z: 0.0,
            },
            x: Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            y: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        let frame = touches(
            &laid,
            (
                Vec3::ZERO,
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                10.0,
                true,
            ),
            1e-8,
        )
        .unwrap();
        let wall = TangentWall {
            origin: Vec3::ZERO,
            axis: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            radius: 10.0,
            frame,
            angles: (0.0, std::f64::consts::PI),
            heights: (0.0, 20.0),
        };
        assert!(beside(&wall, 1e-8));
        // From x = 5 to -4: past the line by 4.
        assert_eq!(
            onto_wall(&laid, &[rect(-5.0, 1.0, 4.0, 2.0)], &[wall], 1e-8),
            Ok(Some(0))
        );
        assert_eq!(
            onto_wall(&laid, &[rect(-5.0, 1.0, -1.0, 2.0)], &[wall], 1e-8),
            Ok(None)
        );
        assert_eq!(
            onto_wall(&laid, &[rect(-5.0, 1.0, 4.0, 2.0)], &[wall, wall], 1e-8),
            Err(EmbossRefusal::OntoSeveralWalls { count: 2 })
        );
        let (on, rolled) = rolled_onto(&laid, &wall).unwrap();
        assert!(
            distance(
                on,
                Vec3 {
                    x: -10.0,
                    y: 0.0,
                    z: 10.0
                }
            ) < 1e-9,
            "{on:?}"
        );
        // Rolled a quarter turn, the frame touches the wall along x = -10, its x along -y; its
        // origin lay on the first line of contact, a quarter turn of arc (5π) back along -x.
        assert!(
            distance(
                rolled.origin,
                Vec3 {
                    x: -10.0,
                    y: 5.0 * std::f64::consts::PI,
                    z: 0.0
                }
            ) < 1e-9,
            "{rolled:?}"
        );
    }

    #[test]
    fn a_sphere_refuses_profiles_reaching_too_far_and_depths_reaching_its_centre() {
        let laid = Plane {
            origin: Vec3 {
                x: 0.0,
                y: 10.0,
                z: 0.0,
            },
            x: Vec3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            y: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        let ball = (Vec3::ZERO, 10.0, true);
        let small = [rect(-2.0, -1.0, 2.0, 1.0)];
        let cones = sphere_cones(ball, &laid, &small, Vec2::new(0.0, 0.0), 1.0, 1e-7).unwrap();
        assert_eq!(cones.radii, (9.8, 11.0));
        assert!((cones.base.origin.y - 12.0).abs() < 1e-12);
        let big = [rect(-5.0, -5.0, 5.0, 5.0)];
        match sphere_cones(ball, &laid, &big, Vec2::new(0.0, 0.0), 1.0, 1e-7) {
            Err(e @ EmbossRefusal::ShrinksOnSphere { .. }) => assert!(
                e.to_string().contains("within 10% only within 3.33 mm"),
                "{e}"
            ),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            sphere_cones(ball, &laid, &small, Vec2::new(0.0, 0.0), -10.5, 1e-7),
            Err(EmbossRefusal::DepthReachesCentre)
        );
    }

    #[test]
    fn a_whole_turn_wall_takes_a_ring_and_a_part_wall_does_not() {
        let z = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        let tau = std::f64::consts::TAU;
        assert_eq!(
            whole_turn_ring(Vec3::ZERO, z, (0.0, tau), (2.0, 5.0)),
            Some((
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 2.0
                },
                3.0
            ))
        );
        assert_eq!(whole_turn_ring(Vec3::ZERO, z, (0.0, 3.0), (2.0, 5.0)), None);
        assert_eq!(checked(0.0, 1), Err(EmbossRefusal::DepthOrProfiles));
        assert_eq!(checked(1.0, 0), Err(EmbossRefusal::DepthOrProfiles));
        assert_eq!(checked(-1.0, 4096), Ok(()));
    }
}
