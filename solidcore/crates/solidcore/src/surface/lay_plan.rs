//! What laying profiles on a face takes besides the laid frame ([`super::lay_frame`]): the way
//! the face's form, the direction and the stop decide, decided before anything is built; the
//! middle and reach of the profiles; the slab a face's material is kept in; the base of the cones
//! over the profiles on a sphere; the contact where a plane runs on onto a round tangent to it;
//! and the band a curved face sags within under the profiles.

use super::lay_frame::{add, dot, roll_frame, scale, sub};
use crate::math::Vec2;
use crate::prism::profile::{Loop2, Region2, Seg2};

/// Which way profiles are raised off their face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayDirection {
    /// Along the sketch's normal, the profiles as projected.
    Sketch,
    /// Along the face's normal, the profiles keeping their size on it.
    Face,
}

/// How a sketch plane meets a cylindrical wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallTouch {
    /// It touches the wall on the side the wall's normal points to (either side of a bore).
    Along,
    /// It touches an outside wall facing into it.
    Against,
    /// It does not touch the wall.
    Off,
}

/// The target face's form, as far as [`lay_path`] needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceForm {
    /// A plane, and how many rounds tangent to it along its edges the laid profiles run onto.
    Plane {
        /// Those rounds.
        walls: usize,
    },
    /// A cylinder, and how the sketch plane meets it.
    Cylinder {
        /// That meeting.
        touch: WallTouch,
    },
    /// A sphere.
    Sphere,
    /// A cone, torus or freeform face.
    Other,
}

/// How profiles go on a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayPath {
    /// Normal to sketch without the stop: wrapped where the sketch touches a cylinder wall the
    /// profiles lie on, else projected.
    Sketched,
    /// Projected onto the face along the laid frame's normal.
    Project,
    /// Wrapped from the laid frame rolled onto the one round the profiles run onto.
    RollOntoWall,
    /// Wrapped round the cylinder from the laid frame.
    Wrap,
    /// Wrapped round a whole wall of the cylinder's radius and kept over the face's wall.
    WrapToEdge,
    /// The profiles' prisms through the face, kept over it.
    PrismsToEdge,
    /// The cones over the profiles from the sphere's centre, kept between its offsets.
    SphereCones,
    /// Projected along the face's normal under the profiles' middle.
    NearFace,
}

/// Why profiles cannot go on a face, known from its form before anything is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayRefusal {
    /// The profiles run off a plane onto more than one round tangent to it.
    SeveralWalls,
    /// The sketch touches an outside wall facing into it.
    FacingIntoWall,
}

impl LayRefusal {
    /// What the person is told.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            LayRefusal::SeveralWalls => {
                "the profiles run off the target face onto more than one round: pick the round, or turn on Stop at face edge"
            }
            LayRefusal::FacingIntoWall => {
                "sketch normal must point outwards from the exterior wall"
            }
        }
    }
}

/// How profiles go on a face of `form`, raised `direction`, trimmed at its edge when `stop`.
///
/// # Errors
/// [`LayRefusal::SeveralWalls`] for profiles laid on a plane that run onto more than one round,
/// and [`LayRefusal::FacingIntoWall`] for a sketch facing into an outside wall it touches,
/// stopped at the edge.
pub fn lay_path(
    direction: LayDirection,
    form: FaceForm,
    stop: bool,
) -> Result<LayPath, LayRefusal> {
    Ok(match (direction, form, stop) {
        (LayDirection::Sketch, _, false) => LayPath::Sketched,
        (
            LayDirection::Sketch,
            FaceForm::Cylinder {
                touch: WallTouch::Along,
            },
            true,
        ) => LayPath::WrapToEdge,
        (
            LayDirection::Sketch,
            FaceForm::Cylinder {
                touch: WallTouch::Against,
            },
            true,
        ) => return Err(LayRefusal::FacingIntoWall),
        (LayDirection::Sketch, _, true) => LayPath::PrismsToEdge,
        (LayDirection::Face, FaceForm::Plane { .. }, true) => LayPath::PrismsToEdge,
        (LayDirection::Face, FaceForm::Plane { walls: 0 }, false) => LayPath::Project,
        (LayDirection::Face, FaceForm::Plane { walls: 1 }, false) => LayPath::RollOntoWall,
        (LayDirection::Face, FaceForm::Plane { .. }, false) => {
            return Err(LayRefusal::SeveralWalls)
        }
        (LayDirection::Face, FaceForm::Cylinder { .. }, false) => LayPath::Wrap,
        (LayDirection::Face, FaceForm::Cylinder { .. }, true) => LayPath::WrapToEdge,
        (LayDirection::Face, FaceForm::Sphere, _) => LayPath::SphereCones,
        (LayDirection::Face, FaceForm::Other, _) => LayPath::NearFace,
    })
}

