//! The geometry of an emboss raised along a face's normal or stopped at a face's edge (#21423):
//! where the profiles are, how far they reach, the slab an emboss keeps over a face, the cone of a
//! profile on a sphere, the walls a plane runs onto, and why such an emboss is refused.
//!
//! The frame laid on the face is [`super::lay_frame`]'s. Everything here is plain arithmetic on
//! points and vectors given as `[x, y, z]` (and profile points as `[u, v]` in a frame); the
//! solids are built by the caller.

use std::f64::consts::TAU;

use crate::prism::profile::{Loop2, Seg2};

/// How much Normal to face lets a length change on a face that does not unroll (a sphere, cone,
/// torus or freeform face): a tenth.
pub const SIZE_TOLERANCE: f64 = 0.10;

/// Why an emboss raised along a face's normal, stopped at a face's edge or put on several faces
/// is refused, in words that say what to change.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EmbossRefusal {
    /// No target face was given.
    #[error("needs at least one target face")]
    NoTargetFace,
    /// More target faces than one emboss takes.
    #[error("at most 64 target faces")]
    TooManyFaces,
    /// The depth or the number of profiles is out of range.
    #[error("needs finite nonzero depth and 1..4096 profiles")]
    DepthOrProfiles,
    /// The sketch's frame or the pick cannot be used: axes not square, or not finite.
    #[error("invalid sketch frame or selected point")]
    BadSketchFrame,
    /// The target face is not one of the body's.
    #[error("selected face")]
    NoSuchFace,
    /// Normal to face: the line along the sketch's normal through the middle of the profiles
    /// misses the target face's surface.
    #[error("the middle of the profiles is not over the target face along the sketch's normal: move the profiles over the face")]
    MiddleOffFace,
    /// Normal to face on a cylinder wall: the sketch faces the other side of the wall.
    #[error("the sketch faces away from this side of the wall: pick the side of the wall the sketch faces, or flip the sketch")]
    FacesAwayFromWall,
    /// Normal to face on a sphere: the sketch faces the other side of it.
    #[error("the sketch faces away from this side of the sphere: pick the side the sketch faces, or flip the sketch")]
    FacesAwayFromSphere,
    /// Normal to face on a cone, torus or freeform face: the sketch faces its other side.
    #[error("the sketch faces away from this side of the face: pick the side the sketch faces, or flip the sketch")]
    FacesAwayFromFace,
    /// A sketch touching an outside wall from inside, which the wrap cannot take.
    #[error("sketch normal must point outwards from the exterior wall")]
    SketchFacesIntoWall,
    /// Stop at face edge on a cylinder wall: the sketch's normal runs along the wall's axis.
    #[error(
        "the sketch's normal runs along the wall's axis, so the profiles never reach the wall"
    )]
    NormalAlongAxis,
    /// Normal to face on a plane: the profiles run off it onto more than one round tangent to it.
    #[error("the profiles run off the target face onto {count} rounds beside it, and an emboss carries on onto one: move them so they run onto one round, or pick it")]
    OntoSeveralWalls {
        /// How many rounds the profiles run onto.
        count: usize,
    },
    /// Normal to face on a sphere: a deboss as deep as the radius.
    #[error("the depth reaches the sphere's centre: make it smaller")]
    DepthReachesCentre,
    /// Normal to face on a sphere: the profiles reach so far from their middle that a length
    /// would shrink by more than [`SIZE_TOLERANCE`].
    #[error("on this sphere a profile keeps its size within 10% only within {most:.2} mm of the middle of the profiles, and they reach {reach:.2} mm: make them smaller, or use Normal to sketch")]
    ShrinksOnSphere {
        /// How far from the middle a profile may reach.
        most: f64,
        /// How far the profiles reach.
        reach: f64,
    },
    /// Normal to face on a cone, torus or freeform face: the face's normal turns so far under the
    /// profiles that a length would change by more than [`SIZE_TOLERANCE`].
    #[error("this face curves too much under the profiles for them to keep their size within 10%: make them smaller, or use Normal to sketch")]
    CurvesTooMuch,
    /// Normal to face: the face has no normal at a point under the profiles.
    #[error("the target face has no normal under the profiles")]
    NoNormal,
    /// Normal to face without the stop: profiles (numbered from 1, of `of`; none named when the
    /// face does not say which) run off the target face.
    #[error("{} the target face: move {} onto the face, or turn on Stop at face edge to trim {}", runs_off(profiles, *of), them(profiles), them(profiles))]
    RunsOffFace {
        /// The profiles that run off, numbered from 1; empty when not known one by one.
        profiles: Vec<usize>,
        /// How many profiles there are.
        of: usize,
    },
    /// Stop at face edge: nothing of the profiles lies over the target face.
    #[error("no part of the profiles lies on the target face")]
    NothingOnFace,
    /// The face cannot be offset to the depth, so the emboss has no slab to keep.
    #[error("this face could not be offset to the emboss's depth ({0}): try a smaller depth, or Normal to sketch")]
    NotOffset(String),
    /// Several target faces: a profile lies over none of them along the sketch's normal.
    #[error(
        "a profile lies over none of the target faces along the sketch's normal: move it over one"
    )]
    OverNoTargetFace,
    /// The profiles would cut the whole body away.
    #[error("the profiles would leave nothing of the body")]
    NothingLeft,
}

