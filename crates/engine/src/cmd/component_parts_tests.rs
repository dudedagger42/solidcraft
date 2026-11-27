// Parts with their own timelines (DM-4.4 prototype).
use crate::Session;
use serde_json::json;

fn part_names(s: &Session) -> Vec<String> {
    s.doc.parts().into_iter().filter(|p| *p != 0).filter_map(|p| s.doc.components.iter().find(|c| c.id == p)).map(|c| c.name.clone()).collect()
}

#[test]
fn new_part_is_a_component_with_its_own_timeline_and_one_undo_step() {
    let mut s = Session::default();
    assert!(!s.doc.has_parts(), "a new design is one part, the design");
    let made = s.execute("part.create", &json!({})).unwrap();
    let id = made["part"].as_u64().unwrap();
    assert_eq!(part_names(&s), vec!["Part 2".to_string()]);
    assert_eq!(s.active_component, id, "the new part is active");
    s.execute("solid.box", &json!({"length": 1, "width": 1, "height": 1, "body_name": "A"})).unwrap();
    let f = s.doc.features.last().unwrap();
    assert_eq!(s.doc.part_of(f.component), id, "its features go in it");
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(!s.doc.has_parts());
}

#[test]
fn make_part_flags_a_component_and_round_trips() {
    let mut s = Session::default();
    s.execute("component.create", &json!({ "name": "Inner" })).unwrap();
    s.execute("component.create", &json!({ "name": "Deep" })).unwrap();
    let inner = s.doc.components.iter().find(|c| c.name == "Inner").unwrap().id;
    let deep = s.doc.components.iter().find(|c| c.name == "Deep").unwrap().id;
    assert_eq!(s.doc.part_of(deep), 0);
    s.execute("component.make_part", &json!({ "component": "Inner" })).unwrap();
    assert_eq!(s.doc.part_of(deep), inner, "a component inside a part belongs to it");
    assert!(s.execute("component.make_part", &json!({ "component": "root" })).is_err());
    let read = solidcraft_doc::Document::from_json(&s.doc.to_json()).unwrap();
    assert!(read.components.iter().find(|c| c.name == "Inner").unwrap().timeline);
    assert!(!s.doc.to_json().contains("\"timeline\": false"), "the flag is only saved when set");
    let plain = solidcraft_doc::Document::from_json(&Session::default().doc.to_json()).unwrap();
    assert!(!plain.has_parts(), "an old design opens as one part");
}

#[test]
fn move_bodies_to_a_new_part_brings_their_history() {
    let mut s = Session::default();
    s.execute("sketch.create", &json!({"plane": "XY"})).unwrap();
    s.execute("sketch.rectangle.two_point", &json!({"p0": [0, 0], "p1": [10, 10]})).unwrap();
    s.execute("sketch.finish", &json!({})).unwrap();
    s.execute("solid.extrude", &json!({"distance": 2, "operation": "new", "body_name": "Plate"})).unwrap();
    let out = s.execute("part.move_bodies", &json!({"bodies": ["Plate"], "new": true})).unwrap();
    assert_eq!(out["mode"], json!("history"), "{out}");
    let part = out["part"].as_u64().unwrap();
    assert!(s.doc.features.iter().all(|f| s.doc.part_of(f.component) == part), "the sketch and extrude moved");
}

#[test]
fn extrude_into_a_new_part_keeps_the_sketch_where_it_is() {
    let mut s = Session::default();
    s.execute("sketch.create", &json!({"plane": "XY"})).unwrap();
    s.execute("sketch.rectangle.two_point", &json!({"p0": [0, 0], "p1": [10, 10]})).unwrap();
    s.execute("sketch.finish", &json!({})).unwrap();
    let sketch = s.doc.features.last().unwrap().id;
    let part = s.execute("part.create", &json!({"name": "Lid"})).unwrap()["part"].as_u64().unwrap();
    s.execute("solid.extrude", &json!({"sketch": sketch, "distance": 2, "operation": "new", "body_name": "Lid"})).unwrap();
    let f = s.doc.features.last().unwrap();
    assert_eq!(s.doc.part_of(f.component), part);
    assert!(s.model.result(f.id).is_some_and(|r| r.error.is_none()), "the extrude builds from the other part's sketch");
    assert_eq!(s.doc.part_of(s.doc.feature(sketch).unwrap().component), 0);
}

#[test]
fn insert_copy_brings_the_files_history_as_a_new_part() {
    let dir = std::env::temp_dir().join(format!("solidcraft-dm44-insert-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bracket.sc");
    let mut src = Session::default();
    src.execute("solid.box", &json!({"length": 10, "width": 20, "height": 3, "body_name": "Plate"})).unwrap();
    src.execute("file.save", &json!({"path": file})).unwrap();
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length": 1, "width": 1, "height": 1, "body_name": "Mine"})).unwrap();
    let out = s.execute("component.insert_copy", &json!({"path": file})).unwrap();
    let parts: Vec<u64> = out["parts"].as_array().unwrap().iter().filter_map(serde_json::Value::as_u64).collect();
    assert_eq!(parts.len(), 1, "{out}");
    let part = parts[0];
    assert_eq!(s.doc.components.iter().find(|c| c.id == part).unwrap().name, "bracket");
    assert!(s.doc.features.iter().any(|f| s.doc.part_of(f.component) == part), "its history came along");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(!s.doc.has_parts(), "one undo step");
    let _ = std::fs::remove_dir_all(&dir);
}
