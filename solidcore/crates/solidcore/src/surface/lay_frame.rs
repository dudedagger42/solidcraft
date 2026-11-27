//! A plane frame laid onto a plane or a cylinder at the point under one of its own points, so a
//! profile drawn in the frame keeps its lengths on the surface.
//!
//! The point is found along the frame's normal. The frame is then turned by the least rotation
//! that takes its normal onto the surface's normal there (the surface's normal on the frame's
//! side), so the profile reads the way it does when it is projected, and is moved so the chosen
//! point lands on the surface. On a cylinder the laid frame is tangent to the wall, and a wrap
//! from it keeps arc length round the axis.

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

fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(a, a).sqrt();
    (n.is_finite() && n > 0.0).then(|| scale(a, 1.0 / n))
}

/// The surface a frame is laid on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaySurface {
    /// A plane.
    Plane {
        /// A point of the plane.
        point: [f64; 3],
        /// Its unit normal.
        normal: [f64; 3],
    },
    /// A cylinder.
    Cylinder {
        /// A point of the axis.
        origin: [f64; 3],
        /// The axis's unit direction.
        axis: [f64; 3],
        /// The radius.
        radius: f64,
    },
}

/// A frame laid on a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaidFrame {
    /// Where the frame's origin goes: the chosen point lands on the surface.
    pub origin: [f64; 3],
    /// The frame's x, turned onto the surface.
    pub x: [f64; 3],
    /// The frame's y, turned onto the surface.
    pub y: [f64; 3],
    /// The point on the surface under the chosen point.
    pub at: [f64; 3],
    /// Whether the surface's own normal there (a plane's, or away from a cylinder's axis) is on
    /// the frame's side: the laid frame's normal is that normal when it is, its reverse when not.
    pub facing: bool,
}

/// Lays the frame (`origin`, unit `x`, unit `y` square to it) onto `surface` at the point under
/// the frame point `(u, v)` along the frame's normal. Of a cylinder's two points under it, the one
/// nearer `near`. `None` where the line along the normal misses the surface or runs along it,
/// or the input is not finite.
#[must_use]
pub fn lay_frame(
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    (u, v): (f64, f64),
    surface: LaySurface,
    near: [f64; 3],
) -> Option<LaidFrame> {
    let normal = unit(cross(x, y))?;
    let p = add(origin, add(scale(x, u), scale(y, v)));
    let (at, face_normal) = match surface {
        LaySurface::Plane { point, normal: n } => {
            let n = unit(n)?;
            let along = dot(normal, n);
            if along.abs() < 1e-12 {
                return None;
            }
            let t = dot(sub(point, p), n) / along;
            (add(p, scale(normal, t)), n)
        }
        LaySurface::Cylinder {
            origin: o,
            axis,
            radius,
        } => {
            let axis = unit(axis)?;
            if !(radius.is_finite() && radius > 0.0) {
                return None;
            }
            let off = |w: [f64; 3]| sub(w, scale(axis, dot(w, axis)));
            let d = off(normal);
            let w = off(sub(p, o));
            let a = dot(d, d);
            if a < 1e-12 {
                return None;
            }
            let b = dot(w, d);
            let disc = b * b - a * (dot(w, w) - radius * radius);
            if disc.is_nan() || disc < 0.0 {
                return None;
            }
            let root = disc.sqrt();
            let hits = [(-b - root) / a, (-b + root) / a].map(|t| add(p, scale(normal, t)));
            let gap = |h: [f64; 3]| dot(sub(h, near), sub(h, near));
            let at = if gap(hits[1]) < gap(hits[0]) {
                hits[1]
            } else {
                hits[0]
            };
            (at, unit(off(sub(at, o)))?)
        }
    };
    let facing = dot(normal, face_normal) >= 0.0;
    let target = if facing {
        face_normal
    } else {
        scale(face_normal, -1.0)
    };
    // Rodrigues' rotation taking `normal` onto `target`; their dot is not negative here.
    let k = cross(normal, target);
    let c = dot(normal, target);
    let turn = |w: [f64; 3]| {
        add(
            add(scale(w, c), cross(k, w)),
            scale(k, dot(k, w) / (1.0 + c)),
        )
    };
    let (lx, ly) = (unit(turn(x))?, unit(turn(y))?);
    let laid_origin = sub(at, add(scale(lx, u), scale(ly, v)));
    let all = [laid_origin, lx, ly, at];
    all.iter()
        .flatten()
        .all(|c| c.is_finite())
        .then_some(LaidFrame {
            origin: laid_origin,
            x: lx,
            y: ly,
            at,
            facing,
        })
}

