//! Emboss Direction (#21423): Normal to sketch, the emboss as it has always been built, or Normal
//! to face, where the profiles are laid on the face keeping their size and raised along its
//! normal; and Stop at face edge, which trims either at the edge of the picked face.
//!
//! Normal to face lays the sketch's frame on the face at the point under the middle of the
//! profiles (solidcore's `lay_frame`), then builds what the face takes from that frame: the
//! exact wrap on a cylinder (radial walls, even depth, arc length kept, carried on onto tangent
//! planes) or the projection on a plane, which is then exact.
//!
//! Stop at face edge builds each profile's solid without the body's trims (the wrap on a whole
//! cylinder of the wall's radius, or the profiles' prisms) and keeps what lies over the picked
//! wall or face: the slab between offsets of its faces, whose sides run along their normals at
//! their edges.

use super::{
    apply_pieces, cylinder_emboss_anchor, emboss_anchor, emboss_cylinder, emboss_cylinder_at, emboss_project_face, face_axial_bounds,
    infer_cylinder_emboss, record, refuse, span, wall_frame,
};
use crate::body::Body;
use crate::offset::{Surf, face_edges, surf_of};
use crate::{BoolOp, Result, guard};
use solidcore_ops::lay_frame::{LaySurface, lay_frame};
use solidcore_record::brep::Brep;
use solidcore_record::{Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_geom::prelude::*;

/// Which way an emboss raises its profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmbossDirection {
    /// Along the sketch's normal, the profiles keeping their size as projected.
    #[default]
    Sketch,
    /// Along the face's normal, the profiles keeping their size on the face.
    Face,
}

/// How much Normal to face lets a length change on a face that does not unroll (a sphere, cone,
/// torus or freeform face): a tenth.
pub const SIZE_TOLERANCE: f64 = 0.10;

const MIDDLE_OFF: &str = "the middle of the profiles is not over the target face along the sketch's normal: move the profiles over the face";

/// The farthest point of the profiles' outlines from `mid`.
fn reach(regions: &[Region2], mid: Vec2) -> f64 {
    regions.iter().flat_map(|r| r.outer.polyline(1e-4)).map(|p| (p - mid).len()).fold(0.0, f64::max)
}

/// Normal to face on a sphere, by the closest point: the tangent plane at the point under the
/// middle of the profiles is laid towards the centre, so each profile's solid is the cone over it
/// from the centre, its walls along the sphere's normals, kept between the offsets of the face.
/// Exact; lengths shrink away from the middle, and profiles reaching so far that one would shrink
/// by more than [`SIZE_TOLERANCE`] are refused.
#[allow(clippy::too_many_arguments)]
fn on_sphere(
    body: &Body,
    face: usize,
    at: Vec3,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    stop: bool,
    (c, r, convex): (Vec3, f64, bool),
) -> Result<Body> {
    guard("emboss normal to face", || {
        let rec = record(body)?;
        let mid = middle(regions).ok_or_else(|| refuse("profile"))?;
        let surface = LaySurface::Sphere { centre: super::arr(c), radius: r };
        let f = lay_frame(super::arr(plane.origin), super::arr(plane.x), super::arr(plane.y), (mid.x, mid.y), surface, super::arr(at))
            .ok_or_else(|| refuse(MIDDLE_OFF))?;
        if convex && !f.facing {
            return Err(refuse("the sketch faces away from this side of the sphere: pick the side the sketch faces, or flip the sketch"));
        }
        let far = reach(regions, mid);
        if solidcore_ops::lay_frame::central_shrink(far, r) > SIZE_TOLERANCE {
            let most = solidcore_ops::lay_frame::central_reach(r, SIZE_TOLERANCE);
            return Err(refuse(&format!(
                "on this sphere a profile keeps its size within 10% only within {most:.2} mm of the middle of the profiles, and they reach {far:.2} mm: make them smaller, or use Normal to sketch"
            )));
        }
        let (lo, hi) = slab_reach(depth, false);
        // The cones' bases lie past the outer offset, on the tangent plane scaled about the centre.
        let k = (r + lo.abs().max(hi.abs()) + 1.0) / r;
        let laid_origin = Vec3::from(f.origin);
        let base = Plane::new(c + (laid_origin - c) * k, Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))?;
        let scaled = |lp: &solidcore_record::Loop2| solidcore_record::Loop2 {
            segs: lp
                .segs
                .iter()
                .map(|s| match *s {
                    Seg2::Arc { center, radius, start, sweep } => Seg2::Arc { center: center * k, radius: radius * k, start, sweep },
                    other => other.map_controls(|p| p * k),
                })
                .collect(),
        };
        let over = cell(body, &rec, face, (lo, hi))?;
        let (r_in, r_out) = if convex { (r + lo, r + hi) } else { (r - hi, r - lo) };
        let shell = crate::boolean(&crate::sphere(c, r_out)?, &crate::sphere(c, r_in)?, BoolOp::Cut)?.ok_or_else(|| refuse("shell"))?;
        let mut pieces = Vec::new();
        let mut off = Vec::new();
        for (i, reg) in regions.iter().enumerate() {
            let mut cone = crate::loft_to_point(&base, &scaled(&reg.outer), c)?;
            for hole in &reg.holes {
                cone = crate::boolean(&cone, &crate::loft_to_point(&base, &scaled(hole), c)?, BoolOp::Cut)?.ok_or_else(|| refuse("profile"))?;
            }
            let piece = crate::boolean(&cone, &over, BoolOp::Intersect)?;
            if !stop {
                let whole = crate::boolean(&cone, &shell, BoolOp::Intersect)?;
                let size = |b: &Option<Body>| b.as_ref().map_or(Ok(0.0), |b| crate::measure(b).map(|m| m.volume));
                let (kept, full) = (size(&piece)?, size(&whole)?);
                if (full - kept).abs() > 1e-9 * (1.0 + full.abs()) {
                    off.push((i + 1).to_string());
                    continue;
                }
            }
            if let Some(piece) = piece {
                pieces.push(piece);
            }
        }
        if !off.is_empty() {
            return Err(refuse(&format!(
                "profile {} of {} runs off the target face: move it onto the face, or turn on Stop at face edge to trim it",
                off.join(", "),
                regions.len()
            )));
        }
        if pieces.is_empty() {
            return Err(refuse("no part of the profiles lies on the target face"));
        }
        apply_pieces(body, pieces, depth)
    })
}

