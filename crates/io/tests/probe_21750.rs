//! Probe for #21750 (scan round 12): STEP import of a winding strip that never returns.
use std::time::{Duration, Instant};

fn run(name: &'static str, secs: u64) {
    let path = format!("{}/tests/probe_21750/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let t0 = Instant::now();
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        let r = solidcraft_io::step_import_feature(&bytes, name).map(|f| format!("{:?}", f.warnings)).map_err(|e| e.to_string());
        let _ = tx.send(r);
    }).unwrap();
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(r) => eprintln!("{name}: {:.2}s {r:?}", t0.elapsed().as_secs_f64()),
        Err(_) => {
            eprintln!("{name}: NO RETURN after {secs}s");
            std::process::exit(0);
        }
    }
}

#[test]
fn probe_21750() {
    run("21-step-winding-strip-control.step", 120);
    run("21-step-winding-strip-control-b.step", 120);
    run("21-step-winding-strip-hang.step", 120);
}
