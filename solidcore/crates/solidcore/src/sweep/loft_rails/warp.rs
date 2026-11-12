//! The warp of a station guided by three or more rails (tangency-labs/solidcore#12188, the
//! branch's `warp_station`, `solve` and `refine`, moved onto the kernel's plain curves).
//!
//! With three or more guided points, one affine move across the station's plane fits the
//! rails best (exact for three), and what is left is taken up by a smooth field of inverse
//! multiquadrics about the guided points. The field is solved through the curves, not their
//! control points: a curve's point is linear in its control points for fixed weights, so the
//! field moves every point of the station the same way wherever it is, and curves that meet
//! still meet. A station's section curves have few control points (a line has two), so
//! [`refine`] gives each curve [`WARP_SPANS`] spans first, the curve's points and parameters
//! unchanged.

use super::{invalid, wire_point, Plain};
use crate::error::Result;
use crate::math::{Point3, Vec3};
use crate::nurbs::crv_basic::Curve as NurbCurve;

/// Spans each curve of a loft of three or more rails is given before the stations are warped.
pub(super) const WARP_SPANS: usize = 8;

/// The points at wire parameters `gs` of `plains` (each curve built once).
fn at_wires(plains: &[Plain], gs: &[f64]) -> Result<Vec<Point3>> {
    let curves = plains
        .iter()
        .map(|p| -> Result<(Plain, NurbCurve)> { Ok((p.clone(), p.nurb()?)) })
        .collect::<Result<Vec<_>>>()?;
    gs.iter().map(|g| wire_point(&curves, *g)).collect()
}

/// `station` moved so that its points guided by three or more rails (`pairs`: each guided
/// point, at wire parameter `gs`, with its rail's point) land on the rails. `normal` is the
/// station's plane's unit normal and `size` the loft's size (module docs).
pub(super) fn warp_station(
    station: &[Plain],
    gs: &[f64],
    pairs: &[(Point3, Point3)],
    normal: Vec3,
    size: f64,
) -> Result<Vec<Plain>> {
    let n = pairs.len() as f64;
    let (a_sum, q_sum) = pairs
        .iter()
        .fold((Vec3::ZERO, Vec3::ZERO), |(a, q), (p, t)| (a + *p, q + *t));
    let (a_mid, q_mid) = (a_sum * (1.0 / n), q_sum * (1.0 / n));
    // Two directions across the station's plane.
    let seed = if normal.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let e1 = (seed - normal * seed.dot(normal))
        .unitize()
        .map_err(|_| invalid("a station's plane"))?;
    let e2 = normal.cross(e1);
    // Least squares for the images U, V of e1, e2: q - q_mid ~ s U + t V.
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
        return Err(invalid(
            "the guide rails meet the loft along one line: three or more rails have to surround it",
        ));
    }
    let u = (bs * tt - bt * st) * (1.0 / det);
    let v = (bt * ss - bs * st) * (1.0 / det);
    let affine = |x: Point3| {
        let d = x - a_mid;
        q_mid + u * d.dot(e1) + v * d.dot(e2) + normal * d.dot(normal)
    };
    let mut moved: Vec<Vec<Point3>> = station
        .iter()
        .map(|c| c.points.iter().map(|p| affine(*p)).collect())
        .collect();
    let with_points = |pts: &[Vec<Point3>]| -> Vec<Plain> {
        station
            .iter()
            .zip(pts)
            .map(|(c, p)| c.with(p.clone()))
            .collect()
    };
    // What the affine move leaves: each guided point's way still to its rail.
    let shaped = with_points(&moved);
    let at = at_wires(&shaped, gs)?;
    let rest: Vec<Vec3> = pairs
        .iter()
        .zip(&at)
        .map(|((_, q), p)| *q - *p)
        .collect();
    let tiny = 1e-9 * size.max(1e-9);
    if rest.iter().all(|r| r.length() <= tiny) {
        return Ok(shaped);
    }
    // The field's width: the mean distance from each guided point to its nearest other.
    let width = at
        .iter()
        .enumerate()
        .map(|(i, p)| {
            at.iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, q)| p.distance(*q))
                .fold(f64::INFINITY, f64::min)
        })
        .sum::<f64>()
        / n;
    let c2 = width.max(tiny).powi(2);
    let phi = |x: Point3, centre: Point3| 1.0 / (x.distance(centre).powi(2) + c2).sqrt();
    // column k: how far each guided point moves for a unit weight on centre k (the field at
    // the control points, through the curves).
    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(at.len());
    for centre in &at {
        let field: Vec<Plain> = station
            .iter()
            .zip(&moved)
            .map(|(c, e)| {
                c.with(
                    e.iter()
                        .map(|p| Vec3::new(phi(*p, *centre), 0.0, 0.0))
                        .collect(),
                )
            })
            .collect();
        columns.push(at_wires(&field, gs)?.iter().map(|p| p.x).collect());
    }
    let m: Vec<Vec<f64>> = (0..at.len())
        .map(|row| {
            columns
                .iter()
                .map(|col| col.get(row).copied().unwrap_or(0.0))
                .collect()
        })
        .collect();
    let w = solve(m, rest).ok_or_else(|| {
        invalid("the guide rails are too close together to follow")
    })?;
    for p in moved.iter_mut().flatten() {
        let d = at
            .iter()
            .zip(&w)
            .fold(Vec3::ZERO, |d, (centre, wk)| d + *wk * phi(*p, *centre));
        *p += d;
    }
    Ok(with_points(&moved))
}

