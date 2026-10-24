//! The CPU reference of the Airflow solver: lattice Boltzmann D3Q19 with BGK collision, Guo body
//! forcing and half-way bounce-back walls, in lattice units (cell size 1, time step 1). It is the
//! oracle the GPU kernels are checked against, so it favours plain code and `f64` over speed.
//! Openings (a velocity inlet, a pressure outlet) use Guo's non-equilibrium extrapolation from
//! the cell just inside. Turbulence: the Smagorinsky model raises each cell's relaxation time with
//! its local shear, read from the non-equilibrium momentum flux (Hou et al. 1996). Walls are
//! where the voxels say, half a cell past the last fluid cell, unless their true place is given
//! ([`WallLink`]): then the bounce-back is interpolated to it (Bouzidi, Firdaouss & Lallemand
//! 2001), so a curved or slanted wall is smooth rather than a staircase.

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

#[derive(Clone)]
pub struct Lattice {
    pub dims: [usize; 3],
    /// Relaxation time; the kinematic viscosity is (tau − ½) / 3.
    pub tau: f64,
    /// Body force per unit volume (lattice units), e.g. a pressure gradient driving a channel.
    pub force: [f64; 3],
    /// Which axes wrap around; on the others the grid's outer faces are walls.
    pub periodic: [bool; 3],
    /// The Smagorinsky constant (0: laminar BGK; about 0.1–0.17 for turbulent flow).
    smagorinsky: f64,
    /// Regularized collision: relax only the non-equilibrium part's stress (its second-order
    /// Hermite projection), which keeps the lattice stable at relaxation times close to ½.
    regularized: bool,
    /// A sponge along the walled grid faces: (width in cells, relaxation time at the face). The
    /// air grows more viscous toward the faces, so waves and eddies die out before they reach the
    /// open boundary there.
    sponge: Option<(usize, f64)>,
    /// With the regularized collision: the relaxation time of the stress's isotropic part (bulk
    /// viscosity). Above the shear one it damps sound waves, which at a low viscosity otherwise
    /// ring for a long time, and leaves the incompressible flow alone.
    bulk: Option<f64>,
    /// Per cell: how much of the Smagorinsky length scale applies there, 0..65535 for 0..1 (the
    /// wall damping, Van Driest's 1 − exp(−y⁺/26)); none: all of it everywhere. Stored as the GPU
    /// reads it, so both apply the same factor.
    damping: Option<Vec<u16>>,
    solid: Vec<bool>,
    f: Vec<f64>,
    next: Vec<f64>,
    /// Opening cells: (cell, the fluid cell just inside it, what is imposed there).
    openings: Vec<(usize, usize, Opening)>,
    /// Links from fluid cells into walls whose true place is known (see [`WallLink`]).
    walls: Vec<WallLink>,
    /// Bodies turning through the lattice ([`crate::rotor`]), and per cell which one covers it
    /// (its index + 1; 0 for none).
    rotors: Vec<crate::Rotor>,
    rotor_of: Vec<u32>,
    /// Each cell's density in the step under way (for the moving walls' term).
    rho_scratch: Vec<f64>,
}

/// A link from fluid cell `cell` along direction `q` into a solid cell, and where along it the
/// wall really is: `frac` of the way to the solid cell's centre (0 < frac ≤ 1; ½ is where
/// half-way bounce-back puts it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallLink {
    pub cell: usize,
    pub q: usize,
    pub frac: f64,
}

/// What an opening imposes on the flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Opening {
    /// A velocity inlet (lattice units).
    Velocity([f64; 3]),
    /// A pressure outlet, as a density (pressure = density / 3).
    Density(f64),
    /// A velocity inlet that lets sound out: the speed along the inflow gives way to the
    /// density's departure from `density` (the inlet's mean), as an acoustic wave leaving
    /// upstream carries it, u = velocity (1 − c (ρ − density) / (ρ |velocity|)). A plain velocity
    /// inlet reflects every pressure wave, so a duct between it and an open end rings like a pipe.
    Inflow { velocity: [f64; 3], density: f64 },
}

/// The lattice's speed of sound, 1/√3.
pub const SOUND: f64 = 0.577_350_269_189_625_8;

