//! #12539: an R1 fillet on the edge where a 1 mm chamfer meets the side wall of a smooth convex
//! outline. Every edge of the chain borders a 1.414 mm chamfer face, so R1 needs 0.414 mm of it;
//! SolidCraft 0.30.1 builds it (233 477.45 mm³) and OpenCascade 7.9 gives 233 478.98 mm³.

use solidcore_record::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_kernel::{Body, chamfer, extrude, fillet, measure_mass, tangent_chain};
use std::f64::consts::{FRAC_PI_2, PI};

/// The reporter's sketch: three circles joined by a 6-point polyline into one convex outline,
/// extruded 45 mm. The last polyline segment is tangent to the right-hand circle; it meets the
/// big circle at one sharp corner.
fn outline_body() -> Body {
    let big = Vec2::new(-8.0, 0.0);
    let (left, right) = (Vec2::new(-22.975, -51.975), Vec2::new(7.0, -51.975));
    let f = Vec2::new(19.278756149952024, -42.515499621543675);
    // Where the line from f towards (-13.475, 0) leaves the big circle.
    let corner = Vec2::new(6.973467439037785, -26.54281254599963);
    let corner_angle = (corner.y - big.y).atan2(corner.x - big.x);
    let f_angle = (f.y - right.y).atan2(f.x - right.x);
    let segs = vec![
        Seg2::Line { a: Vec2::new(-38.475, 0.0), b: Vec2::new(-38.475, -51.975) },
        Seg2::Arc { center: left, radius: 15.5, start: PI, sweep: FRAC_PI_2 },
        Seg2::Line { a: Vec2::new(-22.975, -67.475), b: Vec2::new(7.0, -67.475) },
        Seg2::Arc { center: right, radius: 15.5, start: -FRAC_PI_2, sweep: f_angle + FRAC_PI_2 },
        Seg2::Line { a: f, b: corner },
        Seg2::Arc { center: big, radius: 30.475, start: corner_angle, sweep: PI - corner_angle },
    ];
    extrude(&Plane::XY, &[Region2 { outer: Loop2 { segs }, holes: vec![] }], 0.0, 45.0).unwrap().pop().unwrap()
}

#[test]
fn fillet_the_wall_side_edge_of_a_chamfer_round_a_smooth_outline() {
    let body = outline_body();
    // The features as the reporter's script makes them: each pick's tangent chain.
    let smooth = fillet(&body, &tangent_chain(&body, &[Vec3::new(6.973, -26.543, 22.5)]).unwrap(), 2.0).unwrap();
    let chamfered = chamfer(&smooth, &tangent_chain(&smooth, &[Vec3::new(-38.475, -25.0, 0.0)]).unwrap(), 1.0).unwrap();
    let before = measure_mass(&chamfered).unwrap().volume;
    // OpenCascade: 233 485.16 mm³ after the chamfer.
    assert!((before - 233_485.16).abs() < 0.05, "chamfered volume {before}");
    let wall_side = tangent_chain(&chamfered, &[Vec3::new(-38.475, -25.0, 1.0)]).unwrap();
    let made = fillet(&chamfered, &wall_side, 1.0).expect("the R1 fillet on the chamfer's wall-side edge builds");
    assert!(made.validity().is_empty(), "{:?}", made.validity());
    let removed = before - measure_mass(&made).unwrap().volume;
    // OpenCascade removes 233 485.16 - 233 478.98 = 6.18 mm³: the R1 round between the wall and
    // a 45° chamfer takes r²(tan 22.5° - π/8) = 0.0215 mm² of section along the ~287 mm outline.
    assert!((removed - 6.18).abs() < 0.05, "the round removed {removed} mm³");
    // The round sits between the chamfer and the wall: the sharp edge's neighbourhood is gone,
    // the wall above the round and the chamfer below it stay.
    assert_eq!(made.classify_point(Vec3::new(-38.45, -25.0, 1.0)).unwrap(), solidcraft_kernel::PointClass::Outside);
    assert_eq!(made.classify_point(Vec3::new(-38.4, -25.0, 3.0)).unwrap(), solidcraft_kernel::PointClass::Inside);
}
