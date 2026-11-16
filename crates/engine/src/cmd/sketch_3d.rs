//! 3D sketch drawing: with the sketch's 3D Sketch option on (Sketch Palette), lines, arcs,
//! splines and points may leave the sketch plane. They are kept as drawn 3D curves (wires with
//! fit points, exact arcs keeping their analytic form): drawn and snapped to, movable point by
//! point, and usable as sweep and pipe paths.

use serde_json::Value;
use solidcore_record::{Vec2, Vec3};
use solidcraft_geom::prelude::*;
use solidcraft_sketch::{CurveKind, WireSpace};

use super::sketch::edit;
use super::sketch_store;
use super::{CommandSpec, in_sketch};
use crate::params::{bad, req_vec3, vec3};
use crate::schema::sketch_3d::{Arc3dParams, Curves3dResult, Line3dParams, Move3dParams, Move3dResult, Point3dParams, Point3dResult, Spline3dParams};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session};

const LINE3D_CONTRACT: TypedContract<Line3dParams> = TypedContract::new();
const SPLINE3D_CONTRACT: TypedContract<Spline3dParams> = TypedContract::new();
const ARC3D_CONTRACT: TypedContract<Arc3dParams> = TypedContract::new();
const POINT3D_CONTRACT: TypedContract<Point3dParams> = TypedContract::new();
const MOVE3D_CONTRACT: TypedContract<Move3dParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("sketch.line3d", "3D Line", crate::checked!("sketch.line3d", line3d))
        .icon("line")
        .enabled(in_sketch_3d)
        .contract(&LINE3D_CONTRACT)
        .output(output_schema_for::<Curves3dResult>)
        .params(concat!(
            "points: [[x,y,z]…] world points (2 or more): one 3D line per segment, joined end to end (needs 3D Sketch on)",
            created_doc!()
        )),
    CommandSpec::new("sketch.spline3d", "3D Spline", crate::checked!("sketch.spline3d", spline3d))
        .icon("spline")
        .enabled(in_sketch_3d)
        .contract(&SPLINE3D_CONTRACT)
        .output(output_schema_for::<Curves3dResult>)
        .params(concat!("points: [[x,y,z]…] world fit points (2 or more): a smooth 3D curve through them (needs 3D Sketch on)", created_doc!())),
    CommandSpec::new("sketch.arc3d", "3D Arc", crate::checked!("sketch.arc3d", arc3d))
        .icon("arc3")
        .enabled(in_sketch_3d)
        .contract(&ARC3D_CONTRACT)
        .output(output_schema_for::<Curves3dResult>)
        .params(concat!(
            "start, through, end: [x,y,z] world points: an exact circular arc through them, from start through through to end (needs 3D Sketch on). Stored exactly (centre, radius, angles), so path consumers can treat it as a true arc",
            created_doc!()
        )),
    CommandSpec::new("sketch.point3d", "3D Point", crate::checked!("sketch.point3d", point3d))
        .icon("point")
        .enabled(in_sketch_3d)
        .contract(&POINT3D_CONTRACT)
        .output(output_schema_for::<Point3dResult>)
        .params(concat!("point: [x,y,z] world point (needs 3D Sketch on)", created_doc!())),
    CommandSpec::new("sketch.move3d", "Move 3D Point", crate::checked!("sketch.move3d", move3d))
        .icon("sketch_move")
        .enabled(in_sketch)
        .contract(&MOVE3D_CONTRACT)
        .output(output_schema_for::<Move3dResult>)
        .params("wire: drawn 3D curve or point id, index?: fit point (default 0) | point: a point on the sketch plane (`l1.end`, `p3`: a line's end, a fit point of a spline, a lone point; needs 3D Sketch on); to: [x,y,z] | by: [dx,dy,dz] (world); points joined to it move too. A `point` on the plane first turns the lines and fit-point splines through it (or the lone point) into drawn 3D curves through the same places, which `curves` lists (`wire` and `index` say where the point is now); their constraints and dimensions go. Arcs, circles, ellipses, conics and control-point or closed splines through it are refused"),
];

