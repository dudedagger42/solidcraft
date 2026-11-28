//! Emboss Direction (#21423): Normal to sketch, the emboss as it has always been built, or Normal
//! to face, where the profiles are laid on the face keeping their size and raised along its
//! normal; and Stop at face edge, which trims either at the edge of the picked face.
//!
//! This sequences the calls. How an emboss onto a face is built is decided before anything is
//! built ([`ops::route`], from the face's kind, the direction and the stop), every point, length
//! and frame is solidcore's ([`solidcore_ops::emboss_direction`]), and a route that does not
//! apply refuses with an [`EmbossRefusal`] that names the reason. Normal to face lays the
//! sketch's frame on the face under the middle of the profiles, then builds what the face takes
//! from that frame: the exact wrap on a cylinder, the projection on a plane (or, where the
//! profiles run off it onto the one round tangent to it, the wrap carried on from it), the cones
//! from a sphere's centre, or, on any other face, the projection along its normal within
//! [`ops::SIZE_TOLERANCE`].
//!
//! Stop at face edge builds each profile's solid without the body's trims (the wrap on a whole
//! cylinder of the wall's radius, or the profiles' prisms) and keeps what lies over the picked
//! wall or face: the slab between offsets of its faces, whose sides run along their normals at
//! their edges.

use super::{
    apply_pieces, cylinder_emboss_anchor, emboss_anchor, emboss_cylinder, emboss_cylinder_at, emboss_project_face, face_axial_bounds,
    infer_cylinder_emboss, record, wall_angles,
};
use crate::body::Body;
use crate::offset::{Surf, face_edges, surf_of};
use crate::{BoolOp, KernelError, Result, guard};
use solidcore_ops::cylinder_wall::WallFrame;
use solidcore_ops::emboss_direction::{self as ops, EmbossRefusal, FaceKind, Route, TangentWall};
use solidcore_record::brep::Brep;
use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};

/// Which way an emboss raises its profiles: along the sketch's normal (the default) or the face's.
pub use solidcore_ops::emboss_direction::Direction as EmbossDirection;

/// A step of building an emboss that gave no solid where one was due.
fn failed(what: &str) -> KernelError {
    KernelError::Failed(format!("emboss: {what}"))
}

/// The tolerance of points on `body`'s faces.
fn tol_of(body: &Body) -> f64 {
    (body.size() * 1e-7).max(1e-7)
}

/// The tolerance of a plane touching a wall of `body`, as the wrap takes it.
fn touch_tol(body: &Body) -> f64 {
    (body.size() * 1e-8).max(1e-8)
}

/// The surface of face `face` of `rec`, as the emboss reads it.
fn kind_of(rec: &Brep, face: usize) -> Result<FaceKind> {
    if face >= rec.faces.len() {
        return Err(EmbossRefusal::NoSuchFace.into());
    }
    Ok(match surf_of(rec, face) {
        Some(Surf::Plane { n, d }) => FaceKind::Plane { normal: n, offset: d },
        Some(Surf::Cylinder { o, a, r, convex }) => FaceKind::Cylinder { origin: o, axis: a, radius: r, convex },
        Some(Surf::Sphere { c, r, convex }) => FaceKind::Sphere { centre: c, radius: r, convex },
        _ => FaceKind::Other,
    })
}

/// Emboss (`depth > 0`) or deboss `regions` onto face `face` of `body`, `at` a pick on it,
/// raised `direction`, trimmed at the face's edge when `stop_at_edge`.
#[allow(clippy::too_many_arguments)]
pub fn emboss_on_face(
    body: &Body,
    face: usize,
    at: Vec3,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    direction: EmbossDirection,
    stop_at_edge: bool,
) -> Result<Body> {
    if direction == EmbossDirection::Sketch && !stop_at_edge {
        return as_before(body, face, at, plane, regions, depth);
    }
    body.require_brep("emboss")?;
    ops::checked(depth, regions.len())?;
    let kind = guard("emboss onto a face", || kind_of(&record(body)?, face))?;
    match ops::route(direction, stop_at_edge, &kind) {
        Route::AsBefore => as_before(body, face, at, plane, regions, depth),
        Route::SketchToWallEdge => sketch_to_wall_edge(body, face, at, plane, regions, depth, kind),
        Route::SketchToSphereEdge => {
            let FaceKind::Sphere { centre, .. } = kind else { return Err(EmbossRefusal::NoSuchFace.into()) };
            // A sphere's two crossings lie either side of the plane through its centre square to
            // the sketch's normal: the side the pick is on is kept.
            let side = ops::sphere_half(centre, ops::sketch_normal(plane)?, emboss_anchor(body, face, at)?)?;
            prisms_to_edge(body, face, plane, regions, depth, &[(centre, side)])
        }
        Route::SketchToFaceEdge => prisms_to_edge(body, face, plane, regions, depth, &[]),
        Route::FaceOnPlane => face_on_plane(body, face, at, plane, regions, depth, stop_at_edge, kind),
        Route::FaceOnWall => face_on_wall(body, face, at, plane, regions, depth, stop_at_edge, kind),
        Route::FaceOnSphere => face_on_sphere(body, face, at, plane, regions, depth, stop_at_edge, kind),
        Route::FaceOnCurved => face_on_curved(body, face, at, plane, regions, depth, stop_at_edge),
    }
}

