//! The Airflow solver against flows whose answers are known, at several resolutions, on the GPU
//! (a study run by hand; it prints a table):
//! `cargo test -p solidcraft-gpu --release --test flow_validation -- --ignored --nocapture`.
//! VALIDATE=pipe,square picks the cases and CELLS=6,8,12 the cells across the bore.
//!
//! Laminar flow in a long straight bore, measured where it is fully developed:
//! - a round pipe of diameter D: u(r) = 2U (1 − r²/R²), dp/dx = 32 μ U / D² (Hagen–Poiseuille);
//! - a square duct of side a: peak speed 2.0963 U, dp/dx = 28.455 μ U / a² (f·Re = 56.91,
//!   Shah & London 1978).

use std::sync::Arc;

use solidcraft_geom::{Mesh, Vec3};
use solidcraft_gpu::flow::{Backend, FlowGpu};
use solidcraft_sim::{AIR, FlowSetup, Inlet, Limits, Outlet, Patch, Run, Source, prepare};

/// A straight bore along +x from 0 to `len` (mm): an `n`-sided tube, inner circumradius `r_in`,
/// outer `r_out`, its first vertex at angle `turn`. Closed and outward.
fn tube(n: usize, r_in: f64, r_out: f64, len: f64, turn: f64) -> Mesh {
    let mut m = Mesh::default();
    let ring = |r: f64, x: f64| (0..n).map(move |k| {
        let a = turn + std::f64::consts::TAU * k as f64 / n as f64;
        Vec3::new(x, r * a.cos(), r * a.sin())
    });
    // Vertex blocks: inner at 0, inner at len, outer at 0, outer at len.
    for (r, x) in [(r_in, 0.0), (r_in, len), (r_out, 0.0), (r_out, len)] {
        m.positions.extend(ring(r, x));
    }
    let v = |block: usize, k: usize| (block * n + k % n) as u32;
    for k in 0..n {
        let (a, b) = (k, k + 1);
        // Outer wall faces out (away from the axis), the bore's wall faces in (toward it).
        m.triangles.push([v(2, a), v(3, a), v(3, b)]);
        m.triangles.push([v(2, a), v(3, b), v(2, b)]);
        m.triangles.push([v(0, a), v(1, b), v(1, a)]);
        m.triangles.push([v(0, a), v(0, b), v(1, b)]);
        // End rings: the x = 0 one faces −x, the x = len one +x.
        m.triangles.push([v(0, a), v(2, b), v(0, b)]);
        m.triangles.push([v(0, a), v(2, a), v(2, b)]);
        m.triangles.push([v(1, a), v(1, b), v(3, b)]);
        m.triangles.push([v(1, a), v(3, b), v(3, a)]);
    }
    // Outward: a positive signed volume (the windings above are consistent with each other).
    let vol: f64 = m.triangles.iter().map(|t| {
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        a.dot(b.cross(c)) / 6.0
    }).sum();
    if vol < 0.0 {
        for t in &mut m.triangles {
            t.swap(1, 2);
        }
    }
    m
}

/// The bore's cross-section at `x` as a patch facing +x.
fn across(n: usize, r: f64, x: f64, turn: f64) -> Patch {
    let p = |k: usize| {
        let a = turn + std::f64::consts::TAU * k as f64 / n as f64;
        Vec3::new(x, r * a.cos(), r * a.sin())
    };
    let c = Vec3::new(x, 0.0, 0.0);
    Patch { triangles: (0..n).map(|k| [c, p(k), p(k + 1)]).collect(), normal: Vec3::X }
}

struct Case {
    name: &'static str,
    /// Sides of the bore (4: square) and its first vertex's angle.
    sides: usize,
    turn: f64,
    /// Diameter, or the square's side (mm).
    size: f64,
    /// The bore's circumradius (mm).
    r: f64,
    /// Peak speed over the mean, and dp/dx over μU/size², fully developed.
    peak: f64,
    gradient: f64,
    /// Reynolds number, length in diameters, Smagorinsky constant; and whether dp/dx is judged
    /// against Blasius' turbulent friction factor (0.3164 Re^-¼) rather than `gradient`.
    re: f64,
    lengths: f64,
    turbulence: f64,
    blasius: bool,
}

fn cases() -> Vec<Case> {
    let d = 8.0;
    vec![
        Case { name: "pipe", sides: 256, turn: 0.0, size: d, r: 0.5 * d, peak: 2.0, gradient: 32.0, re: 50.0, lengths: 12.0, turbulence: 0.0, blasius: false },
        // Turbulent pipe flow, Re 5000 (an 8 mm bore at 9.4 m/s): the regime of real ducts.
        Case { name: "turbulent", sides: 256, turn: 0.0, size: d, r: 0.5 * d, peak: f64::NAN, gradient: f64::NAN, re: 5000.0, lengths: 40.0, turbulence: 0.15, blasius: true },
        Case {
            name: "square",
            sides: 4,
            turn: std::f64::consts::FRAC_PI_4,
            size: d,
            r: d / std::f64::consts::SQRT_2,
            peak: 2.0963,
            gradient: 28.455,
            re: 50.0,
            lengths: 12.0,
            turbulence: 0.0,
            blasius: false,
        },
    ]
}

