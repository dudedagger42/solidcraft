//! DM-4.4 prototype: the Move to Part window, and the History's choices for a design with parts.

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
        // The parts, then New Part last.
        let mut options: Vec<&str> = parts.iter().map(|(_, name)| name.as_str()).collect();
        options.push("New Part");
        let mut at = pending.choice.unwrap_or(parts.len());
        crate::ui_kit::dropdown(ui, "dm44_move_to_part", &mut at, &options);
        pending.choice = (at < parts.len()).then_some(at);
        if pending.choice.is_none() {
            crate::ui_kit::row(ui, "Part name", |ui| {
                crate::ui_kit::text_field(ui, &mut pending.name, "Part name");
            });
        }
        ui.separator();
        ui.label("Brings its history when every feature that built it built only these bodies; otherwise transfers them at the marker.");
        ui.horizontal(|ui| {
            ok = crate::ui_kit::button(ui, "OK", crate::ui_kit::Button::Normal).clicked();
            cancel = crate::ui_kit::button(ui, "Cancel", crate::ui_kit::Button::Normal).clicked();
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

/// The vertical History grouped by part (DM-4.4): turning it on docks the History at the left, where
/// the vertical History draws each part as a node of its own tree (`timeline_left::part_nodes`);
/// turning it off puts the History back at the bottom.
pub(crate) fn set_part_tree(app: &mut SolveApp, on: bool) {
    app.tree.part_tree = on;
    // With parts the bottom History is the rows panel; the vertical History is the History itself.
    app.preferences.timeline_left = on;
    // The History opens again in the next frames (the rows panel held the bottom); it is placed then.
    PENDING_DOCK.with(|p| p.set(Some((if on { "left" } else { "bottom" }, 3))));
}

thread_local! {
    /// Where the History goes once it is open, and for how many more frames to place it.
    static PENDING_DOCK: std::cell::Cell<Option<(&'static str, u8)>> = const { std::cell::Cell::new(None) };
}

/// Places the History where the part tree option put it (outside the panels' own drawing).
fn apply_pending_dock(app: &mut SolveApp) {
    let Some((dock, left)) = PENDING_DOCK.with(std::cell::Cell::get) else { return };
    PENDING_DOCK.with(|p| p.set((left > 1).then_some((dock, left - 1))));
    if let Err(e) = app.run("view.history_layout", json!({ "dock": dock })) {
        app.set_status(format!("History layout: {e}"), true);
    }
}

/// How the bottom History shows a design with parts (DM-4.4; a prototype choice in the History's
/// menus, so the variants can be compared).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowsStyle {
    /// (A) The active part's row at full height; the others thin strips with feature ticks that
    /// open on hover. One marker through all.
    Focus,
    /// (B) Each part's row packs its own features; the marker shows in each row at the matching step.
    Packed,
    /// (C) One bar, each chip tinted by its part, with a part filter at the left.
    OneBar,
    /// Build 5: a row per part, features at their places in the one history.
    Aligned,
}

impl RowsStyle {
    pub(crate) const ALL: [RowsStyle; 4] = [RowsStyle::Focus, RowsStyle::Packed, RowsStyle::OneBar, RowsStyle::Aligned];
    pub(crate) fn id(self) -> &'static str {
        match self {
            RowsStyle::Focus => "focus",
            RowsStyle::Packed => "packed",
            RowsStyle::OneBar => "onebar",
            RowsStyle::Aligned => "aligned",
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            RowsStyle::Focus => "Rows: Active Part in Focus (A)",
            RowsStyle::Packed => "Rows: Packed (B)",
            RowsStyle::OneBar => "Rows: One Bar by Part (C)",
            RowsStyle::Aligned => "Rows: Aligned (build 5)",
        }
    }
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// What the one bar (C) shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PartFilter {
    All,
    /// A row per part.
    Split,
    Part(u64),
}

thread_local! {
    static STYLE: std::cell::Cell<RowsStyle> = const { std::cell::Cell::new(RowsStyle::Focus) };
    static FILTER: std::cell::Cell<PartFilter> = const { std::cell::Cell::new(PartFilter::All) };
    static JOINTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn rows_style() -> RowsStyle {
    STYLE.with(std::cell::Cell::get)
}
pub(crate) fn set_rows_style(style: RowsStyle) {
    STYLE.with(|s| s.set(style));
}
pub(crate) fn part_filter() -> PartFilter {
    FILTER.with(std::cell::Cell::get)
}
pub(crate) fn set_part_filter(filter: PartFilter) {
    FILTER.with(|f| f.set(filter));
}

/// Whether a design with parts shows its joints in the History (hidden until asked for from the
/// History's menu). A design without parts shows them as it always has.
pub(crate) fn show_joints(app: &SolveApp) -> bool {
    !app.session.doc.has_parts() || JOINTS.with(std::cell::Cell::get)
}
pub(crate) fn set_show_joints(on: bool) {
    JOINTS.with(|j| j.set(on));
}

/// Whether a design with parts shows the bottom History as the rows panel: every style but the one
/// bar, which is the History bar itself unless split into rows; and the grid under Large Chips.
pub(crate) fn rows_panel(app: &SolveApp) -> bool {
    app.session.doc.has_parts() && (large_chips() || rows_style() != RowsStyle::OneBar || part_filter() == PartFilter::Split)
}

/// The DM-4.4 choices in the History's menus: the rows style, the joints and the part tree.
pub(crate) fn history_items(app: &SolveApp) -> Vec<crate::ui_kit::Item> {
    use crate::ui_kit::Item;
    if !app.session.doc.has_parts() {
        return Vec::new();
    }
    let mut items = vec![Item::sep()];
    for style in RowsStyle::ALL {
        let item = Item::action("ui.dm44RowsStyle", style.label(), "timeline_2d").with(json!({ "style": style.id() }));
        items.push(if rows_style() == style { item.key("current") } else { item });
    }
    if rows_style() == RowsStyle::OneBar {
        let filters = std::iter::once((PartFilter::All, "Filter: All parts".to_string()))
            .chain(std::iter::once((PartFilter::Split, "Filter: All, split".to_string())))
            .chain(crate::timeline::parts_rows::rows(app).into_iter().map(|(part, name, _)| (PartFilter::Part(part), format!("Filter: {name}"))));
        for (filter, label) in filters {
            let params = match filter {
                PartFilter::All => json!({ "filter": "all" }),
                PartFilter::Split => json!({ "filter": "split" }),
                PartFilter::Part(part) => json!({ "filter": "part", "part": part }),
            };
            let item = Item::action("ui.dm44PartFilter", &label, "filter").with(params);
            items.push(if part_filter() == filter { item.key("current") } else { item });
        }
    }
    let joints = JOINTS.with(std::cell::Cell::get);
    items.push(Item::action("ui.dm44Joints", if joints { "Hide Joints" } else { "Show Joints" }, "joint").with(json!({ "on": !joints })));
    // The vertical History (docked at the left) draws each part as a node of its tree.
    let on = app.preferences.timeline_left;
    let label = if on { "Show History as Rows" } else { "Group History by Part" };
    items.push(Item::action("ui.historyPartTree", label, "box").with(json!({ "on": !on })));
    items
}

/// The joint's owner: the innermost part holding both of its components, else the design (0). Most
/// joints are the main design's; a sub-assembly's own joints sit inside its node.
pub(crate) fn joint_owner(doc: &solidcraft_engine::doc::Document, joint: &solidcraft_engine::doc::joints::Joint) -> u64 {
    let parts_above = |occurrence: u64| -> Vec<u64> {
        let mut out = Vec::new();
        let mut c = doc.occurrences.iter().find(|o| o.id == occurrence).map_or(0, |o| o.component);
        for _ in 0..64 {
            let Some(comp) = doc.components.iter().find(|x| x.id == c) else { break };
            if comp.timeline {
                out.push(comp.id);
            }
            if comp.parent == 0 || comp.parent == comp.id {
                break;
            }
            c = comp.parent;
        }
        out
    };
    let a = parts_above(joint.a.part());
    let b = parts_above(joint.b.part());
    // A joint between a part and something outside it belongs above the part.
    a.into_iter()
        .find(|p| b.contains(p))
        .filter(|p| {
            let ca = doc.occurrences.iter().find(|o| o.id == joint.a.part()).map(|o| o.component);
            let cb = doc.occurrences.iter().find(|o| o.id == joint.b.part()).map(|o| o.component);
            ca != Some(*p) && cb != Some(*p)
        })
        .unwrap_or(0)
}

/// The colour that tints a part's chips in the one bar (C).
pub(crate) fn part_color(app: &SolveApp, part: u64) -> egui::Color32 {
    const COLORS: [egui::Color32; 8] = [
        egui::Color32::from_rgb(0x9a, 0xa4, 0xb4),
        egui::Color32::from_rgb(0x2b, 0xb3, 0xa3),
        egui::Color32::from_rgb(0xe0, 0x8a, 0x3c),
        egui::Color32::from_rgb(0x9b, 0x6e, 0xe6),
        egui::Color32::from_rgb(0xe0, 0x60, 0x9a),
        egui::Color32::from_rgb(0xd8, 0xc0, 0x40),
        egui::Color32::from_rgb(0x6c, 0xbf, 0x4a),
        egui::Color32::from_rgb(0xe0, 0x5a, 0x5a),
    ];
    let at = app.session.doc.parts().iter().position(|p| *p == part).unwrap_or(0);
    COLORS[at % COLORS.len()]
}

thread_local! {
    /// A height the rows panel asked for, kept per thread (each test's app is its own).
    static ROWS_HEIGHT: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}

/// The rows panel asks to be this tall; applied before the next frame lays the panels out.
pub(crate) fn request_rows_height(width: f32, height: f32) {
    ROWS_HEIGHT.with(|h| h.set(Some((width, height))));
}

/// Applies a height the rows panel asked for (outside the panels' own drawing).
pub(crate) fn apply_rows_height(app: &mut SolveApp) {
    apply_pending_dock(app);
    let Some((width, height)) = ROWS_HEIGHT.with(std::cell::Cell::take) else { return };
    let params = json!({
        "target": {"type": "instance", "id": "timeline-2d"},
        "size": {"width": width, "height": height},
        "thickness_only": true,
    });
    // The panel may be closed or elsewhere by now: nothing to resize then.
    let _ = app.run_panel_command("panel.resize", &params);
}

thread_local! {
    static LARGE_CHIPS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The rows of a design with parts show the 2D Timeline's grid instead (an option).
pub(crate) fn large_chips() -> bool {
    LARGE_CHIPS.with(std::cell::Cell::get)
}

pub(crate) fn set_large_chips(on: bool) {
    LARGE_CHIPS.with(|l| l.set(on));
}

/// `ui.dm44 {style?, filter?, part?, joints?, part_tree?, large_chips?}` (control port, prototype):
/// sets the History's choices for a design with parts and returns them.
pub(crate) fn control(app: &mut SolveApp, p: &serde_json::Value) -> serde_json::Value {
    if let Some(style) = p["style"].as_str().and_then(RowsStyle::from_id) {
        set_rows_style(style);
    }
    match p["filter"].as_str() {
        Some("all") => set_part_filter(PartFilter::All),
        Some("split") => set_part_filter(PartFilter::Split),
        Some("part") => set_part_filter(p["part"].as_u64().map_or(PartFilter::All, PartFilter::Part)),
        _ => {}
    }
    if let Some(on) = p["joints"].as_bool() {
        set_show_joints(on);
    }
    if let Some(on) = p["large_chips"].as_bool() {
        set_large_chips(on);
    }
    if let Some(on) = p["part_tree"].as_bool() {
        set_part_tree(app, on);
    }
    let filter = match part_filter() {
        PartFilter::All => json!("all"),
        PartFilter::Split => json!("split"),
        PartFilter::Part(part) => json!(part),
    };
    json!({
        "style": rows_style().id(),
        "filter": filter,
        "joints": JOINTS.with(std::cell::Cell::get),
        "large_chips": large_chips(),
        "part_tree": app.preferences.timeline_left,
    })
}
