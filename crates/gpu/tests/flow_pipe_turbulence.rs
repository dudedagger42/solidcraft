//! Fully developed turbulent pipe flow at Re_τ 180 on the GPU, against the DNS of Eggels et al.
//! (J. Fluid Mech. 268:175–209, 1994): a body force holds the wall shear at u_τ, and the bulk
//! velocity the flow settles at should be U_b = 14.7 u_τ (Re_b 5300, close to Blasius' f there),
//! the centreline U_c = 1.31 U_b. A study run by hand; it prints a table:
//! `cargo test -p solidcraft-gpu --release --test flow_pipe_turbulence -- --ignored --nocapture`.
//! CELLS=16,24,32 (cells across the diameter), CS=0.15 (Smagorinsky; 0: none), TURNS=40 (the run
//! length in R/u_τ), DAMP=0 turns Van Driest's wall damping off.

use std::sync::Arc;

use solidcraft_geom::{Mesh, Vec3};
use solidcraft_gpu::flow::{Backend, FlowGpu};
use solidcraft_sim::{Backend as _, Cell, Grid, Lattice, wall_links};

/// A straight `n`-sided tube along +x from `x0` to `x1`, bore radius `r`, wall `w`, outward.
fn tube(n: usize, r: f64, w: f64, x0: f64, x1: f64) -> Mesh {
    let mut m = Mesh::default();
    let ring = |rad: f64, x: f64| {
        (0..n).map(move |k| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            Vec3::new(x, rad * a.cos(), rad * a.sin())
        })
    };
    for (rad, x) in [(r, x0), (r, x1), (r + w, x0), (r + w, x1)] {
        m.positions.extend(ring(rad, x));
    }
    let v = |b: usize, k: usize| (b * n + k % n) as u32;
    for k in 0..n {
        let (a, b) = (k, k + 1);
        m.triangles.extend([
            [v(2, a), v(3, a), v(3, b)],
            [v(2, a), v(3, b), v(2, b)],
            [v(0, a), v(1, b), v(1, a)],
            [v(0, a), v(0, b), v(1, b)],
            [v(0, a), v(2, b), v(0, b)],
            [v(0, a), v(2, a), v(2, b)],
            [v(1, a), v(1, b), v(3, b)],
            [v(1, a), v(3, b), v(3, a)],
        ]);
    }
    let vol: f64 = m.triangles.iter().map(|t| {
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        a.dot(b.cross(c))
    }).sum();
    if vol < 0.0 {
        for t in &mut m.triangles {
            t.swap(1, 2);
        }
    }
    m
}

/// A small deterministic generator for the starting perturbation.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

