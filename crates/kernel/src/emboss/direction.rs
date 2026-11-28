//! Emboss Direction (#21423): Normal to sketch or Normal to face, and Stop at face edge. The way
//! is chosen up front by solidcore's `lay_plan::lay_path`; frames, slabs and bounds come from
//! `solidcore_ops::lay_frame`, and this module sequences the kernel's calls.

use super::{
    apply_pieces, arr, cylinder_emboss_anchor, emboss_anchor, emboss_cylinder, emboss_cylinder_at, emboss_project_face, face_axial_bounds,
    infer_cylinder_emboss, record, refuse, span, wall_angles, wall_frame,
};
use crate::body::Body;
use crate::offset::{Surf, face_edges, surf_of};
use crate::{BoolOp, Result, guard};
use solidcore_ops::lay_frame::{self as lay, FaceForm, LayDirection, LayPath, LaySurface, TouchedWall, WallTouch};
use solidcore_record::brep::Brep;
use solidcore_record::{Plane, Region2, Vec2, Vec3};
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

fn tol_of(body: &Body) -> f64 {
    (body.size() * 1e-7).max(1e-7)
}

fn middle(regions: &[Region2]) -> Result<Vec2> {
    lay::profile_middle(regions).ok_or_else(|| refuse("profile"))
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
        return normal_to_sketch(body, face, at, plane, regions, depth);
    }
    body.require_brep("emboss")?;
    checked(depth, regions)?;
    let rec = guard("emboss on a face", || record(body))?;
    let surf = face_surf(&rec, face)?;
    let way = match direction {
        EmbossDirection::Sketch => LayDirection::Sketch,
        EmbossDirection::Face => LayDirection::Face,
    };
    // Normal to face lays the sketch's frame on a plane or cylinder at the point under the
    // middle of the profiles; spheres and other faces lay their own.
    let laid = match (way, surf) {
        (LayDirection::Face, Some(Surf::Plane { .. } | Surf::Cylinder { .. })) => Some(laid_frame(at, plane, regions, surf)?),
        _ => None,
    };
    let mut onto = Vec::new();
    let form = match surf {
        Some(Surf::Plane { .. }) => {
            if let (Some(laid), false) = (&laid, stop_at_edge) {
                onto = walls_run_onto(body, &rec, face, laid, regions)?;
            }
            FaceForm::Plane { walls: onto.len() }
        }
        Some(Surf::Cylinder { o, a, r, convex }) => {
            let tol = (body.size() * 1e-8).max(1e-8);
            let touch = if wall_frame(plane.normal(), plane.origin - o, a, r, convex, tol).is_some() {
                WallTouch::Along
            } else if wall_frame(plane.normal(), plane.origin - o, a, r, false, tol).is_some() {
                WallTouch::Against
            } else {
                WallTouch::Off
            };
            FaceForm::Cylinder { touch }
        }
        Some(Surf::Sphere { .. }) => FaceForm::Sphere,
        _ => FaceForm::Other,
    };
    let path = lay::lay_path(way, form, stop_at_edge).map_err(|why| refuse(why.message()))?;
    let frame = laid.as_ref().unwrap_or(plane);
    match (path, surf) {
        (LayPath::Sketched, _) => normal_to_sketch(body, face, at, plane, regions, depth),
        (LayPath::Project, _) => emboss_project_face(body, face, emboss_anchor(body, face, at)?, frame, regions, depth),
        (LayPath::RollOntoWall, _) => {
            let (rolled, on) = onto.into_iter().next().ok_or_else(|| refuse("the round the profiles run onto"))?;
            emboss_cylinder_at(body, on, &rolled, regions, depth)
        }
        (LayPath::Wrap, _) => emboss_cylinder_at(body, cylinder_emboss_anchor(body, face, at)?, frame, regions, depth),
        (LayPath::WrapToEdge, Some(Surf::Cylinder { o, a, r, convex })) => wrap_to_edge(body, &rec, face, frame, regions, depth, (o, a, r, convex)),
        (LayPath::PrismsToEdge, _) => {
            let sides = sketch_sides(body, face, at, frame, surf, way)?;
            prisms_to_edge(body, &rec, face, frame, regions, depth, &sides)
        }
        (LayPath::SphereCones, Some(Surf::Sphere { c, r, convex })) => {
            on_sphere(body, &rec, face, at, plane, regions, depth, stop_at_edge, (c, r, convex))
        }
        (LayPath::NearFace, _) => near_face(body, &rec, face, at, plane, regions, depth, stop_at_edge),
        _ => Err(refuse("selected face")),
    }
}

