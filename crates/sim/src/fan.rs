//! Fan studies: a fan's impeller (any body, hand drawn or generated) turning at a set speed in a
//! box of still air, inside whatever housing surrounds it, and what it does: the torque its motor
//! must supply, the shaft power, and the air it moves through its own disc.
//!
//! The box's faces are open to the ambient pressure, so the air the fan draws comes in and the
//! air it blows leaves; a housing's walls are static bodies with curved walls. The impeller is a
//! [`crate::Rotor`]. The time step is set by the blade tips: they move [`TIP_SPEED`] cells per
//! step, well inside the lattice's limit, so the air the blades throw (never faster than the tips)
//! stays within it too.

use solidcraft_geom::{Aabb3, Mesh, Vec3};

use crate::lbm::Opening;
use crate::study::{Air, BULK_TAU, SPONGE_TAU, Units, Warning};
use crate::{Cell, Grid, Lattice, Result, Rotor, SimError};

/// The blade tips' speed through the lattice (cells per step).
pub const TIP_SPEED: f64 = 0.1;

/// What a fan study runs.
#[derive(Clone, Debug, PartialEq)]
pub struct FanSetup {
    /// Cell size, mm.
    pub cell: f64,
    /// How far the box of air extends past the bodies on every side, mm.
    pub pad: f64,
    pub air: Air,
    /// The impeller's speed, revolutions per minute (positive: right-handed about `axis`).
    pub rpm: f64,
    /// A point on the axis and its direction (mm; any length).
    pub origin: Vec3,
    pub axis: Vec3,
    /// The Smagorinsky constant (0: laminar).
    pub turbulence: f64,
    /// Most cells the grid may have.
    pub max_cells: u64,
}

/// A prepared fan study.
#[derive(Clone)]
pub struct FanStudy {
    pub grid: Grid,
    pub lattice: Lattice,
    pub units: Units,
    /// The speed, rad/s.
    pub omega: f64,
    /// The impeller's radius (m) and the middle of its extent along the axis (mm).
    pub radius: f64,
    centre: Vec3,
    axis: Vec3,
    pub warnings: Vec<Warning>,
    steps: u64,
}

/// What a fan does, averaged over its last revolution.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FanResult {
    /// The torque its motor supplies to keep the speed against the air, N·m (positive: along
    /// the turning direction).
    pub torque: f64,
    /// The shaft power, W.
    pub power: f64,
    /// The air through the impeller's disc along the axis, m³/s (negative: against it).
    pub flow: f64,
    /// The blade tips' speed, m/s.
    pub tip_speed: f64,
    /// Revolutions run, and the air time they took, s.
    pub revolutions: f64,
    pub time: f64,
}

fn union(a: Aabb3, b: Aabb3) -> Aabb3 {
    Aabb3 { min: a.min.min(b.min), max: a.max.max(b.max) }
}

