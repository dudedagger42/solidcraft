use solidcore_record::Vec3;

use crate::*;

/// A 60 × 40 × 25 block, with a Ø4 hole 10 deep 2 mm in from its −X side and a 4.8 × 4.8 notch
/// 12.5 deep in the middle of its +Y side, as asked for.
fn block(hole: bool, notch: bool) -> Body {
    let mut b = box_solid(Vec3::new(-30.0, -20.0, 0.0), Vec3::new(30.0, 20.0, 25.0)).unwrap();
    if hole {
        let c = cylinder(Vec3::new(-26.0, 0.0, 15.0), Vec3::Z, 2.0, 10.0).unwrap();
        b = boolean(&b, &c, BoolOp::Cut).unwrap().unwrap();
    }
    if notch {
        let n = box_solid(Vec3::new(-2.4, 17.6, 12.5), Vec3::new(2.4, 22.4, 25.0)).unwrap();
        b = boolean(&b, &n, BoolOp::Cut).unwrap().unwrap();
    }
    b
}

fn round_top(b: &Body, notch: bool) -> Result<Body> {
    let mut edges = vec![Vec3::new(0.0, -20.0, 25.0), Vec3::new(30.0, 0.0, 25.0), Vec3::new(-30.0, 0.0, 25.0)];
    if notch {
        edges.extend([
            Vec3::new(-15.0, 20.0, 25.0),
            Vec3::new(15.0, 20.0, 25.0),
            Vec3::new(0.0, 17.6, 25.0),
            Vec3::new(-2.4, 18.8, 25.0),
            Vec3::new(2.4, 18.8, 25.0),
        ]);
    } else {
        edges.push(Vec3::new(0.0, 20.0, 25.0));
    }
    fillet_removing(b, &edges, 2.8, FilletTurn::Mitre).map(|(b, _)| b)
}

fn volume(hole: bool, notch: bool) -> f64 {
    let out = round_top(&block(hole, notch), notch).unwrap_or_else(|e| panic!("hole {hole}, notch {notch}: {e}"));
    assert!(out.validity().is_empty(), "{:?}", out.validity());
    measure(&out).unwrap().volume
}

/// #22796: a 2.8 round of the top loop runs past the notch's 2.4 mm side edges and into the hole
/// beside the −X edge. Each builds alone, and the two are 20 mm apart, so with both the volume is
/// what each takes from the rounded block added up.
#[test]
fn a_round_past_a_notch_and_into_a_hole_builds() {
    let (neither, hole, notch) = (volume(false, false), volume(true, false), volume(false, true));
    let want = hole + notch - neither;
    let got = volume(true, true);
    assert!((got - want).abs() < 1e-6 * want, "{got} vs {want}");
}

#[test]
fn probe_22796_site() {
    solidcraft_smlib::failure::clear();
    let e = round_top(&block(true, true), true).unwrap_err();
    eprintln!("PROBE err={e}");
    eprintln!("PROBE last={:?}", solidcraft_smlib::failure::last());
    // also the notch with the hole's edge alone left out, and the hole with a single notch side
    for skip in 0..8 {
        let all = [Vec3::new(0.0, -20.0, 25.0), Vec3::new(30.0, 0.0, 25.0), Vec3::new(-30.0, 0.0, 25.0), Vec3::new(-15.0, 20.0, 25.0), Vec3::new(15.0, 20.0, 25.0), Vec3::new(0.0, 17.6, 25.0), Vec3::new(-2.4, 18.8, 25.0), Vec3::new(2.4, 18.8, 25.0)];
        let edges: Vec<Vec3> = all.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, p)| *p).collect();
        let r = fillet_removing(&block(true, true), &edges, 2.8, FilletTurn::Mitre);
        eprintln!("PROBE skip {skip}: {:?}", r.map(|(b, g)| (measure(&b).unwrap().volume, g.len())).map_err(|e| e.to_string()));
    }
    for r in [2.5, 2.6, 2.7, 2.79] {
        let res = fillet_removing(&block(true, true), &[Vec3::new(0.0, -20.0, 25.0), Vec3::new(30.0, 0.0, 25.0), Vec3::new(-30.0, 0.0, 25.0), Vec3::new(-15.0, 20.0, 25.0), Vec3::new(15.0, 20.0, 25.0), Vec3::new(0.0, 17.6, 25.0), Vec3::new(-2.4, 18.8, 25.0), Vec3::new(2.4, 18.8, 25.0)], r, FilletTurn::Mitre);
        eprintln!("PROBE r {r}: {:?}", res.map(|(b, g)| (measure(&b).unwrap().volume, g.len())).map_err(|e| e.to_string()));
        eprintln!("PROBE last={:?}", solidcraft_smlib::failure::last());
    }
}