/// Points along a free-form segment for its bounds and reach.
const FREE_FORM_POINTS: usize = 64;

/// Points of `seg` that bound it: a line's ends, an arc's ends and the points where it runs
/// square to the axes, and points along any other segment.
fn extreme_points(seg: &Seg2) -> Vec<Vec2> {
    match *seg {
        Seg2::Line { a, b } => vec![a, b],
        Seg2::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let mut out = vec![seg.point_at(0.0), seg.point_at(1.0)];
            let (lo, hi) = (start.min(start + sweep), start.max(start + sweep));
            let quarter = std::f64::consts::FRAC_PI_2;
            let mut k = (lo / quarter).ceil();
            // An arc turns at most once round: five quarter turns at most.
            for _ in 0..5 {
                let t = k * quarter;
                if t > hi {
                    break;
                }
                out.push(center + Vec2::new(t.cos(), t.sin()) * radius);
                k += 1.0;
            }
            out
        }
        _ => along(seg),
    }
}

fn along(seg: &Seg2) -> Vec<Vec2> {
    (0..=FREE_FORM_POINTS)
        .map(|i| seg.point_at(i as f64 / FREE_FORM_POINTS as f64))
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect()
}

/// The bounds (lowest and highest corner) of the outer loops of `regions`: exact for lines and
/// arcs, from points along any other segment. `None` without any point.
#[must_use]
pub fn profile_bounds(regions: &[Region2]) -> Option<(Vec2, Vec2)> {
    let mut points = regions
        .iter()
        .flat_map(|r| &r.outer.segs)
        .flat_map(extreme_points);
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            Vec2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Vec2::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// The middle of [`profile_bounds`].
#[must_use]
pub fn profile_middle(regions: &[Region2]) -> Option<Vec2> {
    profile_bounds(regions).map(|(lo, hi)| (lo + hi) * 0.5)
}

/// How far the outer loops of `regions` reach from `mid`: exact for lines and arcs, from points
/// along any other segment.
#[must_use]
pub fn profile_reach(regions: &[Region2], mid: Vec2) -> f64 {
    regions
        .iter()
        .flat_map(|r| &r.outer.segs)
        .map(|seg| farthest(seg, mid))
        .fold(0.0, f64::max)
}

/// The farthest any point of `seg` lies from `p`.
fn farthest(seg: &Seg2, p: Vec2) -> f64 {
    match *seg {
        Seg2::Line { a, b } => a.distance(p).max(b.distance(p)),
        Seg2::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let ends = seg.point_at(0.0).distance(p).max(seg.point_at(1.0).distance(p));
            let away = center - p;
            if away.length() <= 0.0 {
                return radius;
            }
            // The circle's farthest point from `p` is straight on through the centre.
            let t = away.y.atan2(away.x);
            let turn = std::f64::consts::TAU;
            let into = if sweep >= 0.0 {
                (t - start).rem_euclid(turn)
            } else {
                (start - t).rem_euclid(turn)
            };
            if into <= sweep.abs() {
                ends.max(away.length() + radius)
            } else {
                ends
            }
        }
        _ => along(seg)
            .into_iter()
            .map(|q| q.distance(p))
            .fold(0.0, f64::max),
    }
}

