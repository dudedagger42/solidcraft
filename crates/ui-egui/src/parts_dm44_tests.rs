//! DM-4.4 prototype: parts in the History bar, the Browser and Extrude.

use serde_json::json;

use crate::scenario::Harness;

#[test]
fn history_bar_shows_one_parts_timeline() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Base"})).unwrap();
    assert_eq!(crate::timeline_scope::shown(&h.app), None, "no parts: the whole design");
    let part = h.app.run("part.create", json!({"name": "Lid"})).unwrap()["part"].as_u64().unwrap();
    h.app.run("solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [0, 0, 10], "body_name": "Cap"})).unwrap();
    h.frames(2);
    assert_eq!(crate::timeline_scope::shown(&h.app), Some(part));
    let within = crate::timeline_scope::components_within(&h.app, part);
    let lid: Vec<_> = h.app.session.doc.features.iter().filter(|f| within.contains(&f.component)).collect();
    assert_eq!(lid.len(), 1, "only the Lid's feature");
    h.app.run("component.activate", json!({"component": "root"})).unwrap();
    h.frames(2);
    assert_eq!(crate::timeline_scope::shown(&h.app), Some(0), "the design's own part");
    assert!(!crate::timeline_scope::components_within(&h.app, 0).contains(&part), "the Lid's timeline is its own");
}

#[test]
fn extrude_new_part_makes_the_part_then_the_feature_in_one_step() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [10, 10]})).unwrap();
    h.app.finish_sketch();
    h.app.start("solid.extrude");
    h.frames(2);
    {
        let d = h.app.dialog.as_mut().expect("Extrude opened");
        d.op_auto.manual = true;
        d.extra.insert("new_part".into(), json!(true));
        d.extra.insert("part_name".into(), json!("Lid"));
    }
    let cmds = crate::dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(cmds[0].0, "part.create", "{cmds:?}");
    assert_eq!(cmds[0].1["name"], json!("Lid"));
    assert!(cmds.iter().all(|(_, p)| p.get("new_part").is_none()), "the window's own choice is no parameter");
    let done = h.app.session.scratch().compute_commands(&cmds, None).unwrap();
    h.app.session.accept_commands(done).unwrap();
    let lid = h.app.session.doc.components.iter().find(|c| c.name == "Lid").expect("the part").id;
    let last = h.app.session.doc.features.last().unwrap();
    assert_eq!(h.app.session.doc.part_of(last.component), lid);
}

#[test]
fn move_to_part_brings_a_bodys_history() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Base"})).unwrap();
    h.app.run("part.create", json!({"name": "Lid", "activate": false})).unwrap();
    h.frames(2);
    let out = h.app.run("part.move_bodies", json!({"bodies": ["Base"], "part": "Lid"})).unwrap();
    assert_eq!(out["mode"], json!("history"), "{out}");
}

#[test]
fn combine_new_part_puts_the_result_in_a_new_part() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "A"})).unwrap();
    h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [5, 0, 0], "body_name": "B"})).unwrap();
    h.app.run("select.set", json!({"items": []})).unwrap();
    h.app.start("solid.combine");
    h.frames(2);
    {
        let d = h.app.dialog.as_mut().expect("Combine opened");
        d.inputs[0].items = vec![solidcraft_engine::Sel::Body { name: "A".into() }];
        d.inputs[1].items = vec![solidcraft_engine::Sel::Body { name: "B".into() }];
        d.extra.insert("combine_part".into(), json!(true));
    }
    let cmds = crate::dialogs::apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert_eq!(cmds.last().unwrap().0, "component.make_part", "{cmds:?}");
    let done = h.app.session.scratch().compute_commands(&cmds, None).unwrap();
    h.app.session.accept_commands(done).unwrap();
    let part = h.app.session.doc.components.iter().find(|c| c.name == "Part 2").expect("the part");
    assert!(part.timeline);
}
