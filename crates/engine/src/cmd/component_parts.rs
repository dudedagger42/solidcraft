//! Parts with timelines of their own (DM-4.4 prototype): a part is a component flagged
//! `timeline`. New Part, Make Part, Move to part and Insert .sc as an editable copy.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("part.create", "New Part", create).begins_edit().at("SOLID", "ASSEMBLE").icon("box").params(
        "name?: default `Part N`; parent?: component id or name (default: the design); activate?: bool (default true). A part is a \
         component with a timeline of its own",
    ),
    CommandSpec::new("component.make_part", "Make Part", make_part)
        .doc_only()
        .icon("box")
        .params("component: id or name; part?: bool (default true; false makes it a plain component again)"),
    CommandSpec::new("part.move_bodies", "Move to Part", move_bodies).begins_edit().icon("move").params(
        "bodies: [names]; part?: id or name of the part; new?: true makes a new part (name?) for them. Brings the bodies' history \
         when it built only them, else transfers them at the marker",
    ),
    CommandSpec::new("component.copy_sources", "Copied Parts' Sources", copy_sources)
        .noundo()
        .doc_only()
        .icon("link")
        .params("→ links: [{component, file, found, changed}] for each part copied from another design (its file read now)"),
    CommandSpec::new("component.source_relink", "Relink Source", source_relink)
        .doc_only()
        .icon("update")
        .params("component, path: the copied part's source is this file now (its copied version stays the base)"),
    CommandSpec::new("component.source_forget", "Forget Source", source_forget)
        .doc_only()
        .icon("delete")
        .params("component: the copied part no longer remembers its source"),
    CommandSpec::new("component.insert_copy", "Insert Design as Copy", insert_copy).begins_edit().at("SOLID", "ASSEMBLE").icon("import").params(
        "path: a .sc file; name?: the new part's name (default: the file's name). Copies the file's whole history into a new \
         part, editable here; parameters whose names are taken are renamed (`w_2`)",
    ),
];

fn next_part_name(s: &Session) -> String {
    let mut k = s.doc.parts().len() + 1;
    while s.doc.components.iter().any(|c| c.name == format!("Part {k}")) {
        k += 1;
    }
    format!("Part {k}")
}

