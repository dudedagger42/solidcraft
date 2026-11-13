use super::*;
use crate::scenario::Harness;
use solidcraft_engine::geom::Aabb3;

fn perspective(rect: Rect) -> Proj {
    let cam = solidcraft_viewport::Camera { fov: 0.8, ..Default::default() };
    Proj { cam, vp: cam.view_proj(f64::from(rect.width() / rect.height()), 100.0), rect }
}

/// A box in front of the eye has its rectangle on screen; one that reaches behind the eye in
/// perspective has none (its corners in front would make it too small), so the value box keeps
/// clear of the whole view instead.
#[test]
fn bounds_behind_the_eye_give_no_screen_rectangle() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let proj = perspective(rect);
    let (eye, target) = (proj.cam.eye(), proj.cam.target);
    let around = |p: Vec3, r: f64| Aabb3 { min: p - Vec3::new(r, r, r), max: p + Vec3::new(r, r, r) };
    let front = screen_rect(&proj, &around(target, 1.0)).expect("a box at the target is on screen");
    assert!(front.is_positive() && rect.contains_rect(front), "{front:?}");
    let behind = eye + (eye - target) * 0.5;
    let mut through = around(target, 1.0);
    through.add(behind);
    assert_eq!(screen_rect(&proj, &through), None);
}

/// The first Extrude from a sketch has no body yet: the preview alone gives the outline; it
/// is the whole view when the preview reaches behind the eye; nothing without a preview.
#[test]
fn the_preview_gives_the_outline_when_there_is_no_body() {
    let mut h = Harness::new();
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    // A perspective camera, as with the perspective preference on (the harness view is
    // orthographic, where nothing is behind the eye).
    let proj = perspective(rect);
    assert_eq!(model_outline(&h.app, &proj), None);
    let target = proj.cam.target;
    h.app.preview.bounds = Aabb3 { min: target - Vec3::new(5.0, 5.0, 0.0), max: target + Vec3::new(5.0, 5.0, 10.0) };
    h.app.preview.active = true;
    let outline = model_outline(&h.app, &proj).expect("the preview's outline");
    assert_eq!(Some(outline), screen_rect(&proj, &h.app.preview.bounds));
    h.app.preview.rolled_back = true;
    assert_eq!(model_outline(&h.app, &proj), None);
    h.app.preview.rolled_back = false;
    let eye = proj.cam.eye();
    h.app.preview.bounds.add(eye + (eye - target) * 0.5);
    assert_eq!(model_outline(&h.app, &proj), Some(proj.rect));
}