#[test]
#[ignore = "a turbulence study on this machine's GPU"]
fn turbulent_pipe_at_re_tau_180_against_dns() {
    let gpu = match FlowGpu::headless() {
        Ok(g) => Arc::new(g),
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    let backend = Backend::new(gpu.clone());
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.into());
    let cells: Vec<usize> = env("CELLS", "16,24,32").split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let cs: f64 = env("CS", "0.15").parse().unwrap();
    let turns: f64 = env("TURNS", "40").parse().unwrap();
    let damp = env("DAMP", "1") != "0";
    println!("GPU {} | Smagorinsky {cs} | damping {damp} | run {turns} R/u_tau", gpu.adapter_name());
    println!("D cells   cells    tau      steps   time(s)  U_b/u_tau (DNS 14.7)  err     U_c/U_b (1.31)  u'_c/u_tau  state");
    // RETAU=20 runs a laminar check: U_b must be Poiseuille's F R² / (8 ν) = u_τ Re_τ / 4.
    let re_tau: f64 = env("RETAU", "180").parse().unwrap();
    if re_tau < 50.0 {
        println!("laminar check: U_b/u_tau should be {:.2}", re_tau / 4.0);
    }
    for &d in &cells {
        let r = 0.5 * d as f64;
        // u_τ so the centreline (about 19 u_τ) stays near lattice speed 0.1.
        let u_tau = 0.005;
        let nu = u_tau * r / re_tau;
        let tau = 0.5 + 3.0 * nu;
        let force = 2.0 * u_tau * u_tau / r;
        let (nx, ny) = (5 * d, d + 6);
        let origin = Vec3::new(0.0, -0.5 * ny as f64, -0.5 * ny as f64);
        let mut grid = Grid { dims: [nx, ny, ny], origin, spacing: 1.0, cells: vec![Cell::Fluid; nx * ny * ny] };
        let radius_of = |j: usize, k: usize| (origin.y + j as f64 + 0.5).hypot(origin.z + k as f64 + 0.5);
        for k in 0..ny {
            for j in 0..ny {
                if radius_of(j, k) > r {
                    for i in 0..nx {
                        grid.cells[i + nx * (j + ny * k)] = Cell::Solid;
                    }
                }
            }
        }
        let mesh = tube(512, r, 2.0, -4.0, nx as f64 + 4.0);
        let links = wall_links(&[&mesh], &grid).unwrap();
        let damping: Vec<u16> = (0..grid.len())
            .map(|c| {
                let (j, k) = ((c / nx) % ny, c / (nx * ny));
                let y_plus = (r - radius_of(j, k)).max(0.0) * u_tau / nu;
                if damp { ((1.0 - (-y_plus / 26.0).exp()) * 65535.0) as u16 } else { 65535 }
            })
            .collect();
        let mut lattice = Lattice::new(&grid, tau, [force, 0.0, 0.0], [true, false, false])
            .unwrap()
            .with_walls(links)
            .unwrap()
            .with_smagorinsky(cs)
            .unwrap()
            .regularized()
            .with_bulk(tau.max(1.0))
            .unwrap()
            .with_damping(damping)
            .unwrap();
        // Start near the turbulent mean (a 1/7-power profile at U_b ≈ 14.7 u_τ) with strong
        // random and swirling disturbances, so the flow does not settle into laminar flow.
        let mut rng = Lcg(1);
        for k in 0..ny {
            for j in 0..ny {
                let rad = radius_of(j, k);
                if rad > r {
                    continue;
                }
                let base = 1.22 * 14.7 * u_tau * (1.0 - rad / r).max(0.0).powf(1.0 / 7.0);
                for i in 0..nx {
                    let x = i as f64 / nx as f64 * std::f64::consts::TAU;
                    let (y, z) = (origin.y + j as f64 + 0.5, origin.z + k as f64 + 0.5);
                    let swirl = 0.3 * base * (2.0 * x).sin() * (1.0 - (rad / r).powi(2));
                    let n = 0.15 * 14.7 * u_tau;
                    let u = [base + n * rng.next(), -swirl * z / r + n * rng.next(), swirl * y / r + n * rng.next()];
                    lattice.set_equilibrium(i + nx * (j + ny * k), 1.0, u);
                }
            }
        }
        let mut stepper = if env("BACKEND", "gpu") == "cpu" { solidcraft_sim::Cpu.stepper(&lattice).unwrap() } else { backend.stepper(&lattice).unwrap() };
        let total = (turns * r / u_tau) as u64;
        let chunk = 1000u64;
        let t = std::time::Instant::now();
        let (mut ub_sum, mut uc_sum, mut uc2_sum, mut samples, mut steps) = (0.0, 0.0, 0.0, 0usize, 0u64);
        let mut state = "ok".to_string();
        let centre = (nx / 2, ny / 2, ny / 2);
        while steps < total {
            if stepper.run(chunk as u32).is_err() || stepper.read(&mut lattice).is_err() {
                state = format!("failed at {steps}");
                break;
            }
            steps += chunk;
            // Bulk velocity over the bore, and the centreline speed.
            let (mut sum, mut count) = (0.0, 0usize);
            for k in 0..ny {
                for j in 0..ny {
                    if radius_of(j, k) > r {
                        continue;
                    }
                    for i in (0..nx).step_by(2) {
                        if let Some((_, u)) = lattice.moments(i, j, k) {
                            sum += u[0];
                            count += 1;
                        }
                    }
                }
            }
            let ub = sum / count.max(1) as f64;
            if !ub.is_finite() || ub.abs() > 1.0 {
                state = format!("unstable at {steps}");
                break;
            }
            let uc = lattice.moments(centre.0, centre.1, centre.2).map_or(f64::NAN, |(_, u)| u[0]);
            if std::env::var_os("HISTORY").is_some() && steps % (total / 24).max(chunk) < chunk {
                println!("   t = {:5.1} R/u_tau: U_b/u_tau {:6.2}, centreline {:6.2}", steps as f64 * u_tau / r, ub / u_tau, uc / u_tau);
            }
            // Averages over the second half of the run.
            if steps > total / 2 {
                ub_sum += ub;
                uc_sum += uc;
                uc2_sum += uc * uc;
                samples += 1;
            }
        }
        let secs = t.elapsed().as_secs_f64();
        let k = samples.max(1) as f64;
        let (ub, uc) = (ub_sum / k, uc_sum / k);
        let rms = (uc2_sum / k - uc * uc).max(0.0).sqrt();
        println!(
            "{d:>7}  {:>6}  {tau:.5}  {steps:>8}  {secs:>7.1}  {:>12.2}           {:>+6.1}%  {:>8.3}        {:>7.2}     {state}",
            grid.len(),
            ub / u_tau,
            100.0 * (ub / u_tau / 14.7 - 1.0),
            uc / ub,
            rms / u_tau,
        );
    }
}
