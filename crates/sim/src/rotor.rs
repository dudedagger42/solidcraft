//! Rotors: solid bodies that turn through the fixed lattice at a set speed (a fan's impeller), and
//! the force and torque the air puts on them.
//!
//! The body is voxelized once, in its own frame at angle 0, on a grid half the lattice's spacing.
//! At every step a lattice cell of the region the body sweeps is covered when its centre, turned
//! back about the axis by the rotor's angle, falls in a solid cell of that fine grid. Then:
//!
//! - a population streaming from a fluid cell into a covered cell bounces back with Ladd's
//!   moving-wall term at the surface velocity `Ω × r` of the link's midpoint (A. J. C. Ladd,
//!   *Numerical simulations of particulate suspensions via a discretized Boltzmann equation*,
//!   J. Fluid Mech. 271:285–309, 1994), and gives the body the momentum `c_q (f_out + f_back)`;
//! - a cell the body comes to cover gives it the fluid's momentum and stops colliding;
//! - a cell the body uncovers is refilled with the equilibrium at the surface velocity and the
//!   mean density of its fluid neighbours, and takes that momentum from the body (C. K. Aidun,
//!   Y. Lu, E.-J. Ding, J. Fluid Mech. 373:287–311, 1998).
//!
//! The torque is the sum of those momenta's moments about the axis. The blade surface is a
//! staircase at the fine grid's resolution, so like the static walls it is first order in the
//! spacing. Written from the papers; no lattice Boltzmann package's code was read.

use solidcraft_geom::{Mesh, MeshIndex, Vec3};

use crate::{Cell, Grid, Result, SimError, voxelize};

/// The fastest a rotor's surface may move through the lattice (cells per step): its tips must
/// stay well below the lattice's speed of sound (√⅓) for the flow to stay incompressible.
pub const MAX_TIP_SPEED: f64 = 0.2;

/// A body turning about an axis at a constant rate.
#[derive(Clone, Debug)]
pub struct Rotor {
    /// A point on the axis and the unit axis, in the lattice grid's millimetres.
    origin: Vec3,
    axis: Vec3,
    /// The angle turned per step (radians; right-handed about `axis`) and so far.
    omega: f64,
    theta: f64,
    /// The share of `omega` it turns at now (a soft start ramps it from 0 to 1).
    speed: f64,
    /// The parts that turn (blades), at angle 0 on a grid of half the lattice spacing, and their
    /// surface for the links' wall fractions.
    fine: Option<Grid>,
    body: Mesh,
    index: MeshIndex,
    /// The parts that are solids of revolution about the axis (a hub drum, its cones): the same
    /// at every angle, so they stay on the grid where they are and only move their walls (a
    /// turning staircase would cover and uncover cells at their surface every step).
    round: Option<Grid>,
    round_body: Mesh,
    round_index: MeshIndex,
    /// Whether the bounce-back is interpolated to the surface (else half way: a staircase).
    pub curved: bool,
    /// The lattice grid's corner and spacing (millimetres).
    corner: Vec3,
    spacing: f64,
    /// The lattice cells the body may cover, and whether it covers each now.
    pub(crate) swept: Vec<usize>,
    pub(crate) covered: Vec<bool>,
    /// The largest distance of the body from the axis, in lattice cells.
    pub radius: f64,
    /// The momentum and its moment about `origin` the fluid gave the body in the last step
    /// (lattice units: momentum per step, so a force and a torque).
    pub force: [f64; 3],
    pub torque: [f64; 3],
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn scale(a: Vec3, s: f64) -> Vec3 {
    Vec3 { x: a.x * s, y: a.y * s, z: a.z * s }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}

impl Rotor {
    /// `body` turning about the axis through `origin` along `axis` (millimetres, any length) at
    /// `omega` radians per lattice step, in the lattice of `grid` (whose static solid cells the
    /// body may not need). Refused when its tips would move faster than [`MAX_TIP_SPEED`].
    pub fn new(body: &Mesh, grid: &Grid, origin: Vec3, axis: Vec3, omega: f64, max_cells: u64) -> Result<Rotor> {
        Rotor::from_parts(&[body], grid, origin, axis, omega, max_cells)
    }

