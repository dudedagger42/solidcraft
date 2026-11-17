//! TEMPORARY dump for #17356 (probe branch only): the Debug text of a face's surface, to a file.
use crate::body::Body;
use solidcore::topology::brep::p1::faces;
use solidcore::topology::face::p2::surface;
use solidcore_record::brep::Brep as Record;

pub fn dump_face_surface(record: &Record, face_name: &str, path: &str) {
    let Ok((body, _order)) = Body::from_record(record, true) else {
        return;
    };
    let guard = body.kernel();
    let Ok(all) = faces(&guard, None) else {
        return;
    };
    for f in all.iter() {
        if format!("{f:?}") == face_name {
            if let Ok(Some(s)) = surface(&guard, *f) {
                let _ = std::fs::write(path, format!("{s:?}"));
            }
        }
    }
}