/// A command's reply as JSON: built from the type its output schema describes, so the two can't
/// drift apart.
fn reply(cmd: &str, reply: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(reply).map_err(|e| bad(cmd, e.to_string()))
}

fn points(cmd: &str, p: &Value, min: usize) -> Result<Vec<Vec3>> {
    let a = p.get("points").and_then(Value::as_array).ok_or_else(|| bad(cmd, "`points` must list [x,y,z] points"))?;
    if a.len() < min || a.len() > solidcraft_sketch::MAX_FIT_POINTS {
        return Err(bad(cmd, format!("`points` needs {min} to {} points", solidcraft_sketch::MAX_FIT_POINTS)));
    }
    a.iter().map(|v| vec3(v).ok_or_else(|| bad(cmd, "a point must be [x, y, z]"))).collect()
}

fn needs_3d(sk: &solidcraft_sketch::Sketch, cmd: &str) -> Result<()> {
    if sk.view.three_d { Ok(()) } else { Err(bad(cmd, "turn on 3D Sketch (sketch.options sketch_3d: true) to draw off the sketch plane")) }
}

/// The 3D Line, Spline, Arc and Point buttons start interactive tools, so they are only offered
/// while a 3D sketch is being edited; otherwise the tools could only fail on their first click.
fn in_sketch_3d(s: &Session) -> std::result::Result<(), String> {
    let id = sketch_store::active(s).ok_or("no sketch is being edited (Create Sketch or Edit Sketch first)")?;
    match sketch_store::read(&s.doc, id) {
        Ok(sk) if sk.view.three_d => Ok(()),
        Ok(_) => Err("turn on 3D Sketch (sketch.options sketch_3d: true) to draw 3D curves".into()),
        Err(_) => Err("no sketch is being edited (Create Sketch or Edit Sketch first)".into()),
    }
}

/// World command inputs belong to the target sketch's owner, which may differ from the
/// active component when a script supplies an explicit sketch.
fn inverse_owner_frame(s: &Session, p: &Value, cmd: &str) -> Result<solidcraft_doc::Mat> {
    let id = sketch_store::target(s, p, cmd)?;
    let owner = s.doc.feature(id).map(|f| f.component).ok_or_else(|| bad(cmd, "target sketch is missing"))?;
    if owner != 0 && owner != s.active_component {
        if s.doc.occurrences.len() > 100_000 {
            return Err(bad(cmd, "sketch owner placement expansion exceeds supported bounds"));
        }
        let mut children: std::collections::BTreeMap<u64, Vec<&solidcraft_doc::Occurrence>> = std::collections::BTreeMap::new();
        for occurrence in &s.doc.occurrences {
            children.entry(occurrence.parent).or_default().push(occurrence);
        }
        let mut stack = vec![(0, 0usize)];
        let mut count = 0;
        let mut visited = 0;
        while let Some((component, depth)) = stack.pop() {
            for o in children.get(&component).into_iter().flatten() {
                visited += 1;
                if depth >= 64 || visited > 100_000 {
                    return Err(bad(cmd, "sketch owner placement expansion exceeds supported bounds"));
                }
                if o.component == owner {
                    count += 1;
                    if count > 1 {
                        return Err(bad(cmd, "activate the desired sketch owner occurrence before editing world 3D points"));
                    }
                }
                stack.push((o.component, depth + 1));
            }
        }
    }
    let frame = if owner == s.active_component { s.active_component_frame() } else { s.placed_document().component_transform(owner) };
    solidcraft_doc::mat_inverse(&frame).ok_or_else(|| bad(cmd, "the sketch owner placement is not invertible"))
}

fn line3d(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.line3d";
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let pts: Vec<Vec3> = points(cmd, p, 2)?.into_iter().map(|at| solidcraft_doc::apply_point(&inverse, at)).collect();
    let (ids, info) = edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        pts.windows(2).map(|w| sk.add_drawn_wire_in(w, WireSpace::Component).map_err(Into::into)).collect::<Result<Vec<String>>>()
    })?;
    reply(cmd, Curves3dResult { curves: ids, sketch: info, created: None, param_warnings: Vec::new() })
}

