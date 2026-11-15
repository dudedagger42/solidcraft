//! Delete Face with healing: faces are removed and their planar neighbours extend to close the
//! gap, as when a fillet, chamfer, boss or hole is taken off a part.
//!
//! The body is rebuilt from its own schema 4 record. Faces away from the gap are kept as they
//! are. Each neighbouring face (planar) keeps its plane and its edges that don't touch the gap;
//! a loop that ran entirely along deleted faces is dropped for a hole's rim or a boss's
//! footprint, or restored at the moved corners for a face enclosed by rounds. A corner on the
//! gap moves to where its planes meet the neighbours across the gap (a fillet's tangent points
//! go to the sharp corner), and the stretch of loop that bordered deleted faces closes with a
//! straight edge between the moved corners. A corner left on one neighbour only, with nowhere
//! to move (the apex of a V notch or groove), drops out of that edge. Where the gap is left
//! between neighbours on one plane, that plane extends across it: the region is filled and the
//! faces on the plane become one. Anything else (curved neighbours, a gap the neighbours cannot
//! close) is refused, naming the faces and the way out.

use solidcraft_geom::prelude::*;
use std::collections::{BTreeSet, HashMap, HashSet};

use solidcore_ops::cap::{is_face_on_plane, is_point_on_plane};
use solidcore_record::Vec3;
use solidcore_record::brep::{Curve, Frame};
use solidcraft_geom::Aabb3;

use crate::assemble::{AFace, AUse, Assembly};
use crate::body::Body;
use crate::history::{Change, FaceProv};
use crate::offset::{Surf, surf_of};
use crate::{KernelError, Result, guard};

fn fail(m: impl Into<String>) -> KernelError {
    KernelError::Invalid(format!("Delete Face: {}", m.into()))
}

type Key = (i64, i64, i64);

/// A short name for the kind of surface face `f` lies on, for refusal messages that tell the
/// user what a face is and how to get past the refusal.
fn surface_kind(rec: &solidcore_record::brep::Brep, f: usize) -> &'static str {
    match surf_of(rec, f) {
        Some(Surf::Plane { .. }) => "a plane",
        Some(Surf::Cylinder { .. }) => "a cylinder",
        Some(Surf::Sphere { .. }) => "a sphere",
        Some(Surf::Cone { .. }) => "a cone",
        Some(Surf::Torus { .. }) => "a torus (as a fillet or round often is)",
        None => "a free-form (spline) face",
    }
}

/// Key of a point for sharing vertices and edges between rebuilt faces.
fn key(p: Vec3, q: f64) -> Key {
    let r = |x: f64| (x / q).round() as i64;
    (r(p.x), r(p.y), r(p.z))
}

/// The vertices and straight edges the rebuilt faces share, by where they are.
struct Shared {
    q: f64,
    vertices: HashMap<Key, u32>,
    lines: HashMap<(Key, Key), u32>,
    /// Arcs by their ends and centre.
    arcs: HashMap<(Key, Key, Key), u32>,
}

impl Shared {
    fn vertex(&mut self, asm: &mut Assembly, p: Vec3) -> Result<u32> {
        let k = key(p, self.q);
        if let Some(v) = self.vertices.get(&k) {
            return Ok(*v);
        }
        let v = asm.vertex(p)?;
        self.vertices.insert(k, v);
        Ok(v)
    }

    /// The straight edge from `a` to `b`, made once and used both ways.
    fn line(&mut self, asm: &mut Assembly, a: Vec3, b: Vec3) -> Result<AUse> {
        let (ka, kb) = (key(a, self.q), key(b, self.q));
        if let Some(e) = self.lines.get(&(ka, kb)) {
            return Ok(AUse { edge: *e, forward: true, given: None });
        }
        if let Some(e) = self.lines.get(&(kb, ka)) {
            return Ok(AUse { edge: *e, forward: false, given: None });
        }
        let (va, vb) = (self.vertex(asm, a)?, self.vertex(asm, b)?);
        let e = asm.line(va, vb)?;
        self.lines.insert((ka, kb), e);
        Ok(AUse { edge: e, forward: true, given: None })
    }

    /// The arc from `a` to `b` about `c` in the plane normal to `axis`, the way round on the
    /// side `out` points to from the chord `a`–`b` (the deleted face's outward normal: the
    /// material taken away lay there), made once and used both ways.
    fn arc(&mut self, asm: &mut Assembly, a: Vec3, b: Vec3, c: Vec3, axis: Vec3, out: Vec3) -> Result<AUse> {
        let (ka, kb, kc) = (key(a, self.q), key(b, self.q), key(c, self.q));
        if let Some(e) = self.arcs.get(&(ka, kb, kc)) {
            return Ok(AUse { edge: *e, forward: true, given: None });
        }
        if let Some(e) = self.arcs.get(&(kb, ka, kc)) {
            return Ok(AUse { edge: *e, forward: false, given: None });
        }
        let r = a.dist(c);
        if !(r > self.q * 10.0) {
            return Err(fail("an arc of no radius"));
        }
        let x = (a - c) / r;
        let q = self.q;
        let chord = (a + b) * 0.5;
        let way = |n: Vec3| {
            let y = n.cross(x);
            let mut t = (b - c).dot(y).atan2((b - c).dot(x));
            if t <= q {
                t += std::f64::consts::TAU;
            }
            let mid = c + (x * (t / 2.0).cos() + y * (t / 2.0).sin()) * r;
            (y, t, (mid - chord).dot(out))
        };
        let (y1, t1, s1) = way(axis);
        let (y2, t2, s2) = way(axis * -1.0);
        let (y, t) = if s1 >= s2 { (y1, t1) } else { (y2, t2) };
        if s1.max(s2) <= q {
            return Err(fail("an arc with no side to go round"));
        }
        let (va, vb) = (self.vertex(asm, a)?, self.vertex(asm, b)?);
        let frame = Frame { origin: [c.x, c.y, c.z], x_axis: [x.x, x.y, x.z], y_axis: [y.x, y.y, y.z] };
        let e = asm.edge(Curve::Circle { frame, radius: r }, [0.0, t], va, vb)?;
        self.arcs.insert((ka, kb, kc), e);
        Ok(AUse { edge: e, forward: true, given: None })
    }
}

/// A cylinder neighbour that closes round over the gap: axis point, unit axis and radius.
type Wrap = (Vec3, Vec3, f64);

/// The circle where plane `n·x = d` cuts a cylinder of `wraps` square to it, if `a` and `b`
/// both lie on it: its centre and axis.
fn seam_of(l: &[AUse], e: u32) -> bool {
    l.iter().filter(|u| u.edge == e).count() == 2
}

fn is_ruling(asm: &Assembly, u: &AUse, ax: Vec3) -> bool {
    asm.edges.get(u.edge as usize).is_some_and(|e| match &e.curve {
        Curve::Line { start, end } => {
            let v = Vec3::from(*end) - Vec3::from(*start);
            v.cross(ax).len() <= 1e-9 * v.len().max(1.0)
        }
        _ => false,
    })
}

fn wraps_loops(asm: &Assembly, loops: &[Vec<AUse>], gap_edge: &dyn Fn(u32) -> bool, o: Vec3, ax: Vec3, r: f64, tol: f64) -> bool {
    let height = |p: Vec3| (p - o).dot(ax);
    let on_circle = |e: u32, h: f64| {
        asm.edges.get(e as usize).is_some_and(|edge| {
            solidcraft_geom::brep::eval::CurveEval::new(&edge.curve).is_ok_and(|ev| {
                let [t0, t1] = edge.interval;
                (0..=4).all(|i| {
                    ev.point(t0 + (t1 - t0) * f64::from(i) / 4.0).is_some_and(|p| {
                        let along = height(p);
                        (along - h).abs() <= tol && (((p - o) - ax * along).len() - r).abs() <= tol
                    })
                })
            })
        })
    };
    let mut ends: Option<(f64, f64)> = None;
    for l in loops {
        let on: Vec<usize> = (0..l.len()).filter(|k| l.get(*k).is_some_and(|u| gap_edge(u.edge))).collect();
        match on[..] {
            [] => continue,
            [i, j] if j - i != 1 && !(i == 0 && j + 1 == l.len()) => {}
            _ => return false,
        }
        let (Some(ui), Some(uj)) = (on.first().and_then(|k| l.get(*k)), on.get(1).and_then(|k| l.get(*k))) else { return false };
        if !(is_ruling(asm, ui, ax) && is_ruling(asm, uj, ax)) || ends.is_some() {
            return false;
        }
        let Some((a, b)) = asm.ends(ui).and_then(|(a, b)| Some((asm.point(a)?, asm.point(b)?))) else { return false };
        let (h1, h2) = (height(a), height(b));
        // Its own seam may lie between the rulings; it goes when the face closes round.
        if !l
            .iter()
            .filter(|u| !gap_edge(u.edge))
            .all(|u| on_circle(u.edge, h1) || on_circle(u.edge, h2) || (seam_of(l, u.edge) && is_ruling(asm, u, ax)))
        {
            return false;
        }
        ends = Some((h1.min(h2), h1.max(h2)));
    }
    // A pocketed flat's end walls lie inside the solid, past these circles.
    let Some((lo, hi)) = ends else { return false };
    loops
        .iter()
        .flatten()
        .all(|u| asm.ends(u).is_some_and(|(a, b)| [a, b].iter().all(|v| asm.point(*v).is_some_and(|p| (lo - tol..=hi + tol).contains(&height(p))))))
}