/// About how many points [`outline_samples`] takes.
const OUTLINE_SAMPLES: usize = 400;

/// Points of the outlines and holes of `regions` to check a face under: every segment's start
/// and points along it, about [`OUTLINE_SAMPLES`] in all.
#[must_use]
pub fn outline_samples(regions: &[Region2]) -> Vec<Vec2> {
    let segs: Vec<&Seg2> = regions
        .iter()
        .flat_map(Region2::loops)
        .flat_map(|l| &l.segs)
        .collect();
    let each = OUTLINE_SAMPLES
        .checked_div(segs.len())
        .unwrap_or(0)
        .max(1)
        .min(4);
    let stride = segs.len() / OUTLINE_SAMPLES + 1;
    segs.iter()
        .step_by(stride)
        .flat_map(|s| (0..each).map(move |k| s.point_at(k as f64 / each as f64)))
        .collect()
}

/// `lp` scaled by `k` about its frame's origin.
#[must_use]
pub fn scaled_loop(lp: &Loop2, k: f64) -> Loop2 {
    let segs = lp
        .segs
        .iter()
        .map(|s| match *s {
            Seg2::Line { a, b } => Seg2::Line { a: a * k, b: b * k },
            Seg2::Arc {
                center,
                radius,
                start,
                sweep,
            } => Seg2::Arc {
                center: center * k,
                radius: radius * k,
                start,
                sweep,
            },
            Seg2::Cubic { p0, p1, p2, p3 } => Seg2::Cubic {
                p0: p0 * k,
                p1: p1 * k,
                p2: p2 * k,
                p3: p3 * k,
            },
            Seg2::Conic { a, apex, b, w } => Seg2::Conic {
                a: a * k,
                apex: apex * k,
                b: b * k,
                w,
            },
            Seg2::Bezier { n, p } => Seg2::Bezier {
                n,
                p: p.map(|q| q * k),
            },
        })
        .collect();
    Loop2 { segs }
}

/// How far the slab over a face reaches past it, below and above along its outward normal, for
/// `depth` (raised when positive): a little past the face on the side away from the depth, and
/// exactly to the depth, which then bounds prisms run through the body. A tool already `depth`
/// deep (`exact`) takes the margin on both sides.
#[must_use]
pub fn slab_reach(depth: f64, exact: bool) -> (f64, f64) {
    let d = depth.abs();
    let overlap = (d * 0.2).max(0.1).min(2.0);
    let past = if exact { overlap } else { 0.0 };
    if depth > 0.0 {
        (-overlap, d + past)
    } else {
        (-d - past, overlap)
    }
}

/// The radii of the offsets `lo` and `hi` (along the outward normal) of a sphere or cylinder of
/// `radius`, inner first: `convex` where the normal points away from the centre or axis.
#[must_use]
pub fn slab_radii(radius: f64, convex: bool, (lo, hi): (f64, f64)) -> (f64, f64) {
    if convex {
        (radius + lo, radius + hi)
    } else {
        (radius - hi, radius - lo)
    }
}

/// The base of the cones over profiles laid on a sphere (centre `centre`, `radius`) in the
/// tangent plane through `laid_origin`, so the cones reach past the offset `lo` or `hi` farther
/// from the face: that plane scaled about the centre by the returned factor, and the origin it
/// takes. `None` for a radius that is not positive or input that is not finite.
#[must_use]
pub fn sphere_cone_base(
    centre: [f64; 3],
    radius: f64,
    laid_origin: [f64; 3],
    (lo, hi): (f64, f64),
) -> Option<([f64; 3], f64)> {
    if !(radius.is_finite() && radius > 0.0) {
        return None;
    }
    let k = (radius + lo.abs().max(hi.abs()) + 1.0) / radius;
    let origin = add(centre, scale(sub(laid_origin, centre), k));
    (k.is_finite() && origin.iter().all(|c| c.is_finite())).then_some((origin, k))
}