fn spline3d(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.spline3d";
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let pts: Vec<Vec3> = points(cmd, p, 2)?.into_iter().map(|at| solidcraft_doc::apply_point(&inverse, at)).collect();
    let (id, info) = edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        Ok(sk.add_drawn_wire_in(&pts, WireSpace::Component)?)
    })?;
    reply(cmd, Curves3dResult { curves: vec![id], sketch: info, created: None, param_warnings: Vec::new() })
}

fn arc3d(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.arc3d";
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let pts: Vec<Vec3> = ["start", "through", "end"]
        .iter()
        .map(|k| req_vec3(cmd, p, k))
        .collect::<Result<Vec<Vec3>>>()?
        .into_iter()
        .map(|at| solidcraft_doc::apply_point(&inverse, at))
        .collect();
    let (id, info) = edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        Ok(sk.add_drawn_arc_in(&pts, WireSpace::Component)?)
    })?;
    reply(cmd, Curves3dResult { curves: vec![id], sketch: info, created: None, param_warnings: Vec::new() })
}

fn point3d(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.point3d";
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let at = solidcraft_doc::apply_point(&inverse, req_vec3(cmd, p, "point")?);
    let (id, info) = edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        Ok(sk.add_drawn_wire_in(&[at], WireSpace::Component)?)
    })?;
    reply(cmd, Point3dResult { point: id, sketch: info, created: None, param_warnings: Vec::new() })
}

fn move3d(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sketch.move3d";
    if let Some(point) = p.get("point") {
        if p.get("wire").is_some() {
            return Err(bad(cmd, "give `wire` or `point`, not both"));
        }
        if p.get("index").is_some() {
            return Err(bad(cmd, "`index` goes with `wire`"));
        }
        let point = point.as_str().ok_or_else(|| bad(cmd, "`point` must be a sketch point id"))?.to_string();
        return lift_point(s, p, cmd, &point);
    }
    let wire = p.get("wire").and_then(Value::as_str).ok_or_else(|| bad(cmd, "`wire` must be a 3D curve or point id"))?.to_string();
    let index = match p.get("index") {
        None => 0,
        Some(v) => v
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| *v < solidcraft_sketch::MAX_FIT_POINTS)
            .ok_or_else(|| bad(cmd, "`index` must be a fit point index below 500"))?,
    };
    let parse = |name| p.get(name).map(|v| vec3(v).ok_or_else(|| bad(cmd, format!("`{name}` must be [x,y,z]")))).transpose();
    let to = parse("to")?;
    let by = parse("by")?;
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let (n, info) = edit(s, p, cmd, false, |sk, _| {
        let drawn = sk
            .wire_index(&wire)
            .and_then(|w| sk.wires.get(w))
            .filter(|w| !w.fit.is_empty() && w.link.is_none())
            .ok_or_else(|| bad(cmd, format!("`{wire}` is not an editable drawn 3D curve")))?;
        let from = drawn.fit.get(index).copied().ok_or_else(|| bad(cmd, format!("`{wire}` has no drawn point {index}")))?;
        let to = match (to, by) {
            (Some(t), _) => {
                if drawn.space == WireSpace::Component {
                    solidcraft_doc::apply_point(&inverse, t)
                } else {
                    t
                }
            }
            (None, Some(d)) => from + if drawn.space == WireSpace::Component { solidcraft_doc::apply_vector(&inverse, d) } else { d },
            _ => return Err(bad(cmd, "give `to` or `by`")),
        };
        Ok(sk.move_drawn_point(&wire, index, to)?)
    })?;
    reply(cmd, Move3dResult { moved: n, curves: Vec::new(), wire: None, index: None, sketch: info, param_warnings: Vec::new() })
}

