//! Probe for #21752 (scan round 12): a plane record whose y axis is skewed from x by ~1e-14.
use solidcore::eval::Surface as Record;
use solidcore::kernel::Kernel;
use solidcore::surface::from_parameters::from_parameters;

#[test]
fn probe_21752() {
    let k = Kernel::default();
    let (s, c) = 15f64.to_radians().sin_cos();
    for skew in [0.0, 1e-16, 1e-15, 1.36e-14, 1e-13, 1e-12, 1e-11] {
        let x = [c, s, 0.0];
        let y = [skew * c, skew * s, 1.0];
        let rec = Record::Plane { origin: [10010.0, 9997.58, 10.0], u_axis: x, v_axis: y };
        let r = from_parameters(&k, &rec, [[-3.0, 4.0], [-1.0, 2.0]]).map(|_| ());
        eprintln!("skew {skew:e}: {r:?}");
    }
}