impl Opening {
    fn valid(&self) -> bool {
        let speed_ok = |u: &[f64; 3]| u.iter().all(|v| v.is_finite() && v.abs() < 0.3);
        match self {
            Opening::Velocity(u) => speed_ok(u),
            Opening::Density(rho) => rho.is_finite() && *rho > 0.0,
            Opening::Inflow { velocity, density } => speed_ok(velocity) && density.is_finite() && *density > 0.0,
        }
    }
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
        Ok(Lattice {
            dims: grid.dims,
            tau,
            force,
            periodic,
            solid,
            next: f.clone(),
            f,
            openings: Vec::new(),
            smagorinsky: 0.0,
            regularized: false,
            sponge: None,
            bulk: None,
            damping: None,
            walls: Vec::new(),
            rotor_of: Vec::new(),
            rotors: Vec::new(),
            rho_scratch: Vec::new(),
        })
    }

    /// Turn `rotor` through the lattice (see [`crate::rotor`]): the cells it covers at its start
    /// angle stop being fluid. A static wall stays a static wall where the body overlaps it.
    pub fn with_rotor(mut self, mut rotor: crate::Rotor) -> Result<Lattice> {
        let n = self.solid.len();
        if self.rotor_of.is_empty() {
            self.rotor_of = vec![0; n];
        }
        let id = self.rotors.len() as u32 + 1;
        for cell in rotor.place(self.dims) {
            if !self.solid.get(cell).copied().unwrap_or(true) {
                self.solid[cell] = true;
                self.rotor_of[cell] = id;
                self.f.get_mut(cell * Q..(cell + 1) * Q).into_iter().flatten().for_each(|v| *v = 0.0);
            }
        }
        self.rotors.push(rotor);
        Ok(self)
    }

    /// The rotors, with the force and torque the air gave each in the last step.
    pub fn rotors(&self) -> &[crate::Rotor] {
        &self.rotors
    }

    /// The rotors, to change their speed.
    pub fn rotors_mut(&mut self) -> &mut [crate::Rotor] {
        &mut self.rotors
    }

    /// After the streaming, the links into turning bodies (rotor, cell, direction q into the body,
    /// the population f*_q sent, Ladd's term): the bounced population is interpolated to where
    /// the body's surface crosses the link now (Bouzidi et al. 2001, with the moving wall's term
    /// weighted as Lallemand & Luo 2003 give it: whole below half way, 1/(2·frac) beyond), as the
    /// static walls are. Slot opp(q) of the cell holds the half-way value, slot q holds f*_q of
    /// the cell upstream, slot opp(q) of that cell f*_opp(q) of this one. Each link gives its body
    /// the momentum c_q (f_out + f_back) and its moment. Returns the mass each body's links added
    /// to the fluid.
    fn finish_rotor_links(&mut self, links: &[(usize, usize, usize, f64, f64)]) -> Vec<f64> {
        let mut added = vec![0.0; self.rotors.len()];
        let mut local: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
        let mut push = vec![([0.0f64; 3], [0.0f64; 3]); self.rotors.len()];
        // Where the surface crosses each link: a ray cast each, independent, so on every core.
        let mut fracs = vec![0.5; links.len()];
        {
            let (this, links) = (&*self, links);
            let threads = std::thread::available_parallelism().map_or(1, |p| p.get()).min(64).min(links.len() / 256 + 1).max(1);
            let per = links.len().div_ceil(threads).max(1);
            std::thread::scope(|scope| {
                for (b, block) in fracs.chunks_mut(per).enumerate() {
                    scope.spawn(move || {
                        for (o, slot) in block.iter_mut().enumerate() {
                            if let Some(&(r, cell, q, _, _)) = links.get(b * per + o)
                                && let Some(rotor) = this.rotors.get(r)
                                && rotor.curved
                            {
                                *slot = rotor.wall_fraction(this.dims, cell, C[q]);
                            }
                        }
                    });
                }
            });
        }
        for (&(r, cell, q, out, ladd), &frac) in links.iter().zip(&fracs) {
            let Some(rotor) = self.rotors.get(r) else { continue };
            let opp = OPP[q];
            let up = self.coords(cell).and_then(|at| self.neighbour(at, C[opp])).filter(|u| !self.solid.get(*u).copied().unwrap_or(true));
            // The bounced population, and the part of it the moving wall adds (Ladd's term, as the
            // interpolation weights it).
            let (back, moving) = match up {
                Some(_) if frac < 0.5 => {
                    let behind = self.f.get(cell * Q + q).copied().unwrap_or(out);
                    (2.0 * frac * out + (1.0 - 2.0 * frac) * behind - ladd, -ladd)
                }
                Some(up) => {
                    let own = self.f.get(up * Q + opp).copied().unwrap_or(out);
                    (out / (2.0 * frac) + (1.0 - 1.0 / (2.0 * frac)) * own - ladd / (2.0 * frac), -ladd / (2.0 * frac))
                }
                None => (out - ladd, -ladd),
            };
            if let Some(slot) = self.f.get_mut(cell * Q + opp) {
                *slot = back;
            }
            let mid = rotor.link_mid(self.dims, cell, C[q]);
            let m = C[q].map(|x| f64::from(x) * (out + back));
            let t = rotor.moment(mid, m);
            if let Some((f, tq)) = push.get_mut(r) {
                for a in 0..3 {
                    f[a] += m[a];
                    tq[a] += t[a];
                }
            }
            // The mass the moving wall's term added to this cell, given back in the cell itself.
            *local.entry(cell).or_insert(0.0) += moving;
            // What the interpolation itself added (it mixes populations of different cells), given
            // back over the swept fluid, as before.
            if let Some(a) = added.get_mut(r) {
                *a += back - out - moving;
            }
        }
        // Each cell's moving-wall mass is taken back from that cell as density at rest (w_q δρ: no
        // momentum), at once. Spread over the whole swept region instead, the cells in the pockets
        // of a staircase sliding along itself (a hub turning in place) kept gaining it, and with
        // the air's viscosity near the lattice's limit nothing evened it out: they ran away.
        for (cell, d) in local {
            for (q, w) in W.iter().enumerate() {
                if let Some(x) = self.f.get_mut(cell * Q + q) {
                    *x -= w * d;
                }
            }
        }
        for (rotor, (f, t)) in self.rotors.iter_mut().zip(push) {
            for a in 0..3 {
                rotor.force[a] += f[a];
                rotor.torque[a] += t[a];
            }
        }
        added
    }

    /// Turn every rotor one step: a cell a body comes to cover gives it its fluid's momentum and
    /// stops being fluid; a cell it uncovers is refilled with the equilibrium at the body's
    /// surface velocity and its fluid neighbours' mean density, which the body pays for.
    fn turn_rotors(&mut self) {
        let mut rotors = std::mem::take(&mut self.rotors);
        for (r, rotor) in rotors.iter_mut().enumerate() {
            let id = r as u32 + 1;
            rotor.force = [0.0; 3];
            rotor.torque = [0.0; 3];
            // The mass the body took in (covered cells) less the mass refilled (uncovered ones).
            let mut taken = 0.0;
            for (cell, now) in rotor.turn(self.dims) {
                let owner = self.rotor_of.get(cell).copied().unwrap_or(0);
                let solid = self.solid.get(cell).copied().unwrap_or(true);
                let centre = rotor.link_mid(self.dims, cell, [0, 0, 0]);
                if now && !solid {
                    let Some(dist) = self.f.get(cell * Q..(cell + 1) * Q) else { continue };
                    let mut m = [0.0; 3];
                    for (q, v) in dist.iter().enumerate() {
                        for a in 0..3 {
                            m[a] += v * f64::from(C[q][a]);
                        }
                    }
                    let t = rotor.moment(centre, m);
                    for a in 0..3 {
                        rotor.force[a] += m[a];
                        rotor.torque[a] += t[a];
                    }
                    taken += dist.iter().sum::<f64>();
                    self.f.get_mut(cell * Q..(cell + 1) * Q).into_iter().flatten().for_each(|v| *v = 0.0);
                    self.solid[cell] = true;
                    self.rotor_of[cell] = id;
                } else if !now && owner == id {
                    let Some(at) = self.coords(cell) else { continue };
                    let (mut sum, mut count) = (0.0, 0usize);
                    for c in C.iter().skip(1) {
                        if let Some(nb) = self.neighbour(at, *c)
                            && !self.solid.get(nb).copied().unwrap_or(true)
                        {
                            sum += self.f.get(nb * Q..(nb + 1) * Q).map_or(0.0, |d| d.iter().sum::<f64>());
                            count += 1;
                        }
                    }
                    let rho = if count > 0 { sum / count as f64 } else { 1.0 };
                    let u = rotor.velocity_at(centre);
                    self.solid[cell] = false;
                    self.rotor_of[cell] = 0;
                    for q in 0..Q {
                        let e = equilibrium(q, rho, u);
                        if let Some(slot) = self.f.get_mut(cell * Q + q) {
                            *slot = e;
                        }
                    }
                    let m = u.map(|v| rho * v);
                    let t = rotor.moment(centre, m);
                    for a in 0..3 {
                        rotor.force[a] -= m[a];
                        rotor.torque[a] -= t[a];
                    }
                    taken -= rho;
                }
            }
            // Give the difference back to the swept fluid as density at rest (w_q δρ: no
            // momentum), so covering and refilling keep the fluid's mass.
            if taken != 0.0 {
                let fluid: Vec<usize> = rotor.swept.iter().copied().filter(|c| !self.solid.get(*c).copied().unwrap_or(true)).collect();
                if !fluid.is_empty() {
                    let share = taken / fluid.len() as f64;
                    for c in fluid {
                        for (q, w) in W.iter().enumerate() {
                            if let Some(v) = self.f.get_mut(c * Q + q) {
                                *v += w * share;
                            }
                        }
                    }
                }
            }
        }
        self.rotors = rotors;
    }

    /// Put the walls where `links` say (see [`WallLink`]). Each must lead from a fluid cell along
    /// a direction other than rest into a solid cell, with a fraction in (0, 1].
    pub fn with_walls(mut self, links: Vec<WallLink>) -> Result<Lattice> {
        for l in &links {
            let solid = |c: usize| self.solid.get(c).copied().unwrap_or(true);
            let at = self.coords(l.cell);
            let ok = !solid(l.cell)
                && (1..Q).contains(&l.q)
                && l.frac.is_finite()
                && l.frac > 0.0
                && l.frac <= 1.0
                && at.is_some_and(|a| self.neighbour(a, C[l.q]).is_some_and(solid));
            if !ok {
                return Err(SimError::BadOpening);
            }
        }
        self.walls = links;
        Ok(self)
    }

    /// The fluid cell a wall link's interpolation reads from: one step back from the wall, if
    /// that is fluid (else the link stays half-way).
    pub fn upstream(&self, link: &WallLink) -> Option<usize> {
        let at = self.coords(link.cell)?;
        self.neighbour(at, C[OPP.get(link.q).copied()?]).filter(|u| !self.solid.get(*u).copied().unwrap_or(true))
    }

    /// The wall links (see [`Lattice::with_walls`]).
    pub fn walls(&self) -> &[WallLink] {
        &self.walls
    }

    fn coords(&self, cell: usize) -> Option<[usize; 3]> {
        let [nx, ny, nz] = self.dims;
        (cell < nx * ny * nz).then(|| [cell % nx, (cell / nx) % ny, cell / (nx * ny)])
    }

    /// After streaming: each wall link's bounced population, interpolated to where the wall is.
    /// Half-way bounce-back left f*_q(x) in slot opp(q) of x. With the wall nearer than half way
    /// (frac < ½) it is mixed with f*_q of the cell upstream, which streamed into slot q of x;
    /// further (frac > ½), with f*_opp(q) of x, which streamed into slot opp(q) of the cell
    /// upstream. With no fluid upstream it stays half-way. Every slot read here is one no link
    /// writes, so the order does not matter (the GPU runs them at once).
    fn apply_walls(&mut self) {
        for l in &self.walls {
            let opp = OPP[l.q];
            let Some(up) = self.upstream(l) else { continue };
            let a = self.f.get(l.cell * Q + opp).copied().unwrap_or(0.0);
            let value = if l.frac < 0.5 {
                let b = self.f.get(l.cell * Q + l.q).copied().unwrap_or(a);
                2.0 * l.frac * a + (1.0 - 2.0 * l.frac) * b
            } else {
                let c = self.f.get(up * Q + opp).copied().unwrap_or(a);
                a / (2.0 * l.frac) + (1.0 - 1.0 / (2.0 * l.frac)) * c
            };
            if let Some(slot) = self.f.get_mut(l.cell * Q + opp) {
                *slot = value;
            }
        }
    }

    /// Model turbulence with the Smagorinsky constant `cs` (0 turns it off).
    pub fn with_smagorinsky(mut self, cs: f64) -> Result<Lattice> {
        if !cs.is_finite() || !(0.0..=0.5).contains(&cs) {
            return Err(SimError::InvalidRelaxation);
        }
        self.smagorinsky = cs;
        Ok(self)
    }

    /// Relax only the non-equilibrium stress (Latt & Chopard 2006): the populations' higher-order
    /// non-equilibrium content, which grows unstable when the viscosity is low, is dropped.
    pub fn regularized(mut self) -> Lattice {
        self.regularized = true;
        self
    }

    pub fn is_regularized(&self) -> bool {
        self.regularized
    }

    /// Damp the flow over the last `width` cells before each non-periodic grid face, the
    /// relaxation time rising (quadratically) to `tau` at the face.
    pub fn with_sponge(mut self, width: usize, tau: f64) -> Result<Lattice> {
        if width == 0 || !tau.is_finite() || tau < self.tau || tau > 2.0 {
            return Err(SimError::InvalidRelaxation);
        }
        self.sponge = Some((width, tau));
        Ok(self)
    }

    /// Relax the stress's isotropic part with `tau` (regularized collision only).
    pub fn with_bulk(mut self, tau: f64) -> Result<Lattice> {
        if !tau.is_finite() || tau <= 0.5 || tau > 2.0 {
            return Err(SimError::InvalidRelaxation);
        }
        self.bulk = Some(tau);
        Ok(self)
    }

    pub fn bulk(&self) -> Option<f64> {
        self.bulk
    }

    /// Scale the Smagorinsky length scale cell by cell (`damping`, one per cell, 65535 = 1): near
    /// a wall the model's eddy viscosity would swamp the wall's own shear.
    pub fn with_damping(mut self, damping: Vec<u16>) -> Result<Lattice> {
        if damping.len() != self.solid.len() {
            return Err(SimError::BadPopulations);
        }
        self.damping = Some(damping);
        Ok(self)
    }

    pub fn damping(&self) -> Option<&[u16]> {
        self.damping.as_deref()
    }

    pub fn sponge(&self) -> Option<(usize, f64)> {
        self.sponge
    }

    /// The molecular relaxation time at (i, j, k): `tau`, raised in the sponge.
    fn base_tau(&self, at: [usize; 3]) -> f64 {
        let Some((width, face)) = self.sponge else { return self.tau };
        let mut d = usize::MAX;
        for ((v, n), periodic) in at.iter().zip(self.dims).zip(self.periodic) {
            if !periodic {
                d = d.min(*v).min(n.saturating_sub(1 + v));
            }
        }
        if d >= width {
            return self.tau;
        }
        let t = (width - d) as f64 / width as f64;
        self.tau + (face - self.tau) * t * t
    }

    /// The non-equilibrium momentum flux Π = Σ c c (f − feq), as xx, yy, zz, xy, xz, yz.
    fn stress(dist: &[f64], feq: &[f64; Q]) -> [f64; 6] {
        let mut pi = [0.0f64; 6];
        for q in 0..Q {
            let neq = dist.get(q).copied().unwrap_or(0.0) - feq[q];
            let c = C[q].map(f64::from);
            pi[0] += c[0] * c[0] * neq;
            pi[1] += c[1] * c[1] * neq;
            pi[2] += c[2] * c[2] * neq;
            pi[3] += c[0] * c[1] * neq;
            pi[4] += c[0] * c[2] * neq;
            pi[5] += c[1] * c[2] * neq;
        }
        pi
    }

    /// The relaxation time of a cell with density `rho` whose populations are `dist` around the
    /// equilibrium `feq`: the molecular one plus the Smagorinsky eddy viscosity's share.
    fn local_tau(&self, cell: usize, base: f64, dist: &[f64], feq: &[f64; Q], rho: f64) -> f64 {
        if self.smagorinsky == 0.0 {
            return base;
        }
        let g = self.damping.as_ref().and_then(|d| d.get(cell)).map_or(1.0, |g| f64::from(f32::from(*g) / 65535.0));
        let mut pi = [[0.0f64; 3]; 3];
        for q in 0..Q {
            let neq = dist.get(q).copied().unwrap_or(0.0) - feq[q];
            for (a, row) in pi.iter_mut().enumerate() {
                for (b, v) in row.iter_mut().enumerate() {
                    *v += f64::from(C[q][a] * C[q][b]) * neq;
                }
            }
        }
        let norm = pi.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
        let cs2 = self.smagorinsky * self.smagorinsky * g * g;
        0.5 * (base + (base * base + 18.0 * std::f64::consts::SQRT_2 * cs2 * norm / rho).sqrt())
    }

    /// Make fluid `cells` an opening; `inward` points from each of them to the fluid cell just
    /// inside, which must be fluid too.
    pub fn add_opening(&mut self, cells: &[[usize; 3]], inward: [i32; 3], kind: Opening) -> Result<()> {
        if !kind.valid() {
            return Err(SimError::BadOpening);
        }
        for at in cells {
            let cell = self.index(at[0], at[1], at[2]).ok_or(SimError::BadOpening)?;
            let inside = self.neighbour(*at, inward).ok_or(SimError::BadOpening)?;
            if self.solid.get(cell).copied().unwrap_or(true) || self.solid.get(inside).copied().unwrap_or(true) {
                return Err(SimError::BadOpening);
            }
            self.openings.push((cell, inside, kind));
        }
        Ok(())
    }

    /// Set fluid cell `cell` to the equilibrium of density `rho` and velocity `u` (lattice units):
    /// a warm start from an earlier flow. Solid cells and values out of reason are left alone.
    pub fn set_equilibrium(&mut self, cell: usize, rho: f64, u: [f64; 3]) {
        if self.solid.get(cell).copied().unwrap_or(true)
            || !(rho.is_finite() && rho > 0.5 && rho < 2.0)
            || u.iter().any(|v| !v.is_finite() || v.abs() > 0.3)
        {
            return;
        }
        if let Some(dist) = self.f.get_mut(cell * Q..(cell + 1) * Q) {
            for (q, v) in dist.iter_mut().enumerate() {
                *v = equilibrium(q, rho, u);
            }
        }
    }

    /// Impose `kind` on the openings `range` (indices into [`Lattice::openings`]), for example a
    /// fan inlet's new speed.
    pub fn set_openings(&mut self, range: std::ops::Range<usize>, kind: Opening) -> Result<()> {
        let slots = self.openings.get_mut(range).ok_or(SimError::BadOpening)?;
        if !kind.valid() {
            return Err(SimError::BadOpening);
        }
        for o in slots {
            o.2 = kind;
        }
        Ok(())
    }

    /// Rewrite the opening cells: the imposed density or velocity, the other moment and the
    /// non-equilibrium part taken from the cell just inside.
    fn apply_openings(&mut self) {
        for &(cell, inside, kind) in &self.openings {
            let Some(src) = self.f.get(inside * Q..(inside + 1) * Q) else { continue };
            let (rho_in, u_in) = Self::moments_of(src, self.force);
            let (rho, u) = match kind {
                Opening::Velocity(u) => (rho_in, u),
                Opening::Density(rho) => (rho, u_in),
                Opening::Inflow { velocity, density } => {
                    let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2]).sqrt();
                    let ease = if speed > 0.0 { SOUND * (rho_in - density) / rho_in / speed } else { 0.0 };
                    (rho_in, velocity.map(|v| v - v * ease))
                }
            };
            let mut out = [0.0; Q];
            for (q, v) in out.iter_mut().enumerate() {
                let fq = src.get(q).copied().unwrap_or(0.0);
                *v = equilibrium(q, rho, u) + fq - equilibrium(q, rho_in, u_in);
            }
            if let Some(dst) = self.f.get_mut(cell * Q..(cell + 1) * Q) {
                dst.copy_from_slice(&out);
            }
        }
    }

    pub fn viscosity(&self) -> f64 {
        (self.tau - 0.5) / 3.0
    }

    /// The Smagorinsky constant (0: laminar).
    pub fn smagorinsky(&self) -> f64 {
        self.smagorinsky
    }

    /// Which cells are solid, in grid order.
    pub fn solid(&self) -> &[bool] {
        &self.solid
    }

    /// The opening cells: (cell, the fluid cell just inside it, what is imposed there).
    pub fn openings(&self) -> &[(usize, usize, Opening)] {
        &self.openings
    }

    /// Every cell's populations, cell after cell: direction `q` of cell `c` is at `c * 19 + q`
    /// (solid cells hold zeros).
    pub fn populations(&self) -> &[f64] {
        &self.f
    }

    /// Replace the populations, for example with a GPU run's: as many finite values as
    /// [`Lattice::populations`] has. Solid cells keep their zeros.
    pub fn set_populations(&mut self, f: &[f64]) -> Result<()> {
        if f.len() != self.f.len() || f.iter().any(|v| !v.is_finite()) {
            return Err(SimError::BadPopulations);
        }
        for ((dst, src), solid) in self.f.chunks_mut(Q).zip(f.chunks(Q)).zip(&self.solid) {
            if !solid {
                dst.copy_from_slice(src);
            }
        }
        Ok(())
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

    /// Collide fluid cell `cell` (BGK or regularized, Guo forcing): its post-collision populations
    /// into `out`, its density into `rho`. Solid cells are left as they are.
    fn collide_cell(&self, cell: usize, out: &mut [f64], rho_out: &mut f64) {
        if self.solid.get(cell).copied().unwrap_or(true) {
            return;
        }
        let Some(at) = self.coords(cell) else { return };
        let Some(dist) = self.f.get(cell * Q..(cell + 1) * Q) else { return };
        let g = self.force;
        let (rho, u) = Self::moments_of(dist, g);
        *rho_out = rho;
        let mut feq = [0.0; Q];
        for (q, e) in feq.iter_mut().enumerate() {
            *e = equilibrium(q, rho, u);
        }
        let tau = self.local_tau(cell, self.base_tau(at), dist, &feq, rho);
        let pi = if self.regularized { Self::stress(dist, &feq) } else { [0.0; 6] };
        for (q, slot) in out.iter_mut().enumerate().take(Q) {
            let c = C[q].map(f64::from);
            let cu = c[0] * u[0] + c[1] * u[1] + c[2] * u[2];
            let mut source = 0.0;
            for a in 0..3 {
                source += (3.0 * (c[a] - u[a]) + 9.0 * cu * c[a]) * g[a];
            }
            let fq = dist.get(q).copied().unwrap_or(0.0);
            let relaxed = if self.regularized {
                // w (9/2) (c c − I/3) : Π, split into the deviatoric part (shear) and the
                // isotropic part (bulk): w (9/2) (c·Π·c − c² tr/3) and w (9/2) (c² − 1) tr/3.
                let ccpi = c[0] * c[0] * pi[0]
                    + c[1] * c[1] * pi[1]
                    + c[2] * c[2] * pi[2]
                    + 2.0 * (c[0] * c[1] * pi[3] + c[0] * c[2] * pi[4] + c[1] * c[2] * pi[5]);
                let (trace, c2) = ((pi[0] + pi[1] + pi[2]) / 3.0, c[0] * c[0] + c[1] * c[1] + c[2] * c[2]);
                let (shear, iso) = (W[q] * 4.5 * (ccpi - c2 * trace), W[q] * 4.5 * (c2 - 1.0) * trace);
                (1.0 - 1.0 / tau) * shear + (1.0 - 1.0 / self.bulk.unwrap_or(tau)) * iso
            } else {
                (1.0 - 1.0 / tau) * (fq - feq[q])
            };
            *slot = feq[q] + relaxed + (1.0 - 0.5 / tau) * W[q] * source;
        }
    }

    /// Stream into fluid cell `cell`'s slots from the post-collision populations `post` (densities
    /// `rho`): see [`Lattice::step`]. Links into a turning body are pushed onto `links`.
    fn stream_cell(&self, cell: usize, slots: &mut [f64], post: &[f64], rho: &[f64], links: &mut Vec<(usize, usize, usize, f64, f64)>) {
        if self.solid.get(cell).copied().unwrap_or(true) {
            return;
        }
        let Some(at) = self.coords(cell) else { return };
        for (q, slot) in slots.iter_mut().enumerate().take(Q) {
            let from = self.neighbour(at, C[OPP[q]]);
            match from {
                Some(s) if !self.solid.get(s).copied().unwrap_or(true) => {
                    *slot = post.get(s * Q + q).copied().unwrap_or(0.0);
                }
                _ => {
                    // The population that went from here into the wall, along q' = opp(q).
                    let out_dir = OPP[q];
                    let out = post.get(cell * Q + out_dir).copied().unwrap_or(0.0);
                    let mut v = out;
                    let owner = from.and_then(|s| self.rotor_of.get(s).copied()).unwrap_or(0);
                    if owner > 0
                        && let Some(rotor) = self.rotors.get(owner as usize - 1)
                    {
                        let mid = rotor.link_mid(self.dims, cell, C[out_dir]);
                        let u = rotor.velocity_at(mid);
                        let c = C[out_dir].map(f64::from);
                        let ladd = 6.0 * W[out_dir] * rho.get(cell).copied().unwrap_or(1.0) * (c[0] * u[0] + c[1] * u[1] + c[2] * u[2]);
                        v -= ladd;
                        links.push((owner as usize - 1, cell, out_dir, out, ladd));
                    }
                    *slot = v;
                }
            }
        }
    }

    /// The least and greatest density over the fluid cells and the fastest speed (lattice units);
    /// a NaN anywhere makes the range NaN.
    /// How many of a cell's neighbours a rotor covers now.
    pub fn rotor_neighbours(&self, cell: usize) -> usize {
        let Some(at) = self.coords(cell) else { return 0 };
        C.iter().skip(1).filter(|c| self.neighbour(at, **c).is_some_and(|n| self.rotor_of.get(n).copied().unwrap_or(0) > 0)).count()
    }

    /// The cells with the least and the greatest density (for finding where a run goes wrong).
    pub fn extreme_cells(&self) -> (usize, usize) {
        let (mut lo, mut hi) = ((f64::INFINITY, 0usize), (f64::NEG_INFINITY, 0usize));
        for (cell, solid) in self.solid.iter().enumerate() {
            if *solid {
                continue;
            }
            let Some(dist) = self.f.get(cell * Q..(cell + 1) * Q) else { continue };
            let rho: f64 = dist.iter().sum();
            if rho < lo.0 {
                lo = (rho, cell);
            }
            if rho > hi.0 {
                hi = (rho, cell);
            }
        }
        (lo.1, hi.1)
    }

    pub fn extremes(&self) -> (f64, f64, f64) {
        let (mut lo, mut hi, mut fast) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
        for (cell, solid) in self.solid.iter().enumerate() {
            if *solid {
                continue;
            }
            let Some(dist) = self.f.get(cell * Q..(cell + 1) * Q) else { continue };
            let (rho, u) = Self::moments_of(dist, self.force);
            if !rho.is_finite() {
                return (f64::NAN, f64::NAN, f64::NAN);
            }
            lo = lo.min(rho);
            hi = hi.max(rho);
            fast = fast.max((u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt());
        }
        (lo, hi, fast)
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

    /// One time step: collide every fluid cell, then stream to its neighbours (each cell pulling
    /// from the cells upstream); a population heading into a wall comes back the opposite way
    /// (half-way bounce-back).
    pub fn step(&mut self) {
        let profile = std::env::var_os("SIM_PROFILE").is_some();
        let t0 = std::time::Instant::now();
        if !self.rotors.is_empty() {
            self.turn_rotors();
        }
        let t_turn = t0.elapsed();
        let n = self.solid.len();
        // Every cell's update reads only what the step before left and writes only its own slots,
        // so blocks of cells run on separate threads and the result doesn't depend on how many.
        let threads = std::thread::available_parallelism().map_or(1, |p| p.get()).min(64).min(n / 16_384 + 1).max(1);
        let per = n.div_ceil(threads).max(1);
        // Collide: each fluid cell's post-collision populations into its own slots of `next`.
        let mut post = std::mem::take(&mut self.next);
        let mut rho = std::mem::take(&mut self.rho_scratch);
        rho.resize(n, 1.0);
        {
            let this = &*self;
            std::thread::scope(|scope| {
                for (b, (posts, rhos)) in post.chunks_mut(per * Q).zip(rho.chunks_mut(per)).enumerate() {
                    scope.spawn(move || {
                        for (o, (out, r)) in posts.chunks_mut(Q).zip(rhos.iter_mut()).enumerate() {
                            this.collide_cell(b * per + o, out, r);
                        }
                    });
                }
            });
        }
        // Stream: each fluid cell pulls direction q from the cell at −c_q, or, from a wall there,
        // its own population that went into it (half-way bounce-back). Links into a turning body
        // take Ladd's moving-wall term at the link's midpoint for now and are finished after the
        // streaming (see `finish_rotor_links`), in cell order.
        let mut f = std::mem::take(&mut self.f);
        let mut rotor_links: Vec<(usize, usize, usize, f64, f64)> = Vec::new();
        {
            let (this, post, rho) = (&*self, &post, &rho);
            let found: Vec<Vec<(usize, usize, usize, f64, f64)>> = std::thread::scope(|scope| {
                let handles: Vec<_> = f
                    .chunks_mut(per * Q)
                    .enumerate()
                    .map(|(b, dst)| {
                        scope.spawn(move || {
                            let mut links = Vec::new();
                            for (o, slots) in dst.chunks_mut(Q).enumerate() {
                                this.stream_cell(b * per + o, slots, post, rho, &mut links);
                            }
                            links
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
            });
            for links in found {
                rotor_links.extend(links);
            }
        }
        self.f = f;
        self.next = post;
        self.rho_scratch = rho;
        let t_stream = t0.elapsed();
        let added = self.finish_rotor_links(&rotor_links);
        let t_links = t0.elapsed();
        let mut rotors = std::mem::take(&mut self.rotors);
        for (rotor, added) in rotors.iter_mut().zip(added) {
            // What the bounce-back added to the fluid (Ladd's term adds mass on the side the blade
            // pushes, which is denser, more than it takes on the side it leaves): taken back from
            // the swept fluid as density at rest, so the fluid's mass holds.
            if added != 0.0 {
                let fluid: Vec<usize> = rotor.swept.iter().copied().filter(|c| !self.solid.get(*c).copied().unwrap_or(true)).collect();
                if !fluid.is_empty() {
                    let share = added / fluid.len() as f64;
                    for c in fluid {
                        for (q, w) in W.iter().enumerate() {
                            if let Some(x) = self.f.get_mut(c * Q + q) {
                                *x -= w * share;
                            }
                        }
                    }
                }
            }
        }
        self.rotors = rotors;
        let t_mass = t0.elapsed();
        self.apply_walls();
        self.apply_openings();
        if profile {
            eprintln!(
                "step: turn {:?}, collide+stream {:?}, rotor links ({}) {:?}, mass {:?}, walls+openings {:?}",
                t_turn,
                t_stream - t_turn,
                rotor_links.len(),
                t_links - t_stream,
                t_mass - t_links,
                t0.elapsed() - t_mass
            );
        }
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

    /// The same channel with its plates off the half-way places: the lower one 0.3 of a link from
    /// the first fluid cell, the upper one 0.8 (both interpolation branches). Given as wall links,
    /// the parabola between the true plates comes out; half-way bounce-back would put each plate
    /// half a cell out, 20 % and 30 % of a cell wrong.
    #[test]
    fn plane_poiseuille_between_off_grid_walls_matches_the_parabola() {
        let nz = 18usize;
        let bounds = Aabb3 { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 1.0, y: 1.0, z: nz as f64 } };
        let mut grid = Grid::around(bounds, 1.0, 0, 1_000).unwrap();
        for k in [0, nz - 1] {
            let idx = grid.index(0, 0, k).unwrap();
            grid.cells[idx] = Cell::Solid;
        }
        let (low, high) = (0.3, 0.8);
        let (z0, z1) = (1.0 - low, (nz - 2) as f64 + high);
        let g = 1e-6;
        let lattice = Lattice::new(&grid, 0.8, [g, 0.0, 0.0], [true, true, false]).unwrap();
        let (bottom, top) = (lattice.index(0, 0, 1).unwrap(), lattice.index(0, 0, nz - 2).unwrap());
        let links = (1..19)
            .filter_map(|q| match C[q][2] {
                -1 => Some(WallLink { cell: bottom, q, frac: low }),
                1 => Some(WallLink { cell: top, q, frac: high }),
                _ => None,
            })
            .collect();
        let mut lattice = lattice.with_walls(links).unwrap();
        assert_eq!(lattice.walls().len(), 10);
        for _ in 0..20_000 {
            lattice.step();
        }
        let nu = lattice.viscosity();
        let h = z1 - z0;
        let mut worst = 0.0f64;
        for k in 1..nz - 1 {
            let z = k as f64;
            let exact = g / (2.0 * nu) * (z - z0) * (z1 - z);
            let (rho, u) = lattice.moments(0, 0, k).unwrap();
            assert!((rho - 1.0).abs() < 1e-6, "mass is kept: {rho}");
            worst = worst.max((u[0] - exact).abs() / (g * h * h / (8.0 * nu)));
        }
        assert!(worst < 1e-2, "worst error {worst} of the peak speed");
        // A link that doesn't lead into a wall is refused.
        let bad = Lattice::new(&grid, 0.8, [g, 0.0, 0.0], [true, true, false]).unwrap().with_walls(vec![WallLink { cell: bottom, q: 5, frac: 0.5 }]);
        assert!(C[5][2] == 1 && bad.is_err());
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

    /// Air blown into a square duct at a set speed and let out at a set pressure: what goes in
    /// comes out, and once the flow has developed the pressure falls at the exact laminar rate,
    /// dp/dx = ν Q / (0.03514 a⁴) for flow rate Q.
    #[test]
    fn a_duct_with_an_inlet_and_an_outlet_keeps_mass_and_the_laminar_pressure_drop() {
        let mut mesh = solidcraft_geom::Mesh::default();
        let (outer, wall, len) = (18.0, 3.0, 36.0);
        let add = crate::voxelize::tests::add_box;
        add(&mut mesh, [0.0, 0.0, 0.0], [len, outer, wall]);
        add(&mut mesh, [0.0, 0.0, outer - wall], [len, outer, outer]);
        add(&mut mesh, [0.0, 0.0, wall], [len, wall, outer - wall]);
        add(&mut mesh, [0.0, outer - wall, wall], [len, outer, outer - wall]);
        let mut grid = Grid::around(mesh.bounds(), 1.0, 0, 100_000).unwrap();
        crate::voxelize(&mesh, &mut grid).unwrap();
        let [nx, ny, nz] = grid.dims;
        let bore: Vec<(usize, usize)> =
            (0..ny).flat_map(|j| (0..nz).map(move |k| (j, k))).filter(|&(j, k)| grid.get(0, j, k) == Some(Cell::Fluid)).collect();
        let a = outer - 2.0 * wall;
        assert_eq!(bore.len(), (a * a) as usize);
        let u0 = 0.02;
        let mut lattice = Lattice::new(&grid, 0.8, [0.0; 3], [false; 3]).unwrap();
        let inlet: Vec<[usize; 3]> = bore.iter().map(|&(j, k)| [0, j, k]).collect();
        let outlet: Vec<[usize; 3]> = bore.iter().map(|&(j, k)| [nx - 1, j, k]).collect();
        lattice.add_opening(&inlet, [1, 0, 0], Opening::Velocity([u0, 0.0, 0.0])).unwrap();
        lattice.add_opening(&outlet, [-1, 0, 0], Opening::Density(1.0)).unwrap();
        for _ in 0..8_000 {
            lattice.step();
        }
        let section = |i: usize| -> (f64, f64) {
            let (mut flow, mut rho) = (0.0, 0.0);
            for &(j, k) in &bore {
                let (r, u) = lattice.moments(i, j, k).unwrap();
                flow += r * u[0];
                rho += r;
            }
            (flow, rho / bore.len() as f64)
        };
        // A uniform inlet speed also lands on the cells against the walls, where no-slip fights
        // it: on a 12-cell bore the flow delivered is a few percent under u0 · a². Known; a
        // flow-rate inlet that corrects its speed to the target comes later, and results report
        // the measured flow.
        let nominal = u0 * a * a;
        let (q_mid, rho_a) = section(12);
        let (q_out, rho_b) = section(28);
        assert!((q_mid - nominal).abs() / nominal < 0.06, "flow {q_mid} vs {nominal} nominal");
        assert!((q_out - q_mid).abs() / q_mid < 0.002, "mass kept: {q_out} out vs {q_mid}");
        let gradient = (rho_a - rho_b) / 3.0 / 16.0;
        let exact = lattice.viscosity() * q_mid / (0.035_144 * a.powi(4));
        let error = (gradient - exact).abs() / exact;
        assert!(error < 0.05, "dp/dx {gradient:e}, exact {exact:e}, error {error}");
        // Openings are checked like any input.
        assert!(lattice.add_opening(&inlet, [1, 0, 0], Opening::Velocity([1.0, 0.0, 0.0])).is_err());
        assert!(lattice.add_opening(&[[0, 0, 0]], [1, 0, 0], Opening::Density(1.0)).is_err(), "a wall cell");
        assert!(lattice.add_opening(&outlet, [1, 0, 0], Opening::Density(-1.0)).is_err());
    }

    /// The Smagorinsky model adds eddy viscosity only where there is shear: a duct's flow slows
    /// a little, never speeds up, and constant 0 is the laminar solver exactly.
    #[test]
    fn smagorinsky_adds_viscosity_with_shear_and_zero_is_laminar() {
        let bounds = Aabb3 { min: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, max: Vec3 { x: 1.0, y: 1.0, z: 18.0 } };
        let mut grid = Grid::around(bounds, 1.0, 0, 1_000).unwrap();
        for k in [0, 17] {
            let idx = grid.index(0, 0, k).unwrap();
            grid.cells[idx] = Cell::Solid;
        }
        let run = |cs: f64| {
            let mut l = Lattice::new(&grid, 0.55, [1e-5, 0.0, 0.0], [true, true, false]).unwrap().with_smagorinsky(cs).unwrap();
            for _ in 0..30_000 {
                l.step();
            }
            (1..17).map(|k| l.moments(0, 0, k).unwrap().1[0]).sum::<f64>()
        };
        let (laminar, plain, turbulent) = (
            run(0.0),
            {
                let mut l = Lattice::new(&grid, 0.55, [1e-5, 0.0, 0.0], [true, true, false]).unwrap();
                for _ in 0..30_000 {
                    l.step();
                }
                (1..17).map(|k| l.moments(0, 0, k).unwrap().1[0]).sum::<f64>()
            },
            run(0.17),
        );
        assert_eq!(laminar, plain, "constant 0 is plain BGK");
        assert!(turbulent < laminar && turbulent > 0.5 * laminar, "{turbulent} vs {laminar}");
        for cs in [-0.1, 0.9, f64::NAN] {
            assert!(Lattice::new(&grid, 0.8, [0.0; 3], [true; 3]).unwrap().with_smagorinsky(cs).is_err(), "{cs}");
        }
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