/// What a plane curve through a lifted point is as a drawn 3D curve: its point indices, in the
/// order the drawn curve takes them as fit points. Lines and open fit-point splines only.
fn liftable(c: &solidcraft_sketch::Curve, cmd: &str) -> Result<Vec<usize>> {
    if c.link.is_some() {
        return Err(bad(cmd, format!("`{}` is projected geometry; it stays where it was projected", c.id)));
    }
    let what = match &c.kind {
        CurveKind::Line { a, b } => return Ok(vec![*a, *b]),
        CurveKind::Spline { pts, control: false, periodic: false, .. } => return Ok(pts.clone()),
        CurveKind::Spline { .. } => "a control-point or closed spline",
        CurveKind::Circle { .. } => "a circle",
        CurveKind::Arc { .. } => "an arc",
        CurveKind::Ellipse { .. } => "an ellipse",
        CurveKind::Conic { .. } => "a conic",
    };
    Err(bad(cmd, format!("`{}` is {what}: only lines, fit-point splines and lone points can leave the sketch plane", c.id)))
}

/// `sketch.move3d` with `point`: the lines and fit-point splines through a point on the sketch
/// plane (or the lone point) become drawn 3D curves through the same places, then the point
/// moves to `to` or by `by` (world), the curves through it staying joined there.
fn lift_point(s: &mut Session, p: &Value, cmd: &str, point: &str) -> Result<Value> {
    let parse = |name| p.get(name).map(|v| vec3(v).ok_or_else(|| bad(cmd, format!("`{name}` must be [x,y,z]")))).transpose();
    let (to, by) = (parse("to")?, parse("by")?);
    if to.is_none() && by.is_none() {
        return Err(bad(cmd, "give `to` or `by`"));
    }
    let id = sketch_store::target(s, p, cmd)?;
    // The plane in the owner component's coordinates, which drawn curves in `Component`
    // space use.
    let plane = s.model.state().sketch(id).map(|ss| ss.plane).ok_or_else(|| bad(cmd, "the sketch has not been solved"))?;
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let place = |q: Vec2| plane.to_world(q);
    let ((n, curves, k), info) = edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        let missing = || bad(cmd, format!("no point `{point}` in the sketch"));
        let pi = sk.resolve_point(point).filter(|i| *i != 0).ok_or_else(missing)?;
        let at = sk.points.get(pi).ok_or_else(missing)?;
        if at.link.is_some() {
            return Err(bad(cmd, format!("`{point}` is projected geometry; it stays where it was projected")));
        }
        let from = place(at.pos);
        let goal = match (to, by) {
            (Some(t), _) => solidcraft_doc::apply_point(&inverse, t),
            (None, Some(d)) => from + solidcraft_doc::apply_vector(&inverse, d),
            (None, None) => return Err(bad(cmd, "give `to` or `by`")),
        };
        // The points joined to `pi` by coincident constraints are one place: the curves through any
        // of them lift together.
        let roots = super::sketch::coincident_points(sk);
        let group: Vec<usize> = (0..sk.points.len()).filter(|i| roots.get(*i) == roots.get(pi)).collect();
        let through: Vec<usize> = sk.curves.iter().enumerate().filter(|(_, c)| group.iter().any(|g| c.kind.uses(*g))).map(|(i, _)| i).collect();
        let mut fits = Vec::new();
        let mut k = 0;
        for (n, &ci) in through.iter().enumerate() {
            let ids = liftable(&sk.curves[ci], cmd)?;
            if n == 0 {
                k = ids
                    .iter()
                    .position(|i| *i == pi)
                    .or_else(|| ids.iter().position(|i| group.contains(i)))
                    .ok_or_else(|| bad(cmd, format!("`{point}` is not a point of `{}`", sk.curves[ci].id)))?;
            }
            let fit = ids
                .iter()
                .map(|i| sk.points.get(*i).map(|q| place(q.pos)).ok_or_else(|| bad(cmd, "a curve's point is missing")))
                .collect::<Result<Vec<Vec3>>>()?;
            fits.push(fit);
        }
        // A joined point at the origin is not lifted: the origin stays put.
        if group.contains(&0) {
            return Err(bad(cmd, format!("`{point}` is joined to the origin; it stays")));
        }
        // Joined points that no curve uses at all, by id: the removal below may shift indices.
        let lone: Vec<String> =
            group.iter().copied().filter(|g| *g != pi && !sk.curves.iter().any(|c| c.kind.uses(*g))).map(|g| sk.points[g].id.clone()).collect();
        let mut doomed = Vec::new();
        if through.is_empty() {
            fits.push(vec![from]);
            doomed.push(pi);
        } else {
            sk.remove_curves(&through);
        }
        doomed.extend(lone.iter().filter_map(|id| sk.resolve_point(id)));
        sk.remove_points(&doomed);
        let mut made = Vec::new();
        for fit in &fits {
            made.push(sk.add_drawn_wire_in(fit, WireSpace::Component)?);
        }
        let n = sk.move_drawn_point(&made[0], k, goal)?;
        Ok((n, made, k))
    })?;
    let wire = curves.first().cloned();
    reply(cmd, Move3dResult { moved: n, curves, wire, index: Some(k), sketch: info, param_warnings: Vec::new() })
}

