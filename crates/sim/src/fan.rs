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
    /// Ducted: only the box's two faces across the axis are open, the one `axis` points to held
    /// at this gauge pressure (Pa) and the other at ambient, the rest walls (a casing that runs
    /// end to end). None: every face open to ambient (a free fan).
    pub duct: Option<f64>,
    /// The air's speed along the axis at the start, m/s (0: still air). Started near the speed
    /// it settles at, a ducted fan settles in fewer turns.
    pub initial_speed: f64,
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
    /// With a back-pressure: the outlet's openings and the density they reach.
    outlet: Option<(std::ops::Range<usize>, f64)>,
    /// Ducted and started moving: the inlet's openings, holding the starting speed until the
    /// soft start is over (see [`FanStudy::run`]).
    inlet: Option<std::ops::Range<usize>>,
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

/// Set up a fan study: the `impeller` (one or more closed bodies turning together) turning per
/// `setup` among the static `housing` bodies.
pub fn prepare_fan(housing: &[&Mesh], impeller: &[&Mesh], setup: &FanSetup) -> Result<FanStudy> {
    let first = impeller.first().ok_or(SimError::EmptyMesh)?;
    let axis = setup.axis.normalized().ok_or(SimError::BadRotor)?;
    let omega = setup.rpm * std::f64::consts::TAU / 60.0;
    if !(omega.is_finite() && omega != 0.0 && setup.pad.is_finite() && setup.pad >= 0.0 && setup.air.density > 0.0 && setup.air.viscosity > 0.0) {
        return Err(SimError::BadRotor);
    }
    let mut bounds = first.bounds();
    for m in impeller {
        bounds = union(bounds, m.bounds());
    }
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
    for p in impeller.iter().flat_map(|m| m.positions.iter()) {
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
    // SIM_TIP_SPEED overrides the lattice tip speed (temporary, for stability tests).
    let tip_speed = std::env::var("SIM_TIP_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(TIP_SPEED);
    let dt = tip_speed * dx / tip;
    let units = Units { dx, dt, density: setup.air.density };
    let tau = 0.5 + 3.0 * setup.air.viscosity * dt / (dx * dx);
    // SIM_MIN_TAU floors the relaxation time (temporary, for stability tests).
    let tau = tau.max(std::env::var("SIM_MIN_TAU").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0));
    let mut warnings = Vec::new();
    if tau < 0.51 {
        warnings.push(Warning::LowViscosity { tau });
    }
    let mut rotor = Rotor::from_parts(impeller, &grid, setup.origin, axis, omega * dt, setup.max_cells.saturating_mul(8))?;
    if std::env::var_os("SIM_TRACE").is_some() {
        let round: Vec<usize> = impeller
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                crate::is_round(m, setup.origin, {
                    let l = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
                    Vec3 { x: axis.x / l, y: axis.y / l, z: axis.z / l }
                })
            })
            .map(|(i, _)| i)
            .collect();
        eprintln!("trace: round parts (held in place, walls moving) {round:?} of {}", impeller.len());
    }
    // SIM_ROTOR_HALFWAY: the rotor's walls half-way (a staircase), for comparing (temporary).
    if std::env::var_os("SIM_ROTOR_HALFWAY").is_some() {
        rotor.curved = false;
    }
    let mut lattice = Lattice::new(&grid, tau, [0.0; 3], [false; 3])?
        .with_smagorinsky(setup.turbulence)?
        .regularized()
        // SIM_BULK_TAU overrides the bulk relaxation time (temporary, for stability tests).
        .with_bulk(tau.max(std::env::var("SIM_BULK_TAU").ok().and_then(|v| v.parse().ok()).unwrap_or(BULK_TAU)))?
        .with_walls(if std::env::var_os("SIM_STATIC_HALFWAY").is_some() { Vec::new() } else { crate::wall_links(housing, &grid)? })?;
    let pad_cells = (setup.pad / grid.spacing).floor() as usize;
    if pad_cells >= 4 {
        lattice = lattice.with_sponge((pad_cells / 2).min(12), tau.max(SPONGE_TAU))?;
    }
    // The box's faces: ambient pressure, or with a duct only the two across the axis (which must
    // run along a grid axis), the downstream one at the back-pressure.
    let along = [axis.x, axis.y, axis.z].iter().position(|c| c.abs() > 0.999);
    if setup.duct.is_some() && along.is_none() {
        return Err(SimError::BadRotor);
    }
    let mut outlet = None;
    let mut inlet = None;
    // The air's starting speed along the axis (lattice units).
    let start_speed = {
        let u = units.lattice_speed(setup.initial_speed);
        [axis.x * u, axis.y * u, axis.z * u]
    };
    for (inward, face) in [([1i64, 0, 0], 0usize), ([-1, 0, 0], 1), ([0, 1, 0], 2), ([0, -1, 0], 3), ([0, 0, 1], 4), ([0, 0, -1], 5)] {
        let mut density = 1.0;
        let mut upstream = false;
        if let (Some(back), Some(a)) = (setup.duct, along) {
            if face / 2 != a {
                continue;
            }
            // The face `axis` points to is the outlet: its inward normal is against the axis.
            let component = [axis.x, axis.y, axis.z][a];
            if (inward[a] as f64) * component < 0.0 {
                density = units.lattice_density(back);
            } else {
                upstream = true;
            }
        }
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
            let start = lattice.openings().len();
            // The back-pressure comes up with the soft start (see `FanStudy::run`): ambient now.
            // A ducted run started moving holds its inlet at that speed meanwhile: from still air
            // a multistage compressor's slow first turns stall it, and it stays stalled as it
            // comes up to speed (a hung start), so the duty it was designed for is never reached.
            let held = upstream && setup.initial_speed != 0.0;
            // A hard velocity inlet: one that lets sound out gives way to the stalled rotor's push.
            let first = if held { Opening::Velocity(start_speed) } else { Opening::Density(1.0) };
            lattice.add_opening(&at, [inward[0] as i32, inward[1] as i32, inward[2] as i32], first)?;
            if density != 1.0 {
                outlet = Some((start..lattice.openings().len(), density));
            }
            if held {
                inlet = Some(start..lattice.openings().len());
            }
        }
    }
    if setup.initial_speed != 0.0 {
        for c in 0..grid.len() {
            if grid.cells.get(c) == Some(&Cell::Fluid) {
                lattice.set_equilibrium(c, 1.0, start_speed);
            }
        }
    }
    let lattice = lattice.with_rotor(rotor)?;
    Ok(FanStudy { grid, lattice, units, omega, radius: radius / 1000.0, centre, axis, warnings, steps: 0, outlet, inlet })
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
        let trace: Option<u64> = std::env::var("SIM_TRACE").ok().and_then(|v| v.parse().ok()).filter(|n| *n > 0);
        // A soft start: the impeller's speed rises smoothly from nothing over the first turn
        // (started at full speed, the air in the blades' way is struck all at once and the lattice
        // goes unstable), then the back-pressure over the second (raised with the speed, it
        // stalls the impeller before it is up to speed).
        let ramp = per.ceil().max(1.0);
        let smooth = |x: f64| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        for s in 0..total {
            let done = self.steps as f64;
            if done <= 2.0 * ramp {
                if let Some(r) = self.lattice.rotors_mut().first_mut() {
                    r.set_speed(smooth(done / ramp));
                }
                if let Some((range, target)) = self.outlet.clone() {
                    let _ = self.lattice.set_openings(range, Opening::Density(1.0 + (target - 1.0) * smooth(done / ramp - 1.0)));
                }
            }
            // The soft start over, a held inlet lets go: ambient pressure, the flow its own.
            if done > 2.0 * ramp
                && let Some(range) = self.inlet.take()
            {
                let _ = self.lattice.set_openings(range, Opening::Density(1.0));
            }
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
            // SIM_TRACE=N: every N steps, the turns, torque, flow and the lattice's extremes
            // (for finding where a run goes unstable).
            if let Some(n) = trace
                && s % n == 0
            {
                let u = self.units;
                let torque = self.lattice.rotors().first().map_or(0.0, |r| r.axial_torque()) * u.density * u.dx.powi(5) / (u.dt * u.dt);
                let (lo, hi, fastest) = self.lattice.extremes();
                eprintln!(
                    "trace: turn {:.3}, torque {:.3e} N·m, disc flow {:.3} CFM, density {lo:.5}..{hi:.5}, fastest {fastest:.4} (lattice)",
                    self.steps as f64 / per,
                    -torque * self.omega.signum(),
                    self.disc_flow() * crate::study::CFM_PER_M3S,
                );
                let (a, b) = self.lattice.extreme_cells();
                let at = |c: usize| {
                    let [i, j, k] = self.grid.coords(c);
                    let p = self.grid.center(i, j, k);
                    let r = (p - self.centre).dot(self.axis);
                    let off = ((p - self.centre) - self.axis * r).len();
                    format!("[{:.2}, {:.2}, {:.2}] mm (r {off:.2}, rotor-covered neighbours {})", p.x, p.y, p.z, self.lattice.rotor_neighbours(c))
                };
                eprintln!("       lowest at {}, highest at {}", at(a), at(b));
                if !(lo.is_finite() && hi.is_finite()) {
                    break;
                }
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
            duct: None,
            initial_speed: 0.0,
        }
    }

    /// An ASCII STL file as a mesh.
    fn read_stl(path: &std::path::Path) -> Mesh {
        let mut m = Mesh::default();
        let text = std::fs::read_to_string(path).unwrap();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("vertex ") {
                let v: Vec<f64> = rest.split_whitespace().map(|x| x.parse().unwrap()).collect();
                m.positions.push(Vec3::new(v[0], v[1], v[2]));
                if m.positions.len() % 3 == 0 {
                    let b = m.positions.len() as u32 - 3;
                    m.triangles.push([b, b + 1, b + 2]);
                }
            }
        }
        m
    }

    /// Where the time goes on the 30 mm compressor (SIM_PROFILE=1 prints each step's phases).
    #[test]
    #[ignore = "a profile of the compressor case: needs B:/Claude/compressor/geom"]
    fn profile_the_compressor() {
        let dir = std::path::Path::new("B:/Claude/compressor/geom");
        let t = std::time::Instant::now();
        let load = |n: &str| read_stl(&dir.join(format!("{n}.stl")));
        let rotor: Vec<Mesh> = ["rotor_hub", "rotor_row1", "rotor_row2", "rotor_row3", "rotor_row4"].iter().map(|n| load(n)).collect();
        let housing: Vec<Mesh> = ["casing", "stator_row1", "stator_row2", "stator_row3", "stator_row4"].iter().map(|n| load(n)).collect();
        eprintln!("load {:?}", t.elapsed());
        let cell: f64 = std::env::var("CELL").ok().and_then(|c| c.parse().ok()).unwrap_or(0.4);
        let s = FanSetup {
            cell,
            pad: 0.0,
            air: AIR,
            rpm: 54_300.0,
            origin: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            turbulence: 0.15,
            max_cells: 50_000_000,
            duct: Some(0.0),
            initial_speed: 15.0,
        };
        let t = std::time::Instant::now();
        let r: Vec<&Mesh> = rotor.iter().collect();
        let h: Vec<&Mesh> = housing.iter().collect();
        let mut study = prepare_fan(&h, &r, &s).unwrap();
        eprintln!("prepare {:?}: {:?} cells, {} swept", t.elapsed(), study.grid.dims, study.lattice.rotors()[0].swept.len());
        let t = std::time::Instant::now();
        for _ in 0..10 {
            study.lattice.step();
        }
        eprintln!("10 steps {:?}", t.elapsed());
    }

    /// The compressor at rest (temporary probe): with the rotor held still the air stays still,
    /// so the torque must be nought and the density 1 everywhere.
    #[test]
    #[ignore = "a probe of the compressor case: needs B:/Claude/compressor/geom_hub1"]
    fn the_compressor_at_rest_probe() {
        let geom = std::env::var("GEOM").unwrap_or("B:/Claude/compressor/geom_hub1".into());
        let dir = std::path::Path::new(&geom);
        let load = |n: &str| read_stl(&dir.join(format!("{n}.stl")));
        // ROTOR / HOUSING: comma-separated part names (the whole compressor by default).
        let parts = |var: &str, all: &str| std::env::var(var).unwrap_or(all.into()).split(',').map(|n| load(n.trim())).collect::<Vec<Mesh>>();
        let rotor = parts("ROTOR", "rotor_hub,rotor_row1,rotor_row2,rotor_row3,rotor_row4");
        let housing = parts("HOUSING", "casing,stator_row1,stator_row2,stator_row3,stator_row4");
        let cell: f64 = std::env::var("CELL").ok().and_then(|c| c.parse().ok()).unwrap_or(0.4);
        let s = FanSetup {
            cell,
            pad: 0.0,
            air: AIR,
            rpm: 54_300.0,
            origin: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
            turbulence: 0.15,
            max_cells: 50_000_000,
            duct: Some(0.0),
            initial_speed: 0.0,
        };
        let r: Vec<&Mesh> = rotor.iter().collect();
        let h: Vec<&Mesh> = housing.iter().collect();
        let mut study = prepare_fan(&h, &r, &s).unwrap();
        let share: f64 = std::env::var("SHARE").ok().and_then(|c| c.parse().ok()).unwrap_or(0.0);
        let steps: usize = std::env::var("STEPS").ok().and_then(|c| c.parse().ok()).unwrap_or(40);
        // Ramped to `share` over the first half (smoothstep), then held.
        for n in 0..=steps {
            let x = (2.0 * n as f64 / steps as f64).min(1.0);
            study.lattice.rotors_mut()[0].set_speed(share * x * x * (3.0 - 2.0 * x));
            study.lattice.step();
            if n % (steps / 8).max(1) == 0 {
                let rot = &study.lattice.rotors()[0];
                let (lo, hi) = study.lattice.extreme_cells();
                let at = |c: usize| {
                    let [nx, ny, _] = study.grid.dims;
                    let p = study.grid.center(c % nx, (c / nx) % ny, c / (nx * ny));
                    format!("[{:.2}, {:.2}, {:.2}] r {:.2} n{}", p.x, p.y, p.z, p.x.hypot(p.y), study.lattice.rotor_neighbours(c))
                };
                println!("step {n}: torque {:.3e} density {:?} lo {} hi {}", rot.axial_torque(), study.lattice.extremes(), at(lo), at(hi));
            }
        }
    }

    /// Turned one way, a pitched-blade fan blows air along its axis one way and needs a driving
    /// torque; turned the other, it blows the other way. Power is torque times speed.
    #[test]
    #[ignore = "a few minutes on the CPU reference"]
    fn an_axial_fan_blows_along_its_axis_and_needs_torque() {
        let fan = axial_fan(4, 4.0, 16.0);
        let mut forward = prepare_fan(&[], &[&fan], &setup(3000.0)).unwrap();
        let a = forward.run(2.0, |_| true);
        println!("forward: {a:?}, {:?} cells, {:.0} steps per turn", forward.grid.dims, forward.steps_per_revolution());
        assert!(a.torque > 0.0 && a.flow.abs() > 0.0, "{a:?}");
        assert!((a.power - a.torque * 3000.0 * std::f64::consts::TAU / 60.0).abs() < 1e-12 * a.power.abs().max(1.0));
        let mut back = prepare_fan(&[], &[&fan], &setup(-3000.0)).unwrap();
        let b = back.run(2.0, |_| true);
        println!("backward: {b:?}");
        assert!(b.torque > 0.0, "{b:?}");
        assert!(a.flow * b.flow < 0.0, "the air goes the other way: {} vs {}", a.flow, b.flow);
    }
}