/// [`emboss_on_face`] onto several target faces of `body` (`faces`: each face and a pick on it):
/// each profile goes on the target face the middle of its bounds lies over along the sketch's
/// normal (the nearest such along it), and is built there as on that face alone. Each face's
/// emboss is built on the body as given and their material is joined (or cut) together, so one
/// face's change does not move another's.
#[allow(clippy::too_many_arguments)]
pub fn emboss_on_faces(
    body: &Body,
    faces: &[(usize, Vec3)],
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    direction: EmbossDirection,
    stop_at_edge: bool,
) -> Result<Body> {
    let Some(&(face, at)) = faces.first() else {
        return Err(EmbossRefusal::NoTargetFace.into());
    };
    if faces.len() == 1 {
        return emboss_on_face(body, face, at, plane, regions, depth, direction, stop_at_edge);
    }
    if faces.len() > ops::MAX_FACES {
        return Err(EmbossRefusal::TooManyFaces.into());
    }
    body.require_brep("emboss")?;
    ops::checked(depth, regions.len())?;
    let rec = guard("emboss onto several faces", || record(body))?;
    let tol = tol_of(body);
    let mut groups: Vec<Vec<Region2>> = vec![Vec::new(); faces.len()];
    for region in regions {
        let mid = ops::middle(std::slice::from_ref(region))?;
        let p = ops::sketch_point(plane, mid);
        let mut best: Option<(f64, usize)> = None;
        for (k, (f, at)) in faces.iter().enumerate() {
            if let Some(d) = under(body, &rec, *f, *at, plane, (mid, p), tol)?
                && best.is_none_or(|(b, _)| d < b)
            {
                best = Some((d, k));
            }
        }
        let (_, k) = best.ok_or(EmbossRefusal::OverNoTargetFace)?;
        if let Some(group) = groups.get_mut(k) {
            group.push(region.clone());
        }
    }
    let op = if depth > 0.0 { BoolOp::Union } else { BoolOp::Cut };
    let mut result = body.clone();
    for ((face, at), group) in faces.iter().zip(&groups) {
        if group.is_empty() {
            continue;
        }
        let built = emboss_on_face(body, *face, *at, plane, group, depth, direction, stop_at_edge)?;
        let change = if depth > 0.0 { crate::boolean(&built, body, BoolOp::Cut)? } else { crate::boolean(body, &built, BoolOp::Cut)? };
        let Some(change) = change else { continue };
        result = crate::boolean(&result, &change, op)?.ok_or(EmbossRefusal::NothingLeft)?;
    }
    Ok(result)
}

/// How far the sketch point `p` (`mid` on `plane`) is from face `face` along the sketch's normal:
/// on a plane, cylinder or sphere, from the point of the face's surface under it along that line
/// (the nearer of two), when that point lies on the face; on any other face, from where that line
/// meets it nearest the pick `at`. `None` where the line misses the face.
fn under(body: &Body, rec: &Brep, face: usize, at: Vec3, plane: &Plane, (mid, p): (Vec2, Vec3), tol: f64) -> Result<Option<f64>> {
    let kind = kind_of(rec, face)?;
    if kind == FaceKind::Other {
        let (_, normal, start) = ops::line_under(plane, mid, at)?;
        return Ok(body.face_line_hit(face, start, normal)?.map(|hit| ops::distance(hit, p)));
    }
    let Some(on) = ops::point_under(&kind, plane, mid, p) else { return Ok(None) };
    let held = crate::point_closest(on, body, crate::Members::Faces(&[face]))?.is_some_and(|hit| hit.distance <= tol);
    Ok(held.then(|| ops::distance(on, p)))
}

/// Normal to sketch without the stop, the emboss as it has always been built: a sketch tangent to
/// a cylinder wall, outside or a bore, takes the wrap (whose refusals stand); a sketch off the
/// wall and every other face are projected.
fn as_before(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64) -> Result<Body> {
    match cylinder_emboss_anchor(body, face, at) {
        Ok(on) if infer_cylinder_emboss(body, plane, regions, depth)?.is_some() => emboss_cylinder_at(body, on, plane, regions, depth),
        _ => emboss_project_face(body, face, emboss_anchor(body, face, at)?, plane, regions, depth),
    }
}

/// Normal to sketch, stopped at a cylinder wall's edge: a sketch touching the wall wraps as
/// without the stop, trimmed at the wall; one touching an outside wall from inside is refused, as
/// the wrap is; any other sketch's prisms run along its normal and are kept on the half of the
/// wall the pick is on.
#[allow(clippy::too_many_arguments)]
fn sketch_to_wall_edge(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, kind: FaceKind) -> Result<Body> {
    let FaceKind::Cylinder { origin, axis, radius, convex } = kind else { return Err(EmbossRefusal::NoSuchFace.into()) };
    let wall = (origin, axis, radius, convex);
    if let Some(frame) = ops::touches(plane, wall, touch_tol(body)) {
        return wrap_to_edge(body, face, plane, regions, depth, wall, &frame);
    }
    if ops::touches(plane, (origin, axis, radius, false), touch_tol(body)).is_some() {
        return Err(EmbossRefusal::SketchFacesIntoWall.into());
    }
    // A line along the sketch's normal crosses a wall twice, once each side of the plane through
    // the axis square to it: the side the pick is on is kept.
    let side = ops::wall_half(origin, axis, ops::sketch_normal(plane)?, emboss_anchor(body, face, at)?)?;
    prisms_to_edge(body, face, plane, regions, depth, &[(origin, side)])
}

