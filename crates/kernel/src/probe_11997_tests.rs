//! Probe for #11997 (not for a PR).
use crate::step_in::step_import;
use crate::{BoolOp, boolean, validation::check_kept};

#[test]
fn probe_11997() {
    let text = std::fs::read_to_string("B:/Claude/repro-m00028/CountersinkMask.step").unwrap();
    let imp = step_import(&text).unwrap();
    eprintln!("warnings: {:?}", imp.warnings);
    for (i, b) in imp.bodies.iter().enumerate() {
        eprintln!("{i}: {:?} closed {} check {:?}", b.name, b.closed, check_kept(&b.body).err());
    }
    {
        let m = imp.bodies.iter().find(|b| b.name == "Mask-1").unwrap();
        let r = m.body.record();
        let et = r.edges.iter().map(|e| e.tolerance).fold(0.0f64, f64::max);
        let vt = r.vertices.iter().map(|v| v.tolerance).fold(0.0f64, f64::max);
        eprintln!("Mask-1 record tolerance {} max edge {} max vertex {}", r.tolerance, et, vt);
        let mut r2 = r.clone();
        r2.tolerance = et.max(vt) / 1000.0 * 1.01;
        let m2 = crate::Body::from_record(r2).unwrap();
        eprintln!("raised to {}: check {:?}", et.max(vt) / 1000.0 * 1.01, check_kept(&m2).err());
        let mut all = m2.clone();
        for rn in ["Body2", "Body3", "Body2 (3)", "Body2 (4)", "Body2 (5)", "Body2 (6)", "Body2 (7)", "Body2 (8)"] {
            let rb = imp.bodies.iter().find(|b| b.name == rn).unwrap().body.clone();
            let one = boolean(&m2, &rb, BoolOp::Union);
            let chk = match &one { Ok(Some(b)) => format!("{:?}", check_kept(b).err()), Ok(None) => "none".into(), Err(e) => format!("ERR {e}") };
            eprintln!("raised + {rn}: {chk}");
            match boolean(&all, &rb, BoolOp::Union) { Ok(Some(b)) => all = b, x => { eprintln!("all stops at {rn}: {:?}", x.err()); break } }
        }
        let dir = std::path::Path::new("B:/Claude/ct/opus-d/smb/11997");
        std::fs::create_dir_all(dir).unwrap();
        m.body.smlib().unwrap().write(&dir.join("mask_imported.smb"), false).unwrap();
        crate::native::smlib_body(&m2).unwrap().write(&dir.join("mask_raised.smb"), false).unwrap();
        for (rn, f) in [("Body2", "rib1"), ("Body2 (3)", "rib3"), ("Body2 (7)", "rib7")] {
            let rb = &imp.bodies.iter().find(|b| b.name == rn).unwrap().body;
            rb.smlib().unwrap().write(&dir.join(format!("{f}.smb")), false).unwrap();
        }
        eprintln!("all: check {:?} volume {:?}", check_kept(&all).err(), crate::measure(&all).map(|m| m.volume));
    }
    let find = |n: &str| imp.bodies.iter().find(|b| b.name == n).map(|b| b.body.clone());
    for (mask, ribs) in [("Mask-1", 0..8), ("Mask (1)", 8..16)] {
        let Some(m) = find(mask) else { eprintln!("no {mask}"); continue };
        for r in ribs {
            let names = ["Body2", "Body3", "Body2 (3)", "Body2 (4)", "Body2 (5)", "Body2 (6)", "Body2 (7)", "Body2 (8)", "Body2 (2)", "Body3 (2)", "Body2 (3) (2)", "Body2 (4) (2)", "Body2 (5) (2)", "Body2 (6) (2)", "Body2 (7) (2)", "Body2 (8) (2)"];
            let rn = names[r].to_string();
            let Some(rb) = find(&rn) else { eprintln!("no {rn}"); continue };
            let res = boolean(&m, &rb, BoolOp::Union);
            let chk = match &res { Ok(Some(b)) => format!("{:?}", check_kept(b).err()), Ok(None) => "none".into(), Err(e) => format!("ERR {e}") };
            eprintln!("{mask} + {rn}: {chk}");
        }
    }
}

#[test]
fn probe_11997_dome_gaps() {
    for dz in [2e-5, 1e-4, 2e-4, 5e-4] {
        let imp = crate::step_in::step_import(&crate::step_in::tests::dome_with_edges_raised_pub(dz)).unwrap();
        let b = &imp.bodies[0].body;
        let r = b.record();
        let et = r.edges.iter().map(|e| e.tolerance).fold(0.0f64, f64::max);
        let msg = match check_kept(b) { Ok(()) => "ok".to_string(), Err(e) => e.to_string().lines().take(3).collect::<Vec<_>>().join(" | ") };
        eprintln!("dome dz {dz}: brep {} max edge {et} check {msg}", r.tolerance);
    }
}
