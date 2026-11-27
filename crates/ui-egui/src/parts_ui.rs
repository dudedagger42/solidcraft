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
