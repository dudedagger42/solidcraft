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

use solidcraft_geom::{Mesh, Vec3};

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
    /// The body at angle 0 on a grid of half the lattice spacing.
    fine: Grid,
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
        let len = dot(axis, axis).sqrt();
        if !(len.is_finite() && len > 0.0) || !omega.is_finite() {
            return Err(SimError::BadRotor);
        }
        let axis = scale(axis, 1.0 / len);
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
        let mut fine = Grid::around(body.bounds(), 0.5 * s, 1, max_cells)?;
        voxelize(body, &mut fine)?;
        // Every lattice cell within a cell of the swept cylinder.
        let mut swept = Vec::new();
        let [nx, ny, nz] = grid.dims;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let d = sub(grid.center(i, j, k), origin);
                    let along = dot(d, axis);
                    let off = sub(d, scale(axis, along));
                    if along >= lo - s && along <= hi + s && dot(off, off).sqrt() <= radius + s {
                        if let Some(c) = grid.index(i, j, k) {
                            swept.push(c);
                        }
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
            fine,
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

    /// Whether the body at angle `theta` covers point `p` (millimetres): `p` turned back by
    /// `theta` about the axis (Rodrigues' rotation) lies in a solid cell of the fine grid.
    fn covers_at(&self, p: Vec3, theta: f64) -> bool {
        let v = sub(p, self.origin);
        let (s, c) = (-theta).sin_cos();
        let k = self.axis;
        let turned = add(add(scale(v, c), scale(cross(k, v), s)), scale(k, dot(k, v) * (1.0 - c)));
        let q = add(self.origin, turned);
        let f = &self.fine;
        let at = |x: f64, o: f64| ((x - o) / f.spacing).floor();
        let (i, j, kk) = (at(q.x, f.origin.x), at(q.y, f.origin.y), at(q.z, f.origin.z));
        if i < 0.0 || j < 0.0 || kk < 0.0 {
            return false;
        }
        f.get(i as usize, j as usize, kk as usize) == Some(Cell::Solid)
    }

    /// The surface velocity at point `p` (millimetres), in lattice units (cells per step).
    pub(crate) fn velocity_at(&self, p: Vec3) -> [f64; 3] {
        let r = scale(sub(p, self.origin), 1.0 / self.spacing);
        let w = cross(scale(self.axis, self.omega), r);
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
        self.theta += self.omega;
        let theta = self.theta;
        let mut changed = Vec::new();
        for (n, &cell) in self.swept.iter().enumerate() {
            let now = self.covers_at(self.centre(dims, cell), theta);
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

    /// The lattice spacing in millimetres.
    pub fn spacing(&self) -> f64 {
        self.spacing
    }
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
    /// (R2² − R1²) (here per cell of depth, periodic along the axis). Both walls are
    /// staircases, so the radii are only good to a fraction of a cell: within 8 % on R1 = 8.
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
        assert!(error.abs() < 0.08, "torque {torque:e} vs {exact:e}");
        let mass = fluid_mass(&lattice);
        assert!((mass / mass0 - 1.0).abs() < 1e-9, "mass {mass} vs {mass0}");
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
