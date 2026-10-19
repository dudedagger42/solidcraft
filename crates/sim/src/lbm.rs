//! The CPU reference of the Airflow solver: lattice Boltzmann D3Q19 with BGK collision, Guo body
//! forcing and half-way bounce-back walls, in lattice units (cell size 1, time step 1). It is the
//! oracle the GPU kernels are checked against, so it favours plain code and `f64` over speed.
//! Still to come: Smagorinsky viscosity, velocity inlets and pressure outlets.

use crate::{Cell, Grid, Result, SimError};

/// Lattice directions: rest, 6 faces, 12 edges.
pub const C: [[i32; 3]; 19] = [
    [0, 0, 0],
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 0],
    [-1, -1, 0],
    [1, -1, 0],
    [-1, 1, 0],
    [1, 0, 1],
    [-1, 0, -1],
    [1, 0, -1],
    [-1, 0, 1],
    [0, 1, 1],
    [0, -1, -1],
    [0, 1, -1],
    [0, -1, 1],
];
/// Each direction's opposite (the directions above come in opposite pairs after the rest one).
pub const OPP: [usize; 19] = [0, 2, 1, 4, 3, 6, 5, 8, 7, 10, 9, 12, 11, 14, 13, 16, 15, 18, 17];
pub const W: [f64; 19] = {
    let (r, f, e) = (1.0 / 3.0, 1.0 / 18.0, 1.0 / 36.0);
    [r, f, f, f, f, f, f, e, e, e, e, e, e, e, e, e, e, e, e]
};
const Q: usize = 19;

pub struct Lattice {
    pub dims: [usize; 3],
    /// Relaxation time; the kinematic viscosity is (tau − ½) / 3.
    pub tau: f64,
    /// Body force per unit volume (lattice units), e.g. a pressure gradient driving a channel.
    pub force: [f64; 3],
    /// Which axes wrap around; on the others the grid's outer faces are walls.
    pub periodic: [bool; 3],
    solid: Vec<bool>,
    f: Vec<f64>,
    next: Vec<f64>,
}

fn equilibrium(q: usize, rho: f64, u: [f64; 3]) -> f64 {
    let c = C[q];
    let cu = 3.0 * (c[0] as f64 * u[0] + c[1] as f64 * u[1] + c[2] as f64 * u[2]);
    let uu = 1.5 * (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]);
    W[q] * rho * (1.0 + cu + 0.5 * cu * cu - uu)
}

impl Lattice {
    /// Fluid at rest (density 1) on the grid's fluid cells.
    pub fn new(grid: &Grid, tau: f64, force: [f64; 3], periodic: [bool; 3]) -> Result<Lattice> {
        if !tau.is_finite() || tau <= 0.5 || force.iter().any(|g| !g.is_finite()) {
            return Err(SimError::InvalidRelaxation);
        }
        let n = grid.len();
        let len = n.checked_mul(Q).ok_or(SimError::TooManyCells { cells: n as u64, max: (usize::MAX / Q) as u64 })?;
        let solid: Vec<bool> = grid.cells.iter().map(|c| *c == Cell::Solid).collect();
        let mut f = vec![0.0; len];
        for (cell, dist) in f.chunks_mut(Q).enumerate() {
            if !solid.get(cell).copied().unwrap_or(true) {
                for (q, v) in dist.iter_mut().enumerate() {
                    *v = W[q];
                }
            }
        }
        Ok(Lattice { dims: grid.dims, tau, force, periodic, solid, next: f.clone(), f })
    }

    pub fn viscosity(&self) -> f64 {
        (self.tau - 0.5) / 3.0
    }

    fn index(&self, i: usize, j: usize, k: usize) -> Option<usize> {
        let [nx, ny, nz] = self.dims;
        (i < nx && j < ny && k < nz).then_some(i + nx * (j + ny * k))
    }