/// Normal to face on a cone, torus or freeform face: the middle of the profiles goes to the
/// point of the face nearest it, the sketch's frame is laid on the face's tangent plane there,
/// and the profiles are projected along that normal onto the face, raised to the offset of the
/// face at the depth. Over the profiles' outlines the face's normal must stay close enough to
/// that one that no length changes by more than [`SIZE_TOLERANCE`]; then the projection is the
/// closest-point map within that tolerance, and the walls lean off the face's normals by at most
/// the matching angle.
fn near_face(body: &Body, face: usize, plane: &Plane, regions: &[Region2], depth: f64, stop: bool) -> Result<Body> {
    let mid = middle(regions).ok_or_else(|| refuse("profile"))?;
    let faces = [face];
    let closest = |p: Vec3| -> Result<Vec3> {
        Ok(crate::point_closest(p, body, crate::Members::Faces(&faces))?.ok_or_else(|| refuse("the target face has no exact form"))?.to)
    };
    let anchor = closest(plane.to_world(mid))?;
    let normal = body.face_normal_at(face, anchor)?.ok_or_else(|| refuse("the target face has no normal under the middle of the profiles"))?;
    let f = solidcore_ops::lay_frame::lay_frame_at(super::arr(plane.x), super::arr(plane.y), (mid.x, mid.y), super::arr(anchor), super::arr(normal))
        .ok_or_else(|| refuse("laid sketch frame"))?;
    if !f.facing {
        return Err(refuse("the sketch faces away from this side of the face: pick the side the sketch faces, or flip the sketch"));
    }
    let laid = Plane::new(Vec3::from(f.origin), Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))?;
    let outline: Vec<Vec2> = regions.iter().flat_map(|r| r.outer.polyline(1e-3)).collect();
    let step = outline.len() / 400 + 1;
    let mut normals = Vec::new();
    for p in outline.iter().step_by(step) {
        let on = closest(laid.to_world(*p))?;
        if let Some(n) = body.face_normal_at(face, on)? {
            normals.push(super::arr(n));
        }
    }
    let stretch = solidcore_ops::lay_frame::projected_stretch(super::arr(normal), &normals);
    if stretch > SIZE_TOLERANCE {
        return Err(refuse(
            "this face curves too much under the profiles for them to keep their size within 10%: make them smaller, or use Normal to sketch",
        ));
    }
    if stop {
        // Past the face under the profiles, on the far side: the face sags off its tangent plane
        // by at most the reach times the tangent of the largest lean.
        let lean = ((1.0 + stretch).powi(2) - 1.0).max(0.0).sqrt();
        let (lo, hi) = slab_reach(depth, false);
        let below = reach(regions, mid) * lean + lo.abs().max(hi.abs()) + 1.0;
        return prisms_to_edge(body, face, &laid, regions, depth, Some((anchor - normal * below, normal)));
    }
    emboss_project_face(body, face, anchor, &laid, regions, depth)
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
    match (direction, stop_at_edge) {
        (EmbossDirection::Sketch, false) => normal_to_sketch(body, face, at, plane, regions, depth),
        (EmbossDirection::Sketch, true) => sketch_to_edge(body, face, at, plane, regions, depth),
        (EmbossDirection::Face, _) => normal_to_face(body, face, at, plane, regions, depth, stop_at_edge),
    }
}