#[test]
#[ignore = "a validation study on this machine's GPU"]
fn laminar_bores_against_their_exact_solutions() {
    let gpu = match FlowGpu::headless() {
        Ok(g) => Arc::new(g),
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    let backend = Backend::new(gpu.clone());
    println!("GPU: {}", gpu.adapter_name());
    let picked = std::env::var("VALIDATE").unwrap_or_else(|_| "pipe,square".into());
    let cells: Vec<f64> =
        std::env::var("CELLS").unwrap_or_else(|_| "6,8,12,16,24,32".into()).split(',').filter_map(|s| s.trim().parse().ok()).collect();
    println!("case    cells/D  cells(M)  steps   time(s)  kept 0.3→0.7  flow err  peak/U (exact)  peak err   dp/dx err  profile L2   (vs the flow there)");
    for case in cases().into_iter().filter(|c| picked.split(',').any(|p| p.trim() == c.name)) {
        let len = case.lengths * case.size;
        let mean = case.re * AIR.viscosity / (case.size * 1e-3);
        for &n in &cells {
            let cell = case.size / n;
            let mesh = tube(case.sides, case.r, case.r + (2.0 * cell).max(1.0), len, case.turn);
            let setup = FlowSetup {
                cell,
                pad: (8.0 * cell).max(4.0),
                air: AIR,
                inlets: vec![Inlet { name: "in".into(), patch: across(case.sides, case.r, 0.0, case.turn), source: Source::Speed(mean), cap: true }],
                outlets: vec![Outlet { name: "out".into(), patch: across(case.sides, case.r, len, case.turn) }],
                turbulence: if case.turbulence > 0.0 { std::env::var("TURB").ok().and_then(|v| v.parse().ok()).unwrap_or(case.turbulence) } else { 0.0 },
                roughness: 0.0,
                max_cells: 40_000_000,
            };
            let prepared = match prepare(&[&mesh], &setup) {
                Ok(p) => p,
                Err(e) => {
                    println!("{:7} {n:>7}  prepare failed: {e}", case.name);
                    continue;
                }
            };
            if !prepared.warnings.is_empty() {
                println!("   warnings: {:?}", prepared.warnings);
            }
            let total = prepared.grid.len();
            let units = prepared.units;
            let t = std::time::Instant::now();
            let cpu = std::env::var("BACKEND").is_ok_and(|b| b == "cpu");
            let chosen: &dyn solidcraft_sim::Backend = if cpu { &solidcraft_sim::Cpu } else { &backend };
            let mut run = Run::new(prepared, chosen, Limits { max_steps: 400_000, check_every: 1_000, tolerance: 0.0005, soft_start: true }).unwrap();
            let state = run.finish(|_| true);
            let secs = t.elapsed().as_secs_f64();
            let fields = run.fields();
            let Some(sample) = run.average().or_else(|| run.last().cloned()) else {
                println!("{:7} {n:>7}  no measurement: {:?} after {} steps", case.name, state, run.steps);
                continue;
            };
            let _ = &sample;
            // The truth is the imposed inlet speed over the bore's true area: the flow error is how
            // far the voxel bore's flow is from that, and the profile and gradient are judged
            // against that same mean.
            let area = case.sides as f64 * 0.5 * case.r * case.r * (std::f64::consts::TAU / case.sides as f64).sin() * 1e-6;
            // The lattice's own flow: u_x summed over the fluid cells of one cross-section layer.
            let (q3, q7) = (layer_flow(&fields, &units, 0.3 * len, case.r), layer_flow(&fields, &units, 0.7 * len, case.r));
            let flow_err = q7 / (mean * area) - 1.0;
            // The profile and gradient judged against the flow actually there (the solver's flow
            // physics alone); against the imposed flow they would also carry the flow error.
            let u = q7 / area;
            // The profile across the middle of the developed stretch (x = 0.7 L), along y at z = 0.
            let x = 0.7 * len;
            let half = 0.5 * case.size;
            let (mut err2, mut ref2, mut peak) = (0.0, 0.0, 0.0f64);
            for i in 0..=80 {
                let y = -half + case.size * i as f64 / 80.0;
                let exact = if case.sides == 4 { square_profile(y / half, 0.0) * u } else { 2.0 * u * (1.0 - (y / half).powi(2)) };
                if let Some((v, _)) = fields.sample(Vec3::new(x, y, 0.0)) {
                    err2 += (v.x - exact).powi(2);
                    ref2 += exact * exact;
                    peak = peak.max(v.x);
                }
            }
            // How the flow develops: the centreline speed over the mean, and the mean pressure, at
            // tenths of the length.
            if std::env::var_os("ALONG").is_some() {
                let line: Vec<String> = (1..10)
                    .map(|k| {
                        let xk = 0.1 * k as f64 * len;
                        let c = fields.sample(Vec3::new(xk, 0.0, 0.0)).map_or(f64::NAN, |(v, _)| v.x / u);
                        format!("{:.0}D:{c:.2}", xk / case.size)
                    })
                    .collect();
                println!("   centreline u/U along the pipe: {}", line.join(" "));
            }
            // dp/dx from the mean pressure over cross-section slabs at 0.5 L and 0.9 L.
            let slab = |x: f64| fields.box_stats(Vec3::new(x - cell, -half, -half), Vec3::new(x + cell, half, half)).map(|s| s.2);
            let (x1, x2) = if case.blasius { (0.6, 0.9) } else { (0.5, 0.9) };
            let (p1, p2) = (slab(x1 * len), slab(x2 * len));
            let dpdx = match (p1, p2) {
                (Some(a), Some(b)) => (a - b) / ((x2 - x1) * len * 1e-3),
                _ => f64::NAN,
            };
            let exact_dpdx = if case.blasius {
                // Darcy f = 0.3164 Re^-¼; dp/dx = f ρ U² / (2 D), at the Reynolds number of the flow there.
                let re_here = u * case.size * 1e-3 / AIR.viscosity;
                0.3164 * re_here.powf(-0.25) * AIR.density * u * u / (2.0 * case.size * 1e-3)
            } else {
                case.gradient * AIR.viscosity * AIR.density * u / (case.size * 1e-3).powi(2)
            };
            println!(
                "{:7} {n:>7}  {:>7.2}  {:>6}  {:>7.1}  {:>6.4}  {:>+7.2}%  {:>6.3} ({:.3})  {:>+7.2}%  {:>+8.2}%  {:>8.2}%  {:?}",
                case.name,
                total as f64 / 1e6,
                run.steps,
                secs,
                q7 / q3,
                100.0 * flow_err,
                peak / u,
                case.peak,
                100.0 * (peak / u / case.peak - 1.0),
                100.0 * (dpdx / exact_dpdx - 1.0),
                100.0 * (err2 / ref2).sqrt(),
                state.map(|s| format!("{s:?}")).unwrap_or_else(|e| format!("{e}")),
            );
        }
    }
}

/// The flow along +x (m³/s of air at the reference density) through the layer of cells at `x`
/// (mm), over the fluid cells within `r` (mm, plus a cell) of the axis: the lattice's own mass
/// flux, cell by cell, ρu over ρ₀. The lattice carries pressure as density, so air is a little
/// thinner downstream and moves a little faster there; u alone would read the same mass as more.
fn layer_flow(f: &solidcraft_sim::Fields, units: &solidcraft_sim::Units, x: f64, r: f64) -> f64 {
    let i = ((x - f.origin.x) / f.cell).floor() as usize;
    let [nx, ny, nz] = f.dims;
    if i >= nx {
        return f64::NAN;
    }
    let da = (f.cell * 1e-3).powi(2);
    let mut q = 0.0;
    for k in 0..nz {
        for j in 0..ny {
            let (y, z) = (f.origin.y + (j as f64 + 0.5) * f.cell, f.origin.z + (k as f64 + 0.5) * f.cell);
            if y.hypot(z) > r + f.cell {
                continue;
            }
            let c = i + nx * (j + ny * k);
            if f.fluid.get(c).copied().unwrap_or(false) {
                q += units.lattice_density(f64::from(f.pressure[c])) * f64::from(f.velocity[c][0]) * da;
            }
        }
    }
    q
}

/// The fully developed laminar profile of a square duct (half-side 1, coordinates −1..1) over
/// the mean speed, from the series solution (Shah & London 1978).
fn square_profile(y: f64, z: f64) -> f64 {
    static MEAN: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    let mean = *MEAN.get_or_init(|| {
        let n = 200;
        let mut s = 0.0;
        for i in 0..n {
            for j in 0..n {
                s += square_series(-1.0 + (i as f64 + 0.5) * 2.0 / n as f64, -1.0 + (j as f64 + 0.5) * 2.0 / n as f64);
            }
        }
        s / (n * n) as f64
    });
    square_series(y, z) / mean
}

/// The square duct's series, up to a constant: Σ (−1)^i / k³ (1 − cosh(kπz/2)/cosh(kπ/2)) cos(kπy/2).
fn square_series(y: f64, z: f64) -> f64 {
    use std::f64::consts::PI;
    (0..60)
        .map(|i| {
            let k = (2 * i + 1) as f64;
            let b = k * PI / 2.0;
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            s / k.powi(3) * (1.0 - (b * z).cosh() / b.cosh()) * (b * y).cos()
        })
        .sum()
}
