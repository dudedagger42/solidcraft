//! Delete Face for fillet and chamfer faces: solidcore's fillet removal
//! (`SmBrep::DeleteFilletFace`), which takes a four-sided face whose two long edges are tangent
//! to its neighbours off the body and closes the gap with the sharp edge where those neighbours
//! meet when extended. Ring fillets (a boss's foot or top round) go through the same call with
//! `Kernel::fixes.fillet_removal_takes_ring_fillets`, and chamfers (no edge tangent: a box
//! edge's, a hole's) with `Kernel::fixes.fillet_removal_takes_chamfers`; SolidCraft's fix list
//! turns both on.

use super::Body;
use super::blend::{finish, on_copy, points_ok, status, tolerance_ok};
use super::offset::{face_miss, pick_faces};
use crate::{Error, Result};

/// The solid with the fillet or chamfer faces under `points` (each within `tolerance`) taken off,
/// one after another, each gap closed by the sharp edge its two side neighbours meet in. A face
/// solidcore cannot take as a fillet or chamfer (not four-sided or a ring, a chamfer whose
/// direction is ambiguous, or neighbours that do not meet when extended) is refused as
/// solidcore refuses it.
pub(crate) fn delete_fillet_faces(body: &Body, points: &[[f64; 3]], tolerance: f64) -> Result<Body> {
    if !points_ok(points, false) || !tolerance_ok(tolerance) {
        return Err(Error::Argument);
    }
    on_copy("smlib_delete_fillet_faces", body, |k, mut b| {
        let mut faces = match pick_faces(k, &mut b, points, tolerance)? {
            Ok(fs) => fs,
            Err(m) => return Ok(Err(face_miss(m))),
        };
        let mut seen = std::collections::HashSet::new();
        faces.retain(|f| seen.insert(*f));
        match solidcore_ops::blend::delete_fillet_faces(k, &mut b, &faces) {
            Ok(true) => finish(k, b, true),
            Ok(false) => Ok(Err(Error::Argument)),
            Err(e) => Ok(Err(status(e))),
        }
    })
}