fn runs_off(profiles: &[usize], of: usize) -> String {
    let names: Vec<String> = profiles.iter().map(ToString::to_string).collect();
    match names.split_last() {
        None => "a profile runs off".to_owned(),
        Some((last, [])) => format!("profile {last} of {of} runs off"),
        Some((last, rest)) => format!("profiles {} and {last} of {of} run off", rest.join(", ")),
    }
}

fn them(profiles: &[usize]) -> &'static str {
    if profiles.len() > 1 {
        "them"
    } else {
        "it"
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn finite(points: &[[f64; 3]]) -> bool {
    points.iter().flatten().all(|c| c.is_finite())
}

/// Points of a profile loop to bound it by: each line's ends, and `per_curve + 1` points
/// evenly along the parameter of every other segment.
#[must_use]
pub fn loop_points(l: &Loop2, per_curve: usize) -> Vec<[f64; 2]> {
    let n = per_curve.max(1);
    let mut out = Vec::new();
    for seg in &l.segs {
        let ts: Vec<f64> = match seg {
            Seg2::Line { .. } => vec![0.0, 1.0],
            _ => (0..=n).map(|i| i as f64 / n as f64).collect(),
        };
        out.extend(ts.into_iter().map(|t| seg.point_at(t)).map(|p| [p.x, p.y]));
    }
    out.retain(|p| p[0].is_finite() && p[1].is_finite());
    out
}

/// The point a fraction `t` along a profile segment's parameter.
#[must_use]
pub fn segment_point(seg: &Seg2, t: f64) -> [f64; 2] {
    let p = seg.point_at(t);
    [p.x, p.y]
}

/// `count` points evenly along a profile segment's parameter, from its start, short of its end.
#[must_use]
pub fn segment_points(seg: &Seg2, count: usize) -> Vec<[f64; 2]> {
    let n = count.max(1);
    (0..n)
        .map(|i| segment_point(seg, i as f64 / n as f64))
        .collect()
}

/// The middle of the bounds of `points`; `None` for no points.
#[must_use]
pub fn bounds_middle(points: &[[f64; 2]]) -> Option<[f64; 2]> {
    let first = *points.first()?;
    let (lo, hi) = points.iter().fold((first, first), |(l, h), p| {
        (
            [l[0].min(p[0]), l[1].min(p[1])],
            [h[0].max(p[0]), h[1].max(p[1])],
        )
    });
    Some([(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5])
}

/// How far the farthest of `points` is from `from`.
#[must_use]
pub fn reach(points: &[[f64; 2]], from: [f64; 2]) -> f64 {
    points
        .iter()
        .map(|p| (p[0] - from[0]).hypot(p[1] - from[1]))
        .fold(0.0, f64::max)
}

/// The point `(u, v)` of the frame (`origin`, `x`, `y`).
#[must_use]
pub fn frame_point(origin: [f64; 3], x: [f64; 3], y: [f64; 3], [u, v]: [f64; 2]) -> [f64; 3] {
    add(origin, add(scale(x, u), scale(y, v)))
}

/// The unit normal `x × y` of a frame; `None` where `x` and `y` are parallel or not finite.
#[must_use]
pub fn frame_normal(x: [f64; 3], y: [f64; 3]) -> Option<[f64; 3]> {
    let n = cross(x, y);
    let len = dot(n, n).sqrt();
    (len.is_finite() && len > 0.0).then(|| scale(n, 1.0 / len))
}

/// The point `t` along `direction` from `p`.
#[must_use]
pub fn along(p: [f64; 3], direction: [f64; 3], t: f64) -> [f64; 3] {
    add(p, scale(direction, t))
}

/// `v` reversed.
#[must_use]
pub fn reversed(v: [f64; 3]) -> [f64; 3] {
    scale(v, -1.0)
}

/// How far the slab an emboss keeps over a face reaches past it, below and above (along its
/// outward normal), for `depth` (positive raised, negative sunk): a little past the face on the
/// side away from the depth, so the pieces overlap the body rather than touch it, and exactly to
/// the depth, which then bounds prisms run through the body. A tool already `depth` deep
/// (`exact`) takes the margin on both sides.
#[must_use]
pub fn slab_reach(depth: f64, exact: bool) -> (f64, f64) {
    let d = depth.abs();
    let overlap = (d * 0.2).clamp(0.1, 2.0);
    let past = if exact { overlap } else { 0.0 };
    if depth > 0.0 {
        (-overlap, d + past)
    } else {
        (-d - past, overlap)
    }
}

/// The inner and outer radii of the offsets `lo` and `hi` (along the outward normal) of a
/// cylinder or sphere of `radius`: `convex` when its outward normal points away from its axis or
/// centre.
#[must_use]
pub fn offset_radii(radius: f64, (lo, hi): (f64, f64), convex: bool) -> (f64, f64) {
    if convex {
        (radius + lo, radius + hi)
    } else {
        (radius - hi, radius - lo)
    }
}

/// The base of the cones a sphere's emboss is built from: the tangent plane at `contact` on the
/// sphere about `centre` of `radius`, scaled about the centre to lie past the farther offset of
/// `(lo, hi)` by 1. Returns the scaled contact point and the scale, which a profile drawn in the
/// tangent plane about that point takes too.
#[must_use]
pub fn cone_base(
    centre: [f64; 3],
    radius: f64,
    (lo, hi): (f64, f64),
    contact: [f64; 3],
) -> Option<([f64; 3], f64)> {
    let k = (radius + lo.abs().max(hi.abs()) + 1.0) / radius;
    let origin = add(centre, scale(sub(contact, centre), k));
    (k.is_finite() && k > 0.0 && finite(&[origin])).then_some((origin, k))
}

/// The profile loop `l` scaled by `k` about its frame's origin.
#[must_use]
pub fn scaled_loop(l: &Loop2, k: f64) -> Loop2 {
    let s = |p: crate::math::Point2| p * k;
    Loop2 {
        segs: l
            .segs
            .iter()
            .map(|seg| match *seg {
                Seg2::Line { a, b } => Seg2::Line { a: s(a), b: s(b) },
                Seg2::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                } => Seg2::Arc {
                    center: s(center),
                    radius: radius * k,
                    start,
                    sweep,
                },
                Seg2::Cubic { p0, p1, p2, p3 } => Seg2::Cubic {
                    p0: s(p0),
                    p1: s(p1),
                    p2: s(p2),
                    p3: s(p3),
                },
                Seg2::Conic { a, apex, b, w } => Seg2::Conic {
                    a: s(a),
                    apex: s(apex),
                    b: s(b),
                    w,
                },
                Seg2::Bezier { n, p } => Seg2::Bezier { n, p: p.map(s) },
            })
            .collect(),
    }
}

/// Stop at face edge on a face that does not unroll: how far either side of the tangent plane
/// under the middle of the profiles the face under them can lie. It sags off that plane by at
/// most `reach` times the tangent of the largest lean, whose secant is `1 + stretch`, and the
/// slab `(lo, hi)` adds its own reach; `tol` pads it.
#[must_use]
pub fn sag_band(reach: f64, stretch: f64, (lo, hi): (f64, f64), tol: f64) -> f64 {
    let lean = ((1.0 + stretch).powi(2) - 1.0).max(0.0).sqrt();
    reach * lean + lo.abs().max(hi.abs()) + tol * 1e3
}

/// How far a box past a body reaches from `o`, for a body bounded by `min` and `max`: its
/// diagonal, the distance from `o` to `min`, and 1 more.
#[must_use]
pub fn past_reach(min: [f64; 3], max: [f64; 3], o: [f64; 3]) -> f64 {
    let d = sub(max, min);
    let w = sub(o, min);
    dot(d, d).sqrt() + dot(w, w).sqrt() + 1.0
}

/// The lowest and highest heights of `points` above `o` along `direction`; `None` for no points.
#[must_use]
pub fn heights(points: &[[f64; 3]], o: [f64; 3], direction: [f64; 3]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    Some(
        points
            .iter()
            .map(|p| dot(sub(*p, o), direction))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| {
                (l.min(x), h.max(x))
            }),
    )
}

