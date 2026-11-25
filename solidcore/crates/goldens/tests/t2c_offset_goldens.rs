//! PROBE (team2 fable, not for merge): `--unit surface.offset_surface --goldens` as a test, since
//! rbq refuses `cargo run`. Prints the cases the #13610 draft moves.
#![allow(clippy::unwrap_used, clippy::print_stdout)]
use goldens::case::repo_root;
use goldens::{golden, registry, CaseContext, RunOptions};

#[test]
fn t2c_offset_surface_goldens() {
    let ctx = CaseContext { root: repo_root(), seeds: 16 };
    for id in ["surface.offset_surface"] {
        let reg = registry();
        let unit = reg.iter().find(|u| u.id() == id).unwrap();
        let dir = golden::default_dir();
        let old = golden::read(&dir, id).unwrap();
        let known = golden::known_failing(&dir, id).unwrap();
        let res = golden::check_unit(unit.as_ref(), &ctx, &RunOptions::default(), &old, &known, false);
        println!(
            "T2C {id} harness_error={:?} cases={} changed={} added={} removed={} broken={} known={}",
            res.harness_error,
            res.cases(),
            res.changed.len(),
            res.added.len(),
            res.removed.len(),
            res.broken.len(),
            res.known.len()
        );
        for (name, c) in &res.changed {
            println!("T2C changed {name} count={}", c.count);
            for d in &c.details {
                println!("T2C    {d}");
            }
        }
        for (name, why) in res.broken.iter().take(10) {
            println!("T2C broken {name}: {why}");
        }
        println!("T2C added {:?} removed {:?}", res.added, res.removed);
    }
}
