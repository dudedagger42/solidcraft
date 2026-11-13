//! A glass theme's (Liquid Craft's) panels: after the window is laid out, each panel, dialog and
//! bar fill becomes `egui_glass` glass over a backdrop in the theme's colours, with the fill drawn
//! over it more see-through. The widgets keep drawing ordinary fills (`ui_kit::Container`); only
//! the finished paint lists change, and only while a glass theme is on. Over the 3D view the
//! backdrop is the view's own picture, copied each frame, so the model shows through the glass.
//! Text on that glass reads the picture's brightness under it: light text turns dark over a light
//! part and back, with a faint halo where the backdrop is mid-grey and neither colour wins.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Context, Order, Rect, Shape};

use solidcraft_theme::{Glass, Tokens};

/// The backdrop's size in pixels: it is blurred, so a small image stretched over the window will do.
const BACKDROP: usize = 256;

/// The GPU state glass needs: the window's renderer, and the colours of the backdrop uploaded now.
pub(crate) struct GlassState {
    render_state: egui_wgpu::RenderState,
    uploaded: Option<[Color32; 2]>,
    brightness: egui_glass::SceneBrightness,
    /// Each text's side over the glass, from last frame.
    flips: FlipMap,
}

impl GlassState {
    /// Glass's pipelines in the window's pass (one sample, no depth).
    pub(crate) fn install(render_state: &egui_wgpu::RenderState) -> GlassState {
        egui_glass::init(render_state, 1);
        let brightness = egui_glass::init_scene_backdrop(render_state);
        GlassState { render_state: render_state.clone(), uploaded: None, brightness, flips: FlipMap::new() }
    }

    pub(crate) fn render_state(&self) -> &egui_wgpu::RenderState {
        &self.render_state
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
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("glass::view"), view));
}

fn shown_view(ctx: &Context) -> Option<Rect> {
    let view: Option<View> = ctx.data(|d| d.get_temp(egui::Id::unique("glass::view")));
    view.filter(|v| v.pass == ctx.cumulative_pass_nr()).map(|v| v.rect)
}

/// The view's picture, as the viewport's renderer drew it last.
fn picture(resources: &egui_wgpu::CallbackResources) -> Option<(egui_wgpu::wgpu::TextureView, bool)> {
    resources.get::<crate::gpu::Renderer>()?.picture()
}

/// The glass under one fill: the theme's preset with the theme's blur, bend and reach, the window's
/// preferences on top, the accent's tint when chosen, and a shadow only off the window's bars.
fn style(t: &Tokens, glass: Glass, prefs: &crate::prefs::Prefs, rect: &egui::epaint::RectShape, floating: bool) -> egui_glass::GlassStyle {
    let dark = solidcraft_theme::is_dark();
    let mut s = if dark { egui_glass::GlassStyle::panel_dark() } else { egui_glass::GlassStyle::panel() };
    s.corner_radius = f32::from(rect.corner_radius.nw.max(rect.corner_radius.ne));
    s.blur = glass.blur * prefs.glass_blur;
    s.refraction = glass.refraction * prefs.glass_refraction;
    s.reach = glass.reach * prefs.glass_reach;
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

/// One glass pane of the frame, for its text: where it is and what it shows over the backdrop.
struct Pane {
    layer: egui::LayerId,
    rect: Rect,
    style: egui_glass::GlassStyle,
    /// The panel's own fill over the glass, and how much of it shows (0 to 1).
    fill: Color32,
    fill_alpha: f32,
}

/// A colour's luma, 0 to 1, in display (gamma) terms.
fn luma(c: Color32) -> f32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    (0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)) / 255.0
}