/// Rolls a frame tangent to a cylinder (axis through `axis_origin` along unit `axis`, radius
/// `radius`) round it by `angle` without slipping, as a sheet rolls on a rod: the frame turns
/// about the axis and slides along its turned tangent by the arc rolled, so a wrap from the
/// rolled frame lays every frame point where a wrap from the first does. A positive `angle`
/// turns the line of contact towards the axis crossed with the direction to it. Returns the
/// rolled origin, x and y; `None` unless the frame is parallel to the axis or the input is not
/// finite.
#[must_use]
pub fn roll_frame(
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    axis_origin: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    angle: f64,
) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    let axis = unit(axis)?;
    let normal = unit(cross(x, y))?;
    if dot(normal, axis).abs() > 1e-8 || !(radius.is_finite() && angle.is_finite()) {
        return None;
    }
    let w = sub(origin, axis_origin);
    let radial = if dot(w, normal) < 0.0 {
        scale(normal, -1.0)
    } else {
        normal
    };
    let t = cross(axis, radial);
    let along = dot(w, t);
    let contact = sub(origin, scale(t, along));
    let (sin, cos) = angle.sin_cos();
    let turn = |v: [f64; 3]| {
        add(
            add(scale(v, cos), scale(cross(axis, v), sin)),
            scale(axis, dot(axis, v) * (1.0 - cos)),
        )
    };
    let turned_t = turn(t);
    let rolled_contact = add(axis_origin, turn(sub(contact, axis_origin)));
    let rolled = add(rolled_contact, scale(turned_t, along - radius * angle));
    let all = [rolled, turn(x), turn(y)];
    all.iter()
        .flatten()
        .all(|c| c.is_finite())
        .then_some((rolled, turn(x), turn(y)))
}

/// The half of a cylinder a line along `normal` crosses once: the unit part of `normal` square
/// to the unit `axis`, turned towards `towards` (a vector from the axis). A line along `normal`
/// crosses the wall once each side of the plane through the axis square to that direction.
/// `None` where `normal` runs along the axis.
#[must_use]
pub fn crossing_side(axis: [f64; 3], normal: [f64; 3], towards: [f64; 3]) -> Option<[f64; 3]> {
    let axis = unit(axis)?;
    toward(sub(normal, scale(axis, dot(normal, axis))), towards)
}

/// The unit `direction`, turned towards `towards`. `None` where `direction` is zero or not
/// finite.
#[must_use]
pub fn toward(direction: [f64; 3], towards: [f64; 3]) -> Option<[f64; 3]> {
    let d = unit(direction)?;
    Some(if dot(towards, d) < 0.0 {
        scale(d, -1.0)
    } else {
        d
    })
}