fn set_timeline(s: &mut Session, id: u64, on: bool) {
    if let Some(c) = s.doc_mut().components.iter_mut().find(|c| c.id == id) {
        c.timeline = on;
    }
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "part.create";
    let name = str_(p, "name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| next_part_name(s));
    let parent = if p.get("parent").is_some() { component(s, p, "parent", cmd)? } else { 0 };
    let activate = p.get("activate").and_then(Value::as_bool).unwrap_or(true);
    let mut made = new_component(s, &json!({ "name": name, "parent": parent, "activate": activate }))?;
    let id = made["component"].as_u64().ok_or_else(|| bad(cmd, "no part was made"))?;
    set_timeline(s, id, true);
    made["part"] = json!(id);
    Ok(made)
}

fn make_part(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "component.make_part";
    let id = component(s, p, "component", cmd)?;
    if id == 0 {
        return Err(bad(cmd, "the design is always a part"));
    }
    let on = p.get("part").and_then(Value::as_bool).unwrap_or(true);
    set_timeline(s, id, on);
    Ok(json!({ "component": id, "part": on }))
}

/// The features to bring along with `bodies`: those that made them, those that change only them,
/// and the profile sketches only those use.
fn history_of(s: &Session, bodies: &BTreeSet<String>) -> BTreeSet<u64> {
    let mut feats: BTreeSet<u64> = s.body_origins().into_iter().filter(|(b, _)| bodies.contains(b)).map(|(_, f)| f).collect();
    for f in &s.doc.features {
        let inputs = f.kind.input_bodies();
        if !inputs.is_empty() && inputs.iter().chain(f.participants.iter()).all(|b| bodies.contains(b)) {
            feats.insert(f.id);
        }
    }
    for id in feats.clone() {
        let Some(sketch) = s.doc.feature(id).and_then(|f| f.kind.constructor_profile_sketch()) else { continue };
        let shared = s.doc.features.iter().any(|g| !feats.contains(&g.id) && g.kind.constructor_profile_sketch() == Some(sketch));
        if !shared {
            feats.insert(sketch);
        }
    }
    feats
}

fn move_bodies(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "part.move_bodies";
    let bodies = crate::params::string_list(cmd, p, "bodies")?;
    if bodies.is_empty() {
        return Err(bad(cmd, "`bodies` must name bodies"));
    }
    let to = if p.get("new").and_then(Value::as_bool).unwrap_or(false) {
        let made = create(s, &json!({ "name": p.get("name").cloned().unwrap_or(Value::Null), "activate": false }))?;
        made["component"].as_u64().ok_or_else(|| bad(cmd, "no part was made"))?
    } else {
        component(s, p, "part", cmd)?
    };
    let set: BTreeSet<String> = bodies.iter().cloned().collect();
    let features: Vec<u64> =
        history_of(s, &set).into_iter().filter(|id| s.doc.feature(*id).is_some_and(|f| s.doc.part_of(f.component) != to)).collect();
    if !features.is_empty()
        && let Ok(out) = s.execute("component.move_features", &json!({ "features": features, "component": to }))
    {
        return Ok(json!({ "part": to, "mode": "history", "features": features, "moved": out }));
    }
    let out = s.execute("component.move_bodies", &json!({ "bodies": bodies, "component": to }))?;
    Ok(json!({ "part": to, "mode": "transfer", "moved": out }))
}

/// Reads a design file into a session whose design is wrapped in one component, `name`, flagged
/// as a part: the root's features, components and placements move inside it, in place.
fn wrapped_source(path: &std::path::Path, name: &str, cmd: &str) -> Result<(Session, u64, solidcraft_doc::CopySource)> {
    let bytes = std::fs::read(path).map_err(|e| bad(cmd, format!("{}: {e}", path.display())))?;
    let fail = |e: solidcraft_io::IoError| bad(cmd, format!("{}: {e}", path.display()));
    let mut doc = if solidcraft_io::is_design_container(&bytes) {
        let reader = solidcraft_io::DesignContainerReader::new(std::io::Cursor::new(bytes.as_slice())).map_err(fail)?;
        reader.read_document().map_err(fail)?
    } else {
        solidcraft_io::read_design(&bytes).map_err(fail)?
    };
    let source = solidcraft_doc::CopySource {
        path: path.to_string_lossy().into_owned(),
        file_sha256: solidcraft_doc::DesignLink::file_digest(&bytes),
        base: doc.to_json(),
    };
    let old_roots: Vec<u64> = doc.occurrences.iter().filter(|o| o.parent == 0).map(|o| o.id).collect();
    let top: Vec<u64> = doc.components.iter().filter(|c| c.parent == 0).map(|c| c.id).collect();
    let wrap = doc.add_component(Some(name), 0).map_err(|e| bad(cmd, e.to_string()))?;
    for f in doc.features.iter_mut().filter(|f| f.component == 0) {
        f.component = wrap;
    }
    for c in doc.components.iter_mut().filter(|c| top.contains(&c.id)) {
        c.parent = wrap;
    }
    for o in doc.occurrences.iter_mut().filter(|o| old_roots.contains(&o.id)) {
        o.parent = wrap;
    }
    for c in doc.body_components.values_mut().filter(|c| **c == 0) {
        *c = wrap;
    }
    if let Some(c) = doc.components.iter_mut().find(|c| c.id == wrap) {
        c.timeline = true;
    }
    doc.marker = None;
    let mut src = Session::new(doc);
    src.path = Some(path.to_string_lossy().into_owned());
    src.refresh();
    Ok((src, wrap, source))
}

fn insert_copy(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "component.insert_copy";
    let given = str_(p, "path").ok_or_else(|| bad(cmd, "`path` must name a .sc file"))?;
    let mut path = std::path::PathBuf::from(given);
    if path.is_relative()
        && let Some(here) = s.path.as_deref().and_then(|f| std::path::Path::new(f).parent())
    {
        path = here.join(path);
    }
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Inserted".into());
    let name = str_(p, "name").map(str::to_string).unwrap_or(stem);
    let (src, wrap, source) = wrapped_source(&path, &name, cmd)?;
    let occurrence = src.doc.occurrence_of(wrap).map(|o| o.id).ok_or_else(|| bad(cmd, "the copy has no placement"))?;
    let copy = crate::ForeignCopy::of(&src, Vec::new(), vec![vec![occurrence]])?;
    let before: BTreeSet<u64> = s.doc.components.iter().map(|c| c.id).collect();
    let kept = s.foreign_copy.replace(Arc::new(copy));
    let out = foreign::paste_from_design(s, &json!({ "component": 0, "component_path": [], "placement": "world" }));
    s.foreign_copy = kept;
    let out = out?;
    let made: Vec<u64> = s.doc.components.iter().filter(|c| !before.contains(&c.id) && c.parent == 0).map(|c| c.id).collect();
    for id in &made {
        set_timeline(s, *id, true);
        s.doc_mut().sources.insert(*id, source.clone());
    }
    s.refresh();
    Ok(json!({ "parts": made, "pasted": out, "source": path.display().to_string() }))
}

fn resolved(s: &Session, file: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(file);
    match s.path.as_deref().and_then(|f| std::path::Path::new(f).parent()) {
        Some(here) if path.is_relative() => here.join(path),
        _ => path,
    }
}

fn copy_sources(s: &mut Session, _: &Value) -> Result<Value> {
    let links: Vec<Value> = s
        .doc
        .sources
        .iter()
        .map(|(component, source)| {
            let read = std::fs::read(resolved(s, &source.path)).ok();
            let changed = read.as_ref().map(|bytes| solidcraft_doc::DesignLink::file_digest(bytes) != source.file_sha256);
            json!({ "component": component, "file": source.path, "found": read.is_some(), "changed": changed })
        })
        .collect();
    Ok(json!({ "links": links }))
}

fn source_relink(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "component.source_relink";
    let id = component(s, p, "component", cmd)?;
    let path = str_(p, "path").ok_or_else(|| bad(cmd, "`path` must name a .sc file"))?.to_string();
    let source = s.doc_mut().sources.get_mut(&id).ok_or_else(|| bad(cmd, "that part was not copied from another design"))?;
    source.path = path.clone();
    Ok(json!({ "component": id, "path": path }))
}

fn source_forget(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "component.source_forget";
    let id = component(s, p, "component", cmd)?;
    s.doc_mut().sources.remove(&id).ok_or_else(|| bad(cmd, "that part was not copied from another design"))?;
    Ok(json!({ "component": id }))
}

#[cfg(test)]
#[path = "component_parts_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
