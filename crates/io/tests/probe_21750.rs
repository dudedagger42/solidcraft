//! Probe for #21750 (scan round 12): STEP import of a winding strip that never returns.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
    fn pthread_self() -> u64;
    fn pthread_kill(t: u64, sig: i32) -> i32;
}
static WORKER: AtomicU64 = AtomicU64::new(0);
extern "C" fn dump(_: i32) {
    eprintln!("---- worker stack ----\n{}", std::backtrace::Backtrace::force_capture());
}

fn run(name: &'static str, secs: u64) {
    let path = format!("{}/tests/probe_21750/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let t0 = Instant::now();
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        WORKER.store(unsafe { pthread_self() }, Ordering::SeqCst);
        let r = solidcraft_io::step_import_feature(&bytes, name).map(|f| format!("{:?}", f.warnings)).map_err(|e| e.to_string());
        let _ = tx.send(r);
    }).unwrap();
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(r) => eprintln!("{name}: {:.2}s {r:?}", t0.elapsed().as_secs_f64()),
        Err(_) => {
            eprintln!("{name}: NO RETURN after {secs}s");
            unsafe { signal(10, dump as usize) };
            for _ in 0..3 {
                unsafe { pthread_kill(WORKER.load(Ordering::SeqCst), 10) };
                std::thread::sleep(Duration::from_secs(3));
            }
            std::process::exit(0);
        }
    }
}

#[test]
fn probe_21750() {
    run("21-step-winding-strip-control.step", 120);
    run("21-step-winding-strip-control-b.step", 120);
    run("21-step-winding-strip-hang.step", 20);
}