impl Pane {
    /// How bright the glass looks over a backdrop of luma `under`: dimmed or lifted, tinted, then
    /// the panel's thinned fill on top.
    fn shows(&self, under: f32) -> f32 {
        let s = &self.style;
        let tint = f32::from(s.tint.a()) / 255.0;
        let lit = (under * s.brightness).clamp(0.0, 1.0);
        let tinted = egui::lerp(lit..=luma(s.tint), tint);
        egui::lerp(tinted..=luma(self.fill), self.fill_alpha)
    }
}

/// A grey of luma `v` (0 to 1) at alpha `a`.
fn grey(v: f32, a: u8) -> Color32 {
    let v = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(v, v, v, a)
}

/// Text's colour over glass that shows luma `bg`, where the theme drew it for glass of luma
/// `design`: a grey (a label, a dimmed row) keeps its share of the contrast it had there, towards
/// white over dark glass and towards black over light glass, so the light/dim order holds; a
/// coloured text stays as it is. `f` is how far it has faded to the light-glass end. Also the
/// halo's strength, most where `bg` is mid-grey and neither end gives much contrast.
fn flip(c: Color32, bg: f32, design: f32, f: f32) -> (Color32, f32) {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let halo = mid_grey(bg);
    if r.max(g).max(b) - r.min(g).min(b) >= 40 {
        return (c, halo);
    }
    let share = ((luma(c) - design).abs() / design.max(1.0 - design)).clamp(0.0, 1.0);
    let (on_dark, on_light) = (bg + (1.0 - bg) * share, bg - bg * share);
    let ink = grey(egui::lerp(on_dark..=on_light, f), a);
    // Unchanged where the glass is what the theme drew the text for.
    (if (luma(ink) - luma(c)).abs() < 0.02 { c } else { ink }, halo)
}

/// How much a backdrop of luma `bg` needs a halo: most at mid-grey, where neither end gives much
/// contrast.
fn mid_grey(bg: f32) -> f32 {
    1.0 - ((bg - 0.5).abs() / 0.16).clamp(0.0, 1.0)
}

/// Seconds a text takes to fade from one end to the other.
const FLIP_FADE: f32 = 0.1;

/// How quickly a text follows the brightness under it (seconds): slow enough that a part's edge
/// sweeping past doesn't turn the text, which would go faint and come back, row after row.
const FLIP_FOLLOW: f32 = 0.2;
/// How quickly the halo comes and goes (seconds), so passing mid-grey on the way across doesn't
/// flash it.
const FLIP_HALO: f32 = 0.3;
/// How far a moving text can be from where it was last frame and still be the same text (points).
const FLIP_REACH: f32 = 400.0;

/// Which end a text is on, how far it has faded there, and the brightness it has followed to.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Flip {
    light: bool,
    f: f32,
    bg: f32,
    halo: f32,
    settled: bool,
}

impl Flip {
    /// The next frame's over a backdrop of `raw`: the side changes only well past mid-grey, so a
    /// backdrop hovering there can't bounce the text, and the colour fades over time rather than
    /// following the picture's steps.
    fn next(last: Option<Flip>, raw: f32, dt: f32) -> Flip {
        let Some(last) = last else {
            let light = raw > 0.5;
            return Flip { light, f: f32::from(u8::from(light)), bg: raw, halo: mid_grey(raw), settled: true };
        };
        let bg = last.bg + (raw - last.bg) * (1.0 - (-dt / FLIP_FOLLOW).exp());
        let light = if bg > 0.56 {
            true
        } else if bg < 0.44 {
            false
        } else {
            last.light
        };
        let step = dt / FLIP_FADE;
        let f = if light { (last.f + step).min(1.0) } else { (last.f - step).max(0.0) };
        let wanted = mid_grey(bg);
        let halo = last.halo + (wanted - last.halo) * (1.0 - (-dt / FLIP_HALO).exp());
        Flip { light, f, bg, halo, settled: f == f32::from(u8::from(light)) && (raw - bg).abs() < 0.005 && (wanted - halo).abs() < 0.01 }
    }
}

