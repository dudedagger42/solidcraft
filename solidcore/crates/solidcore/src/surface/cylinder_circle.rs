//! The circles of a cylinder that a gap in its face closes over: where a plane square to the
//! axis cuts it, the arc between two of its points on a given side, and whether points lie on
//! its circles and directions along its rulings.

use std::f64::consts::TAU;

use crate::math::Vec3;

/// A cylinder: a point of its axis, its unit axis and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder {
    /// A point of the axis.
    pub origin: Vec3,
    /// The unit axis.
    pub axis: Vec3,
    /// The radius.
    pub radius: f64,
}

impl Cylinder {
    /// How far `p` lies along the axis from [`Cylinder::origin`].
    #[must_use]
    pub fn height(&self, p: Vec3) -> f64 {
        (p - self.origin).dot(self.axis)
    }

    /// The point of the axis level with `p`.
    #[must_use]
    pub fn centre_at(&self, p: Vec3) -> Vec3 {
        self.origin + self.axis * self.height(p)
    }

    /// The cylinder's circle frame starting at `p`: `x_axis` the unit radial direction from the
    /// axis to `p`, `y_axis` the axis crossed with it, so that `x_axis × y_axis` is the axis.
    #[must_use]
    pub fn frame_at(&self, p: Vec3) -> (Vec3, Vec3) {
        let radial = p - self.centre_at(p);
        let x_axis = radial * (1.0 / radial.length());
        (x_axis, self.axis.cross(x_axis))
    }

    /// Whether `p` lies within `tol` of the cylinder's circle at `height`.
    #[must_use]
    pub fn on_circle(&self, p: Vec3, height: f64, tol: f64) -> bool {
        (self.height(p) - height).abs() <= tol
            && ((p - self.centre_at(p)).length() - self.radius).abs() <= tol
    }

    /// Whether the circle of centre `origin`, unit `normal` and `radius` is this cylinder's circle at
    /// `height`: its plane is square to the axis (a tilt `s` moves its rim `radius * s` along the
    /// axis, at most `tol`), its centre lies on the axis at `height`, and its radius is `tol` from
    /// this one's.
    #[must_use]
    pub fn is_circle_at(
        &self,
        origin: Vec3,
        normal: Vec3,
        radius: f64,
        height: f64,
        tol: f64,
    ) -> bool {
        normal.cross(self.axis).length() <= tol / radius.max(1.0)
            && (origin - self.centre_at(origin)).length() <= tol
            && (self.height(origin) - height).abs() <= tol
            && (radius - self.radius).abs() <= tol
    }

    /// Whether `p` and `q` lie on one circle of the cylinder's: the same height within `tol`.
    #[must_use]
    pub fn same_circle(&self, p: Vec3, q: Vec3, tol: f64) -> bool {
        (self.height(p) - self.height(q)).abs() <= tol
    }

    /// Whether `dir` runs along the axis, as a ruling does.
    #[must_use]
    pub fn is_ruling(&self, dir: Vec3) -> bool {
        dir.cross(self.axis).length() <= 1e-9 * dir.length().max(1.0)
    }

    /// The centre of the circle where the plane `normal · x = d`, square to the axis, cuts the
    /// cylinder, when `a` and `b` both lie on that circle within `tol`.
    #[must_use]
    pub fn plane_circle(&self, normal: Vec3, d: f64, a: Vec3, b: Vec3, tol: f64) -> Option<Vec3> {
        let across = normal.dot(self.axis);
        if normal.cross(self.axis).length() > 1e-9 || across.abs() < 1e-9 {
            return None;
        }
        let c = self.origin + self.axis * ((d - normal.dot(self.origin)) / across);
        let on = |p: Vec3| {
            (p.distance(c) - self.radius).abs() <= tol && (normal.dot(p) - d).abs() <= tol
        };
        (on(a) && on(b)).then_some(c)
    }
    /// Whether `other` is this cylinder: the same radius, the same axis direction and the same
    /// axis (`other`'s origin within `tol` of it) within `tol`.
    #[must_use]
    pub fn same_cylinder(&self, other: &Cylinder, tol: f64) -> bool {
        let across = other.origin - self.origin;
        (self.radius - other.radius).abs() <= tol
            && self.axis.cross(other.axis).length() <= 1e-9
            && (across - self.axis * across.dot(self.axis)).length() <= tol
    }

    /// Whether `p` lies within `tol` of the cylinder's wall: its distance from the axis is the radius.
    #[must_use]
    pub fn on_wall(&self, p: Vec3, tol: f64) -> bool {
        ((p - self.centre_at(p)).length() - self.radius).abs() <= tol
    }

