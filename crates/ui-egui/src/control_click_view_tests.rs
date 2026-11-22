//! #20683: on macOS a Control+click opens the context menu, and a Control+drag still orbits.
use egui::{Event, Modifiers, PointerButton};
use solidcraft_engine::geom::Vec3Geom;

const CTRL: Modifiers = Modifiers { ctrl: true, ..Modifiers::NONE };

fn mac_view_point(h: &mut crate::scenario::Harness) -> egui::Pos2 {
    h.ctx.set_os(egui::os::OperatingSystem::Mac);
    h.app.home.open = false;
    h.settle();
    h.app.viewport.rect.unwrap().left_top() + egui::vec2(60.0, 60.0)
}

fn press(at: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: CTRL }
}

#[test]
fn a_control_click_in_the_view_opens_its_context_menu_on_macos() {
    let mut h = crate::scenario::Harness::new();
    let at = mac_view_point(&mut h);
    h.frame_with(vec![Event::ModifiersChanged(CTRL), Event::PointerMoved(at)]);
    h.frame_with(vec![press(at, true)]);
    h.frame_with(vec![press(at, false)]);
    h.frames(2);
    assert!(h.app.viewport.context_menu.is_some(), "Control+click opens the view's context menu");
}

#[test]
fn a_control_drag_in_the_view_still_orbits_on_macos() {
    let mut h = crate::scenario::Harness::new();
    let at = mac_view_point(&mut h);
    let before = h.app.cam.back();
    h.frame_with(vec![Event::ModifiersChanged(CTRL), Event::PointerMoved(at)]);
    h.frame_with(vec![press(at, true)]);
    for step in 1..=8 {
        h.frame_with(vec![Event::PointerMoved(at + egui::vec2(12.0 * step as f32, 0.0))]);
    }
    h.frame_with(vec![press(at + egui::vec2(96.0, 0.0), false)]);
    h.frames(2);
    assert!(h.app.cam.back().dist(before) > 0.05, "Control+drag from empty space orbits the view");
    assert!(h.app.viewport.context_menu.is_none(), "a drag opens no menu");
}

#[test]
fn probe_20683() {
    for mac in [true, false] {
        let mut h = crate::scenario::Harness::new();
        if mac { h.ctx.set_os(egui::os::OperatingSystem::Mac); }
        h.app.home.open = false;
        h.settle();
        let r = h.app.viewport.rect.unwrap();
        let at = r.left_top() + egui::vec2(60.0, 60.0);
        eprintln!("PROBE mac={mac} rect={r:?} at={at:?} menu0={:?} nav={:?}", h.app.viewport.context_menu, h.app.preferences.nav);
        h.frame_with(vec![Event::ModifiersChanged(CTRL), Event::PointerMoved(at)]);
        eprintln!("PROBE moved menu={:?} mouse={:?}", h.app.viewport.context_menu, h.app.viewport.mouse);
        h.frame_with(vec![press(at, true)]);
        eprintln!("PROBE pressed menu={:?}", h.app.viewport.context_menu);
        h.frame_with(vec![press(at, false)]);
        eprintln!("PROBE released menu={:?} sel={:?}", h.app.viewport.context_menu, h.app.session.selection.len());
        h.frames(2);
        eprintln!("PROBE after menu={:?}", h.app.viewport.context_menu);
        let back = h.app.cam.back();
        h.frame_with(vec![Event::PointerMoved(at + egui::vec2(200.0, 100.0))]);
        h.frame_with(vec![Event::PointerButton { pos: at + egui::vec2(200.0, 100.0), button: PointerButton::Primary, pressed: true, modifiers: CTRL }]);
        for k in 1..=8 { h.frame_with(vec![Event::PointerMoved(at + egui::vec2(200.0 + 12.0 * k as f32, 100.0))]); }
        h.frame_with(vec![Event::PointerButton { pos: at + egui::vec2(296.0, 100.0), button: PointerButton::Primary, pressed: false, modifiers: CTRL }]);
        h.frames(2);
        eprintln!("PROBE drag back {back:?} -> {:?}", h.app.cam.back());
    }
}