/// Normal to face on a plane: the frame laid on it. Stopped, its prisms are kept over the face.
/// Otherwise the profiles are projected from it, unless they run off the plane onto the one
/// round tangent to it beside it: then the wrap is carried on from it round that round (#5358).
#[allow(clippy::too_many_arguments)]
fn face_on_plane(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool, kind: FaceKind) -> Result<Body> {
    let laid = ops::lay(&kind, plane, ops::middle(regions)?, at)?;
    if stop {
        return prisms_to_edge(body, face, &laid.plane, regions, depth, &[]);
    }
    let walls = guard("emboss normal to face", || tangent_walls(body, face, &laid.plane))?;
    match ops::onto_wall(&laid.plane, regions, &walls, touch_tol(body))? {
        None => emboss_project_face(body, face, emboss_anchor(body, face, at)?, &laid.plane, regions, depth),
        Some(i) => {
            let wall = walls.get(i).ok_or(EmbossRefusal::NoSuchFace)?;
            let (on, rolled) = ops::rolled_onto(&laid.plane, wall)?;
            emboss_cylinder_at(body, on, &rolled, regions, depth)
        }
    }
}

/// The outside walls beside face `face` (sharing an edge with it) that the plane `laid` touches
/// along that edge.
fn tangent_walls(body: &Body, face: usize, laid: &Plane) -> Result<Vec<TangentWall>> {
    let rec = record(body)?;
    let tol = touch_tol(body);
    let edges = face_edges(&rec, face);
    let mut walls = Vec::new();
    for f in (0..rec.faces.len()).filter(|f| *f != face && face_edges(&rec, *f).iter().any(|e| edges.contains(e))) {
        let Some(Surf::Cylinder { o, a, r, convex: true }) = surf_of(&rec, f) else { continue };
        let Some(frame) = ops::touches(laid, (o, a, r, true), tol) else { continue };
        let Some(angles) = wall_angles(&rec, f, o, a, Vec3::from(frame.radial), Vec3::from(frame.tangent)) else { continue };
        let Some(heights) = face_axial_bounds(&rec, f, o, a)? else { continue };
        let wall = TangentWall { origin: o, axis: a, radius: r, frame, angles, heights };
        if ops::beside(&wall, tol) {
            walls.push(wall);
        }
    }
    Ok(walls)
}

/// Normal to face on a cylinder wall: the wrap from the frame laid on it, trimmed at the wall's
/// edge when stopped.
#[allow(clippy::too_many_arguments)]
fn face_on_wall(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool, kind: FaceKind) -> Result<Body> {
    let FaceKind::Cylinder { origin, axis, radius, convex } = kind else { return Err(EmbossRefusal::NoSuchFace.into()) };
    let laid = ops::lay(&kind, plane, ops::middle(regions)?, at)?;
    if stop {
        let wall = (origin, axis, radius, convex);
        let frame = ops::touches(&laid.plane, wall, touch_tol(body)).ok_or(EmbossRefusal::BadSketchFrame)?;
        return wrap_to_edge(body, face, &laid.plane, regions, depth, wall, &frame);
    }
    emboss_cylinder_at(body, cylinder_emboss_anchor(body, face, at)?, &laid.plane, regions, depth)
}

/// Normal to face on a sphere, by the closest point: the tangent plane at the point under the
/// middle of the profiles is laid towards the centre, so each profile's solid is the cone over it
/// from the centre, its walls along the sphere's normals, kept between the offsets of the face.
/// Exact; lengths shrink away from the middle, and profiles reaching so far that one would shrink
/// by more than [`ops::SIZE_TOLERANCE`] are refused.
#[allow(clippy::too_many_arguments)]
fn face_on_sphere(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool, kind: FaceKind) -> Result<Body> {
    let FaceKind::Sphere { centre, radius, convex } = kind else { return Err(EmbossRefusal::NoSuchFace.into()) };
    let mid = ops::middle(regions)?;
    let laid = ops::lay(&kind, plane, mid, at)?;
    let cones = ops::sphere_cones((centre, radius, convex), &laid.plane, regions, mid, depth, tol_of(body))?;
    guard("emboss normal to face", || {
        let rec = record(body)?;
        let over = cell(body, &rec, face, cones.slab)?;
        let (r_in, r_out) = cones.radii;
        let shell = crate::boolean(&crate::sphere(centre, r_out)?, &crate::sphere(centre, r_in)?, BoolOp::Cut)?.ok_or_else(|| failed("shell"))?;
        let mut pieces = Vec::new();
        let mut off = Vec::new();
        for (i, reg) in cones.profiles.iter().enumerate() {
            let mut cone = crate::loft_to_point(&cones.base, &reg.outer, centre)?;
            for hole in &reg.holes {
                cone = crate::boolean(&cone, &crate::loft_to_point(&cones.base, hole, centre)?, BoolOp::Cut)?.ok_or_else(|| failed("profile"))?;
            }
            let piece = crate::boolean(&cone, &over, BoolOp::Intersect)?;
            if !stop {
                let whole = crate::boolean(&cone, &shell, BoolOp::Intersect)?;
                let size = |b: &Option<Body>| b.as_ref().map_or(Ok(0.0), |b| crate::measure(b).map(|m| m.volume));
                if !ops::kept_whole(size(&piece)?, size(&whole)?) {
                    off.push(i + 1);
                    continue;
                }
            }
            if let Some(piece) = piece {
                pieces.push(piece);
            }
        }
        if !off.is_empty() {
            return Err(EmbossRefusal::RunsOffFace { profiles: off, of: regions.len() }.into());
        }
        if pieces.is_empty() {
            return Err(EmbossRefusal::NothingOnFace.into());
        }
        apply_pieces(body, pieces, depth)
    })
}

