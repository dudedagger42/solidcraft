//! DM-4.4 prototype: the Move to Part window.

use std::sync::Mutex;

use serde_json::json;

use crate::SolveApp;

struct Pending {
    bodies: Vec<String>,
    /// Index into the parts list; `None` is New Part.
    choice: Option<usize>,
    name: String,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Opens Move to Part for `bodies`.
pub(crate) fn open(bodies: Vec<String>) {
    if let Ok(mut p) = PENDING.lock() {
        *p = Some(Pending { bodies, choice: Some(0), name: String::new() });
    }
}

/// Draws the window while it is open.
pub(crate) fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Ok(mut guard) = PENDING.lock() else { return };
    let Some(pending) = guard.as_mut() else { return };
    let doc = &app.session.doc;
    let parts: Vec<(u64, String)> =
        doc.parts().into_iter().map(|p| (p, if p == 0 { doc.name.clone() } else { crate::timeline_fold::name(app, p) })).collect();
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new("Move to Part").id(egui::Id::unique("dm44_move_to_part")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
        let what = if pending.bodies.len() == 1 { pending.bodies[0].clone() } else { format!("{} bodies", pending.bodies.len()) };
        ui.label(format!("Move {what} to:"));
        for (i, (_, name)) in parts.iter().enumerate() {
            ui.radio_value(&mut pending.choice, Some(i), name.as_str());
        }
        ui.radio_value(&mut pending.choice, None, "New Part");
        if pending.choice.is_none() {
            ui.horizontal(|ui| {
                ui.label("Part name");
                ui.text_edit_singleline(&mut pending.name);
            });
        }
        ui.separator();
        ui.label("Brings its history when every feature that built it built only these bodies; otherwise transfers them at the marker.");
        ui.horizontal(|ui| {
            ok = ui.button("OK").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if !(ok || cancel || !open) {
        return;
    }
    let Some(pending) = guard.take() else { return };
    drop(guard);
    if !ok {
        return;
    }
    let params = match pending.choice.and_then(|i| parts.get(i)) {
        Some((part, _)) => json!({ "bodies": pending.bodies, "part": part }),
        None => {
            let name = pending.name.trim();
            json!({ "bodies": pending.bodies, "new": true, "name": if name.is_empty() { serde_json::Value::Null } else { json!(name) } })
        }
    };
    match app.run("part.move_bodies", params) {
        Ok(out) if out["mode"] == "history" => {
            let n = out["features"].as_array().map_or(0, Vec::len);
            app.set_status(format!("Moved with its history ({n} features)"), false);
        }
        Ok(_) => app.set_status("Transferred at the marker: its earlier history stays in its old part", false),
        Err(e) => app.set_status(format!("Move to Part: {e}"), true),
    }
}

/// The vertical History grouped by part (DM-4.4, an option of the History's menu): a node per
/// part with its features in timeline order, and the one global marker drawn in each node after
/// its last feature before the marker.
pub(crate) fn part_tree(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = solidcraft_theme::Tokens::get();
    let doc = app.session.doc.clone();
    let marker = doc.marker.unwrap_or(doc.features.len());
    let selected: std::collections::BTreeSet<u64> = crate::timeline::selected_feature_ids(app).into_iter().collect();
    let active_part = doc.part_of(app.session.active_component);
    let mut edit = None;
    let mut menu = None;
    for part in doc.parts() {
        let name = if part == 0 { doc.name.clone() } else { crate::timeline_fold::name(app, part) };
        let rows: Vec<usize> = (0..doc.features.len()).filter(|i| doc.part_of(doc.features[*i].component) == part).collect();
        let id = ui.make_persistent_id(("dm44_part_node", part));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                if ui.radio(active_part == part, "").on_hover_text("Make this the active part").clicked() {
                    let _ = app.run("component.activate", json!({ "component": part }));
                }
                let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                crate::icons::paint_icon(ui.painter(), rect, "box", 1.0);
                ui.strong(&name);
                ui.weak(format!("{}", rows.len()));
            })
            .body(|ui| {
                let mut drawn = false;
                let marker_line = |ui: &mut egui::Ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
                    ui.painter().hline(rect.x_range(), rect.center().y, egui::Stroke::new(2.0, t.accent));
                };
                for i in &rows {
                    if *i >= marker && !drawn {
                        marker_line(ui);
                        drawn = true;
                    }
                    let f = &doc.features[*i];
                    let mut text = egui::RichText::new(format!("{:02}  {}", i + 1, f.name));
                    if *i >= marker || f.suppressed {
                        text = text.weak();
                    }
                    let response = ui.selectable_label(selected.contains(&f.id), text);
                    if response.clicked() {
                        let _ = app.run("select.set", json!({ "items": [{ "type": "feature", "id": f.id }] }));
                    }
                    if response.double_clicked() {
                        edit = Some(f.id);
                    }
                    if response.secondary_clicked() {
                        menu = Some((f.id, response.rect.left_bottom()));
                    }
                }
                if !drawn && !rows.is_empty() && marker < doc.features.len() {
                    marker_line(ui);
                }
            });
    }
    if let Some(feature) = edit {
        app.edit_feature(feature);
    }
    if let Some((feature, at)) = menu {
        crate::context_menu::open_for(app, at, crate::context_menu::Target::Timeline { feature });
    }
}