    /// A rotor made of several closed bodies turning together (a hub and its blades, which may
    /// overlap): each is voxelized on its own, so the body is their union, and their surfaces
    /// together are cast against for the links' wall fractions. Parts that are solids of
    /// revolution about the axis ([`is_round`]) stay put on the grid and only move their walls.
    pub fn from_parts(parts: &[&Mesh], grid: &Grid, origin: Vec3, axis: Vec3, omega: f64, max_cells: u64) -> Result<Rotor> {
        let len = dot(axis, axis).sqrt();
        if !(len.is_finite() && len > 0.0) || !omega.is_finite() {
            return Err(SimError::BadRotor);
        }
        let axis = scale(axis, 1.0 / len);
        let join = |parts: &[&Mesh]| {
            let mut whole = Mesh::default();
            for part in parts {
                let base = whole.positions.len() as u32;
                whole.positions.extend(part.positions.iter().copied());
                whole.triangles.extend(part.triangles.iter().map(|t| t.map(|v| v + base)));
            }
            whole
        };
        let (round_parts, turning_parts): (Vec<&Mesh>, Vec<&Mesh>) = parts.iter().copied().partition(|m| is_round(m, origin, axis));
        let whole = join(parts);
        let body = &whole;
        let s = grid.spacing;
        // The body's radius about the axis (millimetres), and its extent along it.
        let (mut radius, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
        for p in &body.positions {
            let d = sub(*p, origin);
            let along = dot(d, axis);
            let off = sub(d, scale(axis, along));
            radius = radius.max(dot(off, off).sqrt());
            lo = lo.min(along);
            hi = hi.max(along);
        }
        if !radius.is_finite() || radius <= 0.0 {
            return Err(SimError::EmptyMesh);
        }
        if (omega * radius / s).abs() > MAX_TIP_SPEED {
            return Err(SimError::BadRotor);
        }
        let fine_of = |parts: &[&Mesh]| -> Result<Option<Grid>> {
            if parts.is_empty() {
                return Ok(None);
            }
            let mut fine = Grid::around(join(parts).bounds(), 0.5 * s, 1, max_cells)?;
            for part in parts {
                voxelize(part, &mut fine)?;
            }
            Ok(Some(fine))
        };
        let (fine, round) = (fine_of(&turning_parts)?, fine_of(&round_parts)?);
        let (turning_body, round_body) = (join(&turning_parts), join(&round_parts));
        // Every lattice cell within a cell of the swept cylinder.
        let mut swept = Vec::new();
        let [nx, ny, nz] = grid.dims;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let d = sub(grid.center(i, j, k), origin);
                    let along = dot(d, axis);
                    let off = sub(d, scale(axis, along));
                    if along >= lo - s
                        && along <= hi + s
                        && dot(off, off).sqrt() <= radius + s
                        && let Some(c) = grid.index(i, j, k)
                    {
                        swept.push(c);
                    }
                }
            }
        }
        let covered = vec![false; swept.len()];
        Ok(Rotor {
            origin,
            axis,
            omega,
            theta: 0.0,
            speed: 1.0,
            fine,
            index: MeshIndex::new(&turning_body),
            body: turning_body,
            round,
            round_index: MeshIndex::new(&round_body),
            round_body,
            curved: true,
            corner: grid.origin,
            spacing: s,
            swept,
            covered,
            radius: radius / s,
            force: [0.0; 3],
            torque: [0.0; 3],
        })
    }

    /// The angle turned so far (radians).
    pub fn angle(&self) -> f64 {
        self.theta
    }

    /// The rate (radians per step).
    pub fn omega(&self) -> f64 {
        self.omega
    }

    /// The torque about the axis in the last step (lattice units), positive along the axis: the
    /// air's drag on a driven fan is negative, so the motor must supply minus this.
    pub fn axial_torque(&self) -> f64 {
        dot(Vec3 { x: self.torque[0], y: self.torque[1], z: self.torque[2] }, self.axis)
    }

    /// The centre of lattice cell `c` (millimetres).
    fn centre(&self, dims: [usize; 3], c: usize) -> Vec3 {
        let [nx, ny, _] = dims;
        let (i, j, k) = (c % nx, (c / nx) % ny, c / (nx * ny));
        Vec3 {
            x: self.corner.x + (i as f64 + 0.5) * self.spacing,
            y: self.corner.y + (j as f64 + 0.5) * self.spacing,
            z: self.corner.z + (k as f64 + 0.5) * self.spacing,
        }
    }

    /// Whether the body at angle `theta` covers point `p` (millimetres): `p` lies in a round
    /// part, or `p` turned back by `theta` about the axis (Rodrigues' rotation) lies in a solid
    /// cell of the turning parts' fine grid.
    fn covers_at(&self, p: Vec3, theta: f64) -> bool {
        let solid = |f: &Grid, q: Vec3| {
            let at = |x: f64, o: f64| ((x - o) / f.spacing).floor();
            let (i, j, kk) = (at(q.x, f.origin.x), at(q.y, f.origin.y), at(q.z, f.origin.z));
            i >= 0.0 && j >= 0.0 && kk >= 0.0 && f.get(i as usize, j as usize, kk as usize) == Some(Cell::Solid)
        };
        if self.round.as_ref().is_some_and(|f| solid(f, p)) {
            return true;
        }
        let Some(f) = &self.fine else { return false };
        let v = sub(p, self.origin);
        let (s, c) = (-theta).sin_cos();
        let k = self.axis;
        let turned = add(add(scale(v, c), scale(cross(k, v), s)), scale(k, dot(k, v) * (1.0 - c)));
        solid(f, add(self.origin, turned))
    }

    /// The surface velocity at point `p` (millimetres), in lattice units (cells per step).
    pub(crate) fn velocity_at(&self, p: Vec3) -> [f64; 3] {
        let r = scale(sub(p, self.origin), 1.0 / self.spacing);
        let w = cross(scale(self.axis, self.omega * self.speed), r);
        [w.x, w.y, w.z]
    }

    /// The moment about the axis point of momentum `m` applied at `p` (millimetres; lattice
    /// lengths for the arm).
    pub(crate) fn moment(&self, p: Vec3, m: [f64; 3]) -> [f64; 3] {
        let r = scale(sub(p, self.origin), 1.0 / self.spacing);
        let t = cross(r, Vec3 { x: m[0], y: m[1], z: m[2] });
        [t.x, t.y, t.z]
    }

    /// The centre of a link's midpoint from cell `c` along lattice direction `dir` (millimetres).
    pub(crate) fn link_mid(&self, dims: [usize; 3], c: usize, dir: [i32; 3]) -> Vec3 {
        let p = self.centre(dims, c);
        let h = 0.5 * self.spacing;
        Vec3 { x: p.x + h * f64::from(dir[0]), y: p.y + h * f64::from(dir[1]), z: p.z + h * f64::from(dir[2]) }
    }

    /// Turn one step: the new angle, and for each swept cell whether it is covered now. Returns
    /// the swept cells whose coverage changed, as (cell, now covered).
    pub(crate) fn turn(&mut self, dims: [usize; 3]) -> Vec<(usize, bool)> {
        self.theta += self.omega * self.speed;
        let theta = self.theta;
        // Whether each swept cell is covered now: independent, so in blocks on every core.
        let mut now = vec![false; self.swept.len()];
        let threads = std::thread::available_parallelism().map_or(1, |p| p.get()).min(64).min(now.len() / 8192 + 1).max(1);
        let per = now.len().div_ceil(threads).max(1);
        {
            let this = &*self;
            std::thread::scope(|scope| {
                for (b, block) in now.chunks_mut(per).enumerate() {
                    scope.spawn(move || {
                        for (o, slot) in block.iter_mut().enumerate() {
                            if let Some(&cell) = this.swept.get(b * per + o) {
                                *slot = this.covers_at(this.centre(dims, cell), theta);
                            }
                        }
                    });
                }
            });
        }
        let mut changed = Vec::new();
        for (n, (&cell, now)) in self.swept.iter().zip(now).enumerate() {
            if let Some(was) = self.covered.get_mut(n)
                && *was != now
            {
                *was = now;
                changed.push((cell, now));
            }
        }
        changed
    }

    /// Cover the cells the body covers at its current angle, without stepping (the start).
    pub(crate) fn place(&mut self, dims: [usize; 3]) -> Vec<usize> {
        let theta = self.theta;
        let mut now = Vec::new();
        for (n, &cell) in self.swept.iter().enumerate() {
            let c = self.covers_at(self.centre(dims, cell), theta);
            if let Some(slot) = self.covered.get_mut(n) {
                *slot = c;
            }
            if c {
                now.push(cell);
            }
        }
        now
    }

    /// How far along the link from cell `c` along `dir` the body's surface is at its current
    /// angle, as a fraction of the link (0..1]: the link turned back into the body's frame and
    /// cast against its surface. A link whose cast misses (the voxel said covered, the surface
    /// says not quite) takes the whole link.
    pub(crate) fn wall_fraction(&self, dims: [usize; 3], c: usize, dir: [i32; 3]) -> f64 {
        let from = self.link_mid(dims, c, [0, 0, 0]);
        let d = Vec3 { x: f64::from(dir[0]), y: f64::from(dir[1]), z: f64::from(dir[2]) };
        let length = dot(d, d).sqrt() * self.spacing;
        let back = |v: Vec3| {
            let (s, co) = (-self.theta).sin_cos();
            let k = self.axis;
            add(add(scale(v, co), scale(cross(k, v), s)), scale(k, dot(k, v) * (1.0 - co)))
        };
        let o = add(self.origin, back(sub(from, self.origin)));
        let unit = back(scale(d, 1.0 / dot(d, d).sqrt()));
        let hit = |index: &MeshIndex, body: &Mesh, o: Vec3, unit: Vec3| match index.raycast(body, o, unit) {
            Some((t, _)) if t > 0.0 && t <= length => t,
            _ => f64::INFINITY,
        };
        // The turning parts cast in their own frame, the round ones where they are.
        let t = hit(&self.index, &self.body, o, unit).min(hit(&self.round_index, &self.round_body, from, scale(d, 1.0 / dot(d, d).sqrt())));
        if t.is_finite() { (t / length).max(1e-3) } else { 1.0 }
    }

    /// Whether lattice cell `c` lies in one of the round parts (which stay put as they turn).
    pub(crate) fn round_covers(&self, dims: [usize; 3], c: usize) -> bool {
        let Some(f) = &self.round else { return false };
        let q = self.centre(dims, c);
        let at = |x: f64, o: f64| ((x - o) / f.spacing).floor();
        let (i, j, k) = (at(q.x, f.origin.x), at(q.y, f.origin.y), at(q.z, f.origin.z));
        i >= 0.0 && j >= 0.0 && k >= 0.0 && f.get(i as usize, j as usize, k as usize) == Some(Cell::Solid)
    }

    /// Turn at `share` (0..1) of the full rate from the next step: a soft start.
    pub fn set_speed(&mut self, share: f64) {
        self.speed = share.clamp(0.0, 1.0);
    }

    /// The lattice spacing in millimetres.
    pub fn spacing(&self) -> f64 {
        self.spacing
    }
}