/// Normal to face on a cone, torus or freeform face: the point of the face under the middle of
/// the profiles along the sketch's normal (the meeting nearest the pick `at`) takes the sketch's
/// frame laid on the face's tangent plane there, and the profiles are projected along that normal
/// onto the face, raised to the offset of the face at the depth. Under the profiles' outlines and
/// holes the face's normal must stay close enough to that one that no length there changes by
/// more than [`ops::SIZE_TOLERANCE`]: the projection is then the closest-point map within that
/// tolerance, and the walls lean off the face's normals by at most the matching angle. Inside the
/// outlines the face is not checked.
#[allow(clippy::too_many_arguments)]
fn face_on_curved(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool) -> Result<Body> {
    let mid = ops::middle(regions)?;
    let (_, line, start) = ops::line_under(plane, mid, at)?;
    let (laid, anchor, normal, stretch) = guard("emboss normal to face", || {
        let anchor = body.face_line_hit(face, start, line)?.ok_or(EmbossRefusal::MiddleOffFace)?;
        let normal = body.face_normal_at(face, anchor)?.ok_or(EmbossRefusal::NoNormal)?;
        let laid = ops::lay_on_normal(plane, mid, anchor, normal)?;
        let mut normals = Vec::new();
        for w in ops::outline_samples(&laid, regions) {
            let Some(on) = body.face_line_hit(face, w, normal)? else {
                if stop {
                    continue;
                }
                return Err(EmbossRefusal::RunsOffFace { profiles: Vec::new(), of: regions.len() }.into());
            };
            normals.push(body.face_normal_at(face, on)?.ok_or(EmbossRefusal::NoNormal)?);
        }
        let stretch = ops::curved_stretch(normal, &normals)?;
        Ok((laid, anchor, normal, stretch))
    })?;
    if stop {
        let sides = ops::curved_sides(anchor, normal, regions, mid, (stretch, depth, tol_of(body)));
        return prisms_to_edge(body, face, &laid, regions, depth, &sides);
    }
    emboss_project_face(body, face, anchor, &laid, regions, depth).map_err(|e| match e {
        KernelError::Failed(why) => EmbossRefusal::NotOffset(why).into(),
        other => other,
    })
}

/// The slab over face `face` of `body`, or over the wall it is part of (the faces of its cylinder
/// joined to it across seams): between its offsets `lo` and `hi`, its sides along the faces'
/// normals at their edges. Each face's slab is chosen by its kind first: a whole sphere (its
/// edges only its seam) takes the shell between the offsets' radii and a whole-turn wall face the
/// ring between them along its height, neither of which the kernel thickens; any other face is
/// offset and thickened, and refused by name where it cannot be.
fn cell(body: &Body, rec: &Brep, face: usize, (lo, hi): (f64, f64)) -> Result<Body> {
    let tol = tol_of(body);
    let mut faces = vec![face];
    if let FaceKind::Cylinder { origin, axis, radius, convex } = kind_of(rec, face)? {
        let wall = (origin.into(), axis.into(), radius, convex);
        let same: Vec<usize> = (0..rec.faces.len())
            .filter(|f| {
                *f != face
                    && matches!(surf_of(rec, *f), Some(Surf::Cylinder { o, a, r, convex: c })
                        if solidcore_ops::lay_frame::same_cylinder(wall, (o.into(), a.into(), r, c), tol))
            })
            .collect();
        let mut i = 0;
        while let Some(&f) = faces.get(i) {
            let edges = face_edges(rec, f);
            for g in &same {
                if !faces.contains(g) && face_edges(rec, *g).iter().any(|e| edges.contains(e)) {
                    faces.push(*g);
                }
            }
            i += 1;
        }
    }
    let mut whole: Option<Body> = None;
    for f in faces {
        let kind = kind_of(rec, f)?;
        let turn = match kind {
            FaceKind::Cylinder { origin, axis, .. } => whole_turn(rec, f, origin, axis)?,
            _ => None,
        };
        let slab = match (kind, turn) {
            (FaceKind::Sphere { centre, radius, convex }, _) if seams_only(rec, f) => {
                let (r_in, r_out) = ops::offset_radii(radius, (lo, hi), convex);
                let outer = crate::sphere(centre, r_out)?;
                if r_in > 1e-6 { crate::boolean(&outer, &crate::sphere(centre, r_in)?, BoolOp::Cut)?.ok_or_else(|| failed("shell"))? } else { outer }
            }
            (FaceKind::Cylinder { axis, radius, convex, .. }, Some((base, length))) => {
                let (r_in, r_out) = ops::offset_radii(radius, (lo, hi), convex);
                ring(base, axis, r_in, r_out, length)?
            }
            _ => crate::surfaces::offset_faces(body, &[f], lo, tol)
                .and_then(|sheet| crate::thicken(&sheet, ops::slab_thickness((lo, hi)), false))
                .map_err(|e| EmbossRefusal::NotOffset(e.to_string()))?,
        };
        whole = Some(match whole {
            None => slab,
            Some(w) => crate::boolean(&w, &slab, BoolOp::Union)?.ok_or_else(|| failed("the slab over the wall"))?,
        });
    }
    whole.ok_or_else(|| EmbossRefusal::NoSuchFace.into())
}