/// Whether a wall face spanning the angles `lo ..= hi` about its axis goes the whole way round.
#[must_use]
pub fn whole_turn((lo, hi): (f64, f64)) -> bool {
    hi - lo > TAU - 1e-9
}

/// Whether a wall face of `radius` spanning the angles `lo ..= hi` from a line of contact starts
/// at that line, within `tol` along the wall: the line is one of its edges.
#[must_use]
pub fn starts_at_contact((lo, hi): (f64, f64), radius: f64, tol: f64) -> bool {
    let seam = tol * 10.0 / radius;
    lo.abs() <= seam || hi.abs() <= seam
}

/// Which way from the line of contact a wall face spanning the angles `lo ..= hi` from it runs:
/// `1.0` towards the frame's tangent, `-1.0` away from it; `None` where it runs both ways or
/// neither.
#[must_use]
pub fn wall_side((lo, hi): (f64, f64), radius: f64, tol: f64) -> Option<f64> {
    let seam = tol * 10.0 / radius;
    match (lo >= -seam, hi <= seam) {
        (true, false) => Some(1.0),
        (false, true) => Some(-1.0),
        _ => None,
    }
}

/// Whether any of `points`, in a plane touching a wall along a line through `contact`, lies past
/// that line by more than `tol` on the wall's `side` (`1.0` along `tangent`, `-1.0` against it):
/// a profile there runs off the plane onto the wall.
#[must_use]
pub fn past_contact(
    points: &[[f64; 3]],
    contact: [f64; 3],
    tangent: [f64; 3],
    side: f64,
    tol: f64,
) -> bool {
    points
        .iter()
        .any(|p| dot(sub(*p, contact), tangent) * side > tol)
}

