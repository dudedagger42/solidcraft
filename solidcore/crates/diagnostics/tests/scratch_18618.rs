//! Scratch (#18618, not for the PR): draws the `.smb` bodies `goldens`' scratch test wrote
//! to /tmp/s18618 and prints each picture as base64 with its report.
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

fn draw(smb: &str, tag: &str, extra: &[&str]) {
    for (vi, view) in VIEWS.iter().enumerate() {
        let dir = format!("{OUT}/pics/{tag}-v{vi}");
        let mut args = vec![smb, "--out", &dir, "--view", view, "--size", "1000x750", "--edges-above", "1"];
        args.extend_from_slice(extra);
        let out = Command::new(env!("CARGO_BIN_EXE_solidcore-diag"))
            .args(&args)
            .output()
            .unwrap();
        println!("DIAG {tag} v{vi} {view} {:?} {}", out.status, String::from_utf8_lossy(&out.stderr));
        let stem = std::path::Path::new(smb).file_stem().unwrap().to_string_lossy().into_owned();
        if let Ok(png) = std::fs::read(format!("{dir}/{stem}.png")) {
            println!("PNG {tag}-v{vi} {}", b64(&png));
        }
        if vi == 0 {
            if let Ok(j) = std::fs::read_to_string(format!("{dir}/{stem}.json")) {
                println!("JSON {tag} {}", j.replace('\n', " "));
            }
        }
    }
}

#[test]
fn scratch_18618_draw() {
    for short in ["e1-r0.1", "e2-r0.1", "e5-r0.1", "e5-r0.5"] {
        let edge = std::fs::read_to_string(format!("{OUT}/{short}.edge")).unwrap_or_default();
        println!("EDGE {short} {edge}");
        draw(&format!("{OUT}/{short}.smb"), &format!("{short}-before"), &["--mark-edges", edge.trim()]);
        let result = format!("{OUT}/{short}.2.smb");
        if std::path::Path::new(&result).is_file() {
            draw(&result, &format!("{short}-after"), &[]);
        }
    }
}
