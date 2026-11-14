//! Delete Face for fillet faces: solidcore's fillet removal (`SmBrep::DeleteFilletFace`), which
//! takes a four-sided face whose two long edges are tangent to its neighbours off the body and
//! closes the gap with the sharp edge where those neighbours meet when extended. Ring fillets
//! (a boss's foot or top round) go through the same call with
//! `Kernel::fixes.fillet_removal_takes_ring_fillets`, which SolidCraft's fix list turns on.

use solidcore::blend::fillet_removal::delete_fillet_face;
use solidcore::topology::brep::p2 as brep_p2;

use super::Body;
use super::blend::{finish, on_copy, points_ok, status, tolerance_ok};
use super::offset::{face_miss, pick_faces};
use crate::{Error, Result};

/// The solid with the fillet faces under `points` (each within `tolerance`) taken off, one after
/// another, each gap closed by the sharp edge its two tangent neighbours meet in. A face that is
/// not a four-sided fillet (no pair of opposite edges tangent to its neighbours, or a neighbour
/// that cannot be extended) is refused as solidcore refuses it.
pub(crate) fn delete_fillet_faces(body: &Body, points: &[[f64; 3]], tolerance: f64) -> Result<Body> {
    if !points_ok(points, false) || !tolerance_ok(tolerance) {
        return Err(Error::Argument);
    }
    on_copy("smlib_delete_fillet_faces", body, |k, mut b| {
        if !brep_p2::is_manifold_solid(&b)? {
            return Ok(Err(Error::Argument));
        }
        let editing = b.editing_enabled();
        b.set_editing_enabled(true);
        let mut removed = Ok(());
        // Each face is found by its point just before it goes: removing one changes the faces
        // around it.
        for p in points {
            let face = match pick_faces(k, &mut b, std::slice::from_ref(p), tolerance)? {
                Ok(fs) => match fs.first() {
                    Some(&f) => f,
                    None => {
                        removed = Err(Error::Unsupported);
                        break;
                    }
                },
                Err(m) => {
                    removed = Err(face_miss(m));
                    break;
                }
            };
            if let Err(e) = delete_fillet_face(k, &mut b, face, None) {
                removed = Err(status(e));
                break;
            }
        }
        b.set_editing_enabled(editing);
        b.refresh_cached_topology();
        match removed {
            Ok(()) => finish(k, b, true),
            Err(e) => Ok(Err(e)),
        }
    })
}