/// The point at angle `turn` (from `radial` towards `tangent`) round the wall of `radius` about
/// the axis through `o` along unit `axis`, `height` along it.
#[must_use]
pub fn wall_point(
    o: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    (radial, tangent): ([f64; 3], [f64; 3]),
    turn: f64,
    height: f64,
) -> [f64; 3] {
    let (sin, cos) = turn.sin_cos();
    add(
        along(o, axis, height),
        scale(add(scale(radial, cos), scale(tangent, sin)), radius),
    )
}

/// The point `p` as seen from `o`: `p - o`.
#[must_use]
pub fn from_point(o: [f64; 3], p: [f64; 3]) -> [f64; 3] {
    sub(p, o)
}

/// Two unit axes square to each other and to `normal`, so a frame on them faces along it;
/// `None` for a `normal` of no length or not finite.
#[must_use]
pub fn square_axes(normal: [f64; 3]) -> Option<([f64; 3], [f64; 3])> {
    let n = frame_normal_of(normal)?;
    // The world axis least along the normal keeps the cross product well away from zero.
    let pick = if n[0].abs() <= n[1].abs() && n[0].abs() <= n[2].abs() {
        [1.0, 0.0, 0.0]
    } else if n[1].abs() <= n[2].abs() {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let x = frame_normal_of(cross(pick, n))?;
    Some((x, cross(n, x)))
}

fn frame_normal_of(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = dot(v, v).sqrt();
    (len.is_finite() && len > 0.0).then(|| scale(v, 1.0 / len))
}

/// The interval `lo ..= hi` padded by `margin` each way, as its start and length.
#[must_use]
pub fn padded((lo, hi): (f64, f64), margin: f64) -> (f64, f64) {
    (lo - margin, hi - lo + 2.0 * margin)
}

/// The radius a bore's whole wall is built out to for a wrap `depth` deep: past the wall by the
/// depth, and past that by the depth and 1 more.
#[must_use]
pub fn bore_reach(radius: f64, depth: f64) -> f64 {
    radius + 2.0 * depth.abs() + 1.0
}

/// The interval `lo ..= hi` widened by `margin` each way.
#[must_use]
pub fn widened((lo, hi): (f64, f64), margin: f64) -> (f64, f64) {
    (lo - margin, hi + margin)
}

/// How far past what it spans a tool runs for an emboss `depth` deep: the depth and 1 more.
#[must_use]
pub fn depth_margin(depth: f64) -> f64 {
    depth.abs() + 1.0
}

/// The two half spaces either side of the plane through `anchor` square to the unit `normal`, by
/// `band` each way, that hold what lies within `band` of it: a point on each bounding plane and
/// the side kept.
#[must_use]
pub fn band_sides(anchor: [f64; 3], normal: [f64; 3], band: f64) -> [([f64; 3], [f64; 3]); 2] {
    [
        (along(anchor, normal, -band), normal),
        (along(anchor, normal, band), reversed(normal)),
    ]
}

/// Whether the volume `kept` is the whole volume `full`, to the volumes' rounding.
#[must_use]
pub fn same_volume(kept: f64, full: f64) -> bool {
    (full - kept).abs() <= 1e-9 * (1.0 + full.abs())
}

/// The middle of an interval.
#[must_use]
pub fn middle_of((lo, hi): (f64, f64)) -> f64 {
    0.5 * (lo + hi)
}

/// The distance between two points.
#[must_use]
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point2;

    fn square(a: f64, b: f64) -> Loop2 {
        let p = |x, y| Point2::new(x, y);
        let corners = [p(a, a), p(b, a), p(b, b), p(a, b)];
        Loop2 {
            segs: (0..4)
                .map(|i| Seg2::Line {
                    a: corners[i],
                    b: corners[(i + 1) % 4],
                })
                .collect(),
        }
    }

    #[test]
    fn a_square_has_its_middle_and_reach_at_its_corners() {
        let points = loop_points(&square(1.0, 5.0), 8);
        let mid = bounds_middle(&points).unwrap();
        assert_eq!(mid, [3.0, 3.0]);
        assert!((reach(&points, mid) - 8f64.sqrt()).abs() < 1e-15);
        assert_eq!(bounds_middle(&[]), None);
    }

    #[test]
    fn a_circle_is_bounded_by_its_points_and_scales_about_the_origin() {
        let circle = Loop2 {
            segs: vec![Seg2::Arc {
                center: Point2::new(2.0, 0.0),
                radius: 1.0,
                start: 0.0,
                sweep: TAU,
            }],
        };
        let points = loop_points(&circle, 4);
        assert_eq!(points.len(), 5);
        let mid = bounds_middle(&points).unwrap();
        assert!((mid[0] - 2.0).abs() < 1e-12 && mid[1].abs() < 1e-12);
        match scaled_loop(&circle, 3.0).segs[0] {
            Seg2::Arc { center, radius, .. } => {
                assert_eq!((center.x, center.y, radius), (6.0, 0.0, 3.0));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_slab_reaches_past_the_face_away_from_the_depth() {
        assert_eq!(slab_reach(1.0, false), (-0.2, 1.0));
        assert_eq!(slab_reach(-1.0, false), (-1.0, 0.2));
        assert_eq!(slab_reach(1.0, true), (-0.2, 1.2));
        // The margin is held between 0.1 and 2.
        assert_eq!(slab_reach(0.1, false), (-0.1, 0.1));
        assert_eq!(slab_reach(-20.0, false), (-20.0, 2.0));
        assert_eq!(offset_radii(10.0, (-0.2, 1.0), true), (9.8, 11.0));
        assert_eq!(offset_radii(10.0, (-0.2, 1.0), false), (9.0, 10.2));
    }

    #[test]
    fn a_cone_base_lies_one_past_the_farther_offset() {
        let (origin, k) = cone_base([0.0; 3], 10.0, (-0.2, 1.0), [0.0, 10.0, 0.0]).unwrap();
        assert!((k - 1.2).abs() < 1e-15);
        assert!((origin[1] - 12.0).abs() < 1e-12);
    }

    #[test]
    fn a_wall_runs_one_way_from_its_line_of_contact() {
        let tol = 1e-8;
        assert_eq!(wall_side((0.0, 3.0), 10.0, tol), Some(1.0));
        assert_eq!(wall_side((-3.0, 0.0), 10.0, tol), Some(-1.0));
        assert_eq!(wall_side((-1.0, 1.0), 10.0, tol), None);
        assert!(starts_at_contact((0.0, 3.0), 10.0, tol));
        assert!(!starts_at_contact((-1.0, 1.0), 10.0, tol));
        assert!(whole_turn((-3.2, 3.2)) && !whole_turn((0.0, 3.0)));
        // The plane y = 10 touches the wall about z of radius 10 along x = 0; the wall runs
        // towards -x. A point at x = -1 lies past the line; one at x = 4 does not.
        let (contact, tangent) = ([0.0, 10.0, 0.0], [-1.0, 0.0, 0.0]);
        assert!(past_contact(
            &[[-1.0, 10.0, 3.0]],
            contact,
            tangent,
            1.0,
            tol
        ));
        assert!(!past_contact(
            &[[4.0, 10.0, 3.0]],
            contact,
            tangent,
            1.0,
            tol
        ));
        let p = wall_point(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            10.0,
            ([0.0, 1.0, 0.0], tangent),
            std::f64::consts::FRAC_PI_2,
            5.0,
        );
        assert!(distance(p, [-10.0, 0.0, 5.0]) < 1e-12);
    }

    #[test]
    fn a_face_leaning_under_the_profiles_widens_the_band() {
        // A stretch of 1/cos 45° - 1 leans 45°: a reach of 2 sags by 2.
        let s = std::f64::consts::SQRT_2 - 1.0;
        assert!((sag_band(2.0, s, (-0.2, 1.0), 0.0) - 3.0).abs() < 1e-12);
        assert_eq!(heights(&[], [0.0; 3], [0.0, 0.0, 1.0]), None);
        assert_eq!(
            heights(
                &[[0.0, 0.0, 2.0], [1.0, 1.0, -1.0]],
                [0.0; 3],
                [0.0, 0.0, 1.0]
            ),
            Some((-1.0, 2.0))
        );
        assert!((past_reach([0.0; 3], [3.0, 4.0, 0.0], [3.0, 4.0, 0.0]) - 11.0).abs() < 1e-12);
        assert_eq!(padded((1.0, 3.0), 0.5), (0.5, 3.0));
        assert_eq!(widened((1.0, 3.0), 0.5), (0.5, 3.5));
        assert_eq!(depth_margin(-2.0), 3.0);
        assert!(same_volume(1.0, 1.0 + 1e-12) && !same_volume(1.0, 1.001));
        let [low, high] = band_sides([0.0; 3], [0.0, 0.0, 1.0], 2.0);
        assert_eq!(low, ([0.0, 0.0, -2.0], [0.0, 0.0, 1.0]));
        assert_eq!(high, ([0.0, 0.0, 2.0], [0.0, 0.0, -1.0]));
        assert_eq!(bore_reach(10.0, -1.0), 13.0);
        let (x, y) = square_axes([0.0, 0.0, 2.0]).unwrap();
        assert_eq!(frame_normal(x, y), Some([0.0, 0.0, 1.0]));
    }

    #[test]
    fn refusals_name_the_profiles_that_run_off() {
        let one = EmbossRefusal::RunsOffFace {
            profiles: vec![1],
            of: 2,
        };
        assert_eq!(one.to_string(), "profile 1 of 2 runs off the target face: move it onto the face, or turn on Stop at face edge to trim it");
        let two = EmbossRefusal::RunsOffFace {
            profiles: vec![1, 3],
            of: 3,
        };
        assert!(two
            .to_string()
            .starts_with("profiles 1 and 3 of 3 run off the target face: move them"));
        let any = EmbossRefusal::RunsOffFace {
            profiles: vec![],
            of: 3,
        };
        assert!(any
            .to_string()
            .starts_with("a profile runs off the target face: move it"));
    }
}
