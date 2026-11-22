use crate::scenario::Harness;
use serde_json::{Value, json};

/// Throwaway (#19115 PR screenshot): the app at 1280x720 with one panel floating and the pointer on
/// its left resize edge. Writes SOLIDCRAFT_19115_EVIDENCE/floating-resize-1280x720.png.
#[test]
fn shot19115_floating_card_edge_hover() {
    let Ok(dir) = std::env::var("SOLIDCRAFT_19115_EVIDENCE") else { return };
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.run("solid.box", json!({"corner":[0,0,0],"length":40,"width":30,"height":20,"body_name":"Box"})).unwrap();
    h.call("ui.view", json!({"view":"fit"}));
    h.frames(4);
    let list = h.app.run("panel.list", json!({})).unwrap_or_else(|e| json!({"error": format!("{e:?}")}));
    fn find_sections(v: &Value) -> Option<Value> {
        match v {
            Value::Object(m) => m.get("sections").filter(|s| s.is_array()).cloned().or_else(|| m.values().find_map(find_sections)),
            Value::Array(a) => a.iter().find_map(find_sections),
            _ => None,
        }
    }
    println!("SHOT list head: {}", list.to_string().chars().take(600).collect::<String>());
    let sections = find_sections(&list).unwrap_or(Value::Null);
    println!("SHOT sections: {sections}");
    let target = sections
        .as_array()
        .and_then(|all| {
            all.iter().find(|s| s.pointer("/location/type").and_then(Value::as_str) == Some("dock")).and_then(|s| s.pointer("/tabs/0").cloned())
        })
        .expect("a docked panel to float");
    println!("SHOT floating {target}");
    let reply = h.app.run(
        "panel.float",
        json!({"target": target, "destination": {"type":"new_section","section":"shot-float","index":0,
            "location":{"type":"floating","origin":{"x":760,"y":170}},"size":{"width":300,"height":340}}}),
    )
    .unwrap_or_else(|e| json!({"error": format!("{e:?}")}));
    println!("SHOT float reply: {}", reply.to_string().chars().take(400).collect::<String>());
    h.frames(4);
    let size = egui::vec2(1280.0, 720.0);
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    h.ctx = egui::Context::default();
    crate::fonts::setup_bundled_fonts(&h.ctx);
    let mut textures = std::collections::HashMap::new();
    let hover = egui::pos2(760.0 + 1.0, 170.0 + 200.0);
    for frame in 0..10 {
        let events = if frame >= 6 { vec![egui::Event::PointerMoved(hover)] } else { Vec::new() };
        let mut output = h.ctx.run_ui(
            egui::RawInput { screen_rect: Some(bounds), time: Some(frame as f64 / 60.0), focused: true, events, ..Default::default() },
            |ui| {
                h.app.logic(ui.ctx());
                h.app.ui(ui)
            },
        );
        crate::ui_kit::test_textures_update(&mut textures, &output.textures_delta);
        output.textures_delta.clear();
        if frame == 9 {
            println!("SHOT cursor {:?}", output.platform_output.cursor_icon);
            let image = crate::ui_kit::test_raster(&h.ctx, output.shapes, output.pixels_per_point, bounds, &textures);
            std::fs::create_dir_all(&dir).unwrap();
            image.save(std::path::Path::new(&dir).join("floating-resize-1280x720.png")).unwrap();
        }
    }
}