    /// Whether the straight line from `a` to `b` runs along the axis within `tol`, so a ruling of
    /// the cylinder: `|(b − a) × axis| <= tol`.
    #[must_use]
    pub fn runs_along_axis(&self, a: Vec3, b: Vec3, tol: f64) -> bool {
        (b - a).cross(self.axis).length() <= tol
    }

    /// Where the line `p0 + u·t` meets the cylinder: the points at the two roots of its quadratic
    /// in the plane square to the axis. Empty when it misses or runs along the axis.
    #[must_use]
    pub fn line_hits(&self, p0: Vec3, u: Vec3) -> Vec<Vec3> {
        let w = p0 - self.centre_at(p0);
        let du = u - self.axis * u.dot(self.axis);
        let (qa, qb, qc) = (
            du.dot(du),
            2.0 * du.dot(w),
            w.dot(w) - self.radius * self.radius,
        );
        let disc = qb * qb - 4.0 * qa * qc;
        if qa < 1e-12 || disc < 0.0 {
            return Vec::new();
        }
        [
            (-qb - disc.sqrt()) / (2.0 * qa),
            (-qb + disc.sqrt()) / (2.0 * qa),
        ]
        .map(|t| p0 + u * t)
        .to_vec()
    }

    /// The centre and the outward side of an edge from `a` to `b` that runs round the cylinder:
    /// its sampled points `pts` all lie on the circle at `a`'s height, and so does `b`, within
    /// `tol`. The side is the middle sample's side of the chord, the way round the arc goes.
    #[must_use]
    pub fn round_side(&self, pts: &[Vec3], a: Vec3, b: Vec3, tol: f64) -> Option<(Vec3, Vec3)> {
        let h = self.height(a);
        let on = |p: Vec3| (self.height(p) - h).abs() <= tol && self.on_wall(p, tol);
        let mid = *pts.get(2)?;
        (pts.iter().all(|p| on(*p)) && (self.height(b) - h).abs() <= tol)
            .then(|| (self.centre_at(a), mid - (a + b) * 0.5))
    }
}

/// The line where planes `n1·x = d1` and `n2·x = d2` meet: a point on both and its unit
/// direction. `None` when the planes are parallel.
#[must_use]
pub fn plane_pair_line(n1: Vec3, d1: f64, n2: Vec3, d2: f64) -> Option<(Vec3, Vec3)> {
    let c = n1.cross(n2);
    if c.length() < 1e-9 {
        return None;
    }
    let u = c * (1.0 / c.length());
    let p0 = (n2.cross(u) * d1 + u.cross(n1) * d2) / c.dot(u);
    Some((p0, u))
}

/// An arc about a centre: from its start along `x_axis`, through `sweep` radians towards
/// `y_axis`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    /// From the centre towards the start, unit.
    pub x_axis: Vec3,
    /// A quarter turn on from `x_axis`, unit.
    pub y_axis: Vec3,
    /// The radius.
    pub radius: f64,
    /// The angle swept, in (0, 2π].
    pub sweep: f64,
}

/// The arc from `a` to `b` about `centre` in the plane normal to the unit `axis`, the way round
/// whose middle lies on the side `out` points to from the chord `a`–`b`. `None` for a radius of
/// `tol` or less, or when neither way's middle lies more than `tol` to that side. An end less
/// than `tol` along the circle from the start makes a full turn.
#[must_use]
pub fn arc_toward(a: Vec3, b: Vec3, centre: Vec3, axis: Vec3, out: Vec3, tol: f64) -> Option<Arc> {
    let radius = a.distance(centre);
    if radius.is_nan() || radius <= tol {
        return None;
    }
    let x_axis = (a - centre) * (1.0 / radius);
    let chord = (a + b) * 0.5;
    let way = |n: Vec3| {
        let y_axis = n.cross(x_axis);
        let mut sweep = (b - centre).dot(y_axis).atan2((b - centre).dot(x_axis));
        // An arc length, not an angle, against the length tolerance.
        if sweep * radius <= tol {
            sweep += TAU;
        }
        let half = sweep / 2.0;
        let mid = centre + (x_axis * half.cos() + y_axis * half.sin()) * radius;
        (
            Arc {
                x_axis,
                y_axis,
                radius,
                sweep,
            },
            (mid - chord).dot(out),
        )
    };
    let (one, s1) = way(axis);
    let (other, s2) = way(axis * -1.0);
    (s1.max(s2) > tol).then_some(if s1 >= s2 { one } else { other })
}