/// [`emboss_on_face`] onto several target faces of `body` (`faces`: each face and a pick on it):
/// each profile goes on the target face nearest the middle of its bounds on the sketch, and is
/// built there as on that face alone. Each face's emboss is built on the body as given and their
/// material is joined (or cut) together, so one face's change does not move another's.
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
        return Err(refuse("needs at least one target face"));
    };
    if faces.len() == 1 {
        return emboss_on_face(body, face, at, plane, regions, depth, direction, stop_at_edge);
    }
    if faces.len() > 64 {
        return Err(refuse("at most 64 target faces"));
    }
    body.require_brep("emboss")?;
    checked(depth, regions)?;
    let rec = guard("emboss onto several faces", || record(body))?;
    let tol = (body.size() * 1e-7).max(1e-7);
    let mut groups: Vec<Vec<Region2>> = vec![Vec::new(); faces.len()];
    for region in regions {
        let mid = middle(std::slice::from_ref(region)).ok_or_else(|| refuse("profile"))?;
        let p = plane.to_world(mid);
        let mut best: Option<(f64, usize)> = None;
        for (k, (f, _)) in faces.iter().enumerate() {
            if let Some(d) = under(body, &rec, *f, plane, mid, p, tol)?
                && best.is_none_or(|(b, _)| d < b)
            {
                best = Some((d, k));
            }
        }
        let (_, k) = best.ok_or_else(|| refuse("a profile lies over none of the target faces along the sketch's normal: move it over one"))?;
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
        result = crate::boolean(&result, &change, op)?.ok_or_else(|| refuse("the profiles would leave nothing of the body"))?;
    }
    Ok(result)
}

/// How far the sketch point `p` (`mid` on `plane`) is from face `face` along the sketch's normal:
/// on a plane, cylinder or sphere, from the point of the face's surface under it along that line
/// (the nearer of two), when that point lies on the face; on any other face, from the face's
/// nearest point. `None` where the line misses the face.
fn under(body: &Body, rec: &Brep, face: usize, plane: &Plane, mid: Vec2, p: Vec3, tol: f64) -> Result<Option<f64>> {
    let surface = match surf_of(rec, face) {
        Some(Surf::Plane { n, d }) => LaySurface::Plane { point: super::arr(n * d), normal: super::arr(n) },
        Some(Surf::Cylinder { o, a, r, .. }) => LaySurface::Cylinder { origin: super::arr(o), axis: super::arr(a), radius: r },
        Some(Surf::Sphere { c, r, .. }) => LaySurface::Sphere { centre: super::arr(c), radius: r },
        _ => {
            return Ok(crate::point_closest(p, body, crate::Members::Faces(&[face]))?.map(|hit| hit.distance));
        }
    };
    let Some(f) = lay_frame(super::arr(plane.origin), super::arr(plane.x), super::arr(plane.y), (mid.x, mid.y), surface, super::arr(p)) else {
        return Ok(None);
    };
    let at = Vec3::from(f.at);
    let on = crate::point_closest(at, body, crate::Members::Faces(&[face]))?.is_some_and(|hit| hit.distance <= tol);
    Ok(on.then(|| (at - p).len()))
}

/// A sketch tangent to a cylinder wall, outside or a bore, takes the wrap (whose refusals stand);
/// a sketch off the wall and every other face are projected.
fn normal_to_sketch(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64) -> Result<Body> {
    match cylinder_emboss_anchor(body, face, at) {
        Ok(on) if infer_cylinder_emboss(body, plane, regions, depth)?.is_some() => emboss_cylinder_at(body, on, plane, regions, depth),
        _ => emboss_project_face(body, face, emboss_anchor(body, face, at)?, plane, regions, depth),
    }
}