/// Set up a fan study: `impeller` turning per `setup` among the static `housing` bodies.
pub fn prepare_fan(housing: &[&Mesh], impeller: &Mesh, setup: &FanSetup) -> Result<FanStudy> {
    let axis = setup.axis.normalized().ok_or(SimError::BadRotor)?;
    let omega = setup.rpm * std::f64::consts::TAU / 60.0;
    if !(omega.is_finite() && omega != 0.0 && setup.pad.is_finite() && setup.pad >= 0.0 && setup.air.density > 0.0 && setup.air.viscosity > 0.0) {
        return Err(SimError::BadRotor);
    }
    let mut bounds = impeller.bounds();
    for m in housing {
        bounds = union(bounds, m.bounds());
    }
    let pad = Vec3::new(setup.pad, setup.pad, setup.pad);
    bounds = Aabb3 { min: bounds.min - pad, max: bounds.max + pad };
    let mut grid = Grid::around(bounds, setup.cell, 1, setup.max_cells)?;
    for m in housing {
        crate::voxelize(m, &mut grid)?;
    }
    let [nx, ny, nz] = grid.dims;
    let on_face =
        |i: usize, j: usize, k: usize| usize::from(i == 0 || i + 1 == nx) + usize::from(j == 0 || j + 1 == ny) + usize::from(k == 0 || k + 1 == nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if on_face(i, j, k) >= 2
                    && let Some(cell) = grid.index(i, j, k).and_then(|x| grid.cells.get_mut(x))
                {
                    *cell = Cell::Solid;
                }
            }
        }
    }
    // The impeller's radius and axial middle.
    let (mut radius, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
    for p in &impeller.positions {
        let d = *p - setup.origin;
        let along = d.dot(axis);
        radius = radius.max((d - axis * along).len());
        lo = lo.min(along);
        hi = hi.max(along);
    }
    if !(radius > 0.0) {
        return Err(SimError::EmptyMesh);
    }
    let centre = setup.origin + axis * (0.5 * (lo + hi));
    let dx = grid.spacing / 1000.0;
    let tip = omega.abs() * radius / 1000.0;
    let dt = TIP_SPEED * dx / tip;
    let units = Units { dx, dt, density: setup.air.density };
    let tau = 0.5 + 3.0 * setup.air.viscosity * dt / (dx * dx);
    let mut warnings = Vec::new();
    if tau < 0.51 {
        warnings.push(Warning::LowViscosity { tau });
    }
    let rotor = Rotor::new(impeller, &grid, setup.origin, axis, omega * dt, setup.max_cells.saturating_mul(8))?;
    let mut lattice = Lattice::new(&grid, tau, [0.0; 3], [false; 3])?
        .with_smagorinsky(setup.turbulence)?
        .regularized()
        .with_bulk(tau.max(BULK_TAU))?
        .with_walls(crate::wall_links(housing, &grid)?)?;
    let pad_cells = (setup.pad / grid.spacing).floor() as usize;
    if pad_cells >= 4 {
        lattice = lattice.with_sponge((pad_cells / 2).min(12), tau.max(SPONGE_TAU))?;
    }
    // The box's faces: ambient pressure.
    for (inward, face) in [([1i64, 0, 0], 0usize), ([-1, 0, 0], 1), ([0, 1, 0], 2), ([0, -1, 0], 3), ([0, 0, 1], 4), ([0, 0, -1], 5)] {
        let mut at = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let here = match face {
                        0 => i == 0,
                        1 => i + 1 == nx,
                        2 => j == 0,
                        3 => j + 1 == ny,
                        4 => k == 0,
                        _ => k + 1 == nz,
                    };
                    if !here || on_face(i, j, k) != 1 {
                        continue;
                    }
                    let inside = [i as i64 + inward[0], j as i64 + inward[1], k as i64 + inward[2]].map(|v| v.max(0) as usize);
                    if grid.get(i, j, k) == Some(Cell::Fluid) && grid.get(inside[0], inside[1], inside[2]) == Some(Cell::Fluid) {
                        at.push([i, j, k]);
                    }
                }
            }
        }
        if !at.is_empty() {
            lattice.add_opening(&at, [inward[0] as i32, inward[1] as i32, inward[2] as i32], Opening::Density(1.0))?;
        }
    }
    let lattice = lattice.with_rotor(rotor)?;
    Ok(FanStudy { grid, lattice, units, omega, radius: radius / 1000.0, centre, axis, warnings, steps: 0 })
}

impl FanStudy {
    /// Lattice steps per revolution.
    pub fn steps_per_revolution(&self) -> f64 {
        std::f64::consts::TAU / (self.omega.abs() * self.units.dt)
    }

    /// The air through the impeller's disc along the axis (m³/s): the fluid cells within half a
    /// cell of the plane through its middle, out to its radius.
    pub fn disc_flow(&self) -> f64 {
        let [nx, ny, nz] = self.grid.dims;
        let (h, r) = (0.5 * self.grid.spacing, self.radius * 1000.0);
        let mut flow = 0.0;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let d = self.grid.center(i, j, k) - self.centre;
                    let along = d.dot(self.axis);
                    if along.abs() > h || (d - self.axis * along).len() > r {
                        continue;
                    }
                    if let Some((_, u)) = self.lattice.moments(i, j, k) {
                        flow += u[0] * self.axis.x + u[1] * self.axis.y + u[2] * self.axis.z;
                    }
                }
            }
        }
        // Lattice speed × cell face area; the slab is one cell thick along the axis's direction.
        self.units.speed(flow) * self.units.dx * self.units.dx
    }

    /// Run `revolutions` more turns and report the last one's averages. `progress` gets the
    /// fraction done and may stop the run early (returning false).
    pub fn run(&mut self, revolutions: f64, mut progress: impl FnMut(f64) -> bool) -> FanResult {
        let per = self.steps_per_revolution();
        let total = (revolutions * per).ceil().max(1.0) as u64;
        let last = (per.ceil() as u64).clamp(1, total);
        let (mut torque, mut samples) = (0.0, 0u64);
        let (mut flow, mut flow_samples) = (0.0, 0u64);
        let every = (last / 16).max(1);
        for s in 0..total {
            self.lattice.step();
            self.steps += 1;
            if s >= total - last {
                torque += self.lattice.rotors().first().map_or(0.0, |r| r.axial_torque());
                samples += 1;
                if (total - 1 - s).is_multiple_of(every) {
                    flow += self.disc_flow();
                    flow_samples += 1;
                }
            }
            if s % 256 == 0 && !progress(s as f64 / total as f64) {
                break;
            }
        }
        // Lattice torque (mass · cells² / steps²) to N·m; the air's torque on the impeller is
        // against its turning, the motor's the opposite.
        let u = self.units;
        let air = if samples > 0 { torque / samples as f64 } else { 0.0 } * u.density * u.dx.powi(5) / (u.dt * u.dt);
        let turning = self.omega.signum();
        let motor = -air * turning;
        FanResult {
            torque: motor,
            power: motor * self.omega.abs(),
            flow: if flow_samples > 0 { flow / flow_samples as f64 } else { 0.0 },
            tip_speed: self.omega.abs() * self.radius,
            revolutions: self.steps as f64 / per,
            time: self.steps as f64 * u.dt,
        }
    }
}

