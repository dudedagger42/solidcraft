//! Guide rails for a loft (#201): curves the loft follows between its sections. A rail runs from
//! the first section to the last and passes through every section's curve. Between each two
//! sections, stations are added: each is the plain loft there, moved so that the point of it a
//! rail guides lands on the rail. With one rail the station turns about the loft's direction
//! there and grows or shrinks about its middle (stretches across to the rail, with
//! [`RailFit::Stretch`]), shifted along the loft as far as the rail leads or lags; with two it
//! moves with the first rail, turns with the line between the rails and sizes with their distance
//! (stretches along that line), leaning to follow the second. At the sections the move is none,
//! so the sections and the loft's caps stay exactly as drawn; the loft's smooth surfaces then run
//! through the sections and the stations together.

use mt::{BoundedCurve, ParametricCurve};
use solidcraft_geom::{Plane, Vec3};
use truck_modeling as mt;

use crate::body::{from_p3, p3};
use crate::sweep_path::RailFit;
use crate::{KernelError, Result};

/// Most wires a railed loft is skinned through, sections and stations together.
const MAX_WIRES: usize = 33;
/// Most stations between two sections.
const MAX_PER_SPAN: usize = 15;
/// Most guide rails on one loft.
pub const MAX_RAILS: usize = 16;

type Curve = mt::BSplineCurve<mt::Point3>;

/// Where a rail meets a section: the point of the section's wire it guides (as a wire parameter:
/// the edge's index plus how far along the edge), that point, how far it is from where the rail
/// crosses the section's plane (the nudge onto it), and how far along the rail that crossing is
/// (segment and fraction).
struct Anchor {
    g: f64,
    on: Vec3,
    nudge: Vec3,
    at: (usize, f64),
}

/// A rail as a polyline from the first section, and where it meets each section.
struct Rail {
    pts: Vec<Vec3>,
    anchors: Vec<Anchor>,
}

/// An affine move: `x` goes to `to + m·(x − from)` (`m` by columns).
struct Affine {
    from: Vec3,
    to: Vec3,
    m: [Vec3; 3],
}

impl Affine {
    fn apply(&self, x: Vec3) -> Vec3 {
        let d = x - self.from;
        self.to + self.m[0] * d.x + self.m[1] * d.y + self.m[2] * d.z
    }
}