fn wrap_circle(wraps: &HashMap<usize, Wrap>, n: Vec3, d: f64, a: Vec3, b: Vec3, tol: f64) -> Option<(Vec3, Vec3)> {
    wraps.values().find_map(|&(o, ax, r)| {
        if n.cross(ax).len() > 1e-9 || n.dot(ax).abs() < 1e-9 {
            return None;
        }
        let c = o + ax * ((d - n.dot(o)) / n.dot(ax));
        let on = |p: Vec3| (p.dist(c) - r).abs() <= tol && (n.dot(p) - d).abs() <= tol;
        (on(a) && on(b)).then_some((c, ax))
    })
}

/// The corners along a gap stretch, closed to `a`: straight edges between them, with corners
/// that coincide or lie in line with their neighbours dropped (two rounds meeting in a mitre
/// leave the sharp corner between them).
fn close(pts: Vec<Vec3>, a: Vec3, q: f64) -> Vec<(Vec3, Vec3)> {
    let mut c: Vec<Vec3> = Vec::new();
    for p in pts.into_iter().chain(std::iter::once(a)) {
        if c.last().is_some_and(|l| l.dist(p) <= q * 10.0) {
            continue;
        }
        if let [.., x, y] = c[..]
            && (y - x).cross(p - y).len() <= q * 10.0 * (p - x).len()
            && (y - x).dot(p - y) > 0.0
        {
            c.pop();
        }
        c.push(p);
    }
    c.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect()
}

/// The edges closing a gap stretch through `pts` back to `a` on a plane neighbour: one arc
/// where the stretch runs from one point to another of the circle the plane cuts on a cylinder
/// closing round (its ends stay), else straight edges ([`close`]).
#[allow(clippy::too_many_arguments)]
fn closing(
    shared: &mut Shared,
    asm: &mut Assembly,
    pts: Vec<Vec3>,
    a: Vec3,
    plane: Option<(Vec3, f64)>,
    out: Option<Vec3>,
    wraps: &HashMap<usize, Wrap>,
) -> Result<Vec<AUse>> {
    let q = shared.q;
    if let (Some((n, d)), Some(out), Some(&first)) = (plane, out, pts.first())
        && first.dist(a) > q * 10.0
        && let Some((c, ax)) = wrap_circle(wraps, n, d, first, a, q * 100.0)
    {
        return Ok(vec![shared.arc(asm, first, a, c, ax, out)?]);
    }
    close(pts, a, q).into_iter().map(|(x, y)| shared.line(asm, x, y)).collect()
}