/// The two half-spaces (a point of each bounding plane and the direction into it) that hold the
/// part of a face under profiles reaching `reach` from `anchor`, where the face's unit normal is
/// `normal` and a projection along it stretches lengths by at most `stretch`, with the slab's
/// reach `lo` and `hi` and a margin of a thousand times `tol`: the face sags off its tangent plane
/// by at most the reach times the tangent of the largest lean, either way.
#[must_use]
pub fn sag_band(
    anchor: [f64; 3],
    normal: [f64; 3],
    reach: f64,
    stretch: f64,
    (lo, hi): (f64, f64),
    tol: f64,
) -> [([f64; 3], [f64; 3]); 2] {
    let lean = ((1.0 + stretch).powi(2) - 1.0).max(0.0).sqrt();
    let band = reach * lean + lo.abs().max(hi.abs()) + tol * 1e3;
    [
        (sub(anchor, scale(normal, band)), normal),
        (add(anchor, scale(normal, band)), scale(normal, -1.0)),
    ]
}

/// Of the points `hits` (some missing), where lines along the unit `normal` through `p` meet
/// faces, the one nearest `p`; of two as near, the one behind `p` (against `normal`). `None`
/// without any.
#[must_use]
pub fn nearest_along(p: [f64; 3], normal: [f64; 3], hits: &[Option<[f64; 3]>]) -> Option<usize> {
    hits.iter()
        .enumerate()
        .filter_map(|(i, h)| h.map(|h| (i, dot(sub(p, h), normal))))
        .min_by(|(_, s), (_, t)| s.abs().total_cmp(&t.abs()).then(t.total_cmp(s)))
        .map(|(i, _)| i)
}

/// The point of the axis through `origin` along unit `axis` at `z` along it.
#[must_use]
pub fn on_axis(origin: [f64; 3], axis: [f64; 3], z: f64) -> [f64; 3] {
    add(origin, scale(axis, z))
}

/// A cylindrical wall (`origin` on its axis, unit `axis`, `radius`), seen from a plane that
/// touches it: `radial` from the axis to the line of contact and `tangent` square to both, the
/// way a positive angle about the axis turns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchedWall {
    /// A point of the axis.
    pub origin: [f64; 3],
    /// The axis's unit direction.
    pub axis: [f64; 3],
    /// The radius.
    pub radius: f64,
    /// From the axis to the line of contact.
    pub radial: [f64; 3],
    /// Square to the axis and `radial`.
    pub tangent: [f64; 3],
    /// The angles the wall face spans about the axis from `radial`, towards `tangent`.
    pub angles: (f64, f64),
    /// How far along the axis the wall face runs.
    pub heights: (f64, f64),
}

/// Where a plane frame laid on a flat face runs on onto an outside wall touching it along one of
/// its straight edges: the frame rolled round the wall to the middle of the wall face, which lays
/// every frame point where the plane does, and a point of the wall face there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallContact {
    /// The rolled frame's origin.
    pub origin: [f64; 3],
    /// Its x.
    pub x: [f64; 3],
    /// Its y.
    pub y: [f64; 3],
    /// A point of the wall face at the middle of its angles and heights.
    pub on: [f64; 3],
}

/// Where the frame (`origin`, `x`, `y`), touching `wall` along one of its straight edges (one of
/// its angles within ten times `tol` of the contact), runs on onto it: `None` unless some point
/// of `points` (frame coordinates) lies more than `tol` past the line of contact on the wall's
/// side, or where the frame does not roll.
#[must_use]
pub fn wall_contact(
    (origin, x, y): ([f64; 3], [f64; 3], [f64; 3]),
    wall: &TouchedWall,
    points: &[Vec2],
    tol: f64,
) -> Option<WallContact> {
    let (lo, hi) = wall.angles;
    let seam = tol * 10.0 / wall.radius;
    if lo.abs() > seam && hi.abs() > seam {
        return None;
    }
    let turn = 0.5 * (lo + hi);
    let side = scale(wall.tangent, turn.signum());
    let contact = add(wall.origin, scale(wall.radial, wall.radius));
    let past = points.iter().any(|q| {
        let w = add(origin, add(scale(x, q.x), scale(y, q.y)));
        dot(sub(w, contact), side) > tol
    });
    if !past {
        return None;
    }
    let (ro, rx, ry) = roll_frame(origin, x, y, wall.origin, wall.axis, wall.radius, turn)?;
    let (z_lo, z_hi) = wall.heights;
    let round = add(
        scale(wall.radial, turn.cos()),
        scale(wall.tangent, turn.sin()),
    );
    let on = add(
        on_axis(wall.origin, wall.axis, 0.5 * (z_lo + z_hi)),
        scale(round, wall.radius),
    );
    on.iter().all(|c| c.is_finite()).then_some(WallContact {
        origin: ro,
        x: rx,
        y: ry,
        on,
    })
}