/// The loft's wires with stations added between the sections so that the loft follows `rails`
/// (polylines; see the module notes). `planes` are the sections' planes, `wires` their matched
/// wires (as [`crate::build::loft_wires`] or a sheet loft makes them: the k-th edges of every
/// wire are B-splines on one knot vector), both in loft order; `closed` whether the wires close.
/// Without rails the wires come back as they are.
pub(crate) fn railed_wires(planes: &[Plane], wires: &[mt::Wire], closed: bool, rails: &[Vec<Vec3>], fit: RailFit) -> Result<Vec<mt::Wire>> {
    if rails.is_empty() {
        return Ok(wires.to_vec());
    }
    if rails.len() > MAX_RAILS {
        return Err(KernelError::Invalid(format!("a loft takes 1…{MAX_RAILS} guide rails")));
    }
    let fail = |m: &str| KernelError::Failed(format!("loft rails: {m}"));
    let count = wires.len();
    if count < 2 || planes.len() != count {
        return Err(fail("sections"));
    }
    // Each section's edge curves, and the knot vector the k-th edges share.
    let curves: Vec<Vec<Curve>> = wires
        .iter()
        .map(|w| {
            w.edge_iter()
                .map(|e| match e.curve() {
                    mt::Curve::BSplineCurve(c) if e.orientation() => Some(c),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<_>>()
        .ok_or_else(|| fail("section curves"))?;
    let first = curves.first().ok_or_else(|| fail("sections"))?;
    let n = first.len();
    let knots: Vec<mt::KnotVec> = first.iter().map(|c| c.knot_vec().clone()).collect();
    let sizes: Vec<usize> = first.iter().map(|c| c.control_points().len()).collect();
    if n == 0
        || curves
            .iter()
            .any(|cs| cs.len() != n || cs.iter().zip(&knots).zip(&sizes).any(|((c, k), m)| c.knot_vec() != k || c.control_points().len() != *m))
    {
        return Err(fail("sections parameterised differently"));
    }
    let ctrl: Vec<Vec<Vec<Vec3>>> =
        curves.iter().map(|cs| cs.iter().map(|c| c.control_points().iter().map(|p| from_p3(*p)).collect()).collect()).collect();
    let edges: Vec<Vec<mt::Edge>> = wires.iter().map(|w| w.edge_iter().cloned().collect()).collect();
    let along = crate::build::section_parameters(&edges);
    let mut bounds = solidcraft_geom::Aabb3::EMPTY;
    for p in ctrl.iter().flatten().flatten() {
        if !p.is_finite() {
            return Err(fail("sections"));
        }
        bounds.add(*p);
    }
    let size = bounds.diagonal();
    let tol = (size * 1e-4).max(1e-6);
    let start = centroid(ctrl.first().ok_or_else(|| fail("sections"))?);
    let rails: Vec<Rail> =
        rails.iter().enumerate().map(|(k, r)| rail(r, (rails.len() > 2).then_some(k + 1), start, planes, &curves, tol)).collect::<Result<_>>()?;
    for (ka, a) in rails.iter().enumerate() {
        for (kb, b) in rails.iter().enumerate().skip(ka + 1) {
            if let Some(i) = a.anchors.iter().zip(&b.anchors).position(|(p, q)| p.on.dist(q.on) <= tol) {
                return Err(KernelError::Invalid(if rails.len() == 2 {
                    "the two guide rails meet a section at the same point".into()
                } else {
                    format!("guide rails {} and {} meet section {} at the same point", ka + 1, kb + 1, i + 1)
                }));
            }
        }
    }
    // The plain loft between the sections: each control point across them, straight between two
    // sections, the smooth loft's curve through three or more.
    let across: Vec<Vec<Option<Curve>>> = if count == 2 {
        Vec::new()
    } else {
        (0..n)
            .map(|k| {
                (0..sizes.get(k).copied().unwrap_or(0))
                    .map(|j| {
                        let pts: Vec<Vec3> = ctrl.iter().filter_map(|s| s.get(k).and_then(|e| e.get(j)).copied()).collect();
                        crate::build::across(&pts, &along)
                    })
                    .collect()
            })
            .collect()
    };
    let plain = |v: f64| -> Result<Vec<Vec<Vec3>>> {
        let (Some(a), Some(b)) = (ctrl.first(), ctrl.last()) else { return Err(fail("sections")) };
        if count == 2 {
            return Ok(a.iter().zip(b).map(|(ea, eb)| ea.iter().zip(eb).map(|(p, q)| p.lerp(*q, v)).collect()).collect());
        }
        across.iter().map(|cs| cs.iter().map(|c| c.as_ref().map(|c| from_p3(c.subs(v))).ok_or_else(|| fail("the plain loft"))).collect()).collect()
    };
    // The middle of the plain loft (the middles of the sections' control points, across them as
    // the control points are: the middle is linear in them) and its direction.
    let middles: Vec<Vec3> = ctrl.iter().map(|s| centroid(s)).collect();
    let middle_curve = if count == 2 { None } else { Some(crate::build::across(&middles, &along).ok_or_else(|| fail("the plain loft"))?) };
    let middle = |v: f64| -> Result<(Vec3, Vec3)> {
        let (p, d) = match (&middle_curve, middles.first(), middles.last()) {
            (Some(c), ..) => {
                let d = c.der(v);
                (from_p3(c.subs(v)), Vec3::new(d.x, d.y, d.z))
            }
            (None, Some(a), Some(b)) => (a.lerp(*b, v), *b - *a),
            _ => return Err(fail("sections")),
        };
        let d = d.normalized().ok_or_else(|| KernelError::Invalid("two loft sections sit on top of each other".into()))?;
        Ok((p, d))
    };
    // The sections' normals, turned to agree with the first's.
    let mut normals: Vec<Vec3> = Vec::with_capacity(count);
    for p in planes {
        let n = p.normal();
        let prev = normals.last().copied().unwrap_or(n);
        normals.push(if n.dot(prev) < 0.0 { n * -1.0 } else { n });
    }
    let spans = count - 1;
    let per = (MAX_WIRES.saturating_sub(count) / spans).clamp(1, MAX_PER_SPAN);
    let mut out = Vec::with_capacity(count + spans * per);
    for i in 0..spans {
        out.push(wires.get(i).cloned().ok_or_else(|| fail("sections"))?);
        let (Some(&v0), Some(&v1), Some(&n0), Some(&n1)) = (along.get(i), along.get(i + 1), normals.get(i), normals.get(i + 1)) else {
            return Err(fail("sections"));
        };
        // Each rail's anchors at the span's two sections, and where its walk has got to.
        let mut spans_of: Vec<(&Rail, &Anchor, &Anchor, (usize, f64))> = Vec::with_capacity(rails.len());
        for r in &rails {
            let (Some(a0), Some(a1)) = (r.anchors.get(i), r.anchors.get(i + 1)) else { return Err(fail("rail")) };
            spans_of.push((r, a0, a1, a0.at));
        }
        for q in 1..=per {
            let f = q as f64 / (per + 1) as f64;
            let v = v0 + f * (v1 - v0);
            let station = plain(v)?;
            let (mid, dir) = middle(v)?;
            // The station's plane turns from one section's to the next's.
            let normal = n0.lerp(n1, f).normalized().ok_or_else(|| fail("section planes"))?;
            let mut pairs = Vec::with_capacity(rails.len());
            let mut gs = Vec::with_capacity(rails.len());
            for (r, a0, a1, cursor) in &mut spans_of {
                let g = interpolate_g(a0.g, a1.g, f, n, closed);
                gs.push(g);
                let anchor = wire_point(&station, &knots, g)?;
                // Where the rail crosses the station's plane through the point it guides, nudged
                // so that at each section it is the section's own point (a rail drawn a hair off
                // a section moves nothing there).
                let Some((hit, at)) = crate::sweep_path::rail_crossing(&r.pts, anchor, normal, *cursor, tol) else {
                    return Err(KernelError::Invalid(format!(
                        "a guide rail turns back between sections {} and {}: it has to run on from one section to the next",
                        i + 1,
                        i + 2
                    )));
                };
                *cursor = at;
                let target = hit + a0.nudge * (1.0 - f) + a1.nudge * f;
                pairs.push((anchor, target));
            }
            let moved: Vec<Vec<Vec3>> = if pairs.len() <= 2 {
                let map = station_map(&pairs, mid, dir, fit, size)?;
                station.iter().map(|e| e.iter().map(|p| map.apply(*p)).collect()).collect()
            } else {
                warp_station(&station, &knots, &gs, &pairs, normal, size).map_err(|e| match e {
                    KernelError::Invalid(m) => KernelError::Invalid(format!("between sections {} and {}: {m}", i + 1, i + 2)),
                    e => e,
                })?
            };
            out.push(wire_of(&moved, &knots, closed)?);
        }
    }
    out.push(wires.last().cloned().ok_or_else(|| fail("sections"))?);
    Ok(out)
}

/// The rail `pts` from its end nearer `start` (the first section's middle), where it meets each
/// section: across the section's plane, on the section's curve within `tol`. `label`: the rail's
/// number for its errors (on a loft of three or more rails; one or two need none).
fn rail(pts: &[Vec3], label: Option<usize>, start: Vec3, planes: &[Plane], curves: &[Vec<Curve>], tol: f64) -> Result<Rail> {
    let name = label.map_or_else(|| "a guide rail".to_string(), |k| format!("guide rail {k}"));
    let mut pts = crate::sweep_path::clean(pts).map_err(|_| KernelError::Invalid(format!("{name} needs 2…100000 finite points along its length")))?;
    if let (Some(a), Some(b)) = (pts.first(), pts.last())
        && b.dist(start) < a.dist(start)
    {
        pts.reverse();
    }
    let mut cursor = (0usize, 0.0f64);
    let mut anchors = Vec::with_capacity(planes.len());
    for (i, (plane, section)) in planes.iter().zip(curves).enumerate() {
        let Some((hit, at)) = crate::sweep_path::rail_crossing(&pts, plane.origin, plane.normal(), cursor, tol) else {
            return Err(KernelError::Invalid(format!(
                "{name} does not reach section {}: a rail has to run from the first section to the last, through every one",
                i + 1
            )));
        };
        cursor = at;
        let (g, on, d) = nearest_on(section, hit).ok_or_else(|| KernelError::Failed("loft rails: section".into()))?;
        if d > tol {
            let at = if label.is_some() { format!(" at [{:.3}, {:.3}, {:.3}]", hit.x, hit.y, hit.z) } else { String::new() };
            return Err(KernelError::Invalid(format!(
                "{name} crosses section {}'s plane {d:.3} mm from its curves{at}: a rail has to pass through every section",
                i + 1
            )));
        }
        anchors.push(Anchor { g, on, nudge: on - hit, at });
    }
    Ok(Rail { pts, anchors })
}

/// The point of a section's wire (its edge curves) nearest `p`: as a wire parameter, the point,
/// and how far it is.
fn nearest_on(section: &[Curve], p: Vec3) -> Option<(f64, Vec3, f64)> {
    let mut best: Option<(f64, Vec3, f64)> = None;
    for (k, c) in section.iter().enumerate() {
        let (t0, t1) = c.range_tuple();
        let t = crate::curveblend::param_on(&mt::Curve::BSplineCurve(c.clone()), p)?;
        let q = from_p3(c.subs(t));
        let d = q.dist(p);
        if best.as_ref().is_none_or(|b| d < b.2) {
            best = Some((k as f64 + (t - t0) / (t1 - t0).max(1e-300), q, d));
        }
    }
    best
}

/// A wire parameter `f` of the way from `g0` to `g1`; round a closed wire of `n` edges the
/// shorter way.
fn interpolate_g(g0: f64, g1: f64, f: f64, n: usize, closed: bool) -> f64 {
    let n = n as f64;
    if !closed {
        return (g0 + f * (g1 - g0)).clamp(0.0, n);
    }
    let mut d = g1 - g0;
    if d > n / 2.0 {
        d -= n;
    } else if d < -n / 2.0 {
        d += n;
    }
    (g0 + f * d).rem_euclid(n)
}

/// The point at wire parameter `g` of a wire given by its edges' control points on `knots`.
fn wire_point(ctrl: &[Vec<Vec3>], knots: &[mt::KnotVec], g: f64) -> Result<Vec3> {
    let n = ctrl.len();
    let k = (g.floor().max(0.0) as usize).min(n.saturating_sub(1));
    let (Some(pts), Some(kv)) = (ctrl.get(k), knots.get(k)) else { return Err(KernelError::Failed("loft rails: wire".into())) };
    let c = edge_curve(kv, pts)?;
    let (t0, t1) = c.range_tuple();
    let f = (g - k as f64).clamp(0.0, 1.0);
    Ok(from_p3(c.subs(t0 + (t1 - t0) * f)))
}

/// The middle of a wire's control points.
fn centroid(ctrl: &[Vec<Vec3>]) -> Vec3 {
    let (sum, n) = ctrl.iter().flatten().fold((Vec3::ZERO, 0usize), |(s, n), p| (s + *p, n + 1));
    sum * (1.0 / n.max(1) as f64)
}

/// The move that puts a station's guided points onto the rails (`pairs`: each point with its
/// rail's point), the station's middle `middle`, the loft's direction there `dir` (unit); `size`
/// the loft's size. One rail: turned about `dir` and sized about the middle, shifted along `dir`;
/// two: carried with the first, turned and sized with the line between them, leaning along `dir`.
fn station_map(pairs: &[(Vec3, Vec3)], middle: Vec3, dir: Vec3, fit: RailFit, size: f64) -> Result<Affine> {
    let (from, to, a, r, two) = match pairs {
        [(p, q)] => (middle, middle, *p - middle, *q - middle, false),
        [(p1, q1), (p2, q2)] => (*p1, *q1, *p2 - *p1, *q2 - *q1, true),
        _ => return Err(KernelError::Invalid("a loft takes one or two guide rails".into())),
    };
    let flat = |v: Vec3| v - dir * v.dot(dir);
    let (ap, rp) = (flat(a), flat(r));
    let tiny = 1e-6 * size.max(1e-9);
    let (Some(u0), Some(u1)) = (ap.normalized().filter(|_| ap.len() > tiny), rp.normalized().filter(|_| rp.len() > tiny)) else {
        return Err(KernelError::Invalid(if two {
            "the two guide rails come together between the sections: the loft would shrink to nothing".into()
        } else {
            "a guide rail runs through the middle of the loft: it has to guide a point of the sections away from their middle".into()
        }));
    };
    // Turned about the loft's direction and sized as the sweep's rails size its profile.
    let mut m = crate::sweep_path::rail_frame(dir, u0, dir, u1, rp.len() / ap.len(), fit);
    // Whatever the rails ask along the loft: one rail shifts the station that far, two lean it,
    // in step with the way across.
    let lead = (r - a).dot(dir);
    if two {
        let lean = lead / ap.len();
        for (col, e) in m.iter_mut().zip([Vec3::X, Vec3::Y, Vec3::Z]) {
            *col += dir * (lean * e.dot(u0));
        }
    }
    let shift = if two { Vec3::ZERO } else { dir * lead };
    Ok(Affine { from, to: to + shift, m })
}

/// A station's control points moved so that its points guided by three or more rails (`pairs`:
/// each point, at wire parameter `gs`, with its rail's point) land on the rails. The station's
/// plane (square to `normal`, unit) first takes the affine move that fits the rails best (exact
/// for three); what is left is taken up by a smooth field of inverse multiquadrics about the
/// guided points, solved so that the curve itself (not only its control points) passes through
/// every rail. The field moves points the same way wherever they are, so edges that meet still
/// meet. `size` the loft's size.
fn warp_station(station: &[Vec<Vec3>], knots: &[mt::KnotVec], gs: &[f64], pairs: &[(Vec3, Vec3)], normal: Vec3, size: f64) -> Result<Vec<Vec<Vec3>>> {
    let n = pairs.len() as f64;
    let (a_mid, q_mid) = pairs.iter().fold((Vec3::ZERO, Vec3::ZERO), |(a, q), (p, t)| (a + *p, q + *t));
    let (a_mid, q_mid) = (a_mid * (1.0 / n), q_mid * (1.0 / n));
    // Two directions across the station's plane.
    let seed = if normal.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let e1 = (seed - normal * seed.dot(normal)).normalized().ok_or_else(|| KernelError::Failed("loft rails: station plane".into()))?;
    let e2 = normal.cross(e1);
    // Least squares for the images U, V of e1, e2: q − q̄ ≈ s U + t V.
    let (mut ss, mut st, mut tt) = (0.0, 0.0, 0.0);
    let (mut bs, mut bt) = (Vec3::ZERO, Vec3::ZERO);
    for (p, q) in pairs {
        let d = *p - a_mid;
        let (s, t) = (d.dot(e1), d.dot(e2));
        ss += s * s;
        st += s * t;
        tt += t * t;
        bs += (*q - q_mid) * s;
        bt += (*q - q_mid) * t;
    }
    let det = ss * tt - st * st;
    if det <= 1e-10 * (ss + tt).powi(2).max(1e-300) {
        return Err(KernelError::Invalid("the guide rails meet the loft along one line: three or more rails have to surround it".into()));
    }
    let u = (bs * tt - bt * st) * (1.0 / det);
    let v = (bt * ss - bs * st) * (1.0 / det);
    let affine = |x: Vec3| {
        let d = x - a_mid;
        q_mid + u * d.dot(e1) + v * d.dot(e2) + normal * d.dot(normal)
    };
    let mut moved: Vec<Vec<Vec3>> = station.iter().map(|e| e.iter().map(|p| affine(*p)).collect()).collect();
    // What the affine move leaves: each guided point's way still to its rail.
    let at: Vec<Vec3> = gs.iter().map(|g| wire_point(&moved, knots, *g)).collect::<Result<_>>()?;
    let rest: Vec<Vec3> = pairs.iter().zip(&at).map(|((_, q), p)| *q - *p).collect();
    let tiny = 1e-9 * size.max(1e-9);
    if rest.iter().all(|r| r.len() <= tiny) {
        return Ok(moved);
    }
    // The field's width: the mean distance from each guided point to its nearest other.
    let width = at
        .iter()
        .enumerate()
        .map(|(i, p)| at.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, q)| p.dist(*q)).fold(f64::INFINITY, f64::min))
        .sum::<f64>()
        / n;
    let c2 = width.max(tiny).powi(2);
    let phi = |x: Vec3, centre: Vec3| 1.0 / (x.dist(centre).powi(2) + c2).sqrt();
    // m[i][k]: how far guided point i moves for a unit weight on centre k (the curve is linear in
    // its control points, so this is the field at the control points, through the curve).
    let k = at.len();
    let mut m = vec![vec![0.0; k]; k];
    for (col, centre) in at.iter().enumerate() {
        let field: Vec<Vec<Vec3>> = moved.iter().map(|e| e.iter().map(|p| Vec3::new(phi(*p, *centre), 0.0, 0.0)).collect()).collect();
        for (row, g) in gs.iter().enumerate() {
            m[row][col] = wire_point(&field, knots, *g)?.x;
        }
    }
    let w = solve(m, rest).ok_or_else(|| KernelError::Invalid("the guide rails are too close together to follow".into()))?;
    for p in moved.iter_mut().flatten() {
        let d = at.iter().zip(&w).fold(Vec3::ZERO, |d, (centre, wk)| d + *wk * phi(*p, *centre));
        *p += d;
    }
    Ok(moved)
}

/// `m x = b` (a square system, each `b` a vector) by elimination with partial pivoting; `None`
/// when `m` is singular.
fn solve(mut m: Vec<Vec<f64>>, mut b: Vec<Vec3>) -> Option<Vec<Vec3>> {
    let n = b.len();
    let scale = m.iter().flatten().fold(0.0f64, |a, v| a.max(v.abs()));
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c].abs() <= 1e-12 * scale {
            return None;
        }
        m.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = m[r][c] / m[c][c];
            if f != 0.0 {
                let (above, below) = m.split_at_mut(r);
                for (x, y) in below[0][c..].iter_mut().zip(&above[c][c..]) {
                    *x -= f * y;
                }
                let bc = b[c];
                b[r] -= bc * f;
            }
        }
    }
    let mut x = vec![Vec3::ZERO; n];
    for r in (0..n).rev() {
        let mut acc = b[r];
        for (xj, mj) in x[r + 1..].iter().zip(&m[r][r + 1..]) {
            acc -= *xj * *mj;
        }
        x[r] = acc * (1.0 / m[r][r]);
    }
    Some(x)
}