/// Each text's side by what it says, with where it is.
type FlipMap = HashMap<u64, Vec<(egui::Pos2, Flip)>>;

/// The frame's text flipping: last frame's sides, and this frame's as they are found.
struct Flips {
    last: FlipMap,
    now: FlipMap,
    dt: f32,
}

impl Flips {
    /// This text's side last frame: the nearest with the same words (it may be moving, like a
    /// command's value box), else what stood in the same place (its value just changed).
    fn find(&self, words: u64, at: egui::Pos2) -> Option<Flip> {
        let near = |list: &[(egui::Pos2, Flip)], reach: f32| {
            list.iter().map(|(p, f)| (p.distance(at), *f)).filter(|(d, _)| *d <= reach).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, f)| f)
        };
        self.last.get(&words).and_then(|list| near(list, FLIP_REACH)).or_else(|| self.last.values().find_map(|list| near(list, 4.0)))
    }
}

/// A text's words, to know it again next frame.
fn flip_words(text: &egui::epaint::TextShape) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.galley.job.text.hash(&mut h);
    h.finish()
}

/// The colour a text shape draws in, as far as one colour can say it.
fn text_colour(text: &egui::epaint::TextShape) -> Color32 {
    let first = text.galley.job.sections.first().map_or(Color32::PLACEHOLDER, |s| s.format.color);
    text.override_text_color.unwrap_or(if first == Color32::PLACEHOLDER { text.fallback_color } else { first })
}

/// A text shape flipped against the glass under it, with its halo behind; `None`: as it is.
fn flipped_text(text: &egui::epaint::TextShape, panes: &[&Pane], scene: &egui_glass::SceneLuma, flips: &mut Flips) -> Option<Shape> {
    let rect = text.visual_bounding_rect();
    let pane = panes.iter().rev().find(|p| p.rect.contains(rect.center()))?;
    let words = flip_words(text);
    let last = flips.find(words, rect.min);
    // Off the picture for a moment: on with the brightness it had, rather than the theme's colour.
    let raw = scene.mean(rect).map(|under| pane.shows(under)).or(last.map(|l| l.bg))?;
    let side = Flip::next(last, raw, flips.dt);
    flips.now.entry(words).or_default().push((rect.min, side));
    let colour = text_colour(text);
    let (ink, _) = flip(colour, side.bg, luma(pane.fill), side.f);
    let halo = side.halo;
    if ink == colour && halo < 0.05 {
        return None;
    }
    let mut front = text.clone();
    front.override_text_color = Some(ink);
    if halo < 0.05 {
        return Some(Shape::Text(front));
    }
    // A faint ring of the opposite grey a pixel out, behind the text.
    let back = grey(side.f, 255).gamma_multiply(0.35 * halo);
    let mut parts: Vec<Shape> = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .into_iter()
        .map(|(x, y)| {
            let mut ring = text.clone();
            ring.override_text_color = Some(back);
            ring.pos += egui::vec2(x, y);
            Shape::Text(ring)
        })
        .collect();
    parts.push(Shape::Text(front));
    Some(Shape::Vec(parts))
}

/// Flips the text over the view on the frame's glass panes, in place.
fn flip_texts(shape: &mut Shape, panes: &[&Pane], scene: &egui_glass::SceneLuma, flips: &mut Flips) {
    match shape {
        Shape::Text(text) => {
            if let Some(new) = flipped_text(text, panes, scene, flips) {
                *shape = new;
            }
        }
        Shape::Vec(parts) => parts.iter_mut().for_each(|p| flip_texts(p, panes, scene, flips)),
        _ => {}
    }
}

