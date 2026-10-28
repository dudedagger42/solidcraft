//! The flow solver's capacity and speed on this machine's GPU (a probe, run by hand):
//! `cargo test -p solidcraft-gpu --release --test flow_baseline -- --ignored --nocapture`.

use solidcraft_geom::Vec3;
use solidcraft_gpu::flow::{FlowGpu, MAX_CELLS};
use solidcraft_sim::{Cell, Grid, Lattice};

#[test]
#[ignore = "a probe of this machine's GPU"]
fn flow_capacity_and_speed() {
    let gpu = match FlowGpu::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    println!("adapter: {}", gpu.adapter_name());
    println!("largest population buffer: {:.2} GiB", gpu.max_population_bytes() as f64 / (1u64 << 30) as f64);
    println!("most cells (cap {MAX_CELLS}): {}", gpu.max_cells(MAX_CELLS));
    for millions in [1usize, 4, 8, 16, 24] {
        // A periodic box of still air with a gentle body force: every cell fluid, the solver's
        // full per-cell work with nothing to skip.
        let n = ((millions * 1_000_000) as f64).cbrt().round() as usize;
        let grid = Grid { dims: [n, n, n], origin: Vec3::new(0.0, 0.0, 0.0), spacing: 1.0, cells: vec![Cell::Fluid; n * n * n] };
        let lattice = Lattice::new(&grid, 0.6, [1e-6, 0.0, 0.0], [true, true, true]).unwrap();
        let mut gl = gpu.upload(&lattice).unwrap();
        gl.run(20).unwrap();
        let steps = 200u32;
        let t = std::time::Instant::now();
        gl.run(steps).unwrap();
        let s = t.elapsed().as_secs_f64();
        let cells = (n * n * n) as f64;
        println!(
            "{:>5.1} M cells ({n}³): {:.2} ms/step, {:.0} million cell updates/s",
            cells / 1e6,
            1e3 * s / f64::from(steps),
            cells * f64::from(steps) / s / 1e6
        );
    }
}