fn checked(depth: f64, regions: &[Region2]) -> Result<()> {
    if !depth.is_finite() || depth.abs() < 1e-6 || depth.abs() > 1e6 || regions.is_empty() || regions.len() > 4096 {
        return Err(refuse("needs finite nonzero depth and 1..4096 profiles"));
    }
    Ok(())
}

fn face_surf(rec: &Brep, face: usize) -> Result<Option<Surf>> {
    if face >= rec.faces.len() {
        return Err(refuse("selected face"));
    }
    Ok(surf_of(rec, face))
}

fn normal_to_face(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool) -> Result<Body> {
    body.require_brep("emboss")?;
    checked(depth, regions)?;
    let surf = guard("emboss normal to face", || face_surf(&record(body)?, face))?;
    match surf {
        Some(Surf::Plane { .. } | Surf::Cylinder { .. }) => {}
        Some(Surf::Sphere { c, r, convex }) => return on_sphere(body, face, at, plane, regions, depth, stop, (c, r, convex)),
        _ => return near_face(body, face, plane, regions, depth, stop),
    }
    let (laid, surf) = guard("emboss normal to face", || {
        let rec = record(body)?;
        let surf = face_surf(&rec, face)?;
        let surface = match surf {
            Some(Surf::Plane { n, d }) => LaySurface::Plane { point: super::arr(n * d), normal: super::arr(n) },
            Some(Surf::Cylinder { o, a, r, .. }) => LaySurface::Cylinder { origin: super::arr(o), axis: super::arr(a), radius: r },
            _ => return Err(refuse("selected face")),
        };
        let mid = middle(regions).ok_or_else(|| refuse("profile"))?;
        let f = lay_frame(super::arr(plane.origin), super::arr(plane.x), super::arr(plane.y), (mid.x, mid.y), surface, super::arr(at))
            .ok_or_else(|| refuse(MIDDLE_OFF))?;
        if matches!(surf, Some(Surf::Cylinder { convex: true, .. })) && !f.facing {
            return Err(refuse("the sketch faces away from this side of the wall: pick the side of the wall the sketch faces, or flip the sketch"));
        }
        let laid = Plane::new(Vec3::from(f.origin), Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))?;
        Ok((laid, surf))
    })?;
    match (surf, stop) {
        (Some(Surf::Cylinder { o, a, r, convex }), true) => wrap_to_edge(body, face, &laid, regions, depth, (o, a, r, convex)),
        (Some(Surf::Cylinder { .. }), false) => emboss_cylinder_at(body, cylinder_emboss_anchor(body, face, at)?, &laid, regions, depth),
        (_, true) => prisms_to_edge(body, face, &laid, regions, depth, None),
        (_, false) => {
            // On a plane: projected, or, where the profiles run past it onto a wall tangent to
            // it, wrapped from the plane, which touches that wall (#5358).
            let projected = emboss_project_face(body, face, emboss_anchor(body, face, at)?, &laid, regions, depth);
            if projected.is_ok() {
                return projected;
            }
            match infer_cylinder_emboss(body, &laid, regions, depth) {
                Ok(Some((_, on))) => emboss_cylinder_at(body, on, &laid, regions, depth),
                _ => match rolled_onto_wall(body, face, &laid) {
                    Ok(Some((on, rolled))) => emboss_cylinder_at(body, on, &rolled, regions, depth),
                    _ => projected,
                },
            }
        }
    }
}

