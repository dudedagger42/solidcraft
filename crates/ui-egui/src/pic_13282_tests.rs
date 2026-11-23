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

/// The plate, its hole and r3 rounds on the outer top edges, the top face picked, Fillet open.
fn plate_with_fillet_open() -> Harness {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1280,"height":720}));
    h.app.run("solid.box", json!({"length":60,"width":40,"height":10})).unwrap();
    h.app.run("solid.hole", json!({"position":[30,20,10],"diameter":12})).unwrap();
    h.app.run("solid.fillet", json!({"edges":[[30,0,10],[30,40,10],[0,20,10],[60,20,10]],"radius":3})).unwrap();
    let face = face_sel(&h.app.session, Vec3::new(12.0, 10.0, 10.0)).unwrap();
    h.app.session.selection = vec![face];
    h.step(&json!({"start":"solid.fillet"})).unwrap();
    h.frames(4);
    h
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let (ab, ap) = ([b.x - a.x, b.y - a.y, b.z - a.z], [p.x - a.x, p.y - a.y, p.z - a.z]);
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let d = [ap[0] - t * ab[0], ap[1] - t * ab[1], ap[2] - t * ab[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The polylines of the B-rep edges the dialog's command names (each edge point matched to the
/// body edge it lies on).
fn command_edge_polylines(h: &Harness) -> Vec<Vec<Vec3>> {
    let commands = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    let world = crate::open_meshing::session_world(&h.app.session);
    let mut out = Vec::new();
    for (_, p) in &commands {
        let Some(body) = p["body"].as_str().and_then(|b| world.body(b)) else { continue };
        let m = body.mesh();
        for e in p["edges"].as_array().into_iter().flatten() {
            let at = Vec3::new(e[0].as_f64().unwrap(), e[1].as_f64().unwrap(), e[2].as_f64().unwrap());
            let best = m
                .edges
                .iter()
                .map(|poly| {
                    let d = poly.windows(2).map(|w| segment_distance(at, w[0], w[1])).fold(f64::INFINITY, f64::min);
                    (d, poly)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((d, poly)) = best
                && d < 1e-3
            {
                out.push(poly.to_vec());
            }
        }
    }
    out
}

/// GPU picture of the whole window, 3D view included, with the app's real renderer. Runs only
/// when SOLIDCRAFT_PIC13282_GPU names the PNG to write; without a GPU device it returns early.
/// Writes `<path>` (the app's own frame: the picked face's highlight and the dialog's preview)
/// and `<path stem>-edges.png` (the same frame with the command's edges drawn over it by this probe).
#[test]
fn pic_13282_gpu() {
    let Some(path) = std::env::var_os("SOLIDCRAFT_PIC13282_GPU") else { return };
    let path = std::path::PathBuf::from(path);
    let mut h = plate_with_fillet_open();
    let size = [1280u32, 720u32];
    let Some(mut gpu) = crate::gpu::EguiFixture::new(&mut h.app, size) else {
        eprintln!("PIC13282 gpu: no GPU device, nothing written");
        return;
    };
    let began = std::time::Instant::now();
    let mut frames = 0;
    // Frames until the preview and the view's meshing are done (and a few more to draw them).
    while frames < 20
        || ((h.app.preview.busy || crate::open_meshing::active(&h.app) || h.app.cam_anim.is_some())
            && began.elapsed() < std::time::Duration::from_secs(120))
    {
        let app = &mut h.app;
        gpu.frame(&h.ctx, Vec::new(), |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    println!(
        "PIC13282 gpu: {frames} frames in {:.1}s; preview active {} busy {} error {:?}; viewport callbacks {}",
        began.elapsed().as_secs_f64(),
        h.app.preview.active,
        h.app.preview.busy,
        h.app.preview.error,
        gpu.viewport_callbacks
    );
    let lines = command_edge_polylines(&h);
    println!("PIC13282 gpu: the command takes {} edges", lines.len());
    let save = |pixels: Vec<u8>, to: &std::path::Path| {
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        image::RgbaImage::from_raw(size[0], size[1], pixels).unwrap().save(to).unwrap();
        println!("PIC13282 gpu: wrote {}", to.display());
    };
    let app = &mut h.app;
    let plain = gpu.frame(&h.ctx, Vec::new(), |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    });
    save(plain, &path);
    let app = &mut h.app;
    let marked = gpu.frame(&h.ctx, Vec::new(), |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
        let Some(rect) = app.viewport.rect else { return };
        let proj = crate::viewport::projection(app, rect);
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("pic13282")));
        let stroke = egui::Stroke::new(4.0, egui::Color32::from_rgb(255, 140, 0));
        for poly in &lines {
            let pts: Vec<egui::Pos2> = poly.iter().filter_map(|p| proj.to_screen(*p)).collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        painter.text(
            rect.left_bottom() + egui::vec2(12.0, -12.0),
            egui::Align2::LEFT_BOTTOM,
            format!("orange: the {} edges Fillet takes (drawn by the probe)", lines.len()),
            egui::FontId::proportional(16.0),
            egui::Color32::from_rgb(255, 140, 0),
        );
    });
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    save(marked, &path.with_file_name(format!("{stem}-edges.png")));
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