/// The arc from `a` to `b` about `centre` through `mid`: the way round whose middle lies on
/// `mid`'s side of the chord. See [`arc_toward`].
#[must_use]
pub fn arc_through(a: Vec3, mid: Vec3, b: Vec3, centre: Vec3, axis: Vec3, tol: f64) -> Option<Arc> {
    arc_toward(a, b, centre, axis, mid - (a + b) * 0.5, tol)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cylinder() -> Cylinder {
        Cylinder {
            origin: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            radius: 2.0,
        }
    }

    #[test]
    fn a_circle_about_the_axis_at_its_height_is_on_the_cylinder() {
        let up = Vec3::new(0.0, 0.0, 1.0);
        assert!(cylinder().is_circle_at(Vec3::new(0.0, 0.0, 5.0), up, 2.0, 5.0, 1e-9));
    }

    #[test]
    fn a_tilted_or_off_axis_or_other_radius_circle_is_not() {
        let c = cylinder();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let at = Vec3::new(0.0, 0.0, 5.0);
        let tilted = Vec3::new(1.0, 0.0, 1.0) * (1.0 / 2f64.sqrt());
        assert!(!c.is_circle_at(at, tilted, 2.0, 5.0, 1e-9), "tilted");
        assert!(
            !c.is_circle_at(Vec3::new(0.5, 0.0, 5.0), up, 2.0, 5.0, 1e-9),
            "off the axis"
        );
        assert!(!c.is_circle_at(at, up, 3.0, 5.0, 1e-9), "other radius");
        assert!(!c.is_circle_at(at, up, 2.0, 6.0, 1e-9), "other height");
    }

    #[test]
    fn the_frame_at_a_point_starts_on_its_radial_side() {
        let c = cylinder();
        let p = Vec3::new(0.0, 2.0, 5.0);
        let (x, y) = c.frame_at(p);
        assert!(x.dot(c.axis).abs() < 1e-12, "x square to the axis");
        assert!((x.length() - 1.0).abs() < 1e-12, "x unit");
        assert!((y - c.axis.cross(x)).length() < 1e-12, "y is axis × x");
        assert!((p - c.centre_at(p)).dot(x) > 0.0, "p on the +x side");
        assert!(
            (x - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-12,
            "x points at p"
        );
    }

    #[test]
    fn points_at_one_height_share_a_circle() {
        let c = cylinder();
        assert!(c.same_circle(Vec3::new(1.0, 0.0, 3.0), Vec3::new(0.0, 2.0, 3.0), 1e-9));
        assert!(!c.same_circle(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, 3.5), 0.1));
    }

    #[test]
    fn a_short_arc_on_a_large_circle_is_not_a_full_turn() {
        let r = 1.0e5;
        let (a, b) = (
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(r * 0.02f64.cos(), r * 0.02f64.sin(), 0.0),
        );
        let arc = arc_toward(
            a,
            b,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.024,
        )
        .unwrap();
        assert!((arc.sweep - 0.02).abs() < 1e-12, "{}", arc.sweep);
    }

    #[test]
    fn a_half_circle_through_its_top_point_goes_the_top_way() {
        let z = Vec3::new(0.0, 0.0, 1.0);
        let arc = arc_through(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(-2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            z,
            1e-9,
        )
        .unwrap();
        assert!(
            (arc.sweep - std::f64::consts::PI).abs() < 1e-12,
            "{}",
            arc.sweep
        );
        let half = arc.sweep / 2.0;
        let mid = (arc.x_axis * half.cos() + arc.y_axis * half.sin()) * arc.radius;
        assert!((mid - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-9, "{mid:?}");
    }

    #[test]
    fn the_arc_goes_round_on_the_side_asked() {
        let (a, b) = (Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -10.0, 0.0));
        let z = Vec3::new(0.0, 0.0, 1.0);
        for (out, mid_x) in [(1.0, 10.0), (-1.0, -10.0)] {
            let arc = arc_toward(
                a,
                b,
                Vec3::new(0.0, 0.0, 0.0),
                z,
                Vec3::new(out, 0.0, 0.0),
                1e-9,
            )
            .unwrap();
            let half = arc.sweep / 2.0;
            let mid = (arc.x_axis * half.cos() + arc.y_axis * half.sin()) * arc.radius;
            assert!((mid.x - mid_x).abs() < 1e-9, "{mid:?}");
        }
        assert!(arc_toward(
            a,
            a,
            Vec3::new(0.0, 0.0, 0.0),
            z,
            Vec3::new(0.0, 0.0, 0.0),
            1e-9
        )
        .is_none());
    }

    #[test]
    fn a_plane_square_to_the_axis_cuts_the_circle_its_points_lie_on() {
        let c = Cylinder {
            origin: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            radius: 10.0,
        };
        let z = Vec3::new(0.0, 0.0, 1.0);
        let (a, b) = (Vec3::new(10.0, 0.0, 4.0), Vec3::new(0.0, 10.0, 4.0));
        assert_eq!(
            c.plane_circle(z, 4.0, a, b, 1e-9),
            Some(Vec3::new(0.0, 0.0, 4.0))
        );
        assert_eq!(c.plane_circle(z, 5.0, a, b, 1e-9), None);
        assert_eq!(
            c.plane_circle(Vec3::new(1.0, 0.0, 0.0), 4.0, a, b, 1e-9),
            None
        );
        assert!(c.on_circle(a, 4.0, 1e-9) && !c.on_circle(a, 3.0, 1e-9));
        assert!(c.is_ruling(z * -3.0) && !c.is_ruling(Vec3::new(1.0, 0.0, 1.0)));
    }

    #[test]
    fn two_planes_meet_in_a_line_on_both_planes() {
        let (p0, u) = plane_pair_line(Vec3::new(1.0, 0.0, 0.0), 2.0, Vec3::new(0.0, 1.0, 0.0), 3.0)
            .expect("the planes meet");
        assert!(
            (p0 - Vec3::new(2.0, 3.0, 0.0)).length() < 1e-12,
            "p0 on both planes"
        );
        assert!(
            (u - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-12,
            "u along the line"
        );
        assert!(
            plane_pair_line(Vec3::new(1.0, 0.0, 0.0), 2.0, Vec3::new(2.0, 0.0, 0.0), 3.0).is_none(),
            "parallel planes"
        );
    }

    #[test]
    fn a_line_meets_the_cylinder_at_its_two_roots_or_misses() {
        let c = cylinder();
        let hits = c.line_hits(Vec3::new(0.0, 0.0, 7.0), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(hits.len(), 2);
        assert!((hits[0] - Vec3::new(-2.0, 0.0, 7.0)).length() < 1e-12);
        assert!((hits[1] - Vec3::new(2.0, 0.0, 7.0)).length() < 1e-12);
        assert!(c
            .line_hits(Vec3::new(0.0, 5.0, 7.0), Vec3::new(1.0, 0.0, 0.0))
            .is_empty());
    }

    #[test]
    fn a_cylinder_with_the_same_axis_and_radius_is_the_same_one() {
        let c = cylinder();
        let shifted = Cylinder {
            origin: Vec3::new(0.0, 0.0, 9.0),
            ..c
        };
        assert!(c.same_cylinder(&shifted, 1e-9), "a point of the same axis");
        let off = Cylinder {
            origin: Vec3::new(0.5, 0.0, 0.0),
            ..c
        };
        assert!(!c.same_cylinder(&off, 1e-9), "another axis");
        let wider = Cylinder { radius: 3.0, ..c };
        assert!(!c.same_cylinder(&wider, 1e-9), "another radius");
    }

    #[test]
    fn a_point_on_the_wall_is_within_tol_of_it() {
        let c = cylinder();
        assert!(c.on_wall(Vec3::new(2.0, 0.0, 5.0), 1e-9));
        assert!(!c.on_wall(Vec3::new(1.0, 0.0, 5.0), 1e-9));
    }

    #[test]
    fn a_line_runs_along_the_axis_only_within_tol() {
        let c = cylinder();
        let (o, up) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.1, 0.0, 3.0));
        assert!(c.runs_along_axis(o, Vec3::new(0.0, 0.0, 3.0), 1e-6));
        assert!(!c.runs_along_axis(o, up, 1e-6), "leans off the axis");
        assert!(c.runs_along_axis(o, up, 0.2), "within a looser tol");
    }

    #[test]
    fn a_round_edge_arcs_about_the_axis_on_the_middle_samples_side() {
        let c = cylinder();
        let at = |deg: f64, z: f64| {
            let t = deg.to_radians();
            Vec3::new(2.0 * t.cos(), 2.0 * t.sin(), z)
        };
        let pts: Vec<Vec3> = (0..=4).map(|i| at(f64::from(i) * 22.5, 5.0)).collect();
        let (centre, out) = c
            .round_side(&pts, at(0.0, 5.0), at(90.0, 5.0), 1e-9)
            .expect("round");
        assert!(
            (centre - Vec3::new(0.0, 0.0, 5.0)).length() < 1e-12,
            "centre on the axis"
        );
        assert!(
            out.dot(Vec3::new(1.0, 1.0, 0.0)) > 0.0,
            "the middle's side of the chord"
        );
        assert!(
            c.round_side(&pts, at(0.0, 5.0), at(90.0, 6.0), 1e-9)
                .is_none(),
            "b off the height"
        );
    }
}