/// The whole wall a wrap stopped at the face's edge is built on: its base frame (origin on the
/// axis, `x` along the contact's radial and `y` the way the wrap runs), its length along the
/// axis, and the outer radius a bore's ring takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallSeat {
    /// The base's origin, on the axis.
    pub origin: [f64; 3],
    /// The base's x: the contact's radial, where the whole wall's seam goes opposite.
    pub x: [f64; 3],
    /// The base's y.
    pub y: [f64; 3],
    /// The wall's length along the axis.
    pub length: f64,
    /// A bore's outer radius, past the deepest emboss.
    pub outer: f64,
}

/// The whole wall of `wall`'s radius a wrap of profiles `depth` deep from the frame (`origin`,
/// `x`, `y`), touching it, is built on: the profiles' frame bounds `lo` and `hi` set its length,
/// with a margin of the depth and a unit each end. `convex` for an outside wall.
#[must_use]
pub fn wall_seat(
    (origin, x, y): ([f64; 3], [f64; 3], [f64; 3]),
    (lo, hi): (Vec2, Vec2),
    wall: &TouchedWall,
    convex: bool,
    depth: f64,
) -> Option<WallSeat> {
    let level = |u: f64, v: f64| {
        dot(
            sub(add(origin, add(scale(x, u), scale(y, v))), wall.origin),
            wall.axis,
        )
    };
    let corners = [
        level(lo.x, lo.y),
        level(hi.x, lo.y),
        level(lo.x, hi.y),
        level(hi.x, hi.y),
    ];
    let low = corners.iter().copied().fold(f64::INFINITY, f64::min);
    let high = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let margin = depth.abs() + 1.0;
    let seat = WallSeat {
        origin: on_axis(wall.origin, wall.axis, low - margin),
        x: wall.radial,
        y: if convex {
            wall.tangent
        } else {
            scale(wall.tangent, -1.0)
        },
        length: high - low + 2.0 * margin,
        outer: wall.radius + depth.abs() + margin,
    };
    (seat.length.is_finite() && seat.length > 0.0 && seat.origin.iter().all(|c| c.is_finite()))
        .then_some(seat)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        dot(sub(a, b), sub(a, b)).sqrt() < 1e-12
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region2 {
        let p = [
            Vec2::new(x0, y0),
            Vec2::new(x1, y0),
            Vec2::new(x1, y1),
            Vec2::new(x0, y1),
        ];
        let segs = (0..4)
            .map(|i| Seg2::Line {
                a: p[i],
                b: p[(i + 1) % 4],
            })
            .collect();
        Region2 {
            outer: Loop2 { segs },
            holes: vec![],
        }
    }

    #[test]
    fn the_path_is_chosen_from_the_form_the_direction_and_the_stop() {
        use LayDirection::{Face, Sketch};
        let plane = |walls| FaceForm::Plane { walls };
        let wall = |touch| FaceForm::Cylinder { touch };
        assert_eq!(lay_path(Sketch, FaceForm::Sphere, false), Ok(LayPath::Sketched));
        assert_eq!(lay_path(Sketch, wall(WallTouch::Along), true), Ok(LayPath::WrapToEdge));
        assert_eq!(
            lay_path(Sketch, wall(WallTouch::Against), true),
            Err(LayRefusal::FacingIntoWall)
        );
        assert_eq!(lay_path(Sketch, wall(WallTouch::Off), true), Ok(LayPath::PrismsToEdge));
        assert_eq!(lay_path(Face, plane(0), false), Ok(LayPath::Project));
        assert_eq!(lay_path(Face, plane(1), false), Ok(LayPath::RollOntoWall));
        assert_eq!(lay_path(Face, plane(2), false), Err(LayRefusal::SeveralWalls));
        assert_eq!(lay_path(Face, plane(2), true), Ok(LayPath::PrismsToEdge));
        assert_eq!(lay_path(Face, wall(WallTouch::Off), false), Ok(LayPath::Wrap));
        assert_eq!(lay_path(Face, wall(WallTouch::Off), true), Ok(LayPath::WrapToEdge));
        assert_eq!(lay_path(Face, FaceForm::Sphere, true), Ok(LayPath::SphereCones));
        assert_eq!(lay_path(Face, FaceForm::Other, false), Ok(LayPath::NearFace));
    }

    #[test]
    fn an_arcs_bounds_and_reach_are_exact() {
        // A half disc of radius 2 about (1, 0), above the x axis.
        let half = Region2 {
            outer: Loop2 {
                segs: vec![
                    Seg2::Arc {
                        center: Vec2::new(1.0, 0.0),
                        radius: 2.0,
                        start: 0.0,
                        sweep: std::f64::consts::PI,
                    },
                    Seg2::Line {
                        a: Vec2::new(-1.0, 0.0),
                        b: Vec2::new(3.0, 0.0),
                    },
                ],
            },
            holes: vec![],
        };
        let (lo, hi) = profile_bounds(std::slice::from_ref(&half)).unwrap();
        assert!((lo - Vec2::new(-1.0, 0.0)).length() < 1e-12, "{lo:?}");
        assert!((hi - Vec2::new(3.0, 2.0)).length() < 1e-12, "{hi:?}");
        let mid = profile_middle(std::slice::from_ref(&half)).unwrap();
        assert!((mid - Vec2::new(1.0, 1.0)).length() < 1e-12);
        // From (1, -1) the farthest point is the top of the arc, 3 away.
        let reach = profile_reach(std::slice::from_ref(&half), Vec2::new(1.0, -1.0));
        assert!((reach - 3.0).abs() < 1e-12, "{reach}");
        // From (1, 1) it is a corner, √5 away.
        let reach = profile_reach(std::slice::from_ref(&half), mid);
        assert!((reach - 5f64.sqrt()).abs() < 1e-12, "{reach}");
    }

    #[test]
    fn outline_samples_take_every_segment_and_scaling_keeps_the_shape() {
        let r = rect(0.0, 0.0, 2.0, 1.0);
        let samples = outline_samples(std::slice::from_ref(&r));
        assert_eq!(samples.len(), 16);
        assert!(samples.contains(&Vec2::new(2.0, 0.0)) && samples.contains(&Vec2::new(1.0, 1.0)));
        let big = scaled_loop(&r.outer, 3.0);
        assert_eq!(
            big.segs.first(),
            Some(&Seg2::Line {
                a: Vec2::new(0.0, 0.0),
                b: Vec2::new(6.0, 0.0)
            })
        );
    }

    #[test]
    fn the_slab_reaches_to_the_depth_and_its_radii_follow_the_side() {
        assert_eq!(slab_reach(1.0, false), (-0.2, 1.0));
        assert_eq!(slab_reach(-1.0, true), (-1.2, 0.2));
        assert_eq!(slab_reach(50.0, false), (-2.0, 50.0));
        assert_eq!(slab_radii(10.0, true, (-0.2, 1.0)), (9.8, 11.0));
        assert_eq!(slab_radii(10.0, false, (-0.2, 1.0)), (9.0, 10.2));
    }

    #[test]
    fn the_cone_base_lies_past_the_outer_offset_on_the_scaled_tangent_plane() {
        let (origin, k) = sphere_cone_base([0.0; 3], 10.0, [3.0, 10.0, 0.0], (-0.2, 1.0)).unwrap();
        assert!((k - 1.2).abs() < 1e-12);
        assert!(close(origin, [3.6, 12.0, 0.0]));
        assert!(sphere_cone_base([0.0; 3], 0.0, [3.0, 10.0, 0.0], (-0.2, 1.0)).is_none());
    }

    #[test]
    fn the_band_holds_the_sag_and_the_nearest_hit_wins_ties_behind() {
        let z = [0.0, 0.0, 1.0];
        // A lean of 45 degrees (stretch √2 - 1) over a reach of 2 sags 2 either way.
        let [(below, up), (above, down)] =
            sag_band([0.0; 3], z, 2.0, std::f64::consts::SQRT_2 - 1.0, (-0.5, 1.0), 0.0);
        assert!(close(below, [0.0, 0.0, -3.0]) && close(up, z));
        assert!(close(above, [0.0, 0.0, 3.0]) && close(down, [0.0, 0.0, -1.0]));
        let p = [0.0, 0.0, 5.0];
        let hits = [Some([0.0, 0.0, 9.0]), None, Some([0.0, 0.0, 1.0]), Some([0.0, 0.0, 2.0])];
        assert_eq!(nearest_along(p, z, &hits), Some(3));
        // 4 ahead and 4 behind: behind, against the normal.
        assert_eq!(nearest_along(p, z, &hits[..3]), Some(2));
        assert_eq!(nearest_along(p, z, &[None]), None);
    }

    fn post_wall(angles: (f64, f64)) -> TouchedWall {
        // Radius 10 about z, touched by the plane y = 10 seen from +y.
        TouchedWall {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
            radial: [0.0, 1.0, 0.0],
            tangent: [-1.0, 0.0, 0.0],
            angles,
            heights: (0.0, 20.0),
        }
    }

    #[test]
    fn a_frame_runs_on_onto_a_wall_only_past_its_line_of_contact() {
        // The frame on y = 10 with x along -x: its x past 0 runs onto the quarter wall behind -x.
        let frame = ([0.0, 10.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let quarter = post_wall((0.0, std::f64::consts::FRAC_PI_2));
        let onto = [Vec2::new(-1.0, 5.0), Vec2::new(3.0, 5.0)];
        let c = wall_contact(frame, &quarter, &onto, 1e-9).unwrap();
        let s = std::f64::consts::FRAC_1_SQRT_2;
        assert!(close(c.on, [-10.0 * s, 10.0 * s, 10.0]), "{:?}", c.on);
        // The rolled frame lays the point 5π/4 along x on the middle of the wall.
        let landed = add(c.origin, scale(c.x, 2.5 * std::f64::consts::PI));
        assert!(close(landed, [-10.0 * s, 10.0 * s, 0.0]), "{landed:?}");
        let short = [Vec2::new(-1.0, 5.0), Vec2::new(-0.5, 5.0)];
        assert!(wall_contact(frame, &quarter, &short, 1e-9).is_none());
        // A wall the plane meets inside its angles is not run onto along an edge.
        assert!(wall_contact(frame, &post_wall((-0.5, 0.5)), &onto, 1e-9).is_none());
    }

    #[test]
    fn the_whole_wall_covers_the_profiles_along_the_axis_with_its_margin() {
        let frame = ([0.0, 10.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let wall = post_wall((0.0, 1.0));
        let bounds = (Vec2::new(-7.5, 15.0), Vec2::new(7.5, 25.0));
        let seat = wall_seat(frame, bounds, &wall, true, 1.0).unwrap();
        assert!(close(seat.origin, [0.0, 0.0, 13.0]));
        assert!((seat.length - 14.0).abs() < 1e-12);
        assert!(close(seat.x, [0.0, 1.0, 0.0]) && close(seat.y, [-1.0, 0.0, 0.0]));
        let bore = wall_seat(frame, bounds, &wall, false, -1.0).unwrap();
        assert!(close(bore.y, [1.0, 0.0, 0.0]) && (bore.outer - 13.0).abs() < 1e-12);
    }
}