    /// The neighbour of (i, j, k) along `c`, wrapped on periodic axes; none past a wall face.
    fn neighbour(&self, at: [usize; 3], c: [i32; 3]) -> Option<usize> {
        let mut p = [0usize; 3];
        for a in 0..3 {
            let n = self.dims[a] as i64;
            let mut v = at[a] as i64 + i64::from(c[a]);
            if v < 0 || v >= n {
                if !self.periodic[a] {
                    return None;
                }
                v = v.rem_euclid(n);
            }
            p[a] = v as usize;
        }
        self.index(p[0], p[1], p[2])
    }

    /// Density and velocity of a fluid cell (the velocity includes half the force, as Guo's
    /// scheme defines it).
    pub fn moments(&self, i: usize, j: usize, k: usize) -> Option<(f64, [f64; 3])> {
        let cell = self.index(i, j, k)?;
        if self.solid.get(cell).copied().unwrap_or(true) {
            return None;
        }
        let dist = self.f.get(cell * Q..(cell + 1) * Q)?;
        Some(Self::moments_of(dist, self.force))
    }

    fn moments_of(dist: &[f64], force: [f64; 3]) -> (f64, [f64; 3]) {
        let rho: f64 = dist.iter().sum();
        let mut m = [0.0; 3];
        for (q, v) in dist.iter().enumerate() {
            for a in 0..3 {
                m[a] += v * C[q][a] as f64;
            }
        }
        let u = [(m[0] + 0.5 * force[0]) / rho, (m[1] + 0.5 * force[1]) / rho, (m[2] + 0.5 * force[2]) / rho];
        (rho, u)
    }

    /// One time step: collide every fluid cell, then stream to its neighbours; a population
    /// heading into a wall comes back the opposite way (half-way bounce-back).
    pub fn step(&mut self) {
        let [nx, ny, nz] = self.dims;
        let (tau, g) = (self.tau, self.force);
        let mut post = [0.0; Q];
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let Some(cell) = self.index(i, j, k) else { continue };
                    if self.solid.get(cell).copied().unwrap_or(true) {
                        continue;
                    }
                    let Some(dist) = self.f.get(cell * Q..(cell + 1) * Q) else { continue };
                    let (rho, u) = Self::moments_of(dist, g);
                    for q in 0..Q {
                        let c = C[q].map(f64::from);
                        let cu = c[0] * u[0] + c[1] * u[1] + c[2] * u[2];
                        let mut source = 0.0;
                        for a in 0..3 {
                            source += (3.0 * (c[a] - u[a]) + 9.0 * cu * c[a]) * g[a];
                        }
                        let fq = dist.get(q).copied().unwrap_or(0.0);
                        post[q] = fq - (fq - equilibrium(q, rho, u)) / tau + (1.0 - 0.5 / tau) * W[q] * source;
                    }
                    for (q, value) in post.iter().enumerate() {
                        let to = match self.neighbour([i, j, k], C[q]) {
                            Some(n) if !self.solid.get(n).copied().unwrap_or(true) => n * Q + q,
                            _ => cell * Q + OPP[q],
                        };
                        if let Some(slot) = self.next.get_mut(to) {
                            *slot = *value;
                        }
                    }
                }
            }
        }
        std::mem::swap(&mut self.f, &mut self.next);
    }
}

#[cfg(test)]
mod tests {
    use solidcraft_geom::{Aabb3, Vec3};

    use super::*;

