//! A glass theme's (Liquid Craft's) panels: after the window is laid out, each panel, dialog and
//! bar fill becomes `egui_glass` glass over a backdrop in the theme's colours, with the fill drawn
//! over it more see-through. The widgets keep drawing ordinary fills (`ui_kit::Container`); only
//! the finished paint lists change, and only while a glass theme is on.

use egui::{Color32, ColorImage, Context, Order, Rect, Shape};

use crate::theme::{Glass, Tokens};

/// The backdrop's size in pixels: it is blurred, so a small image stretched over the window will do.
const BACKDROP: usize = 256;

/// The GPU state glass needs: the window's renderer, and the colours of the backdrop uploaded now.
pub(crate) struct GlassState {
    render_state: egui_wgpu::RenderState,
    uploaded: Option<[Color32; 4]>,
}

impl GlassState {
    /// Glass's pipelines in the window's pass (one sample, no depth).
    pub(crate) fn install(render_state: &egui_wgpu::RenderState) -> GlassState {
        egui_glass::init(render_state, 1);
        GlassState { render_state: render_state.clone(), uploaded: None }
    }
}

/// The backdrop: the 3D view's gradient with two soft glows, the accent's and `second`'s, so the
/// blur and the rims have something to bend.
fn backdrop(top: Color32, bottom: Color32, accent: Color32, second: Color32, dark: bool) -> ColorImage {
    let glow = if dark { 0.22 } else { 0.16 };
    let second = second.to_opaque();
    let mut pixels = Vec::with_capacity(BACKDROP * BACKDROP);
    for y in 0..BACKDROP {
        let v = y as f32 / (BACKDROP - 1) as f32;
        let row = top.lerp_to_gamma(bottom, v);
        for x in 0..BACKDROP {
            let u = x as f32 / (BACKDROP - 1) as f32;
            let near = |cx: f32, cy: f32, r: f32| (1.0 - ((u - cx).powi(2) + (v - cy).powi(2)).sqrt() / r).clamp(0.0, 1.0).powi(2);
            let c = row.lerp_to_gamma(accent.to_opaque(), glow * near(0.18, 0.22, 0.55));
            pixels.push(c.lerp_to_gamma(second, glow * near(0.85, 0.8, 0.5)));
        }
    }
    ColorImage::new([BACKDROP, BACKDROP], pixels)
}

/// The glass under one fill: the theme's preset with the theme's blur and rim, the window's
/// preferences on top, the accent's tint when chosen, and a shadow only off the window's bars.
fn style(t: &Tokens, glass: Glass, prefs: &crate::prefs::Prefs, rect: &egui::epaint::RectShape, floating: bool) -> egui_glass::GlassStyle {
    let dark = crate::theme::is_dark();
    let mut s = if dark { egui_glass::GlassStyle::panel_dark() } else { egui_glass::GlassStyle::panel() };
    s.corner_radius = f32::from(rect.corner_radius.nw.max(rect.corner_radius.ne));
    s.blur = glass.blur * prefs.glass_blur;
    s.refraction = glass.refraction;
    if prefs.glass_accent_tint {
        s.tint = t.accent.to_opaque().gamma_multiply(if dark { 34.0 } else { 26.0 } / 255.0);
    }
    if !floating {
        s.shadow = 0.0;
    }
    s
}

/// Turns the frame's panel fills into glass while a glass theme is on; else does nothing.
pub(crate) fn frame(app: &mut crate::SolveApp, ctx: &Context) {
    let t = Tokens::get();
    let (Some(glass), Some(state)) = (t.glass, app.glass.as_mut()) else { return };
    let dark = crate::theme::is_dark();
    let colours = [t.viewport_top, t.viewport_bottom, t.accent, t.box_crossing];
    if state.uploaded != Some(colours) {
        let image = backdrop(t.viewport_top, t.viewport_bottom, t.accent, t.box_crossing, dark);
        egui_glass::set_backdrop(ctx, &state.render_state, &image);
        state.uploaded = Some(colours);
    }
    let screen = ctx.content_rect();
    egui_glass::map_backdrop(ctx, screen, t.viewport_bottom);
    let hosts = [t.panel, t.panel_header, t.toolbar, t.timeline, t.dialog_bg, t.dialog_body, t.app_bar, t.overlay];
    // A fill as see-through as the theme and the preference ask: at 0.5 the theme's own alpha.
    let see_through = (glass.see_through * app.preferences.glass_see_through).clamp(0.0, 1.0);
    let layers: Vec<_> = ctx.memory(|m| m.layer_ids().collect());
    ctx.graphics_mut(|g| {
        for layer in layers {
            let Some(list) = g.get_mut(layer) else { continue };
            let floating = !matches!(layer.order, Order::Background | Order::Middle);
            let mut glassed: Vec<Rect> = Vec::new();
            for i in 0..list.next_idx().0 {
                list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| {
                    let Shape::Rect(r) = &clipped.shape else { return };
                    let small = r.rect.width() < 24.0 || r.rect.height() < 20.0;
                    // The window's own background (the start page, the view's back) stays flat.
                    let whole = r.rect.area() > screen.area() * 0.6;
                    if small || whole || !hosts.contains(&r.fill) || glassed.iter().any(|g| g.contains_rect(r.rect)) {
                        return;
                    }
                    glassed.push(r.rect);
                    let glass_style = style(&t, glass, &app.preferences, r, floating);
                    let alpha = (f32::from(r.fill.a()) / 255.0 * (1.0 - see_through) * 2.0).min(1.0);
                    let mut fill = r.clone();
                    fill.fill = r.fill.to_opaque().gamma_multiply(alpha);
                    clipped.shape = Shape::Vec(vec![egui_glass::glass_shape_in(ctx, r.rect, &glass_style), Shape::Rect(fill)]);
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The backdrop runs from the view's top colour to its bottom one, lit by the accent.
    #[test]
    fn backdrop_is_the_view_gradient_with_an_accent_glow() {
        let (top, bottom, accent) = (Color32::from_rgb(240, 244, 250), Color32::from_rgb(200, 210, 226), Color32::from_rgb(10, 132, 255));
        let image = backdrop(top, bottom, accent, Color32::from_rgb(94, 200, 229), false);
        assert_eq!(image.size, [BACKDROP, BACKDROP]);
        let at = |x: usize, y: usize| image.pixels[y * BACKDROP + x];
        assert!(at(BACKDROP / 2, BACKDROP - 1).r() < at(BACKDROP / 2, 0).r(), "darker at the bottom");
        let glow = at(BACKDROP * 18 / 100, BACKDROP * 22 / 100);
        assert!(glow.b() > glow.r(), "the accent's blue shows near the top left: {glow:?}");
    }
}
