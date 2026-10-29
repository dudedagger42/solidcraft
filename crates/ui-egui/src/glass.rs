//! A glass theme's (Liquid Craft's) panels: after the window is laid out, each panel, dialog and
//! bar fill becomes `egui_glass` glass over a backdrop in the theme's colours, with the fill drawn
//! over it more see-through. The widgets keep drawing ordinary fills (`ui_kit::Container`); only
//! the finished paint lists change, and only while a glass theme is on. Over the 3D view the
//! backdrop is the view's own picture, copied each frame, so the model shows through the glass.

use egui::{Color32, ColorImage, Context, Order, Rect, Shape};

use crate::theme::{Glass, Tokens};

/// The backdrop's size in pixels: it is blurred, so a small image stretched over the window will do.
const BACKDROP: usize = 256;

/// The GPU state glass needs: the window's renderer, and the colours of the backdrop uploaded now.
pub(crate) struct GlassState {
    render_state: egui_wgpu::RenderState,
    uploaded: Option<[Color32; 2]>,
}

impl GlassState {
    /// Glass's pipelines in the window's pass (one sample, no depth).
    pub(crate) fn install(render_state: &egui_wgpu::RenderState) -> GlassState {
        egui_glass::init(render_state, 1);
        egui_glass::init_scene_backdrop(render_state);
        GlassState { render_state: render_state.clone(), uploaded: None }
    }
}

/// The backdrop where the 3D view isn't behind the glass: the view's gradient with two soft,
/// neutral glows (lighter, never the accent), so the rims have something to bend.
fn backdrop(top: Color32, bottom: Color32, light: Color32, dark: bool) -> ColorImage {
    let glow = if dark { 0.10 } else { 0.08 };
    let (accent, second) = (top.lerp_to_gamma(light, 0.5), bottom.lerp_to_gamma(light, 0.4));
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

/// Where this frame shows the 3D view, for the backdrop to map onto.
#[derive(Clone, Copy)]
struct View {
    rect: Rect,
    pass: u64,
}

/// Notes that the 3D view is shown in `rect` this frame: glass over it refracts its picture.
pub(crate) fn note_view(ctx: &Context, rect: Rect) {
    let view = View { rect, pass: ctx.cumulative_pass_nr() };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("glass::view"), view));
}

fn shown_view(ctx: &Context) -> Option<Rect> {
    let view: Option<View> = ctx.data(|d| d.get_temp(egui::Id::new("glass::view")));
    view.filter(|v| v.pass == ctx.cumulative_pass_nr()).map(|v| v.rect)
}

/// The view's picture, as the viewport's renderer drew it last.
fn picture(resources: &egui_wgpu::CallbackResources) -> Option<(egui_wgpu::wgpu::TextureView, bool)> {
    resources.get::<crate::gpu::Renderer>()?.picture()
}

