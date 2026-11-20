//! Scratch (#18618, not for the PR): bless `blend.fillet_executive_p3`'s
//! `sphere_diff_3cylinders` rows with the rig into a copy of `expected/`, and write the
//! cases' input and result bodies as `.smb` for `solidcore-diag`.
#![allow(clippy::all, clippy::pedantic, dead_code, unused_qualifications)]

#[path = "../examples/case_smb.rs"]
mod case_smb;

use std::path::Path;
use std::process::Command;

const OUT: &str = "/tmp/s18618";
const UNIT: &str = "blend.fillet_executive_p3";
const CASES: [&str; 7] = [
    "stubs4-delete-edge-sphere_diff_3cylinders-e1-r0.1",
    "stubs4-delete-edge-sphere_diff_3cylinders-e1-r0.5",
    "stubs4-delete-edge-sphere_diff_3cylinders-e2-r0.1",
    "stubs4-delete-edge-sphere_diff_3cylinders-e4-r0.1",
    "stubs4-delete-edge-sphere_diff_3cylinders-e4-r0.5",
    "stubs4-delete-edge-sphere_diff_3cylinders-e5-r0.1",
    "stubs4-delete-edge-sphere_diff_3cylinders-e5-r0.5",
];

fn rig(args: &[&str]) {
    let out = Command::new(env!("CARGO_BIN_EXE_goldens"))
        .args(args)
        .output()
        .unwrap();
    println!("RIG {args:?} -> {:?}", out.status);
    println!("{}", String::from_utf8_lossy(&out.stdout));
    println!("{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn scratch_18618_bless() {
    let dir = Path::new(OUT).join("expected");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("expected");
    for e in std::fs::read_dir(&here).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            std::fs::copy(&p, dir.join(p.file_name().unwrap())).unwrap();
        }
    }
    let d = dir.to_str().unwrap();
    rig(&["--unit", UNIT, "--goldens", "--goldens-dir", d]);
    rig(&["--unit", UNIT, "--filter", "sphere_diff_3cylinders", "--bless", "--goldens-dir", d]);
    rig(&["--unit", UNIT, "--goldens", "--goldens-dir", d]);
    let file = format!("{UNIT}.jsonl");
    let old = std::fs::read_to_string(here.join(&file)).unwrap();
    let new = std::fs::read_to_string(dir.join(&file)).unwrap();
    let olds: std::collections::BTreeSet<&str> = old.lines().collect();
    for l in new.lines() {
        if !olds.contains(l) {
            println!("NEWLINE {l}");
        }
    }
    for l in old.lines() {
        if !new.lines().any(|n| n == l) {
            println!("OLDLINE {l}");
        }
    }
}

#[test]
fn scratch_18618_smb() {
    std::fs::create_dir_all(OUT).unwrap();
    for case in CASES {
        let short = case.trim_start_matches("stubs4-delete-edge-sphere_diff_3cylinders-");
        let edge = format!("{OUT}/{short}.edge");
        std::env::set_var("S18618_EDGE", &edge);
        let out = format!("{OUT}/{short}.smb");
        let r = case_smb::run(&[UNIT.to_string(), case.to_string(), out]);
        println!("SMB {case}: {r:?} edge {:?}", std::fs::read_to_string(&edge).ok());
    }
}
