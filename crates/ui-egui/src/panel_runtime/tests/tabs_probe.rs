//! Probe: where a docked side panel lands when let go near another panel's edges, no modifier held.
//! Prints only; it runs unchanged on every build being compared.

use super::*;

#[derive(Clone, Copy, Debug)]
enum Setup {
    OtherColumn,
    SameZoneAdjacentColumn,
    SameColumn,
    Floating,
    Bottom,
}

fn probe_harness(setup: Setup) -> (crate::scenario::Harness, &'static str) {
    let mut h = production_content_harness(false, false);
    h.app.preferences.language = "en".into();
    h.frames(3);
    let target = match setup {
        Setup::OtherColumn | Setup::SameZoneAdjacentColumn | Setup::Floating => {
            h.app.run("view.parts_library", serde_json::json!({"visible": true})).unwrap();
            h.frames(4);
            "parts-library"
        }
        Setup::SameColumn => {
            h.app.run("view.history_layout", serde_json::json!({"dock": "under_browser"})).unwrap();
            h.frames(4);
            "history"
        }
        Setup::Bottom => "history",
    };
    let zone = match setup {
        Setup::OtherColumn => Some(Zone::Right),
        Setup::Floating => Some(Zone::Floating),
        _ => None,
    };
    if let Some(zone) = zone {
        probe_click(&mut h, HostControlTarget::Pointer(HostPointerTarget::Menu { instance: id(target) }));
        probe_click(&mut h, HostControlTarget::Shared { instance: id(target), action: HostSharedAction::Move(zone) });
        h.frames(4);
    }
    (h, target)
}

fn probe_click(h: &mut crate::scenario::Harness, target: HostControlTarget) {
    let at = egui_rect(app_control(&h.app, target).bounds).center();
    h.frame_with(vec![egui::Event::PointerMoved(at)]);
    h.frame_with(vec![button_event(at, true)]);
    h.frame_with(vec![button_event(at, false)]);
    h.frames(3);
}

fn probe_grab(h: &crate::scenario::Harness, instance: &str) -> egui::Pos2 {
    let frame = h.app.universal_panel_frame().unwrap();
    let find = |target: HostPointerTarget| frame.controls.iter().find(|c| c.target == HostControlTarget::Pointer(target.clone()));
    let control = find(HostPointerTarget::Header { instance: id(instance) }).or_else(|| find(HostPointerTarget::Tab { instance: id(instance) })).unwrap();
    egui_rect(control.bounds).center()
}

fn probe_card(h: &crate::scenario::Harness, instance: &str) -> HostSection {
    let frame = h.app.universal_panel_frame().unwrap();
    frame.layout.sections.iter().find(|card| card.instance == id(instance) || card.tabs.iter().any(|tab| tab.instance == id(instance))).unwrap().clone()
}

fn probe_where(h: &crate::scenario::Harness, instance: &str) -> String {
    let layout = h.app.panel_runtime().unwrap().layout().layout().clone();
    let section = layout.containing(&id(instance)).unwrap();
    let index = layout.sections.iter().position(|s| s.id == section.id).unwrap();
    format!(
        "{}:{:?}/col={:?}/sec={}#{}/tabs={:?}",
        instance,
        section.location.zone(),
        section.column.as_ref().map(|c| c.0.clone()),
        section.id.0,
        index,
        section.tabs.iter().map(|t| t.0.clone()).collect::<Vec<_>>()
    )
}

fn probe_layout(h: &crate::scenario::Harness) -> String {
    let frame = h.app.universal_panel_frame().unwrap();
    let mut out = format!("cap={:?} ", frame.layout.capacity);
    for card in &frame.layout.sections {
        out += &format!("[{} {:?} {:?}] ", card.instance.0, card.zone, card.outer);
    }
    out
}

fn probe_drop(setup: Setup, point: &str) -> String {
    let source = "browser";
    let (mut h, target) = probe_harness(setup);
    let before = h.app.panel_runtime().unwrap().layout().layout().clone();
    let src_before = probe_where(&h, source);
    let tgt_before = probe_where(&h, target);
    let o = probe_card(&h, target).outer;
    let (x0, y0, w, hh) = (o.origin.x, o.origin.y, o.size.width, o.size.height);
    let to = match point {
        "left-edge" => egui::pos2(x0 + 6.0, y0 + hh * 0.5),
        "right-edge" => egui::pos2(x0 + w - 6.0, y0 + hh * 0.5),
        "top-band" => egui::pos2(x0 + w * 0.5, y0 + 56.0_f32.min(hh * 0.25) * 0.5),
        "bottom-third" => egui::pos2(x0 + w * 0.5, y0 + hh - hh / 6.0),
        _ => egui::pos2(x0 + w * 0.5, y0 + hh * 0.5),
    };
    let from = probe_grab(&h, source);
    h.frame_with(vec![egui::Event::PointerMoved(from)]);
    h.frame_with(vec![button_event(from, true)]);
    h.frame_with(vec![egui::Event::PointerMoved(from + (to - from) * 0.5)]);
    h.frame_with(vec![egui::Event::PointerMoved(to)]);
    let preview = h.app.universal_panel_frame().and_then(|frame| frame.preview.clone());
    h.frame_with(vec![button_event(to, false)]);
    h.frames(4);
    let after = h.app.panel_runtime().unwrap().layout().layout().clone();
    let src = after.containing(&id(source)).unwrap().clone();
    let tgt = after.containing(&id(target)).unwrap().clone();
    let class = if after == before {
        "RETURNED".to_string()
    } else if src.id == tgt.id {
        "TAB".to_string()
    } else if src.location.zone() == Zone::Floating {
        "FLOATED".to_string()
    } else if src.column.is_some() && src.column == tgt.column {
        let si = after.sections.iter().position(|s| s.id == src.id).unwrap();
        let ti = after.sections.iter().position(|s| s.id == tgt.id).unwrap();
        format!("STACKED-{}", if si < ti { "ABOVE" } else { "BELOW" })
    } else if src.location.zone() == tgt.location.zone() {
        "SPLIT-BESIDE".to_string()
    } else {
        format!("MOVED-TO-{:?}", src.location.zone())
    };
    let (target_id, rejection) = preview.map_or((None, None), |p| (p.target.clone(), p.rejection.map(|r| r.detail.to_string())));
    format!(
        "PROBE {setup:?} {point}: {class} | preview={target_id:?} rej={rejection:?} at={to:?} card={o:?}\n    before: {src_before} ; {tgt_before}\n    after:  {} ; {}",
        probe_where(&h, source),
        probe_where(&h, target)
    )
}

#[test]
fn tabs_probe_docked_drop_landings() {
    for setup in [Setup::OtherColumn, Setup::SameZoneAdjacentColumn, Setup::SameColumn, Setup::Floating, Setup::Bottom] {
        let (h, _) = probe_harness(setup);
        eprintln!("LAYOUT {setup:?}: {}", probe_layout(&h));
        for point in ["left-edge", "right-edge", "top-band", "bottom-third", "centre"] {
            eprintln!("{}", probe_drop(setup, point));
        }
    }
}