/// A wall face `f` of the cylinder about `axis` through `o` that goes the whole way round: the
/// base and length of the ring over it. `None` for a part of a turn.
fn whole_turn(rec: &Brep, f: usize, o: Vec3, axis: Vec3) -> Result<Option<(Vec3, f64)>> {
    let Some((x, y)) = ops::axes_about(axis) else { return Ok(None) };
    let Some(angles) = wall_angles(rec, f, o, axis, x, y) else { return Ok(None) };
    let Some(heights) = face_axial_bounds(rec, f, o, axis)? else { return Ok(None) };
    Ok(ops::whole_turn_ring(o, axis, angles, heights))
}

/// Whether every edge of face `f` is a seam: used twice by the face itself.
fn seams_only(rec: &Brep, f: usize) -> bool {
    let Some(face) = rec.faces.get(f) else { return false };
    let mut uses: Vec<u32> = face
        .loops
        .iter()
        .flat_map(|l| match l {
            solidcore_record::brep::Loop::Edges { edges } => edges.iter().map(|u| u.edge).collect(),
            solidcore_record::brep::Loop::Vertex { .. } => Vec::new(),
        })
        .collect();
    uses.sort_unstable();
    uses.chunks(2).all(|pair| matches!(pair, [a, b] if a == b)) && !uses.windows(3).any(|w| matches!(w, [a, b, c] if a == b && b == c))
}

/// The solid between radii `r_in` and `r_out` about `axis` from `base`, `length` long.
fn ring(base: Vec3, axis: Vec3, r_in: f64, r_out: f64, length: f64) -> Result<Body> {
    let outer = crate::cylinder(base, axis, r_out, length)?;
    if r_in <= 1e-6 {
        return Ok(outer);
    }
    crate::boolean(&outer, &crate::cylinder(base, axis, r_in, length)?, BoolOp::Cut)?.ok_or_else(|| failed("ring"))
}

/// The part of `tools` over the face, kept on the side of each of `sides` (a point on a plane and
/// the side kept), joined to (`depth > 0`) or cut from the body.
#[allow(clippy::too_many_arguments)]
fn keep_over(body: &Body, rec: &Brep, face: usize, tools: Vec<Body>, depth: f64, exact: bool, sides: &[(Vec3, Vec3)]) -> Result<Body> {
    let mut over = cell(body, rec, face, ops::slab_reach(depth, exact))?;
    for &(o, s) in sides {
        let half = half_space(body, o, s)?;
        over = crate::boolean(&over, &half, BoolOp::Intersect)?.ok_or(EmbossRefusal::NothingOnFace)?;
    }
    let mut pieces = Vec::new();
    for tool in &tools {
        if let Some(piece) = crate::boolean(tool, &over, BoolOp::Intersect)? {
            pieces.push(piece);
        }
    }
    if pieces.is_empty() {
        return Err(EmbossRefusal::NothingOnFace.into());
    }
    apply_pieces(body, pieces, depth)
}

/// A box past the body on the side of the plane through `o` that `s` points to.
fn half_space(body: &Body, o: Vec3, s: Vec3) -> Result<Body> {
    let b = body.exact_bounds()?;
    let (base, reach) = ops::half_space(b.min, b.max, o, s)?;
    let square = Region2 {
        outer: Loop2::polygon(&[Vec2::new(-reach, -reach), Vec2::new(reach, -reach), Vec2::new(reach, reach), Vec2::new(-reach, reach)]),
        holes: vec![],
    };
    crate::extrude(&base, &[square], 0.0, reach)?.into_iter().next().ok_or_else(|| failed("half space"))
}

/// The profiles' prisms along `plane`'s normal, through the body, kept over the face.
fn prisms_to_edge(body: &Body, face: usize, plane: &Plane, regions: &[Region2], depth: f64, sides: &[(Vec3, Vec3)]) -> Result<Body> {
    guard("emboss to the face edge", || {
        let rec = record(body)?;
        let b = body.exact_bounds()?;
        let (start, end) = ops::through_body(b.min, b.max, plane, depth)?;
        let prisms = crate::extrude(plane, regions, start, end)?;
        keep_over(body, &rec, face, prisms, depth, false, sides)
    })
}