/// A plane tangent to an outside wall face along one of its straight edges (a flat face running
/// on from a round) touches it at its seam, where the wrap does not start. The frame is rolled
/// round the wall to the middle of that face, which lays every profile point where the plane
/// does: the frame and a pick on the face there. Only walls beside the picked `face`.
fn rolled_onto_wall(body: &Body, face: usize, laid: &Plane) -> Result<Option<(Vec3, Plane)>> {
    guard("emboss normal to face", || {
        let rec = record(body)?;
        let tol = (body.size() * 1e-8).max(1e-8);
        let mut found = None;
        let edges = face_edges(&rec, face);
        for f in (0..rec.faces.len()).filter(|f| *f != face && face_edges(&rec, *f).iter().any(|e| edges.contains(e))) {
            let Some(Surf::Cylinder { o, a, r, convex: true }) = surf_of(&rec, f) else { continue };
            let Some((radial, tangent)) = wall_frame(laid.normal(), laid.origin - o, a, r, true, tol) else { continue };
            let Some((lo, hi)) = super::wall_angles(&rec, f, o, a, radial, tangent) else { continue };
            let seam = tol * 10.0 / r;
            if lo.abs() > seam && hi.abs() > seam {
                continue;
            }
            let Some((z_lo, z_hi)) = face_axial_bounds(&rec, f, o, a)? else { continue };
            let turn = 0.5 * (lo + hi);
            let Some((origin, x, y)) = solidcore_ops::lay_frame::roll_frame(
                super::arr(laid.origin),
                super::arr(laid.x),
                super::arr(laid.y),
                super::arr(o),
                super::arr(a),
                r,
                turn,
            ) else {
                continue;
            };
            let Some(rolled) = Plane::new(Vec3::from(origin), Vec3::from(x), Vec3::from(y)) else { continue };
            let on = o + a * (0.5 * (z_lo + z_hi)) + (radial * turn.cos() + tangent * turn.sin()) * r;
            if found.is_some() {
                return Ok(None);
            }
            found = Some((on, rolled));
        }
        Ok(found)
    })
}

/// Normal to sketch, stopped at the face's edge: a sketch tangent to a cylinder wall wraps as
/// without the stop; any other sketch is projected straight along its normal.
fn sketch_to_edge(body: &Body, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64) -> Result<Body> {
    body.require_brep("emboss")?;
    checked(depth, regions)?;
    let surf = guard("emboss to the face edge", || face_surf(&record(body)?, face))?;
    match surf {
        Some(Surf::Cylinder { o, a, r, convex }) => {
            let tol = (body.size() * 1e-8).max(1e-8);
            if wall_frame(plane.normal(), plane.origin - o, a, r, convex, tol).is_some() {
                return wrap_to_edge(body, face, plane, regions, depth, (o, a, r, convex));
            }
            if wall_frame(plane.normal(), plane.origin - o, a, r, false, tol).is_some() {
                return Err(refuse("sketch normal must point outwards from the exterior wall"));
            }
            // A line along the sketch's normal crosses a wall twice, once each side of the plane
            // through the axis square to it: the side the pick is on is kept.
            let pick = emboss_anchor(body, face, at)?;
            let side = solidcore_ops::lay_frame::crossing_side(super::arr(a), super::arr(plane.normal()), super::arr(pick - o))
                .ok_or_else(|| refuse("the sketch's normal runs along the wall's axis, so the profiles never reach the wall"))?;
            prisms_to_edge(body, face, plane, regions, depth, Some((o, Vec3::from(side))))
        }
        // A sphere's two crossings lie either side of the plane through its centre square to
        // the sketch's normal.
        Some(Surf::Sphere { c, .. }) => {
            let pick = emboss_anchor(body, face, at)?;
            let side = solidcore_ops::lay_frame::toward(super::arr(plane.normal()), super::arr(pick - c)).ok_or_else(|| refuse("sketch frame"))?;
            prisms_to_edge(body, face, plane, regions, depth, Some((c, Vec3::from(side))))
        }
        _ => prisms_to_edge(body, face, plane, regions, depth, None),
    }
}

/// The middle of the profiles' bounds.
fn middle(regions: &[Region2]) -> Option<Vec2> {
    let mut points = regions.iter().flat_map(|r| r.outer.polyline(1e-4));
    let first = points.next()?;
    let (lo, hi) = points.fold((first, first), |(l, h), p| (Vec2::new(l.x.min(p.x), l.y.min(p.y)), Vec2::new(h.x.max(p.x), h.y.max(p.y))));
    Some((lo + hi) * 0.5)
}

/// How far the slab over a face reaches past it, below and above (along its outward normal),
/// for `depth`: a little past the face on the side away from the depth, as the projection's, and
/// exactly to the depth, which then bounds prisms run through the body. A tool already `depth`
/// deep (`exact`) takes the margin on both sides.
fn slab_reach(depth: f64, exact: bool) -> (f64, f64) {
    let d = depth.abs();
    let overlap = (d * 0.2).clamp(0.1, 2.0);
    let past = if exact { overlap } else { 0.0 };
    if depth > 0.0 { (-overlap, d + past) } else { (-d - past, overlap) }
}

