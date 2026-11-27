//! Where an emboss on a freeform face offsets it (#21347): the pieces of the face a box cut
//! leaves under the profiles, and whether one of them reaches a pole of its surface, where the
//! kernel never certifies an offset.

pub use solidcore::emboss::patch_outline;

use solidcore::eval::SurfaceEval;
use solidcore_record::brep::{Brep, Loop};
use solidcore_record::Vec3;

use crate::error::{Error, Result};
use crate::query::Query;

/// The faces of `cut`, a cut of `body`, on the surface of `body`'s face `face`: the pieces of it
/// the cut left.
#[must_use]
pub fn pieces_of(body: &Brep, face: usize, cut: &Brep) -> Vec<usize> {
    let Some(on) = body.faces.get(face) else {
        return Vec::new();
    };
    (0..cut.faces.len())
        .filter(|&f| {
            cut.faces
                .get(f)
                .is_some_and(|c| c.parameters() == on.parameters())
        })
        .collect()
}

/// Whether any of `faces` of `record` has a vertex at a pole of its surface, within the
/// vertex's tolerance.
///
/// # Errors
/// [`Error::Argument`] for a face the record does not have; [`Error::Unsupported`] for a
/// surface the kernel cannot evaluate there.
pub fn reaches_pole(record: &Brep, faces: &[usize]) -> Result<bool> {
    let query = Query::new(record);
    for &f in faces {
        let face = record.faces.get(f).ok_or(Error::Argument)?;
        let surface = solidcore_convert::surface(face.parameters())
            .map_err(|_| Error::Unsupported("an emboss face whose surface does not convert"))?;
        let eval = SurfaceEval::new(&surface)
            .map_err(|_| Error::Unsupported("an emboss face whose surface cannot be evaluated"))?;
        let mut vertices = Vec::new();
        for l in &face.loops {
            match l {
                Loop::Edges { edges } => {
                    for e in edges {
                        let edge = record.edges.get(e.edge as usize).ok_or(Error::Argument)?;
                        vertices.extend(edge.vertices);
                    }
                }
                Loop::Vertex { vertex } => vertices.push(*vertex),
            }
        }
        for v in vertices {
            let vertex = record.vertices.get(v as usize).ok_or(Error::Argument)?;
            let [x, y, z] = vertex.point;
            let reach = vertex.tolerance.max(record.tolerance);
            let Some(uv) = query.face_uv(f, Vec3::new(x, y, z), reach) else {
                continue;
            };
            let pole = eval
                .pole_within(uv, reach)
                .map_err(|_| Error::Unsupported("an emboss face whose pole test fails"))?;
            if pole.iter().any(|p| *p) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