/// The sketch's frame laid on a plane or cylinder at the point under the middle of the profiles.
fn laid_frame(at: Vec3, plane: &Plane, regions: &[Region2], surf: Option<Surf>) -> Result<Plane> {
    let surface = match surf {
        Some(Surf::Plane { n, d }) => LaySurface::Plane { point: arr(n * d), normal: arr(n) },
        Some(Surf::Cylinder { o, a, r, .. }) => LaySurface::Cylinder { origin: arr(o), axis: arr(a), radius: r },
        _ => return Err(refuse("selected face")),
    };
    let mid = middle(regions)?;
    let f = lay::lay_frame(arr(plane.origin), arr(plane.x), arr(plane.y), (mid.x, mid.y), surface, arr(at)).ok_or_else(|| refuse(MIDDLE_OFF))?;
    if matches!(surf, Some(Surf::Cylinder { convex: true, .. })) && !f.facing {
        return Err(refuse("the sketch faces away from this side of the wall: pick the side of the wall the sketch faces, or flip the sketch"));
    }
    Plane::new(Vec3::from(f.origin), Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))
}

/// The outside walls beside the plane face `face` that touch the frame `laid` along one of their
/// straight edges and that the profiles run onto past it (#5358): for each, the frame rolled to
/// the middle of the wall face and a pick there.
fn walls_run_onto(body: &Body, rec: &Brep, face: usize, laid: &Plane, regions: &[Region2]) -> Result<Vec<(Plane, Vec3)>> {
    let tol = (body.size() * 1e-8).max(1e-8);
    let edges = face_edges(rec, face);
    let mut onto = Vec::new();
    for f in (0..rec.faces.len()).filter(|f| *f != face && face_edges(rec, *f).iter().any(|e| edges.contains(e))) {
        let Some(Surf::Cylinder { o, a, r, convex: true }) = surf_of(rec, f) else { continue };
        let Some((radial, tangent)) = wall_frame(laid.normal(), laid.origin - o, a, r, true, tol) else { continue };
        let Some(angles) = wall_angles(rec, f, o, a, radial, tangent) else { continue };
        let Some(heights) = face_axial_bounds(rec, f, o, a)? else { continue };
        let wall = TouchedWall { origin: arr(o), axis: arr(a), radius: r, radial: arr(radial), tangent: arr(tangent), angles, heights };
        onto.extend(lay::wall_contact(laid, &wall, regions, tol));
    }
    Ok(onto)
}

/// The half-spaces prisms along the sketch's normal are kept in, stopped at a curved face's edge:
/// a line along that normal crosses a cylinder wall twice, once each side of the plane through
/// the axis square to it, and a sphere either side of the plane through its centre; the side the
/// pick is on is kept.
fn sketch_sides(body: &Body, face: usize, at: Vec3, frame: &Plane, surf: Option<Surf>, way: LayDirection) -> Result<Vec<(Vec3, Vec3)>> {
    if way == LayDirection::Face {
        return Ok(Vec::new());
    }
    Ok(match surf {
        Some(Surf::Cylinder { o, a, .. }) => {
            let pick = emboss_anchor(body, face, at)?;
            let side = lay::crossing_side(arr(a), arr(frame.normal()), arr(pick - o))
                .ok_or_else(|| refuse("the sketch's normal runs along the wall's axis, so the profiles never reach the wall"))?;
            vec![(o, Vec3::from(side))]
        }
        Some(Surf::Sphere { c, .. }) => {
            let pick = emboss_anchor(body, face, at)?;
            let side = lay::toward(arr(frame.normal()), arr(pick - c)).ok_or_else(|| refuse("sketch frame"))?;
            vec![(c, Vec3::from(side))]
        }
        _ => Vec::new(),
    })
}

