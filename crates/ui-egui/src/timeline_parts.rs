//! DM-4.4 prototype: a design with parts shows one History strip per part, stacked, each drawn by
//! the History bar's own code (`history_bar::strip`, the same chips, spacing and marker). A
//! feature keeps its place in the one history, so a part's row leaves the other parts' places
//! empty and one marker runs through every row. A small label at the left of each row names the
//! part, with its active radio and its eye. The 2D Timeline's grid is the Large Chips option.

use super::{
    BAR_HANDLES, BarResults, BarRows, TimelineKind, click_feature, feature_of_sel, feature_tooltip, has_warning, select_one, selected_feature_ids,
};
use crate::SolveApp;
use crate::history_bar;
use crate::ui_kit::Menu;
use egui::{FontId, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
use serde_json::json;
use solidcraft_theme::Tokens;
use std::collections::HashSet;

/// One row: the History bar's height (a 28-point chip with a six-point inset each side).
pub(crate) const ROW_HEIGHT: f32 = 40.0;
/// The part labels' column.
const LABEL_WIDTH: f32 = 150.0;
/// One feature's place along the strip, as `history_bar` lays a chip out.
const STEP_WIDTH: f32 = 32.0;

/// The rows: each part with the features (their places in the history) that belong to it. The
/// design's own features, when it has any outside every part, come first.
pub(crate) fn rows(app: &SolveApp) -> Vec<(u64, String, Vec<bool>)> {
    let doc = &app.session.doc;
    let homes: Vec<u64> = doc.features.iter().map(|f| doc.part_of(f.component)).collect();
    doc.parts()
        .into_iter()
        .filter_map(|part| {
            let mine: Vec<bool> = homes.iter().map(|home| *home == part).collect();
            if part == 0 && !mine.contains(&true) {
                return None;
            }
            let name = if part == 0 {
                if doc.name.is_empty() { "Design".to_string() } else { doc.name.clone() }
            } else {
                doc.components.iter().find(|c| c.id == part).map_or_else(|| format!("Part {part}"), |c| c.name.clone())
            };
            Some((part, name, mine))
        })
        .collect()
}

/// Draws the part rows into the panel's area.
pub(crate) fn part_rows(app: &mut SolveApp, ui: &mut egui::Ui) {
    crate::browser::sync_reveals(app);
    let t = Tokens::get();
    let rows = rows(app);
    let n = app.session.doc.features.len();
    let marker = app.session.doc.marker.unwrap_or(n).min(n);
    let accent = crate::workspace::tab_accent(&t, &app.ui.tab);
    let bar = history_bar::Bar {
        marker_tip: solidcraft_ui_text::tr("ui.history_marker_tip"),
        accent,
        drag_accent: crate::workspace::tab_accent(&t, "SKETCH"),
        marker,
        steps: n,
        floating: true,
        reorder: true,
        handles: BAR_HANDLES,
        paint_marker: false,
    };
    let from_view = super::from_view(app);
    let doc = &app.session.doc;
    let feature_of = |selection: &solidcraft_engine::Sel| feature_of_sel(doc, selection);
    let picked: HashSet<u64> = app
        .session
        .selection
        .iter()
        .filter_map(feature_of)
        .chain(app.dialog.as_ref().map(|dialog| dialog.items()).unwrap_or_default().iter().filter_map(feature_of))
        .collect();
    let bar_rows = BarRows::cached(app, ui);
    let area = ui.max_rect();
    let total = rows.len() as f32 * ROW_HEIGHT;
    let mut actions = Vec::new();
    let mut hovered = None;
    egui::ScrollArea::vertical().id_salt("dm44_part_rows").auto_shrink([false, false]).show(ui, |ui| {
        let top = ui.max_rect().top();
        let (all, _) = ui.allocate_exact_size(vec2(area.width(), total), Sense::hover());
        for (k, (part, name, _)) in rows.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(all.left(), top + k as f32 * ROW_HEIGHT), vec2(LABEL_WIDTH, ROW_HEIGHT));
            label(app, ui, rect, *part, name);
        }
        let strips = Rect::from_min_max(pos2(all.left() + LABEL_WIDTH, all.top()), all.right_bottom());
        ui.painter().line_segment([strips.left_top(), strips.left_bottom()], Stroke::new(1.0, t.border.gamma_multiply(0.5)));
        ui.scope_builder(UiBuilder::new().max_rect(strips), |ui| {
            egui::ScrollArea::horizontal().id_salt("dm44_part_strips").auto_shrink([false, false]).show(ui, |ui| {
                let left = ui.max_rect().left();
                let width = (2.0 * t.space_16 + n as f32 * STEP_WIDTH + t.space_12).max(ui.max_rect().width());
                let (content, _) = ui.allocate_exact_size(vec2(width, total), Sense::hover());
                let mut marker_x = None;
                for (k, (part, _, mine)) in rows.iter().enumerate() {
                    let row = Rect::from_min_size(pos2(left, content.top() + k as f32 * ROW_HEIGHT), vec2(width, ROW_HEIGHT));
                    if k > 0 {
                        ui.painter().line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, t.border.gamma_multiply(0.25)));
                    }
                    let mut steps = PartSteps { app: &*app, rows: &bar_rows, mine, marker, from_view, picked: &picked };
                    let out = history_bar::strip(ui, &bar, row, row, ("dm44_strip", *part), &mut steps);
                    hovered = hovered.or(out.hovered);
                    marker_x = marker_x.or(out.marker.map(|m| m.center().x));
                    actions.extend(out.actions);
                }
                // One marker through every row, as the bar draws its own.
                if let Some(x) = marker_x {
                    let rect =
                        Rect::from_min_max(pos2(x - t.space_12 / 2.0, content.top() + 5.0), pos2(x + t.space_12 / 2.0, content.bottom() - 5.0));
                    crate::ui_kit::history_marker(ui.painter(), rect, false, accent);
                }
            });
        });
    });
    crate::semantic_content::set_timeline_feature_hover(app, hovered);
    let order: Vec<u64> = bar_rows.features.iter().map(|f| f.0).collect();
    for action in actions {
        match action {
            history_bar::Action::Select { step, modifiers } => click_feature(app, &order, step, modifiers),
            history_bar::Action::Edit(id) => app.edit_feature(id),
            history_bar::Action::Menu { step, at } => {
                if !selected_feature_ids(app).contains(&step) {
                    select_one(app, step);
                }
                crate::context_menu::open_for(app, at, crate::context_menu::Target::Timeline { feature: step });
            }
            history_bar::Action::Seek { slot, .. } => {
                let _ = app.run_heavy("timeline.roll_to", if slot >= n { json!({}) } else { json!({ "position": slot }) });
            }
            history_bar::Action::Reorder { step, slot, before } => {
                let from = app.session.doc.feature_index(step).unwrap_or(0);
                let to = if slot > from { slot - 1 } else { slot };
                if to != from || before != (from < marker) {
                    let _ = app.run_heavy("timeline.reorder", json!({ "feature": step, "position": to, "active": before }));
                }
            }
        }
    }
    fit(app, area, rows.len());
}

