//! Scratch (#18618, not for the PR): close-ups of the selected edge before and of the round
//! after, from the `.smb` bodies `goldens`' scratch test wrote to /tmp/s18618.
#![allow(clippy::all, clippy::pedantic)]

use std::process::Command;

const OUT: &str = "/tmp/s18618";
const VIEWS: [&str; 8] = [
    "-35,30", "55,30", "145,30", "235,30", "-35,-30", "55,-30", "145,-30", "235,-30",
];

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn diag(smb: &str, tag: &str, args: &[&str]) -> String {
    let dir = format!("{OUT}/zoom/{tag}");
    let mut all = vec![smb, "--out", &dir, "--size", "900x900", "--edges-above", "1"];
    all.extend_from_slice(args);
    let out = Command::new(env!("CARGO_BIN_EXE_solidcore-diag")).args(&all).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    println!("DIAG {tag} {:?} {}", out.status, String::from_utf8_lossy(&out.stderr));
    let stem = std::path::Path::new(smb).file_stem().unwrap().to_string_lossy().into_owned();
    if let Ok(png) = std::fs::read(format!("{dir}/{stem}.png")) {
        println!("PNG {tag} {}", b64(&png));
    }
    stdout
}

#[test]
fn scratch_18618_zoom() {
    for (short, r) in [("e2-r0.1", "0.5"), ("e5-r0.1", "0.5"), ("e5-r0.5", "2")] {
        let edge = std::fs::read_to_string(format!("{OUT}/{short}.edge")).unwrap();
        let edge = edge.trim();
        for (vi, view) in VIEWS.iter().enumerate() {
            let we = format!("{edge},{r}");
            let out = diag(
                &format!("{OUT}/{short}.smb"),
                &format!("{short}-before-z{vi}"),
                &["--view", view, "--mark-edges", edge, "--window-edge", &we],
            );
            let Some(w) = out.lines().find_map(|l| l.strip_prefix("WINDOW ")) else {
                continue;
            };
            let win = format!("{w},{r}");
            diag(
                &format!("{OUT}/{short}.2.smb"),
                &format!("{short}-after-z{vi}"),
                &["--view", view, "--highlight", "9", "--window", &win],
            );
        }
    }
}