/// Turn the curves `curves` of the target 3D sketch into drawn 3D curves through the same places,
/// moved by `offset` (world). The points `points` move by `offset` too, each on its own: a lone
/// point becomes a drawn 3D point, and the lines and fit-point splines through one that isn't on
/// a moved curve become drawn 3D curves with just that point moved. Returns the new drawn curves.
pub(super) fn lift(s: &mut Session, p: &Value, cmd: &str, curves: &[String], points: &[String], offset: Vec3) -> Result<(Vec<String>, Value)> {
    let id = sketch_store::target(s, p, cmd)?;
    let st = s.world_state()?;
    let plane = st.sketch(id).map(|ss| ss.plane).ok_or_else(|| bad(cmd, "the sketch has no plane"))?;
    let inverse = inverse_owner_frame(s, p, cmd)?;
    let comp = move |q: Vec2| solidcraft_doc::apply_point(&inverse, plane.to_world(q));
    let shift = solidcraft_doc::apply_vector(&inverse, offset);
    edit(s, p, cmd, false, |sk, _| {
        needs_3d(sk, cmd)?;
        let mut which = Vec::new();
        for c in curves {
            which.push(sk.curve_index(c).ok_or_else(|| bad(cmd, format!("no curve `{c}`")))?);
        }
        which.sort_unstable();
        which.dedup();
        // Points moved on their own: lone ones, and ones on curves that stay where they are
        // (those curves are lifted in place, then just the point moves).
        let mut alone: Vec<(usize, Vec3)> = Vec::new();
        let mut on_curves: Vec<usize> = Vec::new();
        let mut in_place: Vec<usize> = Vec::new();
        for r in points {
            let pi = sk.resolve_point(r).filter(|i| *i != 0).ok_or_else(|| bad(cmd, format!("no point `{r}`")))?;
            let through: Vec<usize> = sk.curves.iter().enumerate().filter(|(_, c)| c.kind.uses(pi)).map(|(i, _)| i).collect();
            if through.is_empty() {
                if !alone.iter().any(|(q, _)| *q == pi) {
                    alone.push((pi, sk.points.get(pi).map(|q| comp(q.pos)).ok_or_else(|| bad(cmd, "the point is missing"))?));
                }
            } else if !through.iter().all(|c| which.contains(c)) {
                on_curves.push(pi);
                in_place.extend(through.into_iter().filter(|c| !which.contains(c)));
            }
        }
        in_place.sort_unstable();
        in_place.dedup();
        let moved = which.len();
        let mut all = which.clone();
        all.extend(in_place.iter().copied());
        // Each curve as a drawn 3D curve: its point ids, and its fit points in component space.
        let mut lifted: Vec<(Vec<usize>, Vec<Vec3>)> = Vec::new();
        for &ci in &all {
            let ids = liftable(&sk.curves[ci], cmd)?;
            let fit = ids
                .iter()
                .map(|i| sk.points.get(*i).map(|q| comp(q.pos)).ok_or_else(|| bad(cmd, "a curve's point is missing")))
                .collect::<Result<Vec<Vec3>>>()?;
            lifted.push((ids, fit));
        }
        sk.remove_curves(&all);
        let gone: Vec<usize> = alone.iter().map(|(q, _)| *q).collect();
        sk.remove_points(&gone);
        let mut made = Vec::new();
        // Where each point moved on its own sits on the drawn curves (curve id, fit index).
        let mut placed: Vec<(usize, String, usize)> = Vec::new();
        for (n, (ids, fit)) in lifted.iter().enumerate() {
            let fit: Vec<Vec3> = if n < moved { fit.iter().map(|q| *q + shift).collect() } else { fit.clone() };
            let w = sk.add_drawn_wire_in(&fit, WireSpace::Component)?;
            for (k, q) in ids.iter().enumerate() {
                if n >= moved && on_curves.contains(q) {
                    placed.push((*q, w.clone(), k));
                }
            }
            made.push(w);
        }
        for (_, q) in &alone {
            made.push(sk.add_drawn_wire_in(&[*q + shift], WireSpace::Component)?);
        }
        // Each point on curves lifted in place moves by the offset, once (the first curve it is
        // on; the drawn curves through it stay joined there).
        let mut done: Vec<usize> = Vec::new();
        for (pi, w, k) in &placed {
            if done.contains(pi) {
                continue;
            }
            done.push(*pi);
            if let Some(now) = sk.wire_index(w).and_then(|i| sk.wires.get(i)).and_then(|x| x.fit.get(*k)).copied() {
                sk.move_drawn_point(w, *k, now + shift)?;
            }
        }
        Ok(made)
    })
}