/// The panel grows only until every row shows (three at least): a taller one is put back once the
/// drag ends.
fn fit(app: &SolveApp, area: Rect, rows: usize) {
    let _ = app;
    let needed = rows.max(3) as f32 * ROW_HEIGHT + crate::timeline_2d::canvas::SCROLLBAR_RESERVE_HEIGHT;
    if area.height() - needed <= 4.0 {
        return;
    }
    let t = Tokens::get();
    let chrome = crate::panel_runtime::HostMetrics::default().header_height + 2.0 * t.space_2;
    crate::parts_ui::request_rows_height(area.width().max(1.0), needed + chrome);
}

/// A part's label: its active radio, its name and its eye. A click activates the part; the menu
/// renames it, shows or hides it, and turns Large Chips on.
fn label(app: &mut SolveApp, ui: &mut egui::Ui, rect: Rect, part: u64, name: &str) {
    let t = Tokens::get();
    let p = ui.painter().clone();
    let active = part == app.session.active_component;
    let first_path = app.session.doc.first_path(part);
    let bodies = crate::browser::bodies_of_component(app, part);
    let visible = if part != 0 && !first_path.is_empty() {
        !app.session.occurrence_hidden(&first_path)
    } else {
        bodies.is_empty() || bodies.iter().any(|b| !app.session.visibility.hidden_bodies.contains(b))
    };
    let eye = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(22.0, ROW_HEIGHT));
    let resp = ui.interact(rect, ui.scope_id().with(("dm44_part_label", part)), Sense::click());
    let eye_resp = ui.interact(eye, ui.scope_id().with(("dm44_part_eye", part)), Sense::click());
    crate::scenario::publish_handle(&format!("part_row:{name}"), rect.center());
    let toggle = |app: &mut SolveApp| {
        if part != 0 && !first_path.is_empty() {
            let _ = app.run("browser.visibility", json!({ "visible": !visible, "paths": [first_path.clone()] }));
        } else if !bodies.is_empty() {
            let _ = app.run("browser.visibility", json!({ "items": bodies.clone(), "visible": !visible }));
        }
    };
    if eye_resp.clicked() {
        toggle(app);
    } else if resp.clicked() && !active {
        let _ = app.run("component.activate", json!({ "component": part }));
    }
    if resp.hovered() || eye_resp.hovered() {
        p.rect_filled(rect, 0.0, t.hover);
    } else if active {
        p.rect_filled(rect, 0.0, t.accent.gamma_multiply(0.08));
    }
    let eye_resp = eye_resp.on_hover_text(solidcraft_ui_text::tr(if visible { "ui.hide" } else { "ui.show" }));
    let _ = eye_resp;
    let resp = resp.on_hover_text(if active { format!("{name} (active)") } else { format!("Click to activate {name}") });
    let at = resp.interact_pointer_pos().unwrap_or(rect.left_bottom());
    Menu::context(&resp, |ui| {
        if part != 0 && crate::ui_kit::menu::button(ui, solidcraft_ui_text::tr_label("Rename Component"), "rename").clicked() {
            crate::context_menu::start_rename(app, crate::context_menu::RenameWhat::Component(part), at);
            ui.close();
        }
        if crate::ui_kit::menu::button(ui, solidcraft_ui_text::tr_label("Show/Hide"), if visible { "eye" } else { "suppress" }).clicked() {
            toggle(app);
            ui.close();
        }
        if crate::ui_kit::menu::button(ui, solidcraft_ui_text::tr("ui.dm44_large_chips"), "box").clicked() {
            crate::parts_ui::set_large_chips(true);
            ui.close();
        }
    });
    // The active radio, as the Browser draws it.
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    p.circle_stroke(dot, 5.0, Stroke::new(1.2, if active { t.accent } else { t.text_dim }));
    if active {
        p.circle_filled(dot, 2.8, t.accent);
    }
    let color = if !visible {
        t.text_dim.gamma_multiply(0.55)
    } else if active {
        t.text
    } else {
        t.text_dim
    };
    let left = rect.left() + 23.0;
    let mut job = egui::text::LayoutJob::simple(name.to_string(), FontId::proportional(t.font_small), color, (eye.left() - 2.0 - left).max(10.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    let galley = p.layout_job(job);
    p.galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, color);
    let icon = Rect::from_center_size(eye.center(), vec2(14.0, 14.0));
    crate::icons::paint_icon(&p, icon, if visible { "eye" } else { "suppress" }, if visible { 0.85 } else { 0.4 });
}