#[cfg(test)]
thread_local! {
    static FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether the use's edge has a corner that moves.
fn corner_moves(asm: &Assembly, moved: &HashMap<u32, Vec3>, u: &AUse) -> bool {
    asm.ends(u).is_some_and(|(a, b)| moved.contains_key(&a) || moved.contains_key(&b))
}

/// Where vertex `v` is after the corners move.
fn position(asm: &Assembly, moved: &HashMap<u32, Vec3>, v: u32) -> Result<Vec3> {
    moved.get(&v).copied().or_else(|| asm.point(v)).ok_or_else(|| fail("vertex"))
}

fn arr(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// Whether face `g` lies on `plane` within `tol`: its own plane points the same way and every
/// corner of its loops is on `plane` ([`is_face_on_plane`]).
fn face_on_plane(asm: &Assembly, planes: &HashMap<usize, (Vec3, f64)>, g: usize, plane: (Vec3, f64), tol: f64) -> bool {
    let (Some(own), Some(face)) = (planes.get(&g), asm.faces.get(g).and_then(Option::as_ref)) else { return false };
    let corners: Vec<[f64; 3]> = face.loops.iter().flatten().filter_map(|u| asm.ends(u)).filter_map(|(a, _)| asm.point(a)).map(arr).collect();
    is_face_on_plane(arr(own.0), &corners, arr(plane.0), plane.1, tol)
}

/// Face `i`, or the face it was merged into.
fn surviving(alias: &HashMap<usize, usize>, mut i: usize) -> usize {
    while let Some(j) = alias.get(&i) {
        i = *j;
    }
    i
}

/// Loop `a` and loop `b` joined along the edge `e` that each uses once (in opposite
/// directions): `a` from after `e` round to before it, then `b` likewise. `None` when `e` is
/// not used exactly once in each, both run it the same way, or the joined loop would use an
/// edge twice.
fn splice(a: &[AUse], b: &[AUse], e: u32) -> Option<Vec<AUse>> {
    let once = |l: &[AUse]| if l.iter().filter(|u| u.edge == e).count() == 1 { l.iter().position(|u| u.edge == e) } else { None };
    let (i, j) = (once(a)?, once(b)?);
    if a.get(i)?.forward == b.get(j)?.forward {
        return None;
    }
    let out: Vec<AUse> = a.iter().skip(i + 1).chain(a.iter().take(i)).chain(b.iter().skip(j + 1)).chain(b.iter().take(j)).cloned().collect();
    let edges: HashSet<u32> = out.iter().map(|u| u.edge).collect();
    (!out.is_empty() && edges.len() == out.len()).then_some(out)
}

/// Close what the rebuilt faces leave open where the gap lies between neighbours on one plane
/// (a V notch through a wall, a V groove across a top). Only the closing edges the deletion made
/// count (`opened`, each with the deleted face it stands in for); a sheet's own free boundary
/// stays as it is. Those used by one face only make loops; a loop on a neighbour's plane is that
/// plane extended across the gap, a new face on it merged with the faces of that plane it
/// borders. A loop on no neighbour's plane cannot close (one face of a box: its sides are
/// parallel) and is refused, naming the deleted face (`fallback` when none is known).
fn fill_on_planes(
    asm: &mut Assembly,
    planes: &HashMap<usize, (Vec3, f64)>,
    tol: f64,
    history: &mut Vec<(usize, FaceProv)>,
    opened: &HashMap<u32, usize>,
    fallback: usize,
) -> Result<()> {
    let mut uses: HashMap<u32, Vec<(usize, AUse)>> = HashMap::new();
    for (fi, f) in asm.faces.iter().enumerate() {
        for u in f.iter().flat_map(|f| &f.loops).flatten() {
            if opened.contains_key(&u.edge) {
                uses.entry(u.edge).or_default().push((fi, u.clone()));
            }
        }
    }
    let cannot = |e: u32| {
        let f = opened.get(&e).copied().unwrap_or(fallback);
        fail(format!("the gap left by face {f} cannot close: no neighbouring plane extends across it"))
    };
    // The other side of each edge used once, by where it starts, with the face that uses it.
    let mut open: Vec<u32> = uses.iter().filter(|(_, us)| us.len() == 1).map(|(e, _)| *e).collect();
    open.sort_unstable();
    let mut next: HashMap<u32, (usize, AUse)> = HashMap::new();
    for e in &open {
        let Some((fi, u)) = uses.get(e).and_then(|us| us.first()).cloned() else { continue };
        let back = AUse { edge: *e, forward: !u.forward, given: None };
        let (a, _) = asm.ends(&back).ok_or_else(|| fail("edge"))?;
        if next.insert(a, (fi, back)).is_some() {
            return Err(cannot(*e));
        }
    }
    // Faces merged away, to the face they became part of.
    let mut alias: HashMap<usize, usize> = HashMap::new();
    let mut done: HashSet<u32> = HashSet::new();
    for e in open {
        if done.contains(&e) {
            continue;
        }
        let Some(first) = next.values().find(|(_, u)| u.edge == e).map(|(_, u)| u.clone()) else { continue };
        let (start, _) = asm.ends(&first).ok_or_else(|| fail("edge"))?;
        // Walk the loop, with the face on the other side of each edge. A chain that does not
        // close (it runs into a sheet's free boundary) is left open.
        let (mut lp, mut beside): (Vec<AUse>, Vec<usize>) = (Vec::new(), Vec::new());
        let mut at = start;
        let closed = loop {
            let Some((fi, u)) = next.get(&at).cloned() else { break false };
            if !done.insert(u.edge) {
                return Err(cannot(e));
            }
            at = asm.ends(&u).ok_or_else(|| fail("edge"))?.1;
            lp.push(u);
            beside.push(surviving(&alias, fi));
            if at == start {
                break true;
            }
        };
        if !closed {
            continue;
        }
        // The neighbour plane the loop lies on: every corner on it, and every curved edge on a
        // face of that plane.
        let on = |pl: (Vec3, f64)| {
            lp.iter().zip(&beside).all(|(u, fi)| {
                let corners_on = asm
                    .ends(u)
                    .is_some_and(|(a, b)| [a, b].iter().all(|v| asm.point(*v).is_some_and(|p| is_point_on_plane(arr(p), arr(pl.0), pl.1, tol))));
                let straight = matches!(asm.edges.get(u.edge as usize).map(|x| &x.curve), Some(Curve::Line { .. }));
                corners_on && (straight || face_on_plane(asm, planes, *fi, pl, tol))
            })
        };
        let Some(owner) = beside.iter().copied().find(|fi| planes.get(fi).is_some_and(|pl| on(*pl))) else {
            return Err(cannot(e));
        };
        let plane = *planes.get(&owner).ok_or_else(|| fail("plane"))?;
        let template = asm.faces.get(owner).and_then(Option::as_ref).ok_or_else(|| fail("face"))?.clone();
        // Merge the new face with the faces of that plane it borders, one shared edge at a time;
        // where two outer loops cannot be joined the new face stays a face of its own.
        let mut merged = AFace { loops: vec![lp], domain: None, ..template };
        let mut into: Option<usize> = None;
        let mut origins = Vec::new();
        loop {
            let found = beside.iter().copied().find_map(|g| {
                if Some(g) == into || !face_on_plane(asm, planes, g, plane, tol) {
                    return None;
                }
                let face = asm.faces.get(g).and_then(Option::as_ref)?;
                let (m, o) = (merged.loops.first()?, face.loops.first()?);
                let e = m.iter().map(|u| u.edge).find(|e| o.iter().any(|x| x.edge == *e))?;
                Some((g, splice(m, o, e)?))
            });
            let Some((g, mut joined)) = found else { break };
            let mut face = asm.faces.get_mut(g).and_then(Option::take).ok_or_else(|| fail("face"))?;
            // A face on another surface (or NURBS form) keeps no pcurves: they are about its own
            // parameters, not the merged face's, and are made again on the merged face's surface.
            if face.surface != merged.surface || face.nurbs != merged.nurbs {
                let theirs: HashSet<u32> = face.loops.iter().flatten().map(|u| u.edge).collect();
                for u in joined.iter_mut().chain(face.loops.iter_mut().skip(1).flatten()) {
                    if theirs.contains(&u.edge) {
                        u.given = None;
                    }
                }
            }
            let holes: Vec<Vec<AUse>> = merged.loops.drain(1..).chain(face.loops.into_iter().skip(1)).collect();
            merged.loops = std::iter::once(joined).chain(holes).collect();
            let prov = history.iter().position(|(fi, _)| *fi == g).map(|k| history.remove(k).1);
            match into {
                None => into = Some(g),
                Some(k) => {
                    alias.insert(g, k);
                }
            }
            origins.extend(prov.into_iter().flat_map(|p| p.origins));
        }
        match into {
            Some(g) => {
                if let Some(slot) = asm.faces.get_mut(g) {
                    *slot = Some(merged);
                }
                history.push((g, FaceProv { origins, change: Change::Modified }));
            }
            None => {
                asm.face(merged);
                history.push((asm.faces.len() - 1, crate::record::from_face(0, owner, Change::Modified)));
            }
        }
    }
    Ok(())
}

/// Delete faces (indices in the body's face order, as in its mesh's `tri_face`) and heal the
/// gap by extending the neighbouring planes. The result's history is over `[body]`.
pub fn delete_faces(body: &Body, faces: &[usize]) -> Result<Body> {
    let _span = solidcraft_profile::span("kernel", "delete_faces");
    body.require_brep("Delete Face")?;
    let rec = crate::native::brep_record(body)?;
    let n = rec.faces.len();
    if faces.is_empty() {
        return Err(fail("no faces given"));
    }
    let del: BTreeSet<usize> = faces.iter().copied().collect();
    if let Some(bad) = del.iter().find(|f| **f >= n) {
        return Err(fail(format!("there is no face {bad} (the body has {n})")));
    }
    if del.len() >= n {
        return Err(fail("that would delete every face"));
    }
    let size = body.size().max(1e-9);
    // Set where the only obstacle is a curved neighbour: fillet faces then go through SMLib's
    // fillet removal instead, which extends the fillet's two tangent neighbours whatever they are.
    let mut curved = false;
    let healed = guard("delete face", || {
        let mut asm = Assembly::from_record(&rec)?;
        // Edge and vertex → faces.
        let mut efaces: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut vfaces: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, f) in asm.faces.iter().enumerate() {
            for u in f.iter().flat_map(|f| &f.loops).flatten() {
                let l = efaces.entry(u.edge).or_default();
                if !l.contains(&i) {
                    l.push(i);
                }
                let (a, b) = asm.ends(u).ok_or_else(|| fail("edge"))?;
                for v in [a, b] {
                    let l = vfaces.entry(v).or_default();
                    if !l.contains(&i) {
                        l.push(i);
                    }
                }
            }
        }
        // A boolean can leave one cylinder as two faces; merged, it can close round as one.
        let mut merged_into: HashMap<usize, usize> = HashMap::new();
        // A user's split (Split Face) must stay.
        if !body.regularization_protected && body.split_lines().is_none() {
            let on_gap = |e: &u32| efaces.get(e).is_some_and(|fs| fs.iter().any(|x| del.contains(x)));
            let near: BTreeSet<usize> =
                efaces.values().filter(|fs| fs.iter().any(|f| del.contains(f))).flatten().copied().filter(|f| !del.contains(f)).collect();
            let cyls: Vec<(usize, Vec3, Vec3, f64)> = near
                .iter()
                .filter_map(|&f| match surf_of(&rec, f) {
                    Some(Surf::Cylinder { o, a, r, .. }) => Some((f, o, a, r)),
                    _ => None,
                })
                .collect();
            let tol = size * 1e-6;
            let mut joins: Vec<(usize, usize, AFace)> = Vec::new();
            for (k, &(f, o, a, r)) in cyls.iter().enumerate() {
                for &(g, og, ag, rg) in cyls.iter().skip(k + 1) {
                    let same = (r - rg).abs() <= tol && a.cross(ag).len() <= 1e-9 && ((og - o) - a * (og - o).dot(a)).len() <= tol;
                    if !same || joins.iter().any(|(x, y, _)| [*x, *y].iter().any(|z| *z == f || *z == g)) {
                        continue;
                    }
                    let (Some(ff), Some(gg)) = (asm.faces.get(f).and_then(Option::as_ref), asm.faces.get(g).and_then(Option::as_ref)) else {
                        continue;
                    };
                    let ([lf], [lg]) = (&ff.loops[..], &gg.loops[..]) else { continue };
                    let Some(e) = lf.iter().map(|u| u.edge).find(|e| lg.iter().any(|x| x.edge == *e) && !on_gap(e)) else { continue };
                    let Some(joined) = splice(lf, lg, e) else { continue };
                    if !wraps_loops(&asm, std::slice::from_ref(&joined), &|x| on_gap(&x), o, a, r, tol) {
                        continue;
                    }
                    let joined: Vec<AUse> = joined.into_iter().map(|u| AUse { given: None, ..u }).collect();
                    joins.push((f, g, AFace { loops: vec![joined], nurbs: None, domain: None, ..ff.clone() }));
                }
            }
            for (f, g, face) in joins {
                if let Some(slot) = asm.faces.get_mut(f) {
                    *slot = Some(face);
                }
                if let Some(slot) = asm.faces.get_mut(g) {
                    *slot = None;
                }
                merged_into.insert(g, f);
            }
            for fs in efaces.values_mut().chain(vfaces.values_mut()) {
                for x in fs.iter_mut() {
                    if let Some(&t) = merged_into.get(x) {
                        *x = t;
                    }
                }
                fs.sort_unstable();
                fs.dedup();
            }
        }
        let gap_edge = |e: u32| efaces.get(&e).is_some_and(|fs| fs.iter().any(|x| del.contains(x)));
        // A deleted face's outward normal where it is a plane: the side its material went.
        let outward = |f: usize| match surf_of(&rec, f) {
            Some(Surf::Plane { n, .. }) => Some(n),
            _ => None,
        };
        // Neighbours: faces sharing an edge with a deleted face; they must be planar.
        let mut neighbours: BTreeSet<usize> = BTreeSet::new();
        for fs in efaces.values() {
            if fs.iter().any(|f| del.contains(f)) {
                neighbours.extend(fs.iter().filter(|f| !del.contains(f)));
            }
        }
        if neighbours.is_empty() {
            return Err(fail("the faces don't border the rest of the body"));
        }
        let mut planes: HashMap<usize, (Vec3, f64)> = HashMap::new();
        // Curved neighbours that need not extend: every loop of theirs that touches the gap
        // lies on it whole (a hole's opening in a rod's cylinder), so it goes with the
        // deleted faces and the face keeps its surface and parameter box.
        let mut kept_whole: HashSet<usize> = HashSet::new();
        // Cylinder neighbours that close round over the gap (a flat on a shaft): each loop of
        // theirs on the gap meets it along two straight rulings, not next to each other. One
        // ruling stays as the seam, and the cylinder's circles close over the gap with arcs.
        let mut wraps: HashMap<usize, Wrap> = HashMap::new();
        let mut movers: HashMap<usize, Wrap> = HashMap::new();
        let wraps_round = |f: usize, o: Vec3, ax: Vec3, r: f64| {
            asm.faces.get(f).and_then(Option::as_ref).is_some_and(|face| wraps_loops(&asm, &face.loops, &gap_edge, o, ax, r, size * 1e-6))
        };
        let loses_whole_loops = |f: usize| {
            asm.faces
                .get(f)
                .and_then(Option::as_ref)
                .is_some_and(|face| face.loops.iter().all(|l| l.iter().all(|u| gap_edge(u.edge)) || !l.iter().any(|u| gap_edge(u.edge))))
        };
        for &f in &neighbours {
            match surf_of(&rec, f) {
                Some(Surf::Plane { n, d }) => {
                    planes.insert(f, (n, d));
                }
                Some(Surf::Cylinder { o, a, r, .. }) if wraps_round(f, o, a, r) => {
                    wraps.insert(f, (o, a, r));
                }
                _ if loses_whole_loops(f) => {
                    kept_whole.insert(f);
                }
                // Corners with nowhere to move on it are refused below, keeping DM-0.2's fallback.
                Some(Surf::Cylinder { o, a, r, .. }) => {
                    movers.insert(f, (o, a, r));
                }
                _ => {
                    let by = efaces
                        .values()
                        .find(|fs| fs.contains(&f) && fs.iter().any(|x| del.contains(x)))
                        .and_then(|fs| fs.iter().find(|x| del.contains(x)).copied())
                        .unwrap_or(0);
                    curved = true;
                    return Err(fail(format!(
                        "face {by} ({}), borders face {f}, {}: include face {f} in the selection if it should go too, or take off the feature that made face {f} in the timeline instead; only planar neighbours can extend to close the gap",
                        surface_kind(&rec, by),
                        surface_kind(&rec, f),
                    )));
                }
            }
        }
        // The deleted faces' vertices.
        let gap_vertices: BTreeSet<u32> = vfaces.iter().filter(|(_, fs)| fs.iter().any(|f| del.contains(f))).map(|(v, _)| *v).collect();
        // Corners across the gap: three neighbour planes meeting near the deleted faces.
        let gap: Aabb3 = gap_vertices.iter().filter_map(|v| asm.point(*v)).collect();
        let near_gap = gap.inflated(gap.diagonal().max(size * 1e-3));
        let on = |p: Vec3, f: usize| {
            planes.get(&f).is_some_and(|(n, d)| (n.dot(p) - d).abs() <= size * 1e-6)
                || movers.get(&f).is_some_and(|&(o, ax, r)| (((p - o) - ax * (p - o).dot(ax)).len() - r).abs() <= size * 1e-6)
        };
        let nb: Vec<(Vec3, f64)> = neighbours.iter().filter_map(|f| planes.get(f).copied()).collect();
        let mut corners: Vec<Vec3> = Vec::new();
        for (i, a) in nb.iter().enumerate() {
            for (j, b) in nb.iter().enumerate().skip(i + 1) {
                for c in nb.iter().skip(j + 1) {
                    let den = a.0.dot(b.0.cross(c.0));
                    if den.abs() < 1e-9 {
                        continue;
                    }
                    let x = (b.0.cross(c.0) * a.1 + c.0.cross(a.0) * b.1 + a.0.cross(b.0) * c.1) / den;
                    if x.is_finite() && near_gap.contains(x) {
                        corners.push(x);
                    }
                }
                let u = a.0.cross(b.0);
                if u.len() < 1e-9 || movers.is_empty() {
                    continue;
                }
                let u = u / u.len();
                let p0 = (b.0.cross(u) * a.1 + u.cross(a.0) * b.1) / a.0.cross(b.0).dot(u);
                for &(o, ax, r) in movers.values() {
                    let (w, du) = (p0 - o, u - ax * u.dot(ax));
                    let wp = w - ax * w.dot(ax);
                    let (qa, qb, qc) = (du.dot(du), 2.0 * du.dot(wp), wp.dot(wp) - r * r);
                    let disc = qb * qb - 4.0 * qa * qc;
                    if qa < 1e-12 || disc < 0.0 {
                        continue;
                    }
                    for t in [(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)] {
                        let x = p0 + u * t;
                        if x.is_finite() && near_gap.contains(x) {
                            corners.push(x);
                        }
                    }
                }
            }
        }
        // Where each vertex on the gap goes: the nearest corner on all of its remaining planes.
        let mut moved: HashMap<u32, Vec3> = HashMap::new();
        // Corners on a single neighbour with no corner to go to (a V notch's apex): the
        // neighbour's gap stretch closes straight past them.
        let mut dropped: HashSet<u32> = HashSet::new();
        for v in &gap_vertices {
            let Some(fs) = vfaces.get(v) else { continue };
            let keep: Vec<usize> = fs.iter().copied().filter(|x| !del.contains(x)).collect();
            if keep.is_empty() || keep.len() >= 3 {
                continue;
            }
            // A corner of a loop that goes whole goes with it.
            if keep.iter().all(|x| kept_whole.contains(x)) {
                continue;
            }
            // A corner on a cylinder that closes round stays where it is (the arcs meet there).
            if keep.iter().any(|x| wraps.contains_key(x)) && keep.iter().all(|x| planes.contains_key(x) || wraps.contains_key(x)) {
                continue;
            }
            if keep.iter().any(|x| !planes.contains_key(x) && !movers.contains_key(x)) {
                let f = fs.iter().find(|x| del.contains(x)).copied().unwrap_or(0);
                return Err(fail(format!("a corner of face {f} touches a face that cannot move")));
            }
            let p = asm.point(*v).ok_or_else(|| fail("vertex"))?;
            let best = corners.iter().filter(|c| keep.iter().all(|x| on(**c, *x))).min_by(|a, b| a.dist(p).total_cmp(&b.dist(p)));
            if let Some(c) = best {
                moved.insert(*v, *c);
            } else if let Some(m) = keep.iter().find(|x| movers.contains_key(x)) {
                curved = true;
                let f = fs.iter().find(|x| del.contains(x)).copied().unwrap_or(0);
                return Err(fail(format!(
                    "face {f} ({}), borders face {m}, {}: include face {m} in the selection if it should go too, or take off the feature that made face {m} in the timeline instead; only planar neighbours can extend to close the gap",
                    surface_kind(&rec, f),
                    surface_kind(&rec, *m),
                )));
            } else if keep.len() == 1 {
                dropped.insert(*v);
            }
        }
        let q = size * 1e-7;
        let mut shared = Shared { q, vertices: HashMap::new(), lines: HashMap::new(), arcs: HashMap::new() };
        // The deleted face each closing edge stands in for, to name it if the gap cannot close.
        let mut closed_by: HashMap<u32, usize> = HashMap::new();
        for (i, v) in asm.vertices.iter().enumerate() {
            if !moved.contains_key(&(i as u32)) {
                shared.vertices.insert(key(Vec3::from(v.point), q), i as u32);
            }
        }
        let mut history: Vec<(usize, FaceProv)> = Vec::new();
        let mut rebuilt: Vec<(usize, AFace)> = Vec::new();
        for fi in 0..n {
            if del.contains(&fi) || merged_into.contains_key(&fi) {
                continue;
            }
            let f = asm.faces.get(fi).and_then(Option::as_ref).ok_or_else(|| fail("face"))?.clone();
            if !neighbours.contains(&fi) {
                // Untouched, unless one of its corners moves.
                if f.loops.iter().flatten().any(|u| corner_moves(&asm, &moved, u)) {
                    return Err(fail(format!("face {fi} meets the gap only at a corner; it cannot follow")));
                }
                history.push((fi, crate::record::from_face(0, fi, Change::Kept)));
                continue;
            }
            let mut loops: Vec<Vec<AUse>> = Vec::new();
            for l in &f.loops {
                let gaps = l.iter().filter(|u| gap_edge(u.edge)).count();
                if gaps == 0 && !l.iter().any(|u| corner_moves(&asm, &moved, u)) {
                    loops.push(l.clone());
                    continue;
                }
                if let Some(&(o, ax, _)) = wraps.get(&fi)
                    && gaps == 2
                {
                    // A cylinder closing round: the first ruling on the gap (p to q) gives way to
                    // the arc from p round over the gap to the second ruling's end s, that ruling
                    // back from s to r (it stays as the seam, used both ways), and the arc from r
                    // over the gap to q.
                    let on: Vec<usize> = (0..l.len()).filter(|k| l.get(*k).is_some_and(|u| gap_edge(u.edge))).collect();
                    let (Some(&i), Some(&j)) = (on.first(), on.get(1)) else { return Err(fail("edge")) };
                    let (Some(ui), Some(uj)) = (l.get(i), l.get(j)) else { return Err(fail("edge")) };
                    let (vp, vq) = asm.ends(ui).ok_or_else(|| fail("edge"))?;
                    let (vr, vs) = asm.ends(uj).ok_or_else(|| fail("edge"))?;
                    let pt = |v: u32| asm.point(v).ok_or_else(|| fail("vertex"));
                    let (p, pq, r, ps) = (pt(vp)?, pt(vq)?, pt(vr)?, pt(vs)?);
                    let centre = |x: Vec3| o + ax * (x - o).dot(ax);
                    let (c1, c2) = (centre(p), centre(r));
                    if c1.dist(centre(ps)) > q * 10.0 || c2.dist(centre(pq)) > q * 10.0 {
                        return Err(fail(format!("face {fi}: the gap's rulings do not end on the same circles")));
                    }
                    let by = efaces.get(&ui.edge).and_then(|fs| fs.iter().find(|x| del.contains(x)).copied());
                    let out = by.and_then(outward).ok_or_else(|| fail(format!("face {fi}: the gap over it is not flat")))?;
                    let top = shared.arc(&mut asm, p, ps, c1, ax, out)?;
                    let bottom = shared.arc(&mut asm, r, pq, c2, ax, out)?;
                    // Every use's pcurve is made again on the face's new form (the record's
                    // pcurves are on its old, narrower one).
                    // The second gap ruling is the seam now.
                    let mut edges = Vec::with_capacity(l.len() + 2);
                    for (k, u) in l.iter().enumerate() {
                        if k == i {
                            edges.push(top.clone());
                            edges.push(AUse { edge: uj.edge, forward: !uj.forward, given: None });
                            edges.push(bottom.clone());
                        } else if !gap_edge(u.edge) && seam_of(l, u.edge) {
                            continue;
                        } else {
                            edges.push(AUse { given: None, ..u.clone() });
                        }
                    }
                    if let Some(d) = by {
                        closed_by.entry(top.edge).or_insert(d);
                        closed_by.entry(bottom.edge).or_insert(d);
                    }
                    loops.push(edges);
                    continue;
                }
                if gaps == l.len() {
                    // A hole or boss footprint disappears when its walls are removed. A face
                    // surrounded by rounds does not: its whole boundary moves back to the
                    // intersections of the remaining planes (#1557), its restored edges shared
                    // with the adjacent rebuilt faces.
                    if !l.iter().any(|u| corner_moves(&asm, &moved, u)) {
                        continue;
                    }
                    let mut boundary = Vec::new();
                    for u in l {
                        let (a, b) = asm.ends(u).ok_or_else(|| fail("edge"))?;
                        let (Some(pa), Some(pb)) = (moved.get(&a).copied(), moved.get(&b).copied()) else {
                            return Err(fail(format!("face {fi} has a closed gap boundary that cannot be restored completely")));
                        };
                        if pa.dist(pb) > q * 10.0 {
                            let restored = shared.line(&mut asm, pa, pb)?;
                            asm.piece(restored.edge, u.edge);
                            boundary.push(restored);
                        }
                    }
                    if !boundary.is_empty() {
                        loops.push(boundary);
                    }
                    continue;
                }
                // Start after a gap stretch so stretches don't wrap around.
                let m = l.len();
                let is_gap = |k: usize| l.get(k).is_some_and(|u| gap_edge(u.edge));
                let start = (0..m).find(|i| is_gap((i + m - 1) % m) && !is_gap(*i)).unwrap_or(0);
                let mut edges: Vec<AUse> = Vec::new();
                // The moved corners along a gap stretch.
                let mut pending: Option<Vec<Vec3>> = None;
                let mut pending_by: Option<usize> = None;
                for k in 0..m {
                    let Some(u) = l.get((start + k) % m) else { continue };
                    let (va, vb) = asm.ends(u).ok_or_else(|| fail("edge"))?;
                    let (a, b) = (position(&asm, &moved, va)?, position(&asm, &moved, vb)?);
                    if gap_edge(u.edge) {
                        let stretch = pending.get_or_insert_with(Vec::new);
                        stretch.extend([(va, a), (vb, b)].into_iter().filter(|(v, _)| !dropped.contains(v)).map(|(_, p)| p));
                        pending_by = pending_by.or_else(|| efaces.get(&u.edge).and_then(|fs| fs.iter().find(|x| del.contains(x)).copied()));
                        continue;
                    }
                    if let Some(p) = pending.take() {
                        let by = pending_by.take();
                        for line in closing(&mut shared, &mut asm, p, a, planes.get(&fi).copied(), by.and_then(outward), &wraps)? {
                            if let Some(d) = by {
                                closed_by.entry(line.edge).or_insert(d);
                            }
                            edges.push(line);
                        }
                    }
                    if !corner_moves(&asm, &moved, u) {
                        edges.push(u.clone());
                    } else if matches!(asm.edges.get(u.edge as usize).map(|e| &e.curve), Some(Curve::Line { .. })) {
                        if a.dist(b) > q * 10.0 {
                            // A corner taken on the plane neighbour's other ruling would pull this
                            // edge off the cylinder.
                            if let Some(&(_, ax, _)) = movers.get(&fi)
                                && (b - a).cross(ax).len() > size * 1e-6
                            {
                                curved = true;
                                return Err(fail(format!("face {fi} ({}): a straight edge on it would leave its surface", surface_kind(&rec, fi))));
                            }
                            let stretched = shared.line(&mut asm, a, b)?;
                            asm.piece(stretched.edge, u.edge);
                            edges.push(stretched);
                        }
                    } else if let Some((c, ax, mid)) = movers.values().find_map(|&(o, ax, r)| {
                        let h = (a - o).dot(ax);
                        let edge = asm.edges.get(u.edge as usize)?;
                        let ev = solidcraft_geom::brep::eval::CurveEval::new(&edge.curve).ok()?;
                        let [t0, t1] = edge.interval;
                        let pts: Vec<Vec3> = (0..=4).map(|i| ev.point(t0 + (t1 - t0) * f64::from(i) / 4.0)).collect::<Option<_>>()?;
                        let tol = size * 1e-6;
                        let round = pts.iter().all(|p| {
                            let along = (*p - o).dot(ax);
                            (along - h).abs() <= tol && (((*p - o) - ax * along).len() - r).abs() <= tol
                        });
                        (round && ((b - o).dot(ax) - h).abs() <= tol).then(|| Some((o + ax * h, ax, *pts.get(2)?))).flatten()
                    }) {
                        if a.dist(b) > q * 10.0 {
                            let stretched = shared.arc(&mut asm, a, b, c, ax, mid - (a + b) * 0.5)?;
                            asm.piece(stretched.edge, u.edge);
                            edges.push(stretched);
                        }
                    } else {
                        return Err(fail(format!("face {fi} has a curved edge that would have to stretch")));
                    }
                }
                // A gap stretch at the end closes back to the first edge.
                if let (Some(p), Some(first)) = (pending, edges.first()) {
                    let (v0, _) = asm.ends(first).ok_or_else(|| fail("edge"))?;
                    let a = asm.point(v0).ok_or_else(|| fail("vertex"))?;
                    for line in closing(&mut shared, &mut asm, p, a, planes.get(&fi).copied(), pending_by.and_then(outward), &wraps)? {
                        if let Some(d) = pending_by {
                            closed_by.entry(line.edge).or_insert(d);
                        }
                        edges.push(line);
                    }
                }
                if edges.is_empty() {
                    return Err(fail(format!("face {fi} would lose its boundary")));
                }
                loops.push(edges);
            }
            if loops.is_empty() {
                return Err(fail(format!("face {fi} would lose its boundary")));
            }
            // The face keeps its plane: every moved corner lies on it.
            // A curved neighbour only lost whole loops: its parameter box stands.
            let domain = if kept_whole.contains(&fi) { f.domain } else { None };
            // A cylinder closing round takes a new form over its whole turn.
            let fresh = wraps.contains_key(&fi) || movers.contains_key(&fi);
            let nurbs = if fresh { None } else { f.nurbs.clone() };
            // Stretched edges leave the old form and pcurves wrong.
            if movers.contains_key(&fi) {
                for u in loops.iter_mut().flatten() {
                    u.given = None;
                }
            }
            rebuilt.push((fi, AFace { loops, domain, nurbs, ..f }));
            let mut prov = crate::record::from_face(0, fi, Change::Modified);
            for (g, _) in merged_into.iter().filter(|(_, t)| **t == fi) {
                prov.origins.extend(crate::record::from_face(0, *g, Change::Modified).origins);
            }
            history.push((fi, prov));
        }
        for &fi in &del {
            if let Some(slot) = asm.faces.get_mut(fi) {
                *slot = None;
            }
        }
        for (fi, face) in rebuilt {
            if let Some(slot) = asm.faces.get_mut(fi) {
                *slot = Some(face);
            }
        }
        let fallback = del.first().copied().unwrap_or(0);
        fill_on_planes(&mut asm, &planes, size * 1e-6, &mut history, &closed_by, fallback)?;
        history.sort_by_key(|(fi, _)| *fi);
        let finished = asm.finish().map_err(|e| fail(format!("the neighbouring faces do not close the gap ({e})")))?;
        crate::assemble::body_with_history(finished, &asm, body, history.into_iter().map(|(_, h)| h).collect())
    });
    match healed {
        // The planar healer's refusal stands where the faces are not fillets or chamfers SMLib
        // can take off.
        Err(refusal) if curved => {
            #[cfg(test)]
            FALLBACKS.with(|n| n.set(n.get() + 1));
            let chosen: Vec<usize> = del.iter().copied().collect();
            crate::offset_backend::delete_fillet_faces(body, &chosen).map_err(|e| {
                log::debug!(target: "smlib", "delete face: fillet removal refused too: {e}");
                refusal
            })
        }
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use solidcore_record::Vec3;
    use solidcraft_geom::MeshGeom;

    use super::delete_faces;
    use crate::{BoolOp, boolean, box_solid, chamfer, cylinder, fillet};

    fn volume(b: &crate::Body) -> f64 {
        b.mesh(&crate::MeshTolerance::chord(1e-3)).unwrap().measure().volume
    }

    /// The faces of `b` with every triangle corner passing `keep`.
    fn faces_where(b: &crate::Body, keep: impl Fn(Vec3) -> bool) -> Vec<usize> {
        let m = b.mesh(&crate::MeshTolerance::chord(1e-3)).unwrap();
        let n = m.tri_face.iter().max().map(|x| *x as usize + 1).unwrap_or(0);
        (0..n)
            .filter(|f| {
                m.triangles.iter().zip(&m.tri_face).filter(|(_, x)| **x as usize == *f).all(|(t, _)| m.tri(t).unwrap().iter().all(|p| keep(*p)))
            })
            .collect()
    }

    #[test]
    fn issue1557_delete_closed_fillet_ring_keeps_the_floor() {
        let b = box_solid(Vec3::ZERO, Vec3::new(25.0, 25.0, 10.0)).unwrap();
        let picks = [Vec3::new(12.5, 0.0, 0.0), Vec3::new(25.0, 12.5, 0.0), Vec3::new(12.5, 25.0, 0.0), Vec3::new(0.0, 12.5, 0.0)];
        let rounded = fillet(&b, &picks, 2.0).unwrap();
        let record = crate::brep_record(&rounded).unwrap();
        let rounds: Vec<usize> =
            (0..record.faces.len()).filter(|i| !matches!(crate::offset::surf_of(&record, *i), Some(crate::offset::Surf::Plane { .. }))).collect();
        assert_eq!(rounds.len(), 4);
        let restored = delete_faces(&rounded, &rounds).unwrap();
        assert_eq!(restored.face_count(), 6);
        assert_eq!(restored.unmeshed_faces(), 0);
        assert!((volume(&restored) - 6250.0).abs() < 1e-6);
        assert_eq!(restored.classify_point(Vec3::new(0.1, 0.1, 0.1)).unwrap(), crate::PointClass::Inside);
    }

    #[test]
    fn delete_fillet_chamfer_hole_and_boss_faces() {
        let sharp = 24000.0;
        let b = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 20.0)).unwrap();
        // A fillet on a box edge: deleting it gives the sharp box back, exactly.
        let f = fillet(&b, &[Vec3::new(20.0, 30.0, 20.0)], 5.0).unwrap();
        assert!(volume(&f) < sharp - 1.0);
        let fi = faces_where(&f, |p| p.y > 25.0 - 1e-6 && p.z > 15.0 - 1e-6 && (p.y < 30.0 - 1e-9 || p.z < 20.0 - 1e-9));
        assert_eq!(fi.len(), 1, "{fi:?}");
        let back = delete_faces(&f, &fi).unwrap();
        assert!((volume(&back) - sharp).abs() < 1e-9 * sharp, "{}", volume(&back));
        assert_eq!(back.face_count(), 6);
        // A chamfer likewise.
        let c = chamfer(&b, &[Vec3::new(20.0, 30.0, 20.0)], 4.0).unwrap();
        let ci = faces_where(&c, |p| p.y > 26.0 - 1e-6 && p.z > 16.0 - 1e-6 && (p.y < 30.0 - 1e-9 || p.z < 20.0 - 1e-9));
        let back = delete_faces(&c, &ci).unwrap();
        assert!((volume(&back) - sharp).abs() < 1e-9 * sharp, "{}", volume(&back));
        // A through hole: deleting its wall fills it.
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 10.0)).unwrap();
        let pin = cylinder(Vec3::new(20.0, 15.0, -1.0), Vec3::Z, 4.0, 12.0).unwrap();
        let holed = boolean(&plate, &pin, BoolOp::Cut).unwrap().unwrap();
        let wall = faces_where(&holed, |p| ((p.x - 20.0).hypot(p.y - 15.0) - 4.0).abs() < 1e-2);
        assert!(!wall.is_empty());
        let filled = delete_faces(&holed, &wall).unwrap();
        assert!((volume(&filled) - 12000.0).abs() < 1e-9 * 12000.0, "{}", volume(&filled));
        // A blind hole: its wall and floor.
        let drill = cylinder(Vec3::new(20.0, 15.0, 4.0), Vec3::Z, 3.0, 7.0).unwrap();
        let blind = boolean(&plate, &drill, BoolOp::Cut).unwrap().unwrap();
        let hole = faces_where(&blind, |p| (p.x - 20.0).hypot(p.y - 15.0) < 3.0 + 1e-3 && p.z > 4.0 - 1e-6);
        let filled = delete_faces(&blind, &hole).unwrap();
        assert!((volume(&filled) - 12000.0).abs() < 1e-9 * 12000.0, "{}", volume(&filled));
        // A boss on the plate: deleting its wall and top takes it off.
        let boss = cylinder(Vec3::new(10.0, 10.0, 10.0), Vec3::Z, 3.0, 5.0).unwrap();
        let bossed = boolean(&plate, &boss, BoolOp::Union).unwrap().unwrap();
        let bf = faces_where(&bossed, |p| p.z > 10.0 - 1e-9 && (p.x - 10.0).hypot(p.y - 10.0) < 3.0 + 1e-6);
        let off = delete_faces(&bossed, &bf).unwrap();
        assert!((volume(&off) - 12000.0).abs() < 1e-9 * 12000.0, "{}", volume(&off));
        // A curved neighbour is refused, naming both faces' kinds and the way out.
        let rounded = fillet(&b, &[Vec3::new(20.0, 30.0, 20.0)], 5.0).unwrap();
        let round = faces_where(&rounded, |p| p.y > 25.0 - 1e-6 && p.z > 15.0 - 1e-6 && (p.y < 30.0 - 1e-9 || p.z < 20.0 - 1e-9));
        let top = faces_where(&rounded, |p| (p.z - 20.0).abs() < 1e-9 && (p.y - 30.0).hypot(p.x - 20.0) > 5.0 + 1e-6);
        let top = top.first().copied().unwrap();
        let why = delete_faces(&rounded, &[top]).unwrap_err().to_string();
        assert!(why.contains(&format!("face {top} (a plane), borders face {}, a cylinder", round[0])), "{why}");
        assert!(why.contains("the timeline"), "{why}");

        // Errors name what is wrong.
        assert!(delete_faces(&plate, &[99]).is_err());
        assert!(delete_faces(&plate, &[0, 1, 2, 3, 4, 5]).is_err());
        // One face of a box: the sides cannot close it.
        let top = faces_where(&plate, |p| (p.z - 10.0).abs() < 1e-9);
        let refused = delete_faces(&plate, &top).unwrap_err().to_string();
        assert!(refused.contains("cannot close: no neighbouring plane extends across it"), "{refused}");
        // The refusal names the face deleted, not face 0.
        let bottom = faces_where(&plate, |p| p.z.abs() < 1e-9);
        assert_eq!(bottom.len(), 1);
        assert_ne!(bottom[0], 0);
        let refused = delete_faces(&plate, &bottom).unwrap_err().to_string();
        assert!(refused.contains(&format!("the gap left by face {} cannot close", bottom[0])), "{refused}");
    }

    /// A half-space `n · x <= d` with `n` made unit.
    fn half(n: Vec3, d: f64) -> crate::HalfSpace {
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        crate::HalfSpace { n: n / len, d: d / len }
    }

    /// #14257, #15775: two faces of a V notch or groove whose neighbours across the gap lie on
    /// one plane (the wall either side of the notch, the top either side of the groove) left an
    /// open shell. That plane now extends across the gap and the faces on it become one.
    #[test]
    fn deleting_a_v_notch_or_groove_extends_the_plane_across_it() {
        let block = box_solid(Vec3::ZERO, Vec3::new(40.0, 20.0, 10.0)).unwrap();
        // A V notch through the front wall: mouth x 15..25 at y = 0, apex at (20, 5).
        let wedge = crate::convex_polyhedron(&[
            half(Vec3::new(0.0, 0.0, -1.0), 1.0),
            half(Vec3::new(0.0, 0.0, 1.0), 11.0),
            half(Vec3::new(0.0, -1.0, 0.0), 1.0),
            half(Vec3::new(-1.0, 1.0, 0.0), -15.0),
            half(Vec3::new(1.0, 1.0, 0.0), 25.0),
        ])
        .unwrap();
        let notched = boolean(&block, &wedge, BoolOp::Cut).unwrap().unwrap();
        assert!((volume(&notched) - 7750.0).abs() < 1e-6);
        let sides = faces_where(&notched, |p| ((p.x - 20.0).abs() + p.y - 5.0).abs() < 1e-3);
        assert_eq!(sides.len(), 2, "{sides:?}");
        let back = delete_faces(&notched, &sides).unwrap();
        assert!((volume(&back) - 8000.0).abs() < 1e-6, "{}", volume(&back));
        assert_eq!(back.face_count(), 6);
        assert_eq!(back.unmeshed_faces(), 0);
        assert_eq!(back.classify_point(Vec3::new(20.0, 1.0, 5.0)).unwrap(), crate::PointClass::Inside);
        // A V groove along x across the top: mouth y 5..15 at z = 10, apex at z = 5.
        let cutter = crate::convex_polyhedron(&[
            half(Vec3::new(-1.0, 0.0, 0.0), 1.0),
            half(Vec3::new(1.0, 0.0, 0.0), 41.0),
            half(Vec3::new(0.0, 0.0, 1.0), 11.0),
            half(Vec3::new(0.0, -1.0, -1.0), -15.0),
            half(Vec3::new(0.0, 1.0, -1.0), 5.0),
        ])
        .unwrap();
        let grooved = boolean(&block, &cutter, BoolOp::Cut).unwrap().unwrap();
        assert!((volume(&grooved) - 7000.0).abs() < 1e-6);
        let walls = faces_where(&grooved, |p| (p.y + p.z - 15.0).abs() < 1e-3 || (p.y - p.z - 5.0).abs() < 1e-3);
        assert_eq!(walls.len(), 2, "{walls:?}");
        let back = delete_faces(&grooved, &walls).unwrap();
        assert!((volume(&back) - 8000.0).abs() < 1e-6, "{}", volume(&back));
        assert_eq!(back.face_count(), 6);
        assert_eq!(back.classify_point(Vec3::new(20.0, 10.0, 9.0)).unwrap(), crate::PointClass::Inside);
    }

    /// #16864 review: faces merged into the filled face keep no pcurves from their own surface
    /// frames. Two notched halves, the right one turned half a turn about y: their front faces
    /// lie on one plane, either side of the notch, with opposite u axes.
    #[test]
    fn filling_across_faces_on_two_surfaces_remakes_their_pcurves() {
        let wedge = crate::convex_polyhedron(&[
            half(Vec3::new(0.0, 0.0, -1.0), 1.0),
            half(Vec3::new(0.0, 0.0, 1.0), 11.0),
            half(Vec3::new(0.0, -1.0, 0.0), 1.0),
            half(Vec3::new(-1.0, 1.0, 0.0), -15.0),
            half(Vec3::new(1.0, 1.0, 0.0), 25.0),
        ])
        .unwrap();
        let left = box_solid(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0)).unwrap();
        let right = box_solid(Vec3::new(20.0, 0.0, 0.0), Vec3::new(40.0, 20.0, 10.0)).unwrap();
        let right = crate::transform(&right, Vec3::ZERO, Vec3::new(30.0, 10.0, 5.0), Vec3::new(0.0, 1.0, 0.0), std::f64::consts::PI).unwrap();
        let left = boolean(&left, &wedge, BoolOp::Cut).unwrap().unwrap();
        let right = boolean(&right, &wedge, BoolOp::Cut).unwrap().unwrap();
        let notched = boolean(&left, &right, BoolOp::Union).unwrap().unwrap();
        let fronts = faces_where(&notched, |p| p.y.abs() < 1e-9);
        let surfaces: Vec<_> = fronts.iter().map(|f| notched.record().faces[*f].surface.clone()).collect();
        assert_eq!(fronts.len(), 2, "{fronts:?}");
        assert_ne!(surfaces[0], surfaces[1], "the fronts must be on two surfaces for this test");
        let sides = faces_where(&notched, |p| ((p.x - 20.0).abs() + p.y - 5.0).abs() < 1e-3);
        assert_eq!(sides.len(), 2, "{sides:?}");
        let back = delete_faces(&notched, &sides).unwrap();
        assert!((volume(&back) - 8000.0).abs() < 1e-6, "{}", volume(&back));
        assert_eq!(faces_where(&back, |p| p.y.abs() < 1e-9).len(), 1);
        solidcore_ops::record_check::agreement(back.record()).unwrap();
    }

    /// #16864 review: a sheet's own free boundary is not part of the gap. An open box (no top)
    /// with a chamfered bottom edge: deleting the chamfer heals the corner and leaves the rim open.
    #[test]
    fn deleting_on_a_sheet_leaves_its_free_boundary_open() {
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 20.0, 10.0)).unwrap();
        let cham = chamfer(&plate, &[Vec3::new(20.0, 0.0, 0.0)], 2.0).unwrap();
        let top = faces_where(&cham, |p| (p.z - 10.0).abs() < 1e-9);
        assert_eq!(top.len(), 1);
        let open_box = crate::remove_faces(&cham, &top).unwrap();
        let bevel = faces_where(&open_box, |p| (p.y + p.z - 2.0).abs() < 1e-3);
        assert_eq!(bevel.len(), 1, "{bevel:?}");
        let healed = delete_faces(&open_box, &bevel).unwrap();
        assert_eq!(healed.face_count(), 5);
        assert!(faces_where(&healed, |p| (p.z - 10.0).abs() < 1e-9).is_empty());
        assert_eq!(faces_where(&healed, |p| p.y.abs() < 1e-9).len(), 1);
    }

    /// #16864 review: two V grooves either side of one strip of the top. The first fill merges
    /// the strip into a face; the second must find that face, not the strip's old slot.
    #[test]
    fn two_grooves_beside_one_strip_both_fill() {
        let block = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 10.0)).unwrap();
        let groove = |left: f64, right: f64| {
            crate::convex_polyhedron(&[
                half(Vec3::new(-1.0, 0.0, 0.0), 1.0),
                half(Vec3::new(1.0, 0.0, 0.0), 41.0),
                half(Vec3::new(0.0, 0.0, 1.0), 11.0),
                half(Vec3::new(0.0, -1.0, -1.0), -left),
                half(Vec3::new(0.0, 1.0, -1.0), right),
            ])
            .unwrap()
        };
        let one = boolean(&block, &groove(14.0, 2.0), BoolOp::Cut).unwrap().unwrap();
        let two = boolean(&one, &groove(28.0, 16.0), BoolOp::Cut).unwrap().unwrap();
        assert!((volume(&two) - 10720.0).abs() < 1e-6, "{}", volume(&two));
        // Each wall by its own plane: a top strip has every corner on one wall plane or another.
        let walls: Vec<usize> = [(1.0, 14.0), (-1.0, 2.0), (1.0, 28.0), (-1.0, 16.0)]
            .iter()
            .flat_map(|(s, d)| faces_where(&two, |p| (p.y + s * p.z - d).abs() < 1e-3))
            .collect();
        assert_eq!(walls.len(), 4, "{walls:?}");
        let back = delete_faces(&two, &walls).unwrap();
        assert!((volume(&back) - 12000.0).abs() < 1e-6, "{}", volume(&back));
        assert_eq!(back.face_count(), 6);
        assert_eq!(back.classify_point(Vec3::new(20.0, 8.0, 9.0)).unwrap(), crate::PointClass::Inside);
        assert_eq!(back.classify_point(Vec3::new(20.0, 22.0, 9.0)).unwrap(), crate::PointClass::Inside);
    }

    /// A round whose neighbour is curved (a boss's foot or top round, against the boss's
    /// cylinder) is taken off by SMLib's fillet removal where the planar healer cannot extend
    /// the cylinder: the sharp boss comes back.
    #[test]
    fn deleting_a_round_beside_a_cylinder_gives_the_sharp_boss() {
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 5.0)).unwrap();
        let boss = cylinder(Vec3::new(20.0, 15.0, 5.0), Vec3::Z, 5.0, 8.0).unwrap();
        let bossed = boolean(&plate, &boss, BoolOp::Union).unwrap().unwrap();
        let sharp = 6000.0 + 200.0 * std::f64::consts::PI;
        let round_at = |b: &crate::Body| {
            let record = crate::brep_record(b).unwrap();
            (0..record.faces.len())
                .filter(|i| {
                    !matches!(crate::offset::surf_of(&record, *i), Some(crate::offset::Surf::Plane { .. } | crate::offset::Surf::Cylinder { .. }))
                })
                .collect::<Vec<usize>>()
        };
        for edge in [Vec3::new(25.0, 15.0, 5.0), Vec3::new(25.0, 15.0, 13.0)] {
            let rounded = fillet(&bossed, &[edge], 1.0).unwrap();
            let round = round_at(&rounded);
            assert_eq!(round.len(), 1, "{edge:?}: {round:?}");
            let back = delete_faces(&rounded, &round).unwrap();
            // Exact, not the meshed volume: a 1e-3 chord loses ~0.06 on the Ø10 boss.
            let v = back.volume_at(1e-3).unwrap();
            assert!((v - sharp).abs() < 1e-6 * sharp, "{edge:?}: {v}");
            assert_eq!(back.face_count(), 8, "{edge:?}");
            assert_eq!(back.unmeshed_faces(), 0);
        }
    }

    /// A hole's chamfer (a cone between the plate's top and the hole wall, a curved
    /// neighbour the planar healer cannot extend) is taken off by SMLib's fillet removal as
    /// a chamfer: the plain hole comes back, its volume and faces as before the chamfer.
    #[test]
    fn deleting_a_hole_chamfer_gives_the_plain_hole() {
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 5.0)).unwrap();
        let pin = cylinder(Vec3::new(20.0, 15.0, -1.0), Vec3::Z, 3.0, 7.0).unwrap();
        let holed = boolean(&plate, &pin, BoolOp::Cut).unwrap().unwrap();
        let plain = holed.volume_at(1e-3).unwrap();
        let chamfered = chamfer(&holed, &[Vec3::new(23.0, 15.0, 5.0)], 1.0).unwrap();
        let record = crate::brep_record(&chamfered).unwrap();
        let cone: Vec<usize> =
            (0..record.faces.len()).filter(|i| matches!(crate::offset::surf_of(&record, *i), Some(crate::offset::Surf::Cone { .. }))).collect();
        assert_eq!(cone.len(), 1, "{cone:?}");
        let back = delete_faces(&chamfered, &cone).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - plain).abs() < 1e-6 * plain, "{v} vs {plain}");
        assert_eq!(back.face_count(), holed.face_count());
        assert_eq!(back.unmeshed_faces(), 0);
    }

    /// A rod Ø20, 40 long, with a cross hole Ø4 through it at mid-height: deleting the hole's
    /// wall gives the whole rod back, 4000π. The wall's neighbour at both ends is the rod's
    /// cylinder (the #6121 class: a hole into a curved face).
    #[test]
    fn deleting_a_cross_hole_wall_gives_the_whole_rod() {
        let rod = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let pin = cylinder(Vec3::new(-11.0, 0.0, 20.0), Vec3::X, 2.0, 22.0).unwrap();
        let holed = boolean(&rod, &pin, BoolOp::Cut).unwrap().unwrap();
        let wall = faces_where(&holed, |p| (p.y.hypot(p.z - 20.0) - 2.0).abs() < 1e-2);
        assert!(!wall.is_empty());
        let whole = 4000.0 * std::f64::consts::PI;
        let back = delete_faces(&holed, &wall).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 3);
    }

    /// A shaft Ø20, 40 long, with a flat at x = 8: deleting the flat gives the round shaft
    /// back, 4000π. The flat's neighbours are the two end caps (planes) and the shaft's
    /// cylinder, which has to close over the flat's place (the rest of #5368: a curved
    /// neighbour that extends).
    #[test]
    fn deleting_a_flat_on_a_shaft_gives_the_round_shaft() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(8.0, -11.0, -1.0), Vec3::new(13.0, 11.0, 41.0)).unwrap();
        let flatted = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let flat = faces_where(&flatted, |p| (p.x - 8.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        let whole = 4000.0 * std::f64::consts::PI;
        let back = delete_faces(&flatted, &flat).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 3);
        sound(&back);
    }

    /// A boss Ø10, 8 high, on a 40 × 30 × 5 plate, with a flat at x = 23 down to the plate:
    /// deleting the flat gives the round boss back (6000 + 200π). The plate's top closes over
    /// the flat's foot with an arc of the boss's circle, as the boss's top does at its head.
    #[test]
    fn deleting_a_flat_on_a_boss_gives_the_round_boss() {
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 5.0)).unwrap();
        let boss = cylinder(Vec3::new(20.0, 15.0, 5.0), Vec3::Z, 5.0, 8.0).unwrap();
        let bossed = boolean(&plate, &boss, BoolOp::Union).unwrap().unwrap();
        let cut = box_solid(Vec3::new(23.0, 9.0, 5.0), Vec3::new(26.0, 21.0, 14.0)).unwrap();
        let flatted = boolean(&bossed, &cut, BoolOp::Cut).unwrap().unwrap();
        let flat = faces_where(&flatted, |p| (p.x - 23.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        let whole = 6000.0 + 200.0 * std::f64::consts::PI;
        let back = delete_faces(&flatted, &flat).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 8);
        assert_eq!(back.unmeshed_faces(), 0);
        sound(&back);
    }

    /// The cross hole's rim on the rod chamfered 0.5: the chamfer lies between two cylinders,
    /// whose meeting curve is no circle, so it is no D3 ring and the healer has no edge for it.
    /// Deleting it is refused (the plain hole would need the cylinders' intersection curve).
    #[test]
    fn deleting_a_cross_hole_chamfer_is_refused() {
        let rod = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let pin = cylinder(Vec3::new(-11.0, 0.0, 20.0), Vec3::X, 2.0, 22.0).unwrap();
        let holed = boolean(&rod, &pin, BoolOp::Cut).unwrap().unwrap();
        let x = (100.0f64 - 4.0).sqrt();
        let chamfered = chamfer(&holed, &[Vec3::new(x, 2.0, 20.0)], 0.5).unwrap();
        let record = crate::brep_record(&chamfered).unwrap();
        let new: Vec<usize> = (0..chamfered.face_count())
            .filter(|i| {
                !matches!(crate::offset::surf_of(&record, *i), Some(crate::offset::Surf::Plane { .. } | crate::offset::Surf::Cylinder { .. }))
            })
            .collect();
        assert_eq!(new.len(), 1, "{new:?}");
        assert!(delete_faces(&chamfered, &new).is_err());
    }

    /// The record's pcurves agree with its edges and the body checks clean.
    fn sound(b: &crate::Body) {
        let record = crate::brep_record(b).unwrap();
        solidcore_ops::record_check::agreement(&record).unwrap();
        let problems = b.validity();
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A flat past the axis (x = −3 on a shaft Ø20, more than half the turn cut away): the
    /// cylinder closes round the long way, over where the material went, and the round shaft
    /// comes back, 4000π.
    #[test]
    fn deleting_a_flat_past_the_axis_gives_the_round_shaft() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(-3.0, -11.0, -1.0), Vec3::new(11.0, 11.0, 41.0)).unwrap();
        let flatted = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let flat = faces_where(&flatted, |p| (p.x + 3.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        let whole = 4000.0 * std::f64::consts::PI;
        let back = delete_faces(&flatted, &flat).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 3);
        sound(&back);
    }

    /// A flat pocketed into the middle of a shaft (x = 8, z 10 to 30, with end walls):
    /// deleting the flat alone is refused. Its end walls would have to close to whole discs
    /// inside the solid; the pocket goes by deleting the flat with its end walls.
    #[test]
    fn deleting_a_pocket_flat_alone_is_refused() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(8.0, -11.0, 10.0), Vec3::new(11.0, 11.0, 30.0)).unwrap();
        let pocketed = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let flat = faces_where(&pocketed, |p| (p.x - 8.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        assert!(delete_faces(&pocketed, &flat).is_err());
    }

    #[test]
    fn deleting_a_flat_away_from_the_seam_gives_the_round_shaft() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(-11.0, -11.0, -1.0), Vec3::new(-8.0, 11.0, 41.0)).unwrap();
        let flatted = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let flat = faces_where(&flatted, |p| (p.x + 8.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        let whole = 4000.0 * std::f64::consts::PI;
        let back = delete_faces(&flatted, &flat).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 3);
        sound(&back);
    }

    #[test]
    fn deleting_a_chamfer_on_a_shaft_flat_gives_the_flatted_shaft() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(8.0, -11.0, -1.0), Vec3::new(13.0, 11.0, 41.0)).unwrap();
        let flatted = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let sharp = flatted.volume_at(1e-3).unwrap();
        let chamfered = chamfer(&flatted, &[Vec3::new(8.0, 0.0, 40.0)], 1.0).unwrap();
        assert_eq!(chamfered.face_count(), flatted.face_count() + 1);
        let bevel = faces_where(&chamfered, |p| (p.x + p.z - 47.0).abs() < 1e-4);
        assert_eq!(bevel.len(), 1, "{bevel:?}");
        let back = delete_faces(&chamfered, &bevel).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - sharp).abs() < 1e-6 * sharp, "{v} vs {sharp}");
        assert_eq!(back.face_count(), flatted.face_count());
        sound(&back);
    }

    #[test]
    fn a_round_beside_a_cylinder_still_goes_to_fillet_removal() {
        let plate = box_solid(Vec3::ZERO, Vec3::new(40.0, 30.0, 5.0)).unwrap();
        let boss = cylinder(Vec3::new(20.0, 15.0, 5.0), Vec3::Z, 5.0, 8.0).unwrap();
        let bossed = boolean(&plate, &boss, BoolOp::Union).unwrap().unwrap();
        let rounded = fillet(&bossed, &[Vec3::new(25.0, 15.0, 5.0)], 1.0).unwrap();
        let record = crate::brep_record(&rounded).unwrap();
        let round: Vec<usize> = (0..record.faces.len())
            .filter(|i| {
                !matches!(crate::offset::surf_of(&record, *i), Some(crate::offset::Surf::Plane { .. } | crate::offset::Surf::Cylinder { .. }))
            })
            .collect();
        assert_eq!(round.len(), 1, "{round:?}");
        let before = super::FALLBACKS.with(std::cell::Cell::get);
        let back = delete_faces(&rounded, &round).unwrap();
        assert_eq!(super::FALLBACKS.with(std::cell::Cell::get), before + 1);
        let sharp = 6000.0 + 200.0 * std::f64::consts::PI;
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - sharp).abs() < 1e-6 * sharp, "{v} vs {sharp}");
        assert_eq!(back.face_count(), 8);
    }

    #[test]
    fn a_split_on_the_shaft_stays_through_deleting_its_flat() {
        let shaft = cylinder(Vec3::ZERO, Vec3::Z, 10.0, 40.0).unwrap();
        let cut = box_solid(Vec3::new(8.0, -11.0, -1.0), Vec3::new(13.0, 11.0, 41.0)).unwrap();
        let flatted = boolean(&shaft, &cut, BoolOp::Cut).unwrap().unwrap();
        let round = |p: Vec3| (p.x.hypot(p.y) - 10.0).abs() < 1e-6;
        let side = faces_where(&flatted, round);
        assert_eq!(side.len(), 1, "{side:?}");
        let plane = solidcore_record::Plane { origin: Vec3::new(0.0, 0.0, 20.0), ..solidcore_record::Plane::XY };
        let split = crate::split_faces(&flatted, &side, &crate::SplitTool::Plane(plane)).unwrap();
        assert_eq!(faces_where(&split, round).len(), 2);
        let flat = faces_where(&split, |p| (p.x - 8.0).abs() < 1e-6);
        assert_eq!(flat.len(), 1, "{flat:?}");
        let whole = 4000.0 * std::f64::consts::PI;
        let back = delete_faces(&split, &flat).unwrap();
        let v = back.volume_at(1e-3).unwrap();
        assert!((v - whole).abs() < 1e-6 * whole, "{v} vs {whole}");
        assert_eq!(back.face_count(), 4);
        assert_eq!(faces_where(&back, round).len(), 2);
        sound(&back);
    }
}