/// A wire of B-spline edges with these control points on these knots, joined end to end
/// (closing back on its first edge's start when `closed`).
fn wire_of(ctrl: &[Vec<Vec3>], knots: &[mt::KnotVec], closed: bool) -> Result<mt::Wire> {
    let fail = || KernelError::Failed("loft rails: station".into());
    let n = ctrl.len();
    let mut verts: Vec<mt::Vertex> = ctrl.iter().map(|e| e.first().map(|p| mt::Vertex::new(p3(*p)))).collect::<Option<_>>().ok_or_else(fail)?;
    if !closed {
        verts.push(mt::Vertex::new(p3(*ctrl.last().and_then(|e| e.last()).ok_or_else(fail)?)));
    }
    let mut edges = Vec::with_capacity(n);
    for (i, (pts, kv)) in ctrl.iter().zip(knots).enumerate() {
        let (Some(a), Some(b)) = (verts.get(i), verts.get(if closed { (i + 1) % n } else { i + 1 })) else { return Err(fail()) };
        let c = edge_curve(kv, pts)?;
        edges.push(mt::Edge::new(a, b, mt::Curve::BSplineCurve(c)));
    }
    Ok(edges.into())
}

/// An edge's B-spline from its control points on its knots.
fn edge_curve(kv: &mt::KnotVec, pts: &[Vec3]) -> Result<Curve> {
    Curve::try_new(kv.clone(), pts.iter().map(|p| p3(*p)).collect()).map_err(|e| KernelError::Failed(format!("loft rails: {e}")))
}