/// Whether `mesh` is a solid of revolution about the axis through `origin` along the unit `axis`
/// (a drum, a cone, a lathed hub, faceted or not): every face's normal lies in the plane through
/// the axis and the face's centre, so it has no part along the turning direction there (an
/// area-weighted 10 % allowed: a facet's triangles lean a few per cent from its middle). A
/// blade's big faces lean along the turning by a third or more, so it is not.
pub fn is_round(mesh: &Mesh, origin: Vec3, axis: Vec3) -> bool {
    let (mut off, mut area) = (0.0, 0.0);
    for tri in &mesh.triangles {
        let [a, b, c] = tri.map(|v| mesh.positions[v as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let twice = dot(n, n).sqrt();
        if twice <= 0.0 {
            continue;
        }
        let centre = scale(add(add(a, b), c), 1.0 / 3.0);
        let d = sub(centre, origin);
        let radial = sub(d, scale(axis, dot(d, axis)));
        let r = dot(radial, radial).sqrt();
        area += twice;
        if r > 1e-9 {
            off += dot(n, cross(axis, scale(radial, 1.0 / r))).abs();
        }
    }
    area > 0.0 && off <= 0.1 * area
}

#[cfg(test)]
mod tests {
    use solidcraft_geom::{Aabb3, Mesh, Vec3};

    use super::*;
    use crate::{Cell, Grid, Lattice};

    /// A closed prism of `n` sides about the z axis through (cx, cy), radius `r`, from z0 to z1.
    fn prism(cx: f64, cy: f64, r: f64, z0: f64, z1: f64, n: usize) -> Mesh {
        let mut m = Mesh::default();
        let at = |k: usize, z: f64| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            Vec3 { x: cx + r * a.cos(), y: cy + r * a.sin(), z }
        };
        for k in 0..n {
            m.positions.push(at(k, z0));
            m.positions.push(at(k, z1));
        }
        m.positions.push(Vec3 { x: cx, y: cy, z: z0 });
        m.positions.push(Vec3 { x: cx, y: cy, z: z1 });
        let (b0, b1) = ((2 * n) as u32, (2 * n + 1) as u32);
        for k in 0..n {
            let (a0, a1) = ((2 * k) as u32, (2 * k + 1) as u32);
            let (c0, c1) = ((2 * ((k + 1) % n)) as u32, (2 * ((k + 1) % n) + 1) as u32);
            m.triangles.push([a0, c0, c1]);
            m.triangles.push([a0, c1, a1]);
            m.triangles.push([b0, c0, a0]);
            m.triangles.push([b1, a1, c1]);
        }
        m
    }

    /// The mass of the fluid cells (solid cells hold whatever the double buffer left there).
    fn fluid_mass(l: &Lattice) -> f64 {
        let [nx, ny, nz] = l.dims;
        let mut m = 0.0;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    if let Some((rho, _)) = l.moments(i, j, k) {
                        m += rho;
                    }
                }
            }
        }
        m
    }

    /// A box of `dims` unit cells, fluid inside radius `r_out` of (cx, cy), solid outside.
    fn drum(dims: [usize; 3], cx: f64, cy: f64, r_out: f64) -> Grid {
        let bounds = Aabb3 { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: dims[0] as f64, y: dims[1] as f64, z: dims[2] as f64 } };
        let mut grid = Grid::around(bounds, 1.0, 0, 1_000_000).unwrap();
        for k in 0..dims[2] {
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    let p = grid.center(i, j, k);
                    if ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt() > r_out {
                        let c = grid.index(i, j, k).unwrap();
                        grid.cells[c] = Cell::Solid;
                    }
                }
            }
        }
        grid
    }

    /// Circular Couette flow: a cylinder of radius R1 turning at ω inside a fixed one of radius
    /// R2 drags the fluid between them with the exact torque per length 4π μ ω R1² R2² /
    /// (R2² − R1²) (here per cell of depth, periodic along the axis). With its bounce-back
    /// interpolated to the surface the turning cylinder's torque is within 2 % on R1 = 8 cells (−1.3 %)
    /// (the cylinder is round, so it stays put on the grid and only its wall moves).
    #[test]
    fn a_turning_cylinder_drags_the_fluid_with_the_couette_torque() {
        let (n, c, r1, r2, depth) = (52usize, 26.0, 8.0, 22.0, 3usize);
        let grid = drum([n, n, depth], c, c, r2);
        let body = prism(c, c, r1, -2.0, depth as f64 + 2.0, 96);
        let omega = 0.002;
        let rotor = Rotor::new(&body, &grid, Vec3 { x: c, y: c, z: 0.0 }, Vec3 { x: 0.0, y: 0.0, z: 1.0 }, omega, 10_000_000).unwrap();
        let tau = 0.8;
        let mut lattice = Lattice::new(&grid, tau, [0.0; 3], [false, false, true]).unwrap().with_rotor(rotor).unwrap();
        let mu = (tau - 0.5) / 3.0;
        let exact = 4.0 * std::f64::consts::PI * mu * omega * r1 * r1 * r2 * r2 / (r2 * r2 - r1 * r1) * depth as f64;
        let mass0 = fluid_mass(&lattice);
        let (steps, tail) = (12_000, 2_000);
        let mut sum = 0.0;
        for s in 0..steps {
            lattice.step();
            if s >= steps - tail {
                sum += lattice.rotors()[0].axial_torque();
            }
        }
        let torque = -sum / tail as f64;
        let error = torque / exact - 1.0;
        println!("Couette: torque {torque:.6e} vs exact {exact:.6e} ({:+.2} %)", 100.0 * error);
        assert!(error.abs() < 0.02, "torque {torque:e} vs {exact:e}");
        let mass = fluid_mass(&lattice);
        assert!((mass / mass0 - 1.0).abs() < 1e-9, "mass {mass} vs {mass0}");
    }

    /// A drum, a cone and a faceted lathed hub are round about their axis; a twisted blade and a
    /// box off the axis are not.
    #[test]
    fn round_parts_are_told_from_blades() {
        let z = Vec3 { x: 0.0, y: 0.0, z: 1.0 };
        let o = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
        assert!(is_round(&prism(0.0, 0.0, 5.0, 0.0, 10.0, 24), o, z));
        assert!(!is_round(&prism(3.0, 0.0, 1.0, 0.0, 10.0, 24), o, z));
        let mut blade = Mesh::default();
        // A thin plate leaning 30° along the turning at x = 5.
        let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
        for &(x, t) in &[(4.0, -0.2), (6.0, -0.2), (6.0, 0.2), (4.0, 0.2)] {
            for &h in &[-1.0, 1.0] {
                blade.positions.push(Vec3 { x, y: t * c - h * s, z: t * s + h * c });
            }
        }
        // The box's 12 triangles, outward (corners: 0..8 as (x, t, h) bits).
        let quads = [[0, 2, 4, 6], [1, 7, 5, 3], [0, 1, 3, 2], [4, 5, 7, 6], [0, 6, 7, 1], [2, 3, 5, 4]];
        for q in quads {
            blade.triangles.push([q[0], q[1], q[2]]);
            blade.triangles.push([q[0], q[2], q[3]]);
        }
        assert!(!is_round(&blade, o, z));
    }

    /// A flat paddle turning in a closed drum: cells are covered and uncovered every step, the
    /// fluid follows the paddle's turn, and the fluid's mass stays within a percent over a turn.
    #[test]
    fn a_paddle_stirs_the_drum_and_keeps_its_mass() {
        let (n, c, depth) = (40usize, 20.0, 3usize);
        let grid = drum([n, n, depth], c, c, 18.0);
        let mut body = Mesh::default();
        crate::voxelize::tests::add_box(&mut body, [c - 12.0, c - 1.5, -2.0], [c + 12.0, c + 1.5, depth as f64 + 2.0]);
        let omega = 0.008; // tips at 0.096 cells per step
        let rotor = Rotor::new(&body, &grid, Vec3 { x: c, y: c, z: 0.0 }, Vec3 { x: 0.0, y: 0.0, z: 1.0 }, omega, 10_000_000).unwrap();
        let mut lattice = Lattice::new(&grid, 0.7, [0.0; 3], [false, false, true]).unwrap().with_rotor(rotor).unwrap();
        let mass0 = fluid_mass(&lattice);
        let turn = (std::f64::consts::TAU / omega) as usize;
        for _ in 0..turn {
            lattice.step();
        }
        let drift = fluid_mass(&lattice) / mass0 - 1.0;
        println!("paddle: mass drift over a turn {:+.3} %", 100.0 * drift);
        assert!(drift.abs() < 0.01, "{drift}");
        // The fluid turns with the paddle: positive angular momentum about +z near the tips.
        let (_, u) = lattice.moments(20 + 15, 20, 1).unwrap();
        let (_, v) = lattice.moments(20 - 15, 20, 1).unwrap();
        assert!(u[1] > 0.0 && v[1] < 0.0, "{u:?} {v:?}");
        // The paddle is driven: the air brakes it.
        assert!(lattice.rotors()[0].axial_torque() < 0.0);
        // Tips faster than the lattice allows are refused.
        assert!(Rotor::new(&body, &grid, Vec3 { x: c, y: c, z: 0.0 }, Vec3 { x: 0.0, y: 0.0, z: 1.0 }, 0.05, 1_000_000).is_err());
    }
}
