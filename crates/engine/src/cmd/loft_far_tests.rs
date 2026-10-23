//! Loft caps of sections far from the origin: one closed periodic fit-point spline per section,
//! thin, on tilted planes about 190 mm out.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

/// Forty points round a thin ellipse `a` × `b`, starting off its axes. A thin section has a small
/// area, so its true normal is short next to the fit's error.
fn ellipse(a: f64, b: f64) -> Vec<[f64; 2]> {
    (0..40)
        .map(|i| {
            let t = 0.3 + f64::from(i) * std::f64::consts::TAU / 40.0;
            [a * t.cos(), b * t.sin()]
        })
        .collect()
}

/// A loft's caps sit on the sections' planes. The cap plane was fitted about the sum of the
/// control points (truck divides by the number of wires, not of points), so a periodic spline
/// section about 190 mm from the origin got a tilted normal and the loft failed with "cap:
/// cannot attach a plane to a wire that is not on one plane". The same sections near the
/// origin lofted.
#[test]
fn a_loft_of_periodic_splines_far_from_the_origin_is_capped() {
    for (origin, name) in [([0.0, 0.0, 0.0], "near"), ([183.7, -30.8, -17.7], "far")] {
        let mut s = Session::default();
        let mut sections = Vec::new();
        for (k, (dx, a, b)) in [(0.0, 4.0, 0.4), (4.0, 4.4, 0.4), (8.0, 3.8, 0.4)].into_iter().enumerate() {
            let sketch = format!("S{k}");
            let o = [origin[0] + dx, origin[1], origin[2] + 0.5 * dx];
            run(
                &mut s,
                "sketch.create",
                json!({"plane": {"origin": o, "x_dir": [0.0, 0.9177, 0.3972], "y_dir": [-0.9764, -0.0857, 0.1981]}, "name": sketch}),
            );
            run(&mut s, "sketch.spline.fit_point", json!({"points": ellipse(a, b), "periodic": true}));
            run(&mut s, "sketch.finish", json!({}));
            sections.push(json!({"sketch": sketch}));
        }
        let r = s.execute("solid.loft", &json!({"sections": sections}));
        assert!(r.is_ok(), "{name}: {r:?}");
        let st = s.world_state();
        let body = &st.bodies[0].body;
        let m = solidcraft_kernel::measure(body).unwrap();
        assert!(m.volume > 10.0, "{name}: a closed solid, {} mm³", m.volume);
    }
}
