//! Where an emboss on a freeform face offsets it (#21347): the pieces of the face a box cut
//! leaves under the profiles, and whether one of them reaches a pole of its surface, where the
//! kernel never certifies an offset.

pub use solidcore::emboss::patch_outline;

use solidcore::eval::SurfaceEval;
use solidcore_record::brep::{Brep, Loop};
use solidcore_record::Vec3;

use solidcore::query::api_queries::face_internal_point;

use crate::body::Body;
use crate::error::{Error, Result};
use crate::query::Query;
use crate::session::with_body;

/// The faces of `cut`, a cut of `body`, that are pieces of `body`'s face `face`: those with a
/// point inside them on it, within the two bodies' tolerance.
///
/// # Errors
/// The record conversion's refusals and the kernel's, finding a point inside a face of `cut`.
pub fn pieces_of(body: &Brep, face: usize, cut: &Brep) -> Result<Vec<usize>> {
    let (cut_body, order) = Body::from_record(cut, true)?;
    let inside = with_body("emboss pieces", &cut_body, |k, b| {
        order
            .faces
            .iter()
            .map(|&f| face_internal_point(k, b, f).map(|p| [p.x, p.y, p.z]))
            .collect::<solidcore::Result<Vec<_>>>()
            .map(Ok::<_, Error>)
    })??;
    let query = Query::new(body);
    let accept = body.tolerance.max(cut.tolerance);
    Ok(inside
        .iter()
        .enumerate()
        .filter(|(_, [x, y, z])| query.face_contains(face, Vec3::new(*x, *y, *z), accept))
        .map(|(f, _)| f)
        .collect())
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