/// Normal to face on a sphere, by the closest point: each profile's solid is the cone over it from
/// the centre, kept between the face's offsets. Lengths shrink away from the middle; past
/// [`SIZE_TOLERANCE`] the profiles are refused.
#[allow(clippy::too_many_arguments)]
fn on_sphere(
    body: &Body,
    rec: &Brep,
    face: usize,
    at: Vec3,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    stop: bool,
    (c, r, convex): (Vec3, f64, bool),
) -> Result<Body> {
    guard("emboss normal to face", || {
        let mid = middle(regions)?;
        let surface = LaySurface::Sphere { centre: arr(c), radius: r };
        let f = lay::lay_frame(arr(plane.origin), arr(plane.x), arr(plane.y), (mid.x, mid.y), surface, arr(at)).ok_or_else(|| refuse(MIDDLE_OFF))?;
        if convex && !f.facing {
            return Err(refuse("the sketch faces away from this side of the sphere: pick the side the sketch faces, or flip the sketch"));
        }
        let slab = lay::slab_reach(depth, false);
        let (r_in, r_out) = lay::slab_radii(r, convex, slab);
        if r_in <= tol_of(body) {
            return Err(refuse("the depth reaches the sphere's centre: make it smaller"));
        }
        // Sizes are measured on the face; the cap and floor scale with their radius, as a wrap's
        // do on a cylinder.
        let far = lay::profile_reach(regions, mid);
        if lay::central_shrink(far, r) > SIZE_TOLERANCE {
            let most = lay::central_reach(r, SIZE_TOLERANCE);
            return Err(refuse(&format!(
                "on this sphere a profile keeps its size within 10% only within {most:.2} mm of the middle of the profiles, and they reach {far:.2} mm: make them smaller, or use Normal to sketch"
            )));
        }
        let laid = Plane::new(Vec3::from(f.origin), Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))?;
        let (base, k) = lay::sphere_cone_base(c, r, &laid, slab).ok_or_else(|| refuse("laid sketch frame"))?;
        let over = cell(body, rec, face, slab)?;
        let shell = crate::boolean(&crate::sphere(c, r_out)?, &crate::sphere(c, r_in)?, BoolOp::Cut)?.ok_or_else(|| refuse("shell"))?;
        let mut pieces = Vec::new();
        let mut off = Vec::new();
        for (i, reg) in regions.iter().enumerate() {
            let mut cone = crate::loft_to_point(&base, &lay::scaled_loop(&reg.outer, k), c)?;
            for hole in &reg.holes {
                cone = crate::boolean(&cone, &crate::loft_to_point(&base, &lay::scaled_loop(hole, k), c)?, BoolOp::Cut)?
                    .ok_or_else(|| refuse("profile"))?;
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

/// Normal to face on a cone, torus or freeform face: the sketch's frame laid on the tangent plane
/// under the profiles' middle (the crossing nearest the pick), projected along that normal. The
/// face's normal under the outlines must keep every length there within [`SIZE_TOLERANCE`], so
/// the projection is the closest-point map within it; inside the outlines it is not checked.
#[allow(clippy::too_many_arguments)]
fn near_face(body: &Body, rec: &Brep, face: usize, at: Vec3, plane: &Plane, regions: &[Region2], depth: f64, stop: bool) -> Result<Body> {
    let (laid, anchor, normal, stretch) = guard("emboss normal to face", || {
        let mid = middle(regions)?;
        let n = plane.normal();
        let start = Vec3::from(lay::line_step(arr(plane.to_world(mid)), arr(n), arr(at)));
        let anchor = body.face_line_hit(face, start, n)?.ok_or_else(|| refuse(MIDDLE_OFF))?;
        let normal = body.face_normal_at(face, anchor)?.ok_or_else(|| refuse("the target face has no normal under the middle of the profiles"))?;
        let f = lay::lay_frame_at(arr(plane.x), arr(plane.y), (mid.x, mid.y), arr(anchor), arr(normal)).ok_or_else(|| refuse("laid sketch frame"))?;
        if !f.facing {
            return Err(refuse("the sketch faces away from this side of the face: pick the side the sketch faces, or flip the sketch"));
        }
        let laid = Plane::new(Vec3::from(f.origin), Vec3::from(f.x), Vec3::from(f.y)).ok_or_else(|| refuse("laid sketch frame"))?;
        let mut normals = Vec::new();
        for q in lay::outline_samples(regions) {
            let w = laid.to_world(q);
            let Some(on) = body.face_line_hit(face, w, normal)? else {
                if stop {
                    continue;
                }
                return Err(refuse("a profile runs off the target face: move it onto the face, or turn on Stop at face edge to trim it"));
            };
            let n = body.face_normal_at(face, on)?.ok_or_else(|| refuse("the target face has no normal at a point under the profiles"))?;
            normals.push(arr(n));
        }
        let stretch = lay::projected_stretch(arr(normal), &normals);
        Ok((laid, anchor, normal, stretch))
    })?;
    if stretch > SIZE_TOLERANCE {
        return Err(refuse(
            "this face curves too much under the profiles for them to keep their size within 10%: make them smaller, or use Normal to sketch",
        ));
    }
    if stop {
        let reach = lay::profile_reach(regions, middle(regions)?);
        let sides = lay::sag_band(anchor, normal, reach, stretch, lay::slab_reach(depth, false), tol_of(body));
        return prisms_to_edge(body, rec, face, &laid, regions, depth, &sides);
    }
    emboss_project_face(body, face, anchor, &laid, regions, depth).map_err(|e| match e {
        crate::KernelError::Failed(why) => {
            refuse(&format!("this face could not be offset to the emboss's depth ({why}): try a smaller depth, or Normal to sketch"))
        }
        other => other,
    })
}

/// [`emboss_on_face`] onto several target faces (each face and a pick on it): each profile goes on
/// the face nearest its middle along the sketch's normal. Each face's emboss is built on the body
/// as given and the changes are combined, so one face's change does not move another's.
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
    if faces.len() > MAX_TARGET_FACES {
        return Err(refuse("at most 64 target faces"));
    }
    body.require_brep("emboss")?;
    checked(depth, regions)?;
    let n = plane.normal();
    let mut groups: Vec<Vec<Region2>> = vec![Vec::new(); faces.len()];
    for region in regions {
        let p = plane.to_world(middle(std::slice::from_ref(region))?);
        let hits = faces.iter().map(|(f, _)| body.face_line_hit(*f, p, n)).collect::<Result<Vec<_>>>()?;
        let k = lay::nearest_along(p, n, &hits)
            .ok_or_else(|| refuse("a profile lies over none of the target faces along the sketch's normal: move it over one"))?;
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

/// The most target faces one emboss takes.
pub const MAX_TARGET_FACES: usize = 64;

/// Main's emboss, unchanged: a sketch tangent to a cylinder wall the profiles lie on takes the
/// wrap (whose refusals stand); a sketch off the wall and every other face are projected.
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

/// How the slab over one face is built, known from the face before building it.
enum Slab {
    /// The face offset and thickened.
    Sheet,
    /// A whole sphere (its edges only its seam): the shell between the offsets' radii.
    Shell { c: Vec3, r: f64, convex: bool },
    /// A whole-turn wall face, which the kernel does not thicken: the ring between the offsets'
    /// radii along its height.
    Ring { o: Vec3, a: Vec3, r: f64, convex: bool, heights: (f64, f64) },
}

fn slab_of(rec: &Brep, f: usize) -> Result<Slab> {
    Ok(match surf_of(rec, f) {
        Some(Surf::Sphere { c, r, convex }) if seams_only(rec, f) => Slab::Shell { c, r, convex },
        Some(Surf::Cylinder { o, a, r, convex }) => {
            let frame = Plane::from_normal(o, a).ok_or_else(|| refuse("wall frame"))?;
            let whole_turn = wall_angles(rec, f, o, a, frame.x, frame.y).is_some_and(lay::whole_turn);
            match face_axial_bounds(rec, f, o, a)? {
                Some(heights) if whole_turn => Slab::Ring { o, a, r, convex, heights },
                _ => Slab::Sheet,
            }
        }
        _ => Slab::Sheet,
    })
}

/// The slab over face `face` of `body`, or over the wall it is part of (the faces of its cylinder
/// joined to it across seams): between its offsets `lo` and `hi`, its sides along the faces'
/// normals at their edges.
fn cell(body: &Body, rec: &Brep, face: usize, reach: (f64, f64)) -> Result<Body> {
    let tol = tol_of(body);
    let mut faces = vec![face];
    if let Some(Surf::Cylinder { o, a, r, convex }) = surf_of(rec, face) {
        let wall = (arr(o), arr(a), r, convex);
        let same: Vec<usize> = (0..rec.faces.len())
            .filter(|f| {
                *f != face
                    && matches!(surf_of(rec, *f), Some(Surf::Cylinder { o: fo, a: fa, r: fr, convex: fc })
                        if lay::same_cylinder(wall, (arr(fo), arr(fa), fr, fc), tol))
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
    let (lo, hi) = reach;
    let mut whole: Option<Body> = None;
    for f in faces {
        let slab = match slab_of(rec, f)? {
            Slab::Sheet => crate::thicken(&crate::surfaces::offset_faces(body, &[f], lo, tol)?, hi - lo, false)?,
            Slab::Shell { c, r, convex } => {
                let (r_in, r_out) = lay::slab_radii(r, convex, reach);
                let outer = crate::sphere(c, r_out)?;
                if r_in > 1e-6 { crate::boolean(&outer, &crate::sphere(c, r_in)?, BoolOp::Cut)?.ok_or_else(|| refuse("shell"))? } else { outer }
            }
            Slab::Ring { o, a, r, convex, heights: (z_lo, z_hi) } => {
                let (r_in, r_out) = lay::slab_radii(r, convex, reach);
                ring(lay::on_axis(o, a, z_lo), a, r_in, r_out, z_hi - z_lo)?
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

/// The part of the tools over the face, joined to (`depth > 0`) or cut from the body, within the
/// half-spaces `sides` (a point of each plane and the direction into it).
#[allow(clippy::too_many_arguments)]
fn keep_over(body: &Body, rec: &Brep, face: usize, tools: Vec<Body>, depth: f64, exact: bool, sides: &[(Vec3, Vec3)]) -> Result<Body> {
    let mut over = cell(body, rec, face, lay::slab_reach(depth, exact))?;
    for &(o, s) in sides {
        let half = crate::ops::half_box(body, &Plane::from_normal(o, s).ok_or_else(|| refuse("half space"))?, true)?;
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

/// The profiles' prisms along `plane`'s normal, through the face, kept over it.
fn prisms_to_edge(body: &Body, rec: &Brep, face: usize, plane: &Plane, regions: &[Region2], depth: f64, sides: &[(Vec3, Vec3)]) -> Result<Body> {
    guard("emboss to the face edge", || {
        let (h0, h1) = span(body.exact_bounds()?, plane.origin, plane.normal());
        let d = depth.abs();
        let prisms = crate::extrude(plane, regions, h0 - d - 1.0, h1 + d + 1.0)?;
        keep_over(body, rec, face, prisms, depth, false, sides)
    })
}

/// The wrap from `plane`, tangent to the wall (`o`, `a`, `r`, `convex`), built on a whole wall of
/// its radius and kept over the picked face's wall.
#[allow(clippy::too_many_arguments)]
fn wrap_to_edge(
    body: &Body,
    rec: &Brep,
    face: usize,
    plane: &Plane,
    regions: &[Region2],
    depth: f64,
    (o, a, r, convex): (Vec3, Vec3, f64, bool),
) -> Result<Body> {
    guard("emboss to the face edge", || {
        let tol = (body.size() * 1e-8).max(1e-8);
        let (radial, tangent) =
            wall_frame(plane.normal(), plane.origin - o, a, r, convex, tol).ok_or_else(|| refuse("sketch must touch the wall"))?;
        let wall = TouchedWall {
            origin: arr(o),
            axis: arr(a),
            radius: r,
            radial: arr(radial),
            tangent: arr(tangent),
            angles: (0.0, 0.0),
            heights: (0.0, 0.0),
        };
        // The whole wall's seam goes opposite the line the sketch touches, where the wrap starts,
        // so a profile meets no seam the picked wall does not have.
        let (seat, length, outer) = lay::wall_seat(plane, regions, &wall, convex, depth).ok_or_else(|| refuse("profile"))?;
        let disc = |radius: f64| -> Result<Body> {
            let circle = Region2 { outer: solidcore_record::Loop2::circle_from(Vec2::new(0.0, 0.0), radius, std::f64::consts::PI), holes: vec![] };
            crate::extrude(&seat, &[circle], 0.0, length)?.into_iter().next().ok_or_else(|| refuse("wall"))
        };
        let whole = if convex { disc(r)? } else { crate::boolean(&disc(outer)?, &disc(r)?, BoolOp::Cut)?.ok_or_else(|| refuse("wall"))? };
        let wrapped = emboss_cylinder(&whole, plane, regions, depth)?;
        let tool = if depth > 0.0 { crate::boolean(&wrapped, &whole, BoolOp::Cut)? } else { crate::boolean(&whole, &wrapped, BoolOp::Cut)? };
        let tool = tool.ok_or_else(|| refuse("the profiles"))?;
        keep_over(body, rec, face, vec![tool], depth, true, &[])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcore_record::{Loop2, Seg2};

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
