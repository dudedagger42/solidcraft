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

/// How one row is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    /// The History bar, every feature at its place in the one history (other parts' places empty).
    Full,
    /// A thin strip with a tick per feature at its place (A, a part not in focus).
    Thin,
    /// The History bar with only this part's features, packed (B).
    Packed,
}

/// A thin strip's height (A).
const THIN_HEIGHT: f32 = 12.0;

/// The x of slot `slot` from a strip's left, as `history_bar` places its marker there.
fn slot_x(slot: usize, steps: usize) -> f32 {
    match slot {
        0 => 6.0,
        s if s >= steps => 16.0 + STEP_WIDTH * steps as f32 + 6.0,
        s => 16.0 + STEP_WIDTH * s as f32 - 2.0,
    }
}

/// Draws the part rows into the panel's area, in the chosen style (`parts_ui::RowsStyle`).
pub(crate) fn part_rows(app: &mut SolveApp, ui: &mut egui::Ui) {
    use crate::parts_ui::RowsStyle;
    crate::browser::sync_reveals(app);
    let t = Tokens::get();
    let rows = rows(app);
    let n = app.session.doc.features.len();
    let marker = app.session.doc.marker.unwrap_or(n).min(n);
    let accent = crate::workspace::tab_accent(&t, &app.ui.tab);
    let style = crate::parts_ui::rows_style();
    let active_part = app.session.doc.part_of(app.session.active_component);
    let focus = rows.iter().find(|r| r.0 == active_part).or(rows.first()).map(|r| r.0);
    let hover_id = egui::Id::unique("dm44_rows_hover");
    let hovered_part: Option<u64> = ui.data(|d| d.get_temp(hover_id));
    let looks: Vec<Look> = rows
        .iter()
        .map(|(part, ..)| match style {
            RowsStyle::Packed => Look::Packed,
            RowsStyle::Focus if Some(*part) != focus && Some(*part) != hovered_part => Look::Thin,
            _ => Look::Full,
        })
        .collect();
    let heights: Vec<f32> = looks.iter().map(|l| if *l == Look::Thin { THIN_HEIGHT } else { ROW_HEIGHT }).collect();
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = 0.0;
    for h in &heights {
        tops.push(y);
        y += h;
    }
    let total: f32 = heights.iter().sum();
    let indices: Vec<Vec<usize>> = rows.iter().map(|(_, _, mine)| (0..n).filter(|i| mine[*i]).collect()).collect();
    let longest = indices.iter().map(Vec::len).max().unwrap_or(0);
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
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut actions = Vec::new();
    let mut hovered = None;
    let mut over_row = None;
    egui::ScrollArea::vertical().id_salt("dm44_part_rows").auto_shrink([false, false]).show(ui, |ui| {
        let top = ui.max_rect().top();
        let (all, _) = ui.allocate_exact_size(vec2(area.width(), total), Sense::hover());
        for (k, (part, name, _)) in rows.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(all.left(), top + tops[k]), vec2(LABEL_WIDTH, heights[k]));
            label(app, ui, rect, *part, name, looks[k] == Look::Thin);
            let whole = Rect::from_min_size(rect.min, vec2(all.width(), heights[k]));
            if let Some(p) = pointer
                && whole.contains(p)
                && ui.clip_rect().contains(p)
            {
                over_row = Some(*part);
            }
        }
        let strips = Rect::from_min_max(pos2(all.left() + LABEL_WIDTH, all.top()), all.right_bottom());
        ui.painter().line_segment([strips.left_top(), strips.left_bottom()], Stroke::new(1.0, t.border.gamma_multiply(0.5)));
        ui.scope_builder(UiBuilder::new().max_rect(strips), |ui| {
            egui::ScrollArea::horizontal().id_salt("dm44_part_strips").auto_shrink([false, false]).show(ui, |ui| {
                let left = ui.max_rect().left();
                let places = if style == RowsStyle::Packed { longest } else { n };
                let width = (2.0 * t.space_16 + places as f32 * STEP_WIDTH + t.space_12).max(ui.max_rect().width());
                let (content, _) = ui.allocate_exact_size(vec2(width, total), Sense::hover());
                let mut marker_x = None;
                let mut packed_hover = None;
                for (k, (part, _, mine)) in rows.iter().enumerate() {
                    let row = Rect::from_min_size(pos2(left, content.top() + tops[k]), vec2(width, heights[k]));
                    if k > 0 {
                        ui.painter().line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, t.border.gamma_multiply(0.25)));
                    }
                    match looks[k] {
                        Look::Full => {
                            let mut steps = PartSteps { app: &*app, rows: &bar_rows, mine, marker, from_view, picked: &picked, own: None };
                            let out = history_bar::strip(ui, &bar, row, row, ("dm44_strip", *part), &mut steps);
                            hovered = hovered.or(out.hovered);
                            marker_x = marker_x.or(out.marker.map(|m| m.center().x));
                            actions.extend(out.actions);
                        }
                        Look::Thin => thin_strip(ui, row, &indices[k], marker, &picked, &bar_rows, accent.color, *part),
                        Look::Packed => {
                            let own = &indices[k];
                            let before = own.iter().filter(|i| **i < marker).count();
                            let packed = history_bar::Bar { marker: before, steps: own.len(), reorder: false, paint_marker: true, ..bar };
                            let mut steps = PartSteps { app: &*app, rows: &bar_rows, mine, marker, from_view, picked: &picked, own: Some(own) };
                            let out = history_bar::strip(ui, &packed, row, row, ("dm44_packed", *part), &mut steps);
                            packed_hover = packed_hover.or(out.hovered);
                            hovered = hovered.or(out.hovered);
                            for action in out.actions {
                                // The marker dropped where it already is changes nothing.
                                if matches!(action, history_bar::Action::Seek { slot, .. } if slot == before) {
                                    continue;
                                }
                                actions.push(match action {
                                    // A slot of this row is a place in the one history.
                                    history_bar::Action::Seek { slot, before } => history_bar::Action::Seek {
                                        slot: own.get(slot).copied().unwrap_or_else(|| own.last().map_or(n, |i| i + 1).max(marker)),
                                        before,
                                    },
                                    other => other,
                                });
                            }
                        }
                    }
                }
                // One marker through every row, as the bar draws its own.
                if let Some(x) = marker_x {
                    let rect =
                        Rect::from_min_max(pos2(x - t.space_12 / 2.0, content.top() + 5.0), pos2(x + t.space_12 / 2.0, content.bottom() - 5.0));
                    crate::ui_kit::history_marker(ui.painter(), rect, false, accent);
                }
                // Packed: the step under the pointer lines up with the same moment in every other row.
                if let Some(at) = packed_hover.and_then(|id| app.session.doc.feature_index(id)) {
                    for (k, own) in indices.iter().enumerate() {
                        if own.contains(&at) {
                            continue;
                        }
                        let slot = own.iter().filter(|i| **i < at).count();
                        let x = left + slot_x(slot, own.len());
                        let y0 = content.top() + tops[k];
                        ui.painter()
                            .line_segment([pos2(x, y0 + 6.0), pos2(x, y0 + heights[k] - 6.0)], Stroke::new(2.0, accent.color.gamma_multiply(0.55)));
                    }
                }
            });
        });
    });
    ui.data_mut(|d| match over_row {
        Some(part) => {
            d.insert_temp(hover_id, part);
        }
        None => {
            d.remove::<u64>(hover_id);
        }
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
    // Rows show three at least; Focus shows exactly what it draws.
    let needed = if style == RowsStyle::Focus { total } else { rows.len().max(3) as f32 * ROW_HEIGHT };
    fit(ui, area, needed);
}

/// A part not in focus (A): a thin strip with a tick at each of its features' places, accent when
/// picked, faded after the marker. Hovering the row opens it.
#[allow(clippy::too_many_arguments)]
fn thin_strip(
    ui: &mut egui::Ui,
    row: Rect,
    own: &[usize],
    marker: usize,
    picked: &HashSet<u64>,
    bar_rows: &BarRows,
    accent: egui::Color32,
    part: u64,
) {
    let t = Tokens::get();
    let p = ui.painter();
    for at in own {
        let x = row.left() + 16.0 + STEP_WIDTH * *at as f32;
        let tick = Rect::from_min_size(pos2(x + 2.0, row.center().y - 2.0), vec2(24.0, 4.0));
        let id = bar_rows.features.get(*at).map_or(0, |f| f.0);
        let color = if picked.contains(&id) {
            accent
        } else if *at >= marker {
            t.text_dim.gamma_multiply(0.35)
        } else {
            t.text_dim.gamma_multiply(0.8)
        };
        p.rect_filled(tick, 2.0, color);
    }
    crate::scenario::publish_rect(&format!("dm44_thin:{part}"), row);
}

/// The panel takes the rows' height: a taller or shorter one is put back once a drag ends.
fn fit(ui: &egui::Ui, area: Rect, rows_height: f32) {
    if ui.input(|i| i.pointer.any_down()) {
        return;
    }
    let needed = rows_height + crate::timeline_2d::canvas::SCROLLBAR_RESERVE_HEIGHT;
    if area.height() - needed <= 4.0 {
        return;
    }
    let t = Tokens::get();
    let chrome = crate::panel_runtime::HostMetrics::default().header_height + 2.0 * t.space_2;
    crate::parts_ui::request_rows_height(area.width().max(1.0), needed + chrome);
}

/// A part's label: its active radio, its name and its eye. A click activates the part; the menu
/// renames it, shows or hides it, and turns Large Chips on.
fn label(app: &mut SolveApp, ui: &mut egui::Ui, rect: Rect, part: u64, name: &str, thin: bool) {
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
    let eye = if thin { Rect::NOTHING } else { Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(22.0, ROW_HEIGHT)) };
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
    // The active radio, as the Browser draws it (a thin strip has only its name).
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    if !thin {
        p.circle_stroke(dot, 5.0, Stroke::new(1.2, if active { t.accent } else { t.text_dim }));
        if active {
            p.circle_filled(dot, 2.8, t.accent);
        }
    }
    let color = if !visible {
        t.text_dim.gamma_multiply(0.55)
    } else if active {
        t.text
    } else {
        t.text_dim
    };
    let left = rect.left() + 23.0;
    let right = if thin { rect.right() - 4.0 } else { eye.left() - 2.0 };
    let font = if thin { t.font_micro } else { t.font_small };
    let mut job = egui::text::LayoutJob::simple(name.to_string(), FontId::proportional(font), color, (right - left).max(10.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    let galley = p.layout_job(job);
    p.galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, color);
    if !thin {
        let icon = Rect::from_center_size(eye.center(), vec2(14.0, 14.0));
        crate::icons::paint_icon(&p, icon, if visible { "eye" } else { "suppress" }, if visible { 0.85 } else { 0.4 });
    }
}

/// One part's row: its features at their places in the history, the other places left empty; or,
/// packed (`own`), only its features, one slot each.
struct PartSteps<'a> {
    app: &'a SolveApp,
    rows: &'a BarRows,
    /// Which of the history's features are this part's.
    mine: &'a [bool],
    marker: usize,
    from_view: [Option<u64>; 2],
    picked: &'a HashSet<u64>,
    /// Packed: this part's features' places in the history, in order.
    own: Option<&'a [usize]>,
}