/// The entry `(r, c)` of a row-major system's matrix, its absolute value (0 past its end).
fn abs_at(rows: &[(Vec<f64>, Vec3)], r: usize, c: usize) -> f64 {
    rows.get(r)
        .and_then(|(row, _)| row.get(c))
        .map_or(0.0, |v| v.abs())
}

/// `m x = b` (a square system, each `b` a vector) by elimination with partial pivoting; `None`
/// when `m` is singular or not square.
fn solve(m: Vec<Vec<f64>>, b: Vec<Vec3>) -> Option<Vec<Vec3>> {
    let n = b.len();
    if m.len() != n || m.iter().any(|row| row.len() != n) {
        return None;
    }
    let mut rows: Vec<(Vec<f64>, Vec3)> = m.into_iter().zip(b).collect();
    let scale = rows
        .iter()
        .flat_map(|(row, _)| row.iter())
        .fold(0.0f64, |a, v| a.max(v.abs()));
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| abs_at(&rows, i, c).total_cmp(&abs_at(&rows, j, c)))?;
        rows.swap(c, p);
        let (head, tail) = rows.split_at_mut(c + 1);
        let (prow, pb) = head.last()?;
        let pivot = prow.get(c).copied()?;
        if pivot.abs() <= 1e-12 * scale {
            return None;
        }
        for (row, bv) in tail.iter_mut() {
            let f = row.get(c).copied()? / pivot;
            if f != 0.0 {
                for (x, y) in row.iter_mut().skip(c).zip(prow.iter().skip(c)) {
                    *x -= f * y;
                }
                *bv -= *pb * f;
            }
        }
    }
    // back substitution, the solution built last unknown first
    let mut sol: Vec<Vec3> = Vec::with_capacity(n);
    for r in (0..n).rev() {
        let (row, bv) = rows.get(r)?;
        let mut acc = *bv;
        for (k, xj) in sol.iter().enumerate() {
            acc -= *xj * row.get(n - 1 - k).copied()?;
        }
        let diag = row.get(r).copied()?;
        sol.push(acc * (1.0 / diag));
    }
    sol.reverse();
    Some(sol)
}

/// `c` the same curve with more control points for a warp to move: a straight line (two
/// control points, which can only move as a whole) as a cubic, then knots inserted until it has
/// `spans` even spans. The curve's points and parameters are unchanged, and two curves that pair
/// still pair after it.
#[allow(clippy::cast_precision_loss)] // at most WARP_SPANS
pub(super) fn refine(c: &Plain, spans: usize) -> Result<Plain> {
    let (Some(&t0), Some(&t1)) = (c.knots.get(c.degree), c.knots.get(c.points.len())) else {
        return Err(invalid("a curve's knots"));
    };
    let unweighted = c
        .weights
        .as_ref()
        .is_none_or(|w| w.iter().all(|x| (x - 1.0).abs() < 1e-12));
    let plain = if c.degree == 1 && c.points.len() == 2 && unweighted {
        let (Some(a), Some(b)) = (c.points.first(), c.points.get(1)) else {
            return Err(invalid("a curve's control points"));
        };
        Plain {
            degree: 3,
            knots: vec![t0, t0, t0, t0, t1, t1, t1, t1],
            points: (0..4)
                .map(|i| *a + (*b - *a) * (f64::from(i) / 3.0))
                .collect(),
            weights: None,
        }
    } else {
        c.clone()
    };
    let mut new_knots: Vec<f64> = Vec::new();
    for i in 1..spans {
        let u = t0 + (t1 - t0) * i as f64 / spans as f64;
        if plain
            .knots
            .iter()
            .any(|k| (k - u).abs() <= 1e-12 * (1.0 + u.abs()))
        {
            continue;
        }
        new_knots.push(u);
    }
    if new_knots.is_empty() {
        return Ok(plain);
    }
    let refined = plain.nurb()?.refine(&new_knots)?;
    let cp = refined.cpts();
    let weights = plain
        .weights
        .as_ref()
        .map(|_| cp.iter().map(|q| q.w).collect());
    Ok(Plain {
        degree: plain.degree,
        knots: refined.knots().to_vec(),
        points: cp.iter().map(|q| q.to_euclid()).collect(),
        weights,
    })
}
