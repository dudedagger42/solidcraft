//! Probe for #13282 pictures (not for merge): the Fillet dialog after a face pick on a plate with
//! a hole and its outer top edges rounded, rastered and printed as base64 between markers.

use crate::dialogs::{apply_commands, face_sel};
use crate::scenario::Harness;
use serde_json::json;
use solidcore_record::Vec3;

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[test]
fn pic_13282_fillet_dialog_after_face_pick() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1280,"height":720}));
    h.app.run("solid.box", json!({"length":60,"width":40,"height":10})).unwrap();
    h.app.run("solid.hole", json!({"position":[30,20,10],"diameter":12})).unwrap();
    h.app.run("solid.fillet", json!({"edges":[[30,0,10],[30,40,10],[0,20,10],[60,20,10]],"radius":3})).unwrap();
    let face = face_sel(&h.app.session, Vec3::new(12.0, 10.0, 10.0)).unwrap();
    h.app.session.selection = vec![face];
    h.step(&json!({"start":"solid.fillet"})).unwrap();
    h.frames(4);
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    println!("PIC13282 commands: {commands:?}");

    let ctx = crate::fonts::test_context();
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
    let mut textures = Default::default();
    for frame in 0..6 {
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(bounds), ..Default::default() }, |ui| {
            h.app.logic(ui.ctx());
            h.app.ui(ui);
        });
        crate::ui_kit::test_textures_update(&mut textures, &output.textures_delta);
        output.textures_delta.clear();
        if frame == 5 {
            let img = crate::ui_kit::test_raster(&ctx, output.shapes, output.pixels_per_point, bounds, &textures);
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).unwrap();
            let text = b64(png.get_ref());
            println!("PIC13282-BEGIN");
            for line in text.as_bytes().chunks(76) {
                println!("{}", std::str::from_utf8(line).unwrap());
            }
            println!("PIC13282-END");
        }
    }
}