/// The wrap from `plane`, touching the wall (`o`, `a`, `r`, `convex`) along the line `frame`,
/// built on a whole wall of its radius and kept over the picked face's wall.
fn wrap_to_edge(
    body: &Body,
    face: usize,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    (o, a, r, convex): (Vec3, Vec3, f64, bool),
    frame: &WallFrame,
) -> Result<Body> {
    guard("emboss to the face edge", || {
        let rec = record(body)?;
        // The whole wall's seam is put opposite the line the sketch touches, where the wrap
        // starts, so a profile meets no seam the picked wall does not have.
        let (seat, length) = ops::whole_wall(plane, regions, (o, a, convex), frame, depth)?;
        let disc = |radius: f64| -> Result<Body> {
            let circle = Region2 {
                outer: Loop2 {
                    segs: vec![Seg2::Arc { center: Vec2::new(0.0, 0.0), radius, start: std::f64::consts::PI, sweep: std::f64::consts::TAU }],
                },
                holes: vec![],
            };
            crate::extrude(&seat, &[circle], 0.0, length)?.into_iter().next().ok_or_else(|| failed("wall"))
        };
        let whole = if convex {
            disc(r)?
        } else {
            crate::boolean(&disc(ops::bore_reach(r, depth))?, &disc(r)?, BoolOp::Cut)?.ok_or_else(|| failed("wall"))?
        };
        let wrapped = emboss_cylinder(&whole, plane, regions, depth)?;
        let tool = if depth > 0.0 { crate::boolean(&wrapped, &whole, BoolOp::Cut)? } else { crate::boolean(&whole, &wrapped, BoolOp::Cut)? };
        let tool = tool.ok_or_else(|| failed("the profiles"))?;
        keep_over(body, &rec, face, vec![tool], depth, true, &[])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcore_record::Loop2;

    fn volume(b: &Body) -> f64 {
        crate::measure(b).unwrap().volume
    }
    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Region2 {
        Region2 { outer: Loop2::polygon(&[Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]), holes: vec![] }
    }
    fn face_where(body: &Body, which: impl Fn(Surf) -> bool) -> usize {
        let rec = record(body).unwrap();
        (0..rec.faces.len()).find(|f| surf_of(&rec, *f).is_some_and(&which)).unwrap()
    }
    /// A cylinder of radius 10 about z, 20 tall from z = 0.
    fn post() -> Body {
        crate::cylinder(Vec3::ZERO, Vec3::Z, 10., 20.).unwrap()
    }
    /// The plane y = `y` seen from +y, its x along -x and its y up.
    fn facing_y(y: f64) -> Plane {
        Plane::new(Vec3::Y * y, -Vec3::X, Vec3::Z).unwrap()
    }
    fn wall(body: &Body) -> usize {
        face_where(body, |s| matches!(s, Surf::Cylinder { .. }))
    }

    #[test]
    fn normal_to_face_wraps_a_width_into_the_same_arc_with_even_depth_and_radial_walls() {
        let body = post();
        let before = volume(&body);
        let w = wall(&body);
        // 15 wide and 5 tall, from a sketch 20 off the wall: projected, it would hold its width
        // as a chord; wrapped it is 1.5 rad of arc.
        let profile = rect(-7.5, 7.5, 7.5, 12.5);
        for (depth, y) in [(1., 30.), (-1., 30.), (1., 0.)] {
            let result =
                emboss_on_face(&body, w, Vec3::new(0., 10., 10.), &facing_y(y), std::slice::from_ref(&profile), depth, EmbossDirection::Face, false)
                    .unwrap();
            assert_eq!(result.shell_count(), 1);
            let rr: f64 = 10. + depth;
            let want = 0.5 * 1.5 * (rr * rr - 100.) * 5.;
            assert!((volume(&result) - before - want).abs() < 1e-6, "depth {depth} from y {y}: {} vs {want}", volume(&result) - before);
            if depth > 0. {
                // The raised part ends at 0.75 rad either side of +y, at both radii: its walls run
                // along the radius.
                let raised = crate::boolean(&result, &body, BoolOp::Cut).unwrap().unwrap();
                let b = raised.exact_bounds().unwrap();
                assert!((b.max.x - 11. * 0.75f64.sin()).abs() < 1e-6, "{b:?}");
                assert!((b.min.y - 10. * 0.75f64.cos()).abs() < 1e-6, "{b:?}");
                assert!((b.max.y - 11.).abs() < 1e-6 && (b.min.z - 7.5).abs() < 1e-6 && (b.max.z - 12.5).abs() < 1e-6, "{b:?}");
            }
        }
    }

    #[test]
    fn normal_to_sketch_is_the_emboss_as_before() {
        let body = post();
        let w = wall(&body);
        let profile = rect(-3., 7.5, 3., 12.5);
        let at = Vec3::new(0., 10., 10.);
        for plane in [facing_y(30.), facing_y(10.)] {
            let new = emboss_on_face(&body, w, at, &plane, std::slice::from_ref(&profile), 1., EmbossDirection::Sketch, false).unwrap();
            let old = match cylinder_emboss_anchor(&body, w, at) {
                Ok(on) if infer_cylinder_emboss(&body, &plane, std::slice::from_ref(&profile), 1.).unwrap().is_some() => {
                    emboss_cylinder_at(&body, on, &plane, std::slice::from_ref(&profile), 1.)
                }
                _ => emboss_project_face(&body, w, emboss_anchor(&body, w, at).unwrap(), &plane, std::slice::from_ref(&profile), 1.),
            }
            .unwrap();
            assert_eq!(volume(&new), volume(&old));
            assert_eq!(new.face_count(), old.face_count());
        }
    }

    #[test]
    fn the_stop_at_the_edge_trims_a_wrap_running_off_the_wall() {
        let body = post();
        let before = volume(&body);
        let w = wall(&body);
        // From z = 15 to 25 on a wall 20 tall: half of it lies on the wall.
        let profile = rect(-7.5, 15., 7.5, 25.);
        let at = Vec3::new(0., 10., 10.);
        for direction in [EmbossDirection::Face, EmbossDirection::Sketch] {
            let e = emboss_on_face(&body, w, at, &facing_y(10.), std::slice::from_ref(&profile), 1., direction, false).map(|_| ()).unwrap_err();
            assert!(e.to_string().contains("endcap"), "{direction:?}: {e}");
            for depth in [1., -1.] {
                let result = emboss_on_face(&body, w, at, &facing_y(10.), std::slice::from_ref(&profile), depth, direction, true).unwrap();
                let rr: f64 = 10. + depth;
                let want = 0.5 * 1.5 * (rr * rr - 100.) * 5.;
                assert!((volume(&result) - before - want).abs() < 1e-6, "{direction:?} {depth}: {} vs {want}", volume(&result) - before);
            }
        }
    }

    #[test]
    fn several_tangent_faces_take_the_wrap_and_the_stop_keeps_the_picked_one() {
        let body = super::super::tests::round_ended(15.);
        let before = volume(&body);
        // The flat face y = 10, seen from +y; the profile runs from x = 5 on it round onto the
        // wall past x = 0, 6 tall.
        let flat = face_where(&body, |s| matches!(s, Surf::Plane { n, .. } if n.y > 0.5));
        let sketch = Plane::new(Vec3::new(0., 25., 10.), -Vec3::X, Vec3::Z).unwrap();
        let profile = rect(-5., -3., 4., 3.);
        let at = Vec3::new(5., 10., 10.);
        let wrapped = emboss_on_face(&body, flat, at, &sketch, std::slice::from_ref(&profile), 0.5, EmbossDirection::Face, false).unwrap();
        let (on_plane, on_wall) = (5. * 6., 4. * 6.);
        let want = 0.5 * on_plane + on_wall * (10.5f64 * 10.5 - 100.) / 20.;
        assert!((volume(&wrapped) - before - want).abs() < 1e-6, "{} vs {want}", volume(&wrapped) - before);
        let stopped = emboss_on_face(&body, flat, at, &sketch, std::slice::from_ref(&profile), 0.5, EmbossDirection::Face, true).unwrap();
        assert!((volume(&stopped) - before - 0.5 * on_plane).abs() < 1e-6, "{}", volume(&stopped) - before);
    }

    #[test]
    fn normal_to_face_on_a_sphere_is_the_cone_from_its_centre_within_the_size_tolerance() {
        let ball = crate::sphere(Vec3::ZERO, 10.).unwrap();
        let before = volume(&ball);
        let face = face_where(&ball, |s| matches!(s, Surf::Sphere { .. }));
        let at = Vec3::new(0., 10., 0.);
        // A 4 x 2 rectangle on the tangent plane at radius 10, raised 1: its solid angle from the
        // centre times (11³ - 10³) / 3.
        let solid_angle = 4. * (8. / ((16f64 + 400.) * (4. + 400.)).sqrt()).asin();
        for (depth, shell) in [(1., 1331. - 1000.), (-1., 729. - 1000.)] {
            let result = emboss_on_face(&ball, face, at, &facing_y(20.), &[rect(-2., -1., 2., 1.)], depth, EmbossDirection::Face, false).unwrap();
            let want = solid_angle * shell / 3.;
            assert!((volume(&result) - before - want).abs() < 1e-6, "{depth}: {} vs {want}", volume(&result) - before);
        }
        // 10 x 10 reaches 7.07 from its middle, where a length would shrink by a third.
        let e = emboss_on_face(&ball, face, at, &facing_y(20.), &[rect(-5., -5., 5., 5.)], 1., EmbossDirection::Face, false).map(|_| ()).unwrap_err();
        assert!(e.to_string().contains("within 10% only within 3.33 mm"), "{e}");
        let e =
            emboss_on_face(&ball, face, at, &facing_y(20.), &[rect(-1., -1., 1., 1.)], -10.5, EmbossDirection::Face, false).map(|_| ()).unwrap_err();
        assert!(e.to_string().contains("reaches the sphere's centre"), "{e}");
    }

    #[test]
    fn normal_to_face_on_a_cone_keeps_sizes_within_the_tolerance_or_refuses() {
        // A cone from radius 10 at z = 0 to 5 at z = 10: at z = 5 its wall is 7.5 out.
        let circle = |r: f64| Loop2 { segs: vec![Seg2::Arc { center: Vec2::new(0., 0.), radius: r, start: 0., sweep: std::f64::consts::TAU }] };
        let cone = crate::loft(&[(Plane::XY, circle(10.)), (Plane::new(Vec3::Z * 10., Vec3::X, Vec3::Y).unwrap(), circle(5.))]).unwrap();
        let before = volume(&cone);
        let at = Vec3::new(0., 7.5, 5.);
        let side = (0..cone.face_count())
            .min_by(|a, b| {
                let gap = |f: &usize| crate::point_closest(at, &cone, crate::Members::Faces(&[*f])).unwrap().map_or(f64::INFINITY, |h| h.distance);
                gap(a).total_cmp(&gap(b))
            })
            .unwrap();
        let result = emboss_on_face(&cone, side, at, &facing_y(20.), &[rect(-1., 4., 1., 6.)], 0.5, EmbossDirection::Face, false).unwrap();
        assert_eq!(result.shell_count(), 1);
        // 2 x 2 raised 0.5, within the tolerance either way.
        let added = volume(&result) - before;
        assert!((added - 2.).abs() < 0.2, "{added}");
        let stopped = emboss_on_face(&cone, side, at, &facing_y(20.), &[rect(-1., 4., 1., 6.)], 0.5, EmbossDirection::Face, true).unwrap();
        assert!((volume(&stopped) - before - added).abs() < 1e-6, "{}", volume(&stopped) - before);
        // From a sketch through the axis the pick says which side: the same point under the
        // middle, the same emboss.
        let through = emboss_on_face(&cone, side, at, &facing_y(0.), &[rect(-1., 4., 1., 6.)], 0.5, EmbossDirection::Face, false).unwrap();
        assert!((volume(&through) - volume(&result)).abs() < 1e-6);
        // 10 wide, it reaches round to where the wall leans 45° off the middle's.
        let e = emboss_on_face(&cone, side, at, &facing_y(20.), &[rect(-5., 3., 5., 7.)], 0.5, EmbossDirection::Face, false).map(|_| ()).unwrap_err();
        assert!(e.to_string().contains("curves too much"), "{e}");
    }

    #[test]
    fn several_separate_faces_each_take_the_profiles_over_them() {
        let block = crate::box_solid(Vec3::ZERO, Vec3::new(40., 40., 20.)).unwrap();
        let before = volume(&block);
        let top = face_where(&block, |s| matches!(s, Surf::Plane { n, .. } if n.z > 0.5));
        let front = face_where(&block, |s| matches!(s, Surf::Plane { n, .. } if n.y < -0.5));
        let faces = [(top, Vec3::new(20., 20., 20.)), (front, Vec3::new(20., 0., 10.))];
        // A sketch leaning 45° over the edge between them: the square 10 up it lies over the top,
        // the one 10 down over the front.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let sketch = Plane::new(Vec3::new(20., -5., 25.), Vec3::X, Vec3::new(0., s, s)).unwrap();
        let squares = [rect(-1., 9., 1., 11.), rect(-1., -11., 1., -9.)];
        // Normal to face: each 2 x 2 laid on its face and raised 1; Normal to sketch: each
        // projected, so stretched by √2 one way.
        for (direction, want) in [(EmbossDirection::Face, 8.), (EmbossDirection::Sketch, 8. * std::f64::consts::SQRT_2)] {
            let result = emboss_on_faces(&block, &faces, &sketch, &squares, 1., direction, false).unwrap();
            assert_eq!(result.shell_count(), 1);
            assert!((volume(&result) - before - want).abs() < 1e-6, "{direction:?}: {} vs {want}", volume(&result) - before);
        }
        let e = emboss_on_faces(&block, &faces, &sketch, &[rect(30., -1., 32., 1.)], 1., EmbossDirection::Face, false).map(|_| ()).unwrap_err();
        assert!(e.to_string().contains("over none of the target faces"), "{e}");
    }

    #[test]
    fn the_stop_on_a_sphere_marks_only_the_side_picked() {
        let ball = crate::sphere(Vec3::ZERO, 10.).unwrap();
        let face = face_where(&ball, |s| matches!(s, Surf::Sphere { .. }));
        let result =
            emboss_on_face(&ball, face, Vec3::new(0., 10., 0.), &facing_y(20.), &[rect(-2., -2., 2., 2.)], 1., EmbossDirection::Sketch, true)
                .unwrap();
        let b = result.exact_bounds().unwrap();
        assert!(b.max.y > 10.9 && (b.min.y + 10.).abs() < 1e-6, "{b:?}");
    }

    #[test]
    fn the_stop_keeps_to_the_picked_wall_past_a_groove_in_it() {
        // Two walls of radius 10, z 0 to 8 and 12 to 20, either side of a groove 1 deep.
        let grooved = crate::boolean(&post(), &ring(Vec3::Z * 8., Vec3::Z, 9., 11., 4.).unwrap(), BoolOp::Cut).unwrap().unwrap();
        let before = volume(&grooved);
        let lower = face_where(&grooved, |s| matches!(s, Surf::Cylinder { r, .. } if (r - 10.).abs() < 1e-9));
        let rec = record(&grooved).unwrap();
        let at = match face_axial_bounds(&rec, lower, Vec3::ZERO, Vec3::Z).unwrap() {
            Some((lo, _)) if lo > 6. => Vec3::new(0., 10., 16.),
            _ => Vec3::new(0., 10., 4.),
        };
        let result = emboss_on_face(&grooved, lower, at, &facing_y(10.), &[rect(-2., 6., 2., 14.)], 1., EmbossDirection::Face, true).unwrap();
        // 0.4 rad of the ring from radius 10 to 11, 2 tall: only the part on the picked wall.
        let want = 0.5 * 0.4 * 21. * 2.;
        assert!((volume(&result) - before - want).abs() < 1e-6, "{} vs {want}", volume(&result) - before);
    }

    #[test]
    fn the_stop_refuses_a_sketch_facing_into_an_outside_wall_as_the_wrap_does() {
        let body = post();
        let inward = Plane::new(Vec3::Y * 10., Vec3::X, Vec3::Z).unwrap();
        for stop in [false, true] {
            let e =
                emboss_on_face(&body, wall(&body), Vec3::new(0., 10., 10.), &inward, &[rect(-2., 8., 2., 12.)], 1., EmbossDirection::Sketch, stop)
                    .map(|_| ())
                    .unwrap_err();
            assert!(e.to_string().contains("outwards"), "stop {stop}: {e}");
        }
    }
}