/// The slab over face `face` of `body`, or over the wall it is part of (the faces of its cylinder
/// joined to it across seams): between its offsets `lo` and `hi`, its sides along the faces'
/// normals at their edges. A whole-turn wall face, which the kernel cannot thicken, takes the
/// ring between those radii along its height.
fn cell(body: &Body, rec: &Brep, face: usize, (lo, hi): (f64, f64)) -> Result<Body> {
    let tol = (body.size() * 1e-7).max(1e-7);
    let mut faces = vec![face];
    if let Some(Surf::Cylinder { o, a, r, convex }) = surf_of(rec, face) {
        let wall = (super::arr(o), super::arr(a), r, convex);
        let same: Vec<usize> = (0..rec.faces.len())
            .filter(|f| {
                *f != face
                    && matches!(surf_of(rec, *f), Some(Surf::Cylinder { o: fo, a: fa, r: fr, convex: fc })
                        if solidcore_ops::lay_frame::same_cylinder(wall, (super::arr(fo), super::arr(fa), fr, fc), tol))
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
        let slab = match crate::surfaces::offset_faces(body, &[f], lo, tol).and_then(|sheet| crate::thicken(&sheet, hi - lo, false)) {
            Ok(slab) => slab,
            Err(e) => {
                // A whole sphere, whose edges are only its seam (each used twice), takes the shell
                // between those radii.
                if let Some(Surf::Sphere { c, r, convex }) = surf_of(rec, f)
                    && seams_only(rec, f)
                {
                    let (r_in, r_out) = if convex { (r + lo, r + hi) } else { (r - hi, r - lo) };
                    let outer = crate::sphere(c, r_out)?;
                    let shell = if r_in > 1e-6 { crate::boolean(&outer, &crate::sphere(c, r_in)?, BoolOp::Cut)? } else { Some(outer) };
                    whole = Some(shell.ok_or_else(|| refuse("shell"))?);
                    continue;
                }
                let Some(Surf::Cylinder { o, a, r, convex }) = surf_of(rec, f) else { return Err(e) };
                let frame = Plane::from_normal(o, a).ok_or_else(|| refuse("wall frame"))?;
                let whole_turn = super::wall_angles(rec, f, o, a, frame.x, frame.y).is_some_and(|(l, h)| h - l > std::f64::consts::TAU - 1e-9);
                let Some((z_lo, z_hi)) = face_axial_bounds(rec, f, o, a)?.filter(|_| whole_turn) else { return Err(e) };
                let (r_in, r_out) = if convex { (r + lo, r + hi) } else { (r - hi, r - lo) };
                ring(o + a * z_lo, a, r_in, r_out, z_hi - z_lo)?
            }
        };
        whole = Some(match whole {
            None => slab,
            Some(w) => crate::boolean(&w, &slab, BoolOp::Union)?.ok_or_else(|| refuse("the slab over the wall"))?,
        });
    }
    whole.ok_or_else(|| refuse("selected face"))
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
    crate::boolean(&outer, &crate::cylinder(base, axis, r_in, length)?, BoolOp::Cut)?.ok_or_else(|| refuse("ring"))
}

/// The part of `tool` over the face, joined to (`depth > 0`) or cut from the body.
#[allow(clippy::too_many_arguments)]
fn keep_over(body: &Body, rec: &Brep, face: usize, tools: Vec<Body>, depth: f64, exact: bool, side: Option<(Vec3, Vec3)>) -> Result<Body> {
    let mut over = cell(body, rec, face, slab_reach(depth, exact))?;
    if let Some((o, s)) = side {
        let half = half_space(body, o, s)?;
        over = crate::boolean(&over, &half, BoolOp::Intersect)?.ok_or_else(|| refuse("the profiles do not reach the target face"))?;
    }
    let mut pieces = Vec::new();
    for tool in &tools {
        if let Some(piece) = crate::boolean(tool, &over, BoolOp::Intersect)? {
            pieces.push(piece);
        }
    }
    if pieces.is_empty() {
        return Err(refuse("no part of the profiles lies on the target face"));
    }
    apply_pieces(body, pieces, depth)
}

/// A box past the body on the side of the plane through `o` that `s` points to.
fn half_space(body: &Body, o: Vec3, s: Vec3) -> Result<Body> {
    let b = body.exact_bounds()?;
    let reach = (b.max - b.min).len() + (o - b.min).len() + 1.0;
    let base = Plane::from_normal(o, s).ok_or_else(|| refuse("half space"))?;
    let square = Region2 {
        outer: solidcore_record::Loop2::polygon(&[
            Vec2::new(-reach, -reach),
            Vec2::new(reach, -reach),
            Vec2::new(reach, reach),
            Vec2::new(-reach, reach),
        ]),
        holes: vec![],
    };
    crate::extrude(&base, &[square], 0.0, reach)?.into_iter().next().ok_or_else(|| refuse("half space"))
}

/// The profiles' prisms along `plane`'s normal, through the face, kept over it.
fn prisms_to_edge(body: &Body, face: usize, plane: &Plane, regions: &[Region2], depth: f64, side: Option<(Vec3, Vec3)>) -> Result<Body> {
    guard("emboss to the face edge", || {
        let rec = record(body)?;
        let (h0, h1) = span(body.exact_bounds()?, plane.origin, plane.normal());
        let d = depth.abs();
        let prisms = crate::extrude(plane, regions, h0 - d - 1.0, h1 + d + 1.0)?;
        keep_over(body, &rec, face, prisms, depth, false, side)
    })
}

/// The wrap from `plane`, tangent to the wall (`o`, `a`, `r`, `convex`), built on a whole wall of
/// its radius and kept over the picked face's wall.
fn wrap_to_edge(
    body: &Body,
    face: usize,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    (o, a, r, convex): (Vec3, Vec3, f64, bool),
) -> Result<Body> {
    guard("emboss to the face edge", || {
        let rec = record(body)?;
        let d = depth.abs();
        let mut bounds = solidcraft_geom::Aabb3::EMPTY;
        regions.iter().flat_map(|reg| reg.outer.polyline(1e-4)).for_each(|p| bounds.add(plane.to_world(p)));
        if bounds.is_empty() {
            return Err(refuse("profile"));
        }
        let heights = span(bounds, o, a);
        let margin = d + 1.0;
        let (z_lo, length) = (heights.0 - margin, heights.1 - heights.0 + 2.0 * margin);
        // The whole wall's seam is put opposite the line the sketch touches, where the wrap
        // starts, so a profile meets no seam the picked wall does not have.
        let tol = (body.size() * 1e-8).max(1e-8);
        let (radial, tangent) =
            wall_frame(plane.normal(), plane.origin - o, a, r, convex, tol).ok_or_else(|| refuse("sketch must touch the wall"))?;
        let seat = Plane::new(o + a * z_lo, radial, if convex { tangent } else { -tangent }).ok_or_else(|| refuse("wall frame"))?;
        let disc = |radius: f64| -> Result<Body> {
            let circle = Region2 {
                outer: solidcore_record::Loop2 {
                    segs: vec![Seg2::Arc { center: Vec2::new(0.0, 0.0), radius, start: std::f64::consts::PI, sweep: std::f64::consts::TAU }],
                },
                holes: vec![],
            };
            crate::extrude(&seat, &[circle], 0.0, length)?.into_iter().next().ok_or_else(|| refuse("wall"))
        };
        let whole = if convex { disc(r)? } else { crate::boolean(&disc(r + d + margin)?, &disc(r)?, BoolOp::Cut)?.ok_or_else(|| refuse("wall"))? };
        let wrapped = emboss_cylinder(&whole, plane, regions, depth)?;
        let tool = if depth > 0.0 { crate::boolean(&wrapped, &whole, BoolOp::Cut)? } else { crate::boolean(&whole, &wrapped, BoolOp::Cut)? };
        let tool = tool.ok_or_else(|| refuse("the profiles"))?;
        keep_over(body, &rec, face, vec![tool], depth, true, None)
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
    }

    #[test]
    fn normal_to_face_on_a_cone_keeps_sizes_within_the_tolerance_or_refuses() {
        // A cone of radius 10 at z = 0 to its apex at z = 20: at z = 5 its wall is 7.5 out.
        let circle = Loop2 { segs: vec![Seg2::Arc { center: Vec2::new(0., 0.), radius: 10., start: 0., sweep: std::f64::consts::TAU }] };
        let cone = crate::loft_to_point(&Plane::XY, &circle, Vec3::new(0., 0., 20.)).unwrap();
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
        let e = emboss_on_face(&cone, side, at, &facing_y(20.), &[rect(-6., 3., 6., 7.)], 0.5, EmbossDirection::Face, false).map(|_| ()).unwrap_err();
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
