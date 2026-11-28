//! Wheel input for the kit's scrolled strips. Camera input never reads the wheel here: it goes
//! through the app's navigation handler.

use egui::Ui;

/// Take a predominantly horizontal wheel gesture for a hovered scroll viewport.
/// Leave vertical input for its enclosing page, and leave horizontal input
/// unconsumed when this viewport is already at the requested end.
pub fn take_horizontal_wheel(ui: &Ui, offset: f32, max_offset: f32) -> f32 {
    ui.input_mut(|input| {
        let delta = input.smooth_scroll_delta;
        if delta.x == 0.0 || delta.x.abs() <= delta.y.abs() {
            return 0.0;
        }
        if (offset - delta.x).clamp(0.0, max_offset) != offset {
            input.smooth_scroll_delta.x = 0.0;
        }
        delta.x
    })
}

/// Over `ui`'s visible area, on top of any popup, turn a purely vertical wheel into a sideways one,
/// so a plain wheel scrolls a horizontal strip that can also scroll up and down. A wheel that
/// already moves sideways (a trackpad, or Shift, which egui turns into a sideways wheel) is left as
/// it is, and so is a wheel anywhere else.
pub fn vertical_wheel_sideways(ui: &Ui) {
    let over =
        ui.input(|input| input.pointer.hover_pos()).is_some_and(|pos| ui.clip_rect().contains(pos) && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()));
    if over {
        ui.input_mut(|input| {
            if input.smooth_scroll_delta.x == 0.0 {
                input.smooth_scroll_delta = egui::vec2(input.smooth_scroll_delta.y, 0.0);
            }
        });
    }
}

/// Give a panel the wheel delta left by its children, one movable axis at a time.
/// Boundary deltas remain available to the panel host's input fallback.
pub fn panel_scroll_offset(ctx: &egui::Context, previous: egui::Vec2, maximum: egui::Vec2) -> egui::Vec2 {
    let mut offset = previous;
    ctx.input_mut(|input| {
        if maximum.x > 0.0 && input.smooth_scroll_delta.x != 0.0 {
            offset.x = (previous.x - input.smooth_scroll_delta.x).clamp(0.0, maximum.x);
            if offset.x != previous.x {
                input.smooth_scroll_delta.x = 0.0;
            }
        }
        if maximum.y > 0.0 && input.smooth_scroll_delta.y != 0.0 {
            offset.y = (previous.y - input.smooth_scroll_delta.y).clamp(0.0, maximum.y);
            if offset.y != previous.y {
                input.smooth_scroll_delta.y = 0.0;
            }
        }
    });
    offset
}