    /// A channel between two plates (z walls), periodic along x and y, driven by a body force:
    /// plane Poiseuille flow, u(z) = g / (2ν) · (z − z0)(z1 − z), with the walls half a cell
    /// beyond the last fluid cells. BGK with half-way bounce-back reproduces it closely.
    #[test]
    fn plane_poiseuille_flow_matches_the_parabola() {
        let (nz, h) = (18usize, 16.0);
        let bounds = Aabb3 { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 1.0, y: 1.0, z: nz as f64 } };
        let mut grid = Grid::around(bounds, 1.0, 0, 1_000).unwrap();
        assert_eq!(grid.dims, [1, 1, nz]);
        for k in [0, nz - 1] {
            let idx = grid.index(0, 0, k).unwrap();
            grid.cells[idx] = Cell::Solid;
        }
        let g = 1e-6;
        let mut lattice = Lattice::new(&grid, 0.8, [g, 0.0, 0.0], [true, true, false]).unwrap();
        for _ in 0..20_000 {
            lattice.step();
        }
        let nu = lattice.viscosity();
        let mut worst = 0.0f64;
        for k in 1..nz - 1 {
            let z = k as f64 - 0.5; // from the lower wall, which sits at k = ½
            let exact = g / (2.0 * nu) * z * (h - z);
            let (rho, u) = lattice.moments(0, 0, k).unwrap();
            assert!((rho - 1.0).abs() < 1e-9, "mass is kept: {rho}");
            assert!(u[1].abs() < 1e-12 && u[2].abs() < 1e-12, "{u:?}");
            worst = worst.max((u[0] - exact).abs() / (g * h * h / (8.0 * nu)));
        }
        assert!(worst < 5e-3, "worst error {worst} of the peak speed");
    }

    /// The whole path: a square duct as a closed mesh, voxelized, then driven along its bore.
    /// Laminar flow in a square duct of side a carries Q = g a⁴ / (12 ν) · (1 − 192/π⁵ Σ
    /// tanh(nπ/2)/n⁵, n odd) ≈ 0.03514 g a⁴ / ν.
    #[test]
    fn square_duct_flow_rate_matches_the_series_solution() {
        let mut mesh = solidcraft_geom::Mesh::default();
        let (outer, wall, len) = (20.0, 4.0, 2.0);
        let add = crate::voxelize::tests::add_box;
        add(&mut mesh, [0.0, 0.0, 0.0], [len, outer, wall]);
        add(&mut mesh, [0.0, 0.0, outer - wall], [len, outer, outer]);
        add(&mut mesh, [0.0, 0.0, wall], [len, wall, outer - wall]);
        add(&mut mesh, [0.0, outer - wall, wall], [len, outer, outer - wall]);
        let mut grid = Grid::around(mesh.bounds(), 1.0, 0, 10_000).unwrap();
        crate::voxelize(&mesh, &mut grid).unwrap();
        let a = outer - 2.0 * wall;
        assert_eq!(grid.count(Cell::Fluid), (len * a * a) as usize);
        let g = 1e-6;
        let mut lattice = Lattice::new(&grid, 0.8, [g, 0.0, 0.0], [true, false, false]).unwrap();
        for _ in 0..12_000 {
            lattice.step();
        }
        let flow: f64 = (0..grid.dims[1])
            .flat_map(|j| (0..grid.dims[2]).map(move |k| (j, k)))
            .filter_map(|(j, k)| lattice.moments(0, j, k))
            .map(|(_, u)| u[0])
            .sum();
        let sum: f64 = [1.0f64, 3.0, 5.0, 7.0, 9.0].iter().map(|n| (n * std::f64::consts::PI / 2.0).tanh() / n.powi(5)).sum();
        let exact = g * a.powi(4) / (12.0 * lattice.viscosity()) * (1.0 - 192.0 / std::f64::consts::PI.powi(5) * sum);
        let error = (flow - exact).abs() / exact;
        assert!(error < 0.02, "flow {flow:e}, exact {exact:e}, error {error}");
    }

    #[test]
    fn unstable_relaxation_is_refused() {
        let bounds = Aabb3 { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 2.0, y: 2.0, z: 2.0 } };
        let grid = Grid::around(bounds, 1.0, 0, 1_000).unwrap();
        for tau in [0.5, 0.1, f64::NAN] {
            assert!(Lattice::new(&grid, tau, [0.0; 3], [true; 3]).is_err(), "{tau}");
        }
        assert!(Lattice::new(&grid, 0.8, [f64::INFINITY, 0.0, 0.0], [true; 3]).is_err());
    }
}