/// One part's row: its features at their places in the history, the other places left empty.
struct PartSteps<'a> {
    app: &'a SolveApp,
    rows: &'a BarRows,
    /// Which of the history's features are this part's.
    mine: &'a [bool],
    marker: usize,
    from_view: [Option<u64>; 2],
    picked: &'a HashSet<u64>,
}

impl history_bar::Contents for PartSteps<'_> {
    fn entries(&mut self, s: &mut history_bar::Strip<'_, '_>) {
        let t = Tokens::get();
        let app = self.app;
        let sketch_states = crate::sketch_state::SketchStates::of(app);
        let results = BarResults::of(&self.rows.features, self.rows.unique_ids, &app.session.model.results);
        for (at, feature) in self.rows.features.iter().enumerate() {
            if !self.mine.get(at).copied().unwrap_or(false) {
                // Another part's place: the same width as a chip, nothing drawn.
                s.chip(at, at + 1, false, 0, |_, _, _| 28.0);
                continue;
            }
            let (id, name, icon, suppressed, ty, kind) = feature;
            let sketch_state = matches!(kind, TimelineKind::Sketch).then(|| sketch_states.get(*id)).flatten();
            let res = results.get(&app.session.model, at, *id);
            let rolled = at >= self.marker;
            let numbered = format!("#{} {name}", at + 1);
            let tip = || feature_tooltip(&numbered, ty, res, rolled, sketch_state);
            s.step(&history_bar::Step {
                id: *id,
                slot: at,
                label: name,
                icon,
                icon_accent: self.rows.accents.get(at).copied().flatten(),
                state: history_bar::StepState {
                    selected: self.picked.contains(id),
                    highlighted: self.from_view.contains(&Some(*id)),
                    rolled_back: rolled,
                    suppressed: *suppressed,
                    last_done: at + 1 == self.marker,
                },
                error: res.is_some_and(|r| r.error.is_some()),
                warning: res.is_some_and(has_warning),
                ink: sketch_state.and_then(|state| state.ink(&t)),
                tip: &tip,
                handle: self.rows.handles.get(at),
            });
        }
    }
}
