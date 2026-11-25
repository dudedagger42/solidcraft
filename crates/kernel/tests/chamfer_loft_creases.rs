//! #21800: the four crease edges of a square-to-circle loft run out tangentially on the round
//! rim, each at a point of its own. A chamfer of them (or of one) must build: no other blend
//! meets a crease at either end.

use solidcore_record::{Loop2, Plane, Vec2, Vec3};
use solidcraft_kernel::{Body, chamfer, loft, measure};

fn square_to_circle() -> Body {
    let square = Loop2::polygon(&[
        Vec2::new(-20.0, -20.0),
        Vec2::new(20.0, -20.0),
        Vec2::new(20.0, 20.0),
        Vec2::new(-20.0, 20.0),
    ]);
    let top = Plane { origin: Vec3::new(0.0, 0.0, 40.0), ..Plane::XY };
    loft(&[(Plane::XY, square), (top, Loop2::circle(Vec2::ZERO, 10.0))]).unwrap()
}

/// The midpoints of the four creases: each runs from a corner of the square to the circle.
fn crease_mids() -> Vec<Vec3> {
    let c = 10.0 / 2f64.sqrt();
    [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .iter()
        .map(|&(sx, sy)| Vec3::new(sx * (20.0 + c) / 2.0, sy * (20.0 + c) / 2.0, 20.0))
        .collect()
}

#[test]
fn chamfer_the_creases_of_a_square_to_circle_loft() {
    let body = square_to_circle();
    assert!(body.validity().is_empty(), "{:?}", body.validity());
    let volume = measure(&body).unwrap().volume;
    let picks = crease_mids();
    for distance in [0.5, 1.0] {
        let one = chamfer(&body, &picks[..1], distance)
            .unwrap_or_else(|e| panic!("one crease at {distance}: {e}"));
        assert!(one.validity().is_empty(), "one crease at {distance}: {:?}", one.validity());
        let removed_one = volume - measure(&one).unwrap().volume;
        assert!(removed_one > 0.0, "one crease at {distance} removed {removed_one}");

        let four = chamfer(&body, &picks, distance)
            .unwrap_or_else(|e| panic!("four creases at {distance}: {e}"));
        assert!(four.validity().is_empty(), "four creases at {distance}: {:?}", four.validity());
        let removed_four = volume - measure(&four).unwrap().volume;
        assert!(
            (removed_four - 4.0 * removed_one).abs() <= 1e-6 * removed_four,
            "at {distance}: four creases removed {removed_four}, one removed {removed_one}"
        );
    }
}