/// The glass under one fill: the theme's preset with the theme's blur and rim, the window's
/// preferences on top, the accent's tint when chosen, and a shadow only off the window's bars.
fn style(t: &Tokens, glass: Glass, prefs: &crate::prefs::Prefs, rect: &egui::epaint::RectShape, floating: bool) -> egui_glass::GlassStyle {
    let dark = crate::theme::is_dark();
    let mut s = if dark { egui_glass::GlassStyle::panel_dark() } else { egui_glass::GlassStyle::panel() };
    s.corner_radius = f32::from(rect.corner_radius.nw.max(rect.corner_radius.ne));
    s.blur = glass.blur * prefs.glass_blur;
    s.refraction = glass.refraction;
    // A wide, smooth lens at the rim, a trace of dispersion and a light rim: it reads as glass
    // bending what is behind it, not a frosted sheet.
    s.edge_width = (s.corner_radius * 1.5).clamp(16.0, 36.0);
    s.chromatic = 0.08;
    s.specular = 0.35;
    s.border = 0.3;
    if dark {
        s.brightness = 0.86;
    }
    if prefs.glass_tint_accent {
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
    let colours = [t.viewport_top, t.viewport_bottom];
    if state.uploaded != Some(colours) {
        // Lighter than the view: the text's colour on a dark theme, a dialog's on a light one.
        let light = if dark { t.text } else { t.dialog_body.to_opaque() };
        let image = backdrop(t.viewport_top, t.viewport_bottom, light, dark);
        egui_glass::set_backdrop(ctx, &state.render_state, &image);
        state.uploaded = Some(colours);
    }
    let screen = ctx.content_rect();
    let view = shown_view(ctx);
    egui_glass::map_backdrop(ctx, view.unwrap_or(screen), t.viewport_bottom);
    let hosts = [t.panel, t.panel_header, t.toolbar, t.timeline, t.dialog_bg, t.dialog_body, t.app_bar, t.overlay];
    // A fill as see-through as the theme and the preference ask: at 0 the theme's own alpha.
    let see_through = (glass.see_through * app.preferences.glass_see_through).clamp(0.0, 1.0);
    // The panel fills to turn into glass, found under the context's lock; the glass is made
    // outside it (it reads the context), then put in their place.
    // The docked panels paint in the background layer, which isn't an area; dialogs are areas.
    let mut layers = vec![egui::LayerId::background()];
    layers.extend(ctx.memory(|m| m.layer_ids().filter(|l| *l != egui::LayerId::background()).collect::<Vec<_>>()));
    let found = ctx.graphics_mut(|g| {
        let mut found = Vec::new();
        for layer in layers {
            let Some(list) = g.get_mut(layer) else { continue };
            let floating = !matches!(layer.order, Order::Background | Order::Middle);
            let mut glassed: Vec<(Rect, egui::Color32)> = Vec::new();
            for i in 0..list.next_idx().0 {
                list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| {
                    // A frame with a shadow paints as [shadow, fill]; the shadow is a rect too.
                    let (part, r) = match &clipped.shape {
                        Shape::Rect(r) => (None, r),
                        Shape::Vec(parts) => match parts.iter().position(|p| matches!(p, Shape::Rect(r) if hosts.contains(&r.fill))) {
                            Some(j) if let Shape::Rect(r) = &parts[j] => (Some(j), r),
                            _ => return,
                        },
                        _ => return,
                    };
                    let small = r.rect.width() < 24.0 || r.rect.height() < 20.0;
                    // The window's own background (the start page, the view's back) stays flat.
                    let whole = r.rect.area() > screen.area() * 0.6;
                    if small || whole || !hosts.contains(&r.fill) {
                        return;
                    }
                    // A fill inside glass already made stays a fill, but see-through too: else a
                    // bar painted twice (the History's strip and its inset) covers its own glass.
                    let inside = glassed.iter().find(|(g, _)| g.contains_rect(r.rect)).map(|(_, fill)| *fill);
                    if inside.is_none() {
                        glassed.push((r.rect, r.fill));
                    }
                    found.push((layer, i, part, r.clone(), floating, inside));
                });
            }
        }
        found
    });
    let glass_shapes: Vec<_> = found
        .into_iter()
        .map(|(layer, i, part, r, floating, inside)| {
            let alpha = f32::from(r.fill.a()) / 255.0 * (1.0 - see_through);
            let mut fill = r.clone();
            // The same fill again inside its glass adds nothing; another (a header) is thinned.
            fill.fill = r.fill.to_opaque().gamma_multiply(if inside == Some(r.fill) { 0.0 } else { alpha });
            if inside.is_some() {
                return (layer, i, part, Shape::Rect(fill));
            }
            let glass_style = style(&t, glass, &app.preferences, &r, floating);
            (layer, i, part, Shape::Vec(vec![egui_glass::glass_shape_in(ctx, r.rect, &glass_style), Shape::Rect(fill)]))
        })
        .collect();
    // The view's picture becomes the backdrop before the first glass of the frame is prepared;
    // a picture at half the view's pixels will do, as it is blurred.
    let copy = view.filter(|_| !glass_shapes.is_empty()).map(|rect| {
        let px = rect.size() * ctx.pixels_per_point() * 0.5;
        egui_glass::scene_backdrop_shape(rect, [px.x as u32, px.y as u32], std::sync::Arc::new(picture))
    });
    ctx.graphics_mut(|g| {
        for (layer, i, part, shape) in glass_shapes {
            let Some(list) = g.get_mut(layer) else { continue };
            list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| match (part, &mut clipped.shape) {
                (Some(j), Shape::Vec(parts)) => parts[j] = shape,
                _ => clipped.shape = shape,
            });
        }
        // After the glass is in place, so the first shape's index still meant what it found.
        if let Some(copy) = copy
            && let Some(list) = g.get_mut(egui::LayerId::background())
            && list.next_idx().0 > 0
        {
            list.mutate_shape(egui::layers::ShapeIdx(0), |clipped| {
                let first = std::mem::replace(&mut clipped.shape, Shape::Noop);
                clipped.shape = Shape::Vec(vec![copy, first]);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The backdrop runs from the view's top colour to its bottom one, with a neutral glow.
    #[test]
    fn backdrop_is_the_view_gradient_with_a_neutral_glow() {
        let (top, bottom) = (Color32::from_rgb(30, 36, 50), Color32::from_rgb(10, 13, 19));
        let image = backdrop(top, bottom, Color32::from_rgb(226, 230, 238), true);
        assert_eq!(image.size, [BACKDROP, BACKDROP]);
        let at = |x: usize, y: usize| image.pixels[y * BACKDROP + x];
        assert!(at(BACKDROP / 2, BACKDROP - 1).r() < at(BACKDROP / 2, 0).r(), "darker at the bottom");
        let (glow, plain) = (at(BACKDROP * 18 / 100, BACKDROP * 22 / 100), top.lerp_to_gamma(bottom, 0.22));
        assert!(glow.r() > plain.r() && glow.g() > plain.g(), "lighter near the top left: {glow:?}");
    }
}