/// Move on drawn 3D geometry: each picked drawn point (`(wire, Some(index))`) or whole drawn
/// curve (`(wire, None)`) moves as the picked plane geometry does: `f` moves its place along the
/// plane (sketch coordinates, turn included); its height off the plane is kept. Returns how many
/// points moved, and the sketch's state.
pub(super) fn move_drawn(
    s: &mut Session,
    p: &Value,
    cmd: &str,
    picks: &[(String, Option<usize>)],
    z: f64,
    f: &dyn Fn(Vec2) -> Vec2,
) -> Result<(usize, Value)> {
    let id = sketch_store::target(s, p, cmd)?;
    let world = s.world_state()?.sketch(id).map(|ss| ss.plane).ok_or_else(|| bad(cmd, "the sketch has no plane"))?;
    let inverse = inverse_owner_frame(s, p, cmd)?;
    // The plane as component-space drawn points see it.
    let component = solidcore_record::Plane {
        origin: solidcraft_doc::apply_point(&inverse, world.origin),
        x: solidcraft_doc::apply_vector(&inverse, world.x),
        y: solidcraft_doc::apply_vector(&inverse, world.y),
    };
    edit(s, p, cmd, false, |sk, _| {
        let mut n = 0;
        for (wire, index) in picks {
            let drawn = sk
                .wire_index(wire)
                .and_then(|w| sk.wires.get(w))
                .filter(|w| !w.fit.is_empty() && w.link.is_none())
                .ok_or_else(|| bad(cmd, format!("`{wire}` is not an editable drawn 3D curve")))?;
            let plane = if drawn.space == WireSpace::Component { component } else { world };
            let which: Vec<usize> = match index {
                Some(i) if *i < drawn.fit.len() => vec![*i],
                Some(i) => return Err(bad(cmd, format!("`{wire}` has no drawn point {i}"))),
                None => (0..drawn.fit.len()).collect(),
            };
            let normal = plane.normal();
            let moved: Vec<(usize, Vec3)> = which
                .into_iter()
                .filter_map(|i| drawn.fit.get(i).map(|q| (i, *q)))
                .map(|(i, q)| (i, plane.to_world(f(plane.to_local(q))) + normal * ((q - plane.origin).dot(normal) + z)))
                .collect();
            for (i, to) in moved {
                sk.move_drawn_point(wire, i, to)?;
                n += 1;
            }
        }
        Ok(n)
    })
}

#[cfg(test)]
#[path = "sketch_3d_frame_tests.rs"]
mod frame_tests;
