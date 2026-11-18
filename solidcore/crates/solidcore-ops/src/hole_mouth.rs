//! How high a hole's entry face stands over its rim ([`solidcore::query::hole_mouth`]).

use solidcore::query::hole_mouth as hm;
use solidcore::Point3;

use crate::body::Body;
use crate::session::with_body;
use crate::{Error, Result};

impl Body {
    /// The height this body's faces at `spot` reach above the plane across the axis `dir`
    /// there, within `radius` of the axis ([`hm::hole_mouth_lip`]): exact for a planar face, the
    /// tangent plane's bound for a convex cylinder, cone or sphere. `None` where `spot` is not
    /// on the body's boundary or the axis enters no face there.
    ///
    /// # Errors
    /// [`Error::Argument`] for a number that is not finite; [`Error::Unsupported`] for an
    /// entered face that is neither planar nor convex as above; the kernel's error.
    pub fn hole_mouth_lip(
        &self,
        spot: [f64; 3],
        dir: [f64; 3],
        radius: f64,
    ) -> Result<Option<f64>> {
        if !(spot.iter().chain(&dir).all(|x| x.is_finite()) && radius.is_finite()) {
            return Err(Error::Argument);
        }
        let (spot, dir) = (
            Point3::new(spot[0], spot[1], spot[2]),
            Point3::new(dir[0], dir[1], dir[2]),
        );
        with_body("hole_mouth_lip", self, |k, b| {
            hm::hole_mouth_lip(k, b, spot, dir, radius).map(Ok)
        })?
    }
}

#[cfg(test)]
mod tests {
    use crate::boolean::{boolean, BooleanOp};
    use crate::{make_box, make_cylinder, make_sphere, session, Error};

    fn all_fixes() {
        session::set_thread_fixes(solidcore::kernel::UpstreamFixes::NAMES.iter().copied()).unwrap();
    }

    /// A hole of radius 3 entering a box's top face 30° off its normal, leaning 22.5° from +x:
    /// the face stands r tan 30° over the rim, whichever way the hole leans.
    #[test]
    fn hole_mouth_lip_of_a_box_face_entered_at_an_angle_is_the_rim_height() {
        all_fixes();
        let b = make_box([0.0, 0.0, 0.0], [40.0, 40.0, 20.0]).unwrap();
        let (angle, turn, radius) = (30f64.to_radians(), 22.5f64.to_radians(), 3.0);
        let dir = [
            angle.sin() * turn.cos(),
            angle.sin() * turn.sin(),
            -angle.cos(),
        ];
        let lip = b
            .hole_mouth_lip([20.0, 20.0, 20.0], dir, radius)
            .unwrap()
            .unwrap();
        let want = radius * angle.tan();
        assert!((lip - want).abs() < 1e-9 * want, "lip {lip} vs {want}");
        assert_eq!(
            b.hole_mouth_lip([20.0, 20.0, 20.0], [0.0, 0.0, -1.0], radius)
                .unwrap(),
            Some(0.0)
        );
        // In the air, inside the material, and on a face the axis leaves by: no entry face.
        assert_eq!(
            b.hole_mouth_lip([20.0, 20.0, 25.0], dir, radius).unwrap(),
            None
        );
        assert_eq!(
            b.hole_mouth_lip([20.0, 20.0, 10.0], dir, radius).unwrap(),
            None
        );
        assert_eq!(
            b.hole_mouth_lip([20.0, 20.0, 0.0], dir, radius).unwrap(),
            None
        );
    }

    /// On an edge the higher of its two faces counts; a convex wall is bounded by its tangent
    /// plane; a bore's wall is refused.
    #[test]
    fn hole_mouth_lip_on_an_edge_a_round_wall_and_a_bore() {
        all_fixes();
        let b = make_box([0.0, 0.0, 0.0], [40.0, 40.0, 20.0]).unwrap();
        let s = 0.5f64.sqrt();
        let lip = b
            .hole_mouth_lip([40.0, 20.0, 20.0], [-s, 0.0, -s], 3.0)
            .unwrap()
            .unwrap();
        assert!((lip - 3.0).abs() < 1e-9, "{lip}");
        let bar = make_cylinder([0.0, 0.0, 0.0], 10.0, 40.0).unwrap();
        let radial = bar
            .hole_mouth_lip([10.0, 0.0, 20.0], [-1.0, 0.0, 0.0], 2.0)
            .unwrap()
            .unwrap();
        assert!(radial.abs() < 1e-9, "{radial}");
        let leaning = bar
            .hole_mouth_lip([10.0, 0.0, 20.0], [-s, 0.0, -s], 2.0)
            .unwrap()
            .unwrap();
        assert!((leaning - 2.0).abs() < 1e-9, "{leaning}");
        let ball = make_sphere([0.0, 0.0, 0.0], 5.0).unwrap();
        let top = ball
            .hole_mouth_lip([0.0, 5.0, 0.0], [0.0, -1.0, 0.0], 1.0)
            .unwrap()
            .unwrap();
        assert!(top.abs() < 1e-9, "{top}");
        let bore = make_cylinder([0.0, 0.0, -1.0], 6.0, 42.0).unwrap();
        let tube = boolean(bar, bore, BooleanOp::Subtract).unwrap().unwrap();
        assert!(matches!(
            tube.hole_mouth_lip([6.0, 0.0, 20.0], [1.0, 0.0, 0.0], 2.0),
            Err(Error::Unsupported(_))
        ));
        // The apex of a drill point's pit has no normal.
        let block = make_box([0.0, 0.0, 0.0], [40.0, 30.0, 20.0]).unwrap();
        let tip = crate::make_cone([20.0, 15.0, 0.0], 4.0, 0.0, 2.4).unwrap();
        let pit = boolean(block, tip, BooleanOp::Subtract).unwrap().unwrap();
        assert!(matches!(
            pit.hole_mouth_lip([20.0, 15.0, 2.4], [0.0, 0.0, 1.0], 6.0),
            Err(Error::Unsupported(_))
        ));
    }
}