impl history_bar::Contents for PartSteps<'_> {
    fn entries(&mut self, s: &mut history_bar::Strip<'_, '_>) {
        let t = Tokens::get();
        let app = self.app;
        let sketch_states = crate::sketch_state::SketchStates::of(app);
        let results = BarResults::of(&self.rows.features, self.rows.unique_ids, &app.session.model.results);
        let places: Vec<usize> = match self.own {
            Some(own) => own.to_vec(),
            None => (0..self.rows.features.len()).collect(),
        };
        for (slot, at) in places.into_iter().enumerate() {
            let Some(feature) = self.rows.features.get(at) else { continue };
            if !self.mine.get(at).copied().unwrap_or(false) {
                // Another part's place: the same width as a chip, nothing drawn.
                s.chip(slot, slot + 1, false, 0, |_, _, _| 28.0);
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
                slot,
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

/// One bar by part (C) is the History bar itself, unless split into rows.
pub(crate) fn one_bar(app: &SolveApp) -> bool {
    app.session.doc.has_parts() && crate::parts_ui::rows_style() == crate::parts_ui::RowsStyle::OneBar
}

/// The one bar's part filter at `x` in the bar `r`: All parts, one part, or All, split (a row per
/// part). Returns where it ends.
pub(crate) fn filter(app: &mut SolveApp, ui: &mut egui::Ui, x: f32, r: Rect) -> f32 {
    use crate::parts_ui::PartFilter;
    let t = Tokens::get();
    let parts = rows(app);
    let current = crate::parts_ui::part_filter();
    let name_of = |f: PartFilter| match f {
        PartFilter::All => "All parts".to_string(),
        PartFilter::Split => "All, split".to_string(),
        PartFilter::Part(p) => parts.iter().find(|r| r.0 == p).map_or_else(|| "Part".to_string(), |r| r.1.clone()),
    };
    let rect = Rect::from_min_size(pos2(x + t.space_4, r.center().y - 12.0), vec2(132.0, 24.0));
    let mut choices = vec![PartFilter::All, PartFilter::Split];
    choices.extend(parts.iter().map(|row| PartFilter::Part(row.0)));
    let labels: Vec<String> = choices.iter().map(|f| name_of(*f)).collect();
    let options: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut at = choices.iter().position(|f| *f == current).unwrap_or(0);
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_width(rect.width());
        crate::ui_kit::dropdown(ui, "dm44_part_filter", &mut at, &options);
    });
    let chosen = choices.get(at).copied().unwrap_or(PartFilter::All);
    crate::scenario::publish_handle("dm44:part_filter", rect.center());
    if chosen != current {
        crate::parts_ui::set_part_filter(chosen);
    }
    rect.right() + t.space_8
}
