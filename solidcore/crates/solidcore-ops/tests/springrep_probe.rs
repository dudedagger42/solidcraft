//! Scratch probe (not for a PR): springrep's boolean cases against solidcore-ops.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::print_stdout)]

use std::f64::consts::PI;

use solidcore_ops::boolean::boolean;
use solidcore_ops::{make_box, make_cone, make_cylinder, make_sphere, session, Body, BooleanOp, CheckLevel};

fn all_fixes() {
    session::set_thread_fixes(solidcore::kernel::UpstreamFixes::NAMES.iter().copied()).unwrap();
}

type M = [[f64; 4]; 4];

/// Rows are the images of x, y, z; row 3 the translation.
fn frame(x: [f64; 3], y: [f64; 3], z: [f64; 3], t: [f64; 3]) -> M {
    [
        [x[0], x[1], x[2], 0.0],
        [y[0], y[1], y[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

fn norm(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Rotation by `t` about the axis through `c` along `k` (Rodrigues), as row-vector rows.
fn rotation(c: [f64; 3], k: [f64; 3], t: f64) -> M {
    let k = norm(k);
    let (s, co) = (t.sin(), t.cos());
    let v = 1.0 - co;
    // column-vector matrix R
    let r = [
        [co + k[0] * k[0] * v, k[0] * k[1] * v - k[2] * s, k[0] * k[2] * v + k[1] * s],
        [k[1] * k[0] * v + k[2] * s, co + k[1] * k[1] * v, k[1] * k[2] * v - k[0] * s],
        [k[2] * k[0] * v - k[1] * s, k[2] * k[1] * v + k[0] * s, co + k[2] * k[2] * v],
    ];
    // p' = R (p - c) + c; row form: rows are R's columns
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[j][i];
        }
    }
    for j in 0..3 {
        let rc: f64 = (0..3).map(|i| r[j][i] * c[i]).sum();
        m[3][j] = c[j] - rc;
    }
    m[3][3] = 1.0;
    m
}

/// A cylinder of radius `r`, `len` long, from `base` along the unit `z`.
fn rod(base: [f64; 3], z: [f64; 3], r: f64, len: f64) -> Body {
    let c = make_cylinder([0.0; 3], r, len).unwrap();
    let z = norm(z);
    let helper = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let y = norm(cross(z, helper));
    let x = cross(y, z);
    c.transform(&frame(x, y, z, base)).unwrap();
    c
}

fn bx(lo: [f64; 3], hi: [f64; 3]) -> Body {
    make_box(lo, [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]).unwrap()
}

/// Runs `op`, prints what came back, and returns (volume, valid, solids) or the error.
fn report(name: &str, a: Body, b: Body, op: BooleanOp) -> Result<Option<(f64, bool, usize)>, String> {
    let r = boolean(a, b, op);
    let out = match r {
        Err(e) => Err(format!("{e:?}")),
        Ok(None) => Ok(None),
        Ok(Some(body)) => {
            let v = body.volume(1e-9).map_err(|e| format!("volume {e:?}"));
            let chk = body.check(CheckLevel::All).map_err(|e| format!("check {e:?}"));
            let solids = body.solid_count().map_err(|e| format!("count {e:?}"));
            match (v, chk, solids) {
                (Ok(v), Ok(c), Ok(s)) => {
                    if !c.is_valid() {
                        println!("PROBE {name}: check text: {}", c.text.lines().take(4).collect::<Vec<_>>().join(" | "));
                    }
                    Ok(Some((v, c.is_valid(), s)))
                }
                (v, c, s) => Err(format!("{v:?} {:?} {s:?}", c.map(|c| c.is_valid()))),
            }
        }
    };
    println!("PROBE {name}: {out:?}");
    out
}

fn expect(name: &str, got: Result<Option<(f64, bool, usize)>, String>, want: f64, rel: f64) {
    let (v, valid, _) = got.unwrap_or_else(|e| panic!("{name}: error {e}")).unwrap_or_else(|| panic!("{name}: empty"));
    assert!(valid, "{name}: invalid");
    assert!((v - want).abs() <= rel * want.abs().max(1e-30), "{name}: volume {v} vs {want}");
}

fn ball_v(r: f64) -> f64 {
    4.0 / 3.0 * PI * r * r * r
}

#[test]
fn p01_balls_touching_at_a_point_unite() {
    all_fixes();
    let a = make_sphere([5.0, 0.0, 0.0], 5.0).unwrap();
    let b = make_sphere([15.0, 0.0, 0.0], 5.0).unwrap();
    expect("p01", report("p01", a, b, BooleanOp::Union), 2.0 * ball_v(5.0), 1e-7);
}

#[test]
fn p02_ball_on_a_box_face_unites_at_a_point() {
    all_fixes();
    let a = bx([0.0; 3], [10.0; 3]);
    let s = make_sphere([5.0, 5.0, 13.0], 3.0).unwrap();
    expect("p02", report("p02", a, s, BooleanOp::Union), 1000.0 + ball_v(3.0), 1e-7);
}

#[test]
fn p03_cone_apex_on_a_box_face_unites() {
    all_fixes();
    // base r3 at z = -6, apex at the box's floor z = 0
    let c = make_cone([0.0, 0.0, -6.0], 3.0, 0.0, 6.0).unwrap();
    let a = bx([-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
    expect("p03", report("p03", a, c, BooleanOp::Union), 4000.0 + PI * 9.0 * 6.0 / 3.0, 1e-7);
}

#[test]
fn p04_cones_apex_to_apex_unite() {
    all_fixes();
    let a = make_cone([0.0, 0.0, -6.0], 3.0, 0.0, 6.0).unwrap();
    let b = make_cone([0.0, 0.0, -5.0], 2.0, 0.0, 5.0).unwrap();
    b.transform(&frame([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [0.0; 3])).unwrap();
    let want = PI * 9.0 * 6.0 / 3.0 + PI * 4.0 * 5.0 / 3.0;
    expect("p04", report("p04", a, b, BooleanOp::Union), want, 1e-7);
}

#[test]
fn p05_cube_less_a_ball_touching_its_six_faces() {
    all_fixes();
    let a = bx([0.0; 3], [10.0; 3]);
    let s = make_sphere([5.0; 3], 5.0).unwrap();
    expect("p05", report("p05", a, s, BooleanOp::Subtract), 1000.0 - ball_v(5.0), 1e-7);
}

#[test]
fn p06_box_less_an_inscribed_cylinder_is_four_corners() {
    all_fixes();
    let a = bx([0.0; 3], [10.0, 10.0, 8.0]);
    let c = make_cylinder([5.0, 5.0, -1.0], 5.0, 10.0).unwrap();
    expect("p06", report("p06", a, c, BooleanOp::Subtract), 800.0 - PI * 25.0 * 8.0, 1e-7);
}

#[test]
fn p07_block_less_a_bore_flush_to_its_top() {
    all_fixes();
    let a = bx([0.0; 3], [12.0, 10.0, 8.0]);
    let c = rod([-1.0, 5.0, 5.0], [1.0, 0.0, 0.0], 3.0, 14.0);
    expect("p07", report("p07", a, c, BooleanOp::Subtract), 960.0 - PI * 9.0 * 12.0, 1e-7);
}

#[test]
fn p08_rod_inside_a_box_against_a_wall_leaves_a_void() {
    all_fixes();
    let a = bx([0.0; 3], [10.0; 3]);
    let c = make_cylinder([2.0, 5.0, 2.0], 2.0, 6.0).unwrap();
    expect("p08", report("p08", a, c, BooleanOp::Subtract), 1000.0 - PI * 4.0 * 6.0, 1e-7);
}

#[test]
fn p09_relief_bore_leaves_thin_annuli() {
    all_fixes();
    let body = make_cylinder([0.0; 3], 52.5, 45.0).unwrap();
    let bore = make_cylinder([0.0, 0.0, -1.0], 45.0, 47.0).unwrap();
    let relief = make_cylinder([0.0, 0.0, 10.0], 45.05, 24.0).unwrap();
    let tube = boolean(body, bore, BooleanOp::Subtract).unwrap().unwrap();
    let want = PI * (52.5f64.powi(2) - 45.0f64.powi(2)) * 45.0 - PI * (45.05f64.powi(2) - 2025.0) * 24.0;
    expect("p09", report("p09", tube, relief, BooleanOp::Subtract), want, 1e-7);
}

#[test]
fn p10_faces_just_off_coplanar_unite() {
    all_fixes();
    for off in [1.5e-7, 2e-5, 1e-4] {
        let a = bx([0.0; 3], [20.0, 20.0, 10.0]);
        let b = bx([20.0, 0.0, off], [40.0, 20.0, 10.0 + off]);
        expect(&format!("p10 {off}"), report(&format!("p10 {off}"), a, b, BooleanOp::Union), 8000.0, 1e-7);
    }
}

#[test]
fn p11_micrometre_cylinders_crossing_aslant() {
    all_fixes();
    let a = || rod([0.0, 0.0, -0.015], [0.0, 0.0, 1.0], 0.007173807691792167, 0.03);
    let b = || {
        let z = [0.2734748768191249, -0.9104945125904419, -0.3101793582614065];
        let x = [0.9618791461242749, 0.25886555049919935, 0.08818806617667722];
        let c = make_cylinder([0.0; 3], 0.0053024717958101335, 0.03).unwrap();
        c.transform(&frame(x, cross(z, x), z, [-0.002532118538179013, 0.0072643840011615635, 0.008785851568869818]))
            .unwrap();
        c
    };
    let va = PI * 0.007173807691792167f64.powi(2) * 0.03;
    let vb = PI * 0.0053024717958101335f64.powi(2) * 0.03;
    let u = report("p11 union", a(), b(), BooleanOp::Union);
    let i = report("p11 common", a(), b(), BooleanOp::Intersect);
    let d = report("p11 a-b", a(), b(), BooleanOp::Subtract);
    let e = report("p11 b-a", b(), a(), BooleanOp::Subtract);
    let vi = i.unwrap().unwrap().0;
    expect("p11 union", u, va + vb - vi, 1e-6);
    expect("p11 a-b", d, va - vi, 1e-6);
    expect("p11 b-a", e, vb - vi, 1e-6);
}

#[test]
fn p12_plane_a_millionth_off_a_cube_top_cuts_a_wedge() {
    all_fixes();
    for s in [1e-6f64, 1e-5, 1e-4] {
        let cube = bx([-5.0, -5.0, -10.0], [5.0, 5.0, 0.0]);
        let above = bx([-20.0, -20.0, 0.0], [20.0, 20.0, 20.0]);
        // turn about x so +z goes to (0, -s, 1)
        above.transform(&rotation([0.0; 3], [1.0, 0.0, 0.0], s.atan())).unwrap();
        let wedge = 10.0 * 5.0 * 5.0 * s / 2.0;
        let got = report(&format!("p12 common {s}"), cube.copy().unwrap(), above.copy().unwrap(), BooleanOp::Intersect);
        let cut = report(&format!("p12 cut {s}"), cube, above, BooleanOp::Subtract);
        println!("PROBE p12 {s}: wedge {wedge} common {got:?} cut {cut:?} want cut {}", 1000.0 - wedge);
    }
}

#[test]
fn p13_flush_faces_crossing_at_a_hair() {
    all_fixes();
    for (turn, off) in [(1.56e-5f64, 0.0f64), (1.56e-5, -1e-5), (2.4e-5, 0.0), (2.4e-5, -1e-5), (5e-5, -2e-5), (1e-4, -5e-5)] {
        let b = || {
            let b = bx([2.0, 1.0, 1.0], [4.0, 3.0, 3.0]);
            b.transform(&rotation([4.0, 2.2, 1.9], [0.0, 0.6, 0.8], turn)).unwrap();
            b.transform(&frame([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [off, 0.0, 0.0])).unwrap();
            b
        };
        let a = || bx([0.0; 3], [4.0; 3]);
        let tag = format!("p13 {turn} {off}");
        let common = report(&format!("{tag} common"), a(), b(), BooleanOp::Intersect);
        let joined = report(&format!("{tag} union"), a(), b(), BooleanOp::Union);
        let cut = report(&format!("{tag} cut"), a(), b(), BooleanOp::Subtract);
        let c = common.unwrap().unwrap();
        let j = joined.unwrap().unwrap();
        let k = cut.unwrap().unwrap();
        println!("PROBE {tag}: valid {} {} {} | common+union-72 = {:e} | cut-(64-common) = {:e}", c.1, j.1, k.1, c.0 + j.0 - 72.0, k.0 - (64.0 - c.0));
    }
}

#[test]
fn p14_equal_boxes_a_millionth_apart_unite_as_one() {
    all_fixes();
    let a = || bx([0.0; 3], [2.0, 3.0, 5.0]);
    let b = || {
        let b = bx([0.0; 3], [2.0, 3.0, 5.0]);
        b.transform(&rotation([1.0, 1.5, 2.5], [0.6, 0.48, 0.64], 1e-6)).unwrap();
        b.transform(&frame([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1e-6, -7e-7, 5e-7])).unwrap();
        b
    };
    let gap_area = 1e-5 * 62.0;
    for op in [BooleanOp::Union, BooleanOp::Intersect] {
        let r = report(&format!("p14 {op:?}"), a(), b(), op).unwrap().unwrap();
        assert!(r.1, "p14 {op:?} invalid");
        assert!((r.0 - 30.0).abs() <= gap_area, "p14 {op:?}: {}", r.0);
    }
    let cut = report("p14 cut", a(), b(), BooleanOp::Subtract).unwrap();
    if let Some(c) = cut {
        assert!(c.1 && c.0 <= gap_area, "p14 cut {c:?}");
    }
}

#[test]
fn p15_plane_a_hair_off_parallel_to_a_holes_axis() {
    all_fixes();
    let plate = bx([0.0; 3], [10.0, 10.0, 4.0]);
    let hole = rod([5.0, 5.0, -1.0], [0.0, 5.7e-10, 1.0], 1.6, 6.0);
    let holed = boolean(plate, hole, BooleanOp::Subtract).unwrap().unwrap();
    let keep = bx([-1.0, -1.0, -1.0], [11.0, 5.5, 5.0]);
    // the segment of the circle beyond the chord 0.5 off the centre
    let (r, c) = (1.6f64, 0.5f64);
    let seg = r * r * (c / r).acos() - c * (r * r - c * c).sqrt();
    let want = (55.0 - (PI * r * r - seg)) * 4.0;
    expect("p15", report("p15", holed, keep, BooleanOp::Intersect), want, 1e-6);
}

#[test]
fn p16_touching_balls_less_a_slab_through_the_contact_part() {
    all_fixes();
    let a = make_sphere([5.0, 0.0, 0.0], 5.0).unwrap();
    let b = make_sphere([15.0, 0.0, 0.0], 5.0).unwrap();
    let u = boolean(a, b, BooleanOp::Union).unwrap().unwrap();
    let slab = bx([9.0, -6.0, -6.0], [11.0, 6.0, 6.0]);
    let cap = PI * (3.0 * 5.0 - 1.0) / 3.0;
    let got = report("p16", u, slab, BooleanOp::Subtract);
    if let Ok(Some((_, _, s))) = &got {
        assert_eq!(*s, 2, "p16 solids");
    }
    expect("p16", got, 2.0 * (ball_v(5.0) - cap), 1e-7);
}

#[test]
fn p17_box_less_inscribed_cylinder_less_slot_through_one_contact() {
    all_fixes();
    let a = bx([0.0; 3], [10.0, 10.0, 8.0]);
    let c = make_cylinder([5.0, 5.0, -1.0], 5.0, 10.0).unwrap();
    let ring = boolean(a, c, BooleanOp::Subtract).unwrap().unwrap();
    let slot = bx([9.0, 4.0, -1.0], [11.0, 6.0, 9.0]);
    // the slot takes 1 x 2 x 8 less the bore's part inside it from the ring
    let ring_v = 800.0 - PI * 25.0 * 8.0;
    let got = report("p17", ring, slot, BooleanOp::Subtract);
    let (v, valid, _) = got.unwrap().unwrap();
    assert!(valid && v < ring_v && v > ring_v - 16.0, "p17 {v}");
}

fn ray(name: &str, b: &Body, o: [f64; 3], d: [f64; 3]) -> Option<f64> {
    let (_, order) = b.to_record().unwrap();
    let r = b.fire_ray(&order, o, d);
    println!("PROBE {name}: {r:?}");
    r.unwrap().map(|h| h.distance)
}

#[test]
fn r01_rays() {
    all_fixes();
    // from inside a cylinder a kilometre across, out through its side
    let big = make_cylinder([0.0; 3], 1.0e6, 10.0).unwrap();
    let d = ray("r01 km cylinder", &big, [0.0, 0.0, 5.0], [1.0, 0.0, 0.0]);
    // parallel to a 45 degree cone's generators: meets the side at t = 13.4 (unit 13.4 sqrt 2)
    let cone = make_cone([0.0; 3], 10.0, 0.0, 10.0).unwrap();
    let c = ray("r01 cone generator", &cone, [-15.0, 3.0, 20.0], [1.0, 0.0, -1.0]);
    let torus = solidcore_ops::make_torus([0.0; 3], 10.0, 3.0).unwrap();
    let t = ray("r01 torus diameter", &torus, [-20.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let a = ray("r01 torus axis", &torus, [0.0, 0.0, 20.0], [0.0, 0.0, -1.0]);
    // a lens micrometres across: two balls of radius 1e-3 whose centres are 1.6e-3 apart
    let lens = boolean(make_sphere([0.0; 3], 1e-3).unwrap(), make_sphere([1.6e-3, 0.0, 0.0], 1e-3).unwrap(), BooleanOp::Intersect)
        .unwrap()
        .unwrap();
    let l = ray("r01 lens", &lens, [0.8e-3, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!((d.unwrap() - 1.0e6).abs() < 1e-3, "km {d:?}");
    assert!((c.unwrap() - 13.4 * 2f64.sqrt()).abs() < 1e-6, "cone {c:?}");
    assert!((t.unwrap() - 7.0).abs() < 1e-9, "torus {t:?}");
    assert!(a.is_none(), "axis {a:?}");
    assert!((l.unwrap() - 0.6e-3).abs() < 1e-9, "lens {l:?}");
}