/// Whether two cylinders (a point of the axis, its unit direction, the radius, and whether the
/// normal points away from the axis) are one surface within `tol`.
#[must_use]
pub fn same_cylinder(
    a: ([f64; 3], [f64; 3], f64, bool),
    b: ([f64; 3], [f64; 3], f64, bool),
    tol: f64,
) -> bool {
    let (ao, aa, ar, ac) = a;
    let (bo, ba, br, bc) = b;
    let w = sub(bo, ao);
    let off = sub(w, scale(aa, dot(w, aa)));
    ac == bc
        && (ar - br).abs() <= tol
        && dot(aa, ba).abs() > 1.0 - 1e-8
        && dot(off, off).sqrt() <= tol
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        dot(sub(a, b), sub(a, b)).sqrt() < 1e-12
    }

    #[test]
    fn a_frame_parallel_to_a_plane_moves_onto_it_unturned() {
        let plane = LaySurface::Plane {
            point: [0.0, 0.0, 5.0],
            normal: [0.0, 0.0, 1.0],
        };
        let f = lay_frame(
            [1.0, 2.0, 30.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (3.0, 4.0),
            plane,
            [0.0; 3],
        )
        .unwrap();
        assert!(close(f.at, [4.0, 6.0, 5.0]));
        assert!(close(f.origin, [1.0, 2.0, 5.0]));
        assert!(close(f.x, [1.0, 0.0, 0.0]) && close(f.y, [0.0, 1.0, 0.0]));
        assert!(f.facing);
    }

    #[test]
    fn a_tilted_frame_turns_onto_a_plane_keeping_lengths() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let plane = LaySurface::Plane {
            point: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        };
        // Tilted 45 degrees about x: its y runs up the slope.
        let f = lay_frame(
            [0.0, 0.0, 10.0],
            [1.0, 0.0, 0.0],
            [0.0, s, s],
            (0.0, 0.0),
            plane,
            [0.0; 3],
        )
        .unwrap();
        assert!(close(f.at, [0.0, 10.0, 0.0]));
        assert!(close(f.x, [1.0, 0.0, 0.0]) && close(f.y, [0.0, 1.0, 0.0]));
    }

    #[test]
    fn a_frame_through_a_cylinders_axis_lands_on_the_side_nearest_the_pick() {
        let wall = LaySurface::Cylinder {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
        };
        // The XZ plane seen from +y: x is -X, y is Z.
        let (o, x, y) = ([0.0; 3], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let front = lay_frame(o, x, y, (0.0, 3.0), wall, [0.0, 10.0, 0.0]).unwrap();
        assert!(close(front.at, [0.0, 10.0, 3.0]) && front.facing);
        assert!(close(front.x, x) && close(front.origin, [0.0, 10.0, 0.0]));
        let back = lay_frame(o, x, y, (0.0, 3.0), wall, [0.0, -10.0, 0.0]).unwrap();
        assert!(close(back.at, [0.0, -10.0, 3.0]) && !back.facing);
        // Off the middle the frame turns about the axis to stay tangent: 6 along x is the
        // point at x = -6, and the laid x runs round the wall there.
        let side = lay_frame(o, x, y, (6.0, 0.0), wall, [0.0, 10.0, 0.0]).unwrap();
        assert!(close(side.at, [-6.0, 8.0, 0.0]));
        assert!(close(side.x, [-0.8, -0.6, 0.0]));
        assert!(close(add(side.origin, scale(side.x, 6.0)), side.at));
    }

    #[test]
    fn a_rolled_frame_meets_the_wall_where_the_arc_rolled_ends() {
        // The plane y = 10 touching a radius 10 rod about z along x = 0, x along -x.
        let (o, x, y) = ([0.0, 10.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let quarter = std::f64::consts::FRAC_PI_2;
        let (ro, rx, ry) = roll_frame(o, x, y, [0.0; 3], [0.0, 0.0, 1.0], 10.0, quarter).unwrap();
        // A quarter turn towards -x: the contact is at x = -10, where the frame point 5 pi along
        // its x now touches, and the frame's x runs down -y.
        assert!(close(rx, [0.0, -1.0, 0.0]) && close(ry, y));
        let touching = add(ro, scale(rx, 5.0 * std::f64::consts::PI));
        assert!(close(touching, [-10.0, 0.0, 0.0]), "{touching:?}");
        assert!(roll_frame(o, x, [0.0, 1.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0], 10.0, 1.0).is_none());
    }

    #[test]
    fn a_line_crosses_a_wall_once_on_the_picked_side_and_coaxial_walls_are_one() {
        let z = [0.0, 0.0, 1.0];
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let side = crossing_side(z, [0.0, s, s], [0.0, -3.0, 0.0]).unwrap();
        assert!(close(side, [0.0, -1.0, 0.0]));
        assert!(crossing_side(z, z, [1.0, 0.0, 0.0]).is_none());
        let back = toward([0.0, 2.0, 0.0], [0.0, -1.0, 5.0]).unwrap();
        assert!(close(back, [0.0, -1.0, 0.0]));
        let wall = ([0.0; 3], z, 10.0, true);
        assert!(same_cylinder(
            wall,
            ([0.0, 0.0, 7.0], [0.0, 0.0, -1.0], 10.0, true),
            1e-9
        ));
        assert!(!same_cylinder(
            wall,
            ([0.0, 0.0, 7.0], z, 10.0, false),
            1e-9
        ));
        assert!(!same_cylinder(wall, ([0.1, 0.0, 0.0], z, 10.0, true), 1e-9));
    }

    #[test]
    fn a_line_missing_the_surface_or_along_its_axis_has_no_frame() {
        let wall = LaySurface::Cylinder {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
        };
        let o = [0.0, 30.0, 0.0];
        assert!(lay_frame(o, [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (11.0, 0.0), wall, o).is_none());
        assert!(lay_frame(o, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (0.0, 0.0), wall, o).is_none());
        let plane = LaySurface::Plane {
            point: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        };
        assert!(lay_frame(o, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 0.0), plane, o).is_none());
    }
}