/// Turns the frame's panel fills into glass while a glass theme is on; else does nothing.
pub(crate) fn frame(app: &mut crate::SolveApp, ctx: &Context) {
    let t = Tokens::get();
    let (Some(glass), Some(state)) = (t.glass, app.glass.as_mut()) else { return };
    let dark = solidcraft_theme::is_dark();
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
    // With a background picture the copy is the whole window, so docked panels see it as well.
    let sky = view.is_some() && crate::gpu::sky_shown(ctx);
    egui_glass::map_backdrop(ctx, if sky { screen } else { view.unwrap_or(screen) }, t.viewport_bottom);
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
    let mut panes = Vec::new();
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
            panes.push(Pane { layer, rect: r.rect, style: glass_style, fill: r.fill.to_opaque(), fill_alpha: alpha });
            (layer, i, part, Shape::Vec(vec![egui_glass::glass_shape_in(ctx, r.rect, &glass_style), Shape::Rect(fill)]))
        })
        .collect();
    // The view's brightness as last read back, while the view is shown.
    let scene = view.and_then(|_| state.brightness.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone());
    // The view's picture becomes the backdrop before the first glass of the frame is prepared;
    // a picture at half the view's pixels will do, as it is blurred.
    let copy = view.filter(|_| !glass_shapes.is_empty()).map(|rect| {
        if sky {
            let px = screen.size() * ctx.pixels_per_point() * 0.5;
            return egui_glass::scene_backdrop_over_sky(screen, rect, [px.x as u32, px.y as u32], std::sync::Arc::new(picture));
        }
        let px = rect.size() * ctx.pixels_per_point() * 0.5;
        egui_glass::scene_backdrop_shape(rect, [px.x as u32, px.y as u32], std::sync::Arc::new(picture))
    });
    let mut flips = Flips { last: std::mem::take(&mut state.flips), now: FlipMap::new(), dt: ctx.input(|i| i.stable_dt) };
    ctx.graphics_mut(|g| {
        for (layer, i, part, shape) in glass_shapes {
            let Some(list) = g.get_mut(layer) else { continue };
            list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| match (part, &mut clipped.shape) {
                (Some(j), Shape::Vec(parts)) => parts[j] = shape,
                _ => clipped.shape = shape,
            });
        }
        if let Some(scene) = &scene {
            let mut layers: Vec<_> = panes.iter().map(|p| p.layer).collect();
            layers.dedup();
            for layer in layers {
                let on: Vec<&Pane> = panes.iter().filter(|p| p.layer == layer).collect();
                let Some(list) = g.get_mut(layer) else { continue };
                for i in 0..list.next_idx().0 {
                    list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| flip_texts(&mut clipped.shape, &on, scene, &mut flips));
                }
            }
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
    if flips.now.values().flatten().any(|(_, side)| !side.settled) {
        ctx.request_repaint();
    }
    state.flips = flips.now;
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

    /// Grey text keeps its share of the contrast over a lighter or darker glass: towards white
    /// over dark, towards black over light, a dimmed row still dimmer than a label; a coloured
    /// text stays; the halo is there only near mid-grey.
    #[test]
    fn grey_text_flips_against_the_glass_with_a_halo_at_mid_grey() {
        let flip = |c, bg: f32, design| flip(c, bg, design, if bg > 0.5 { 1.0 } else { 0.0 });
        let design = 0.12;
        let (label, dimmed) = (grey(0.85, 255), grey(0.55, 255));
        assert_eq!(flip(label, design, design), (label, 0.0), "as drawn over the theme's own glass");
        let (on_grey, dimmed_on_grey) = (flip(label, 0.4, design).0, flip(dimmed, 0.4, design).0);
        assert!(luma(dimmed_on_grey) - 0.4 > 0.25 && luma(on_grey) > luma(dimmed_on_grey), "lighter over mid-dark glass");
        let (on_light, halo) = flip(label, 0.85, design);
        assert!(luma(on_light) < 0.2 && halo == 0.0, "dark over a light part: {on_light:?}");
        let dimmed_on_light = flip(dimmed, 0.85, design).0;
        assert!(luma(dimmed_on_light) > luma(on_light) && luma(dimmed_on_light) < 0.6, "a dimmed row stays dimmer");
        assert!(luma(flip(grey(0.12, 255), 0.15, 0.95).0) > 0.8, "dark text of a light theme over dark glass turns light");
        assert!(flip(label, 0.5, design).1 > 0.9, "a halo at mid-grey");
        let accent = egui::Color32::from_rgb(60, 200, 90);
        assert_eq!(flip(accent, 0.85, design).0, accent, "a coloured text keeps its colour");
    }

    /// Over a backdrop hovering at mid-grey the text stays put; past it, it fades over time.
    #[test]
    fn text_fades_across_and_mid_grey_cannot_bounce_it() {
        let dark = Flip::next(None, 0.3, 0.0);
        assert_eq!((dark.light, dark.f), (false, 0.0));
        let dt = 1.0 / 60.0;
        let mut side = dark;
        for bg in [0.49, 0.55, 0.45, 0.52, 0.5] {
            side = Flip::next(Some(Flip { bg, ..side }), bg, dt);
            assert_eq!((side.light, side.f), (false, 0.0), "no change at {bg}");
        }
        // A step in the picture reaches the text gradually, and the fade never jumps.
        let (mut last_f, mut last_bg) = (side.f, side.bg);
        let mut frames = 0;
        while !side.settled && frames < 300 || frames == 0 {
            side = Flip::next(Some(side), 0.8, dt);
            assert!(side.f - last_f <= dt / FLIP_FADE + 1e-4, "a step of {} from {last_f}", side.f - last_f);
            assert!(side.bg - last_bg < 0.05, "brightness jumped {} to {}", last_bg, side.bg);
            (last_f, last_bg) = (side.f, side.bg);
            frames += 1;
        }
        assert!(side.light && side.settled && frames > 18, "{side:?} after {frames}");
        // A part's edge sweeping past is too brief to turn the text, or to bring the halo up.
        let mut swept = Flip::next(None, 0.65, 0.0);
        for _ in 0..5 {
            swept = Flip::next(Some(swept), 0.1, dt);
            assert!(swept.light && swept.f == 1.0 && swept.halo < 0.3, "{swept:?}");
        }
        // The ink moves with the fade, not with where the backdrop is.
        let label = grey(0.85, 255);
        assert!(luma(flip(label, 0.8, 0.12, 0.0).0) > 0.9 && luma(flip(label, 0.8, 0.12, 1.0).0) < 0.2);
    }

    /// A moving text is still itself, and a changed value in the same place is too.
    #[test]
    fn a_moving_or_changed_text_keeps_its_side() {
        let side = Flip { light: true, f: 0.4, bg: 0.7, halo: 0.0, settled: false };
        let at = egui::pos2(100.0, 100.0);
        let flips = Flips { last: FlipMap::from([(1, vec![(at, side)])]), now: FlipMap::new(), dt: 0.0 };
        assert_eq!(flips.find(1, egui::pos2(300.0, 100.0)), Some(side), "same words, moved");
        assert_eq!(flips.find(2, at + egui::vec2(1.0, 0.0)), Some(side), "new words, same place");
        assert_eq!(flips.find(2, egui::pos2(300.0, 100.0)), None, "a different text");
    }

    /// The read back luma under a rect is the mean of the cells it covers; off the picture, none.
    #[test]
    fn scene_luma_means_the_cells_under_a_rect() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let scene = egui_glass::SceneLuma { rect, size: [2, 1], luma: vec![0.2, 0.8] };
        assert_eq!(scene.mean(Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(10.0, 10.0))), Some(0.2));
        assert_eq!(scene.mean(Rect::from_min_size(egui::pos2(40.0, 10.0), egui::vec2(20.0, 10.0))), Some(0.5));
        assert_eq!(scene.mean(Rect::from_min_size(egui::pos2(200.0, 10.0), egui::vec2(10.0, 10.0))), None);
    }
}