#[cfg(test)]
mod tests {
    use solidcraft_geom::{Mesh, Vec3};

    use super::*;
    use crate::study::AIR;

    /// A flat-bladed axial fan of `blades` plates pitched 30° about their own radial line, hub to
    /// tip `hub`..`tip` mm, chord 8 mm, 1 mm thick, about the z axis.
    fn axial_fan(blades: usize, hub: f64, tip: f64) -> Mesh {
        let mut m = Mesh::default();
        let pitch = 30f64.to_radians();
        for b in 0..blades {
            let a = std::f64::consts::TAU * b as f64 / blades as f64;
            let (radial, tangent) = (Vec3::new(a.cos(), a.sin(), 0.0), Vec3::new(-a.sin(), a.cos(), 0.0));
            // The plate's chord leans `pitch` out of the plane of rotation.
            let chord = tangent * pitch.cos() + Vec3::new(0.0, 0.0, 1.0) * pitch.sin();
            let thick = tangent * -pitch.sin() + Vec3::new(0.0, 0.0, 1.0) * pitch.cos();
            let corner = |r: f64, c: f64, t: f64| radial * r + chord * c + thick * t;
            let base = m.positions.len() as u32;
            for k in 0..8 {
                let r = if k & 1 == 0 { hub } else { tip };
                let c = if k & 2 == 0 { -4.0 } else { 4.0 };
                let t = if k & 4 == 0 { -0.5 } else { 0.5 };
                m.positions.push(corner(r, c, t));
            }
            // The box's faces, wound like `add_box` (the frame radial, chord, thick is right-handed).
            for [p, q, r, s] in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
                m.triangles.push([base + p, base + q, base + r]);
                m.triangles.push([base + p, base + r, base + s]);
            }
        }
        // The hub: a box about the axis.
        crate::voxelize::tests::add_box(&mut m, [-hub, -hub, -5.0], [hub, hub, 5.0]);
        m
    }

    fn setup(rpm: f64) -> FanSetup {
        FanSetup {
            cell: 1.0,
            pad: 12.0,
            air: AIR,
            rpm,
            origin: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            turbulence: 0.15,
            max_cells: 2_000_000,
        }
    }

    /// Turned one way, a pitched-blade fan blows air along its axis one way and needs a driving
    /// torque; turned the other, it blows the other way. Power is torque times speed.
    #[test]
    #[ignore = "a few minutes on the CPU reference"]
    fn an_axial_fan_blows_along_its_axis_and_needs_torque() {
        let fan = axial_fan(4, 4.0, 16.0);
        let mut forward = prepare_fan(&[], &fan, &setup(3000.0)).unwrap();
        let a = forward.run(2.0, |_| true);
        println!("forward: {a:?}, {:?} cells, {:.0} steps per turn", forward.grid.dims, forward.steps_per_revolution());
        assert!(a.torque > 0.0 && a.flow.abs() > 0.0, "{a:?}");
        assert!((a.power - a.torque * 3000.0 * std::f64::consts::TAU / 60.0).abs() < 1e-12 * a.power.abs().max(1.0));
        let mut back = prepare_fan(&[], &fan, &setup(-3000.0)).unwrap();
        let b = back.run(2.0, |_| true);
        println!("backward: {b:?}");
        assert!(b.torque > 0.0, "{b:?}");
        assert!(a.flow * b.flow < 0.0, "the air goes the other way: {} vs {}", a.flow, b.flow);
    }
}
