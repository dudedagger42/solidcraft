//! Stable UI rendering boundary for the icon artwork. Compiled only at L5.
//!
//! Command, menu, browser and timeline icons are the SVG masters in `assets/icons/ui/design/svg`,
//! rendered by [`crate::icon_svg`]: colours come from the default `palette.toml` roles/accents
//! and optional active-theme overrides, never arbitrary caller tints. An icon with no master
//! yet draws the placeholder.
//! The glyph, marker, navigation and pointer sources below are not command icons.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::{Color32, Painter, Rect, TextureHandle, TextureOptions, pos2};

use crate::icon_svg::{self, Look};

#[path = "../../../assets/icons/ui/source/glyphs.rs"]
pub(crate) mod glyphs;
#[path = "../../../assets/icons/ui/source/markers.rs"]
pub(crate) mod markers;
#[path = "../../../assets/icons/ui/source/navigation.rs"]
pub(crate) mod navigation;
#[path = "../../../assets/icons/ui/source/pointer.rs"]
pub(crate) mod pointer;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Finish {
    Artwork,
    Ink(Color32),
    Danger,
    /// The artwork's coloured parts as their brightness, so a tint multiplied in takes the place of
    /// the accent and keeps the shading (RGB wave).
    WaveAccent,
    /// The artwork without its coloured parts: its whites and greys, drawn as they are (RGB wave).
    WaveNeutral,
}

/// Textures already rasterised: one per (master, look, pixel size, finish).
#[derive(Clone, Default)]
struct Cache(Arc<Mutex<CacheEntries>>);

/// Drop rasterised icon textures so the next paint uploads them again (wake from sleep, #8749).
pub(crate) fn clear_texture_cache(ctx: &egui::Context) {
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::new("sc_icon_cache"), Cache::default).clone());
    if let Ok(mut entries) = cache.0.lock() {
        entries.textures.clear();
    }
}

#[derive(Default)]
struct CacheEntries {
    palette: Option<icon_svg::Palette>,
    textures: HashMap<(&'static str, Look, u32, Finish), TextureHandle>,
}

/// Draw palette-owned icon artwork with an explicit opacity (0 to 1).
pub fn paint_icon(p: &Painter, r: Rect, name: &str, opacity: f32) {
    paint_on(p, r, name, None, opacity);
}

/// Draw the icon `name` for a surface that is always dark (the application bar), whatever the
/// theme: `dark: Some(true)`. `None` follows the theme.
pub fn paint_on(p: &Painter, r: Rect, name: &str, dark: Option<bool>, opacity: f32) {
    paint_with_ink(p, r, name, dark, opacity, None, Mirror::None);
}

/// Monochrome flat controls on the application bar use its text token exactly.
pub(crate) fn paint_app_bar(p: &Painter, r: Rect, name: &str, opacity: f32) {
    paint_with_ink(p, r, name, Some(true), opacity, Some(crate::theme::Tokens::get().app_bar_text), Mirror::None);
}

/// Paint a single-colour icon in `ink`.
pub(crate) fn paint_ink(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::None);
}

/// Paint a single-colour icon in `ink`, mirrored top to bottom (a chevron pointing up).
pub(crate) fn paint_ink_flipped(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::Vertical);
}

/// Paint a single-colour icon mirrored left to right, reusing the original master.
pub(crate) fn paint_ink_mirrored(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::Horizontal);
}

#[derive(Clone, Copy)]
enum Mirror {
    None,
    Vertical,
    Horizontal,
}

fn paint_with_ink(p: &Painter, r: Rect, name: &str, dark: Option<bool>, opacity: f32, ink: Option<Color32>, mirror: Mirror) {
    let ctx = p.ctx();
    let resolved = icon_svg::resolve(name);
    let look = Look::themed(dark.unwrap_or_else(|| ctx.global_style().visuals.dark_mode), resolved.accent, crate::theme::vision());
    paint_resolved(p, r, resolved, look, opacity, ink.map_or(Finish::Artwork, Finish::Ink), mirror);
}

/// Draw with a named palette ramp, preserving outlines, passive faces and transparency.
/// `None` explicitly selects base blue; an unknown name retains the icon's own accent.
/// Existing painters continue to select the icon's semantic accent.
pub fn paint_accented(p: &Painter, r: Rect, name: &str, accent: Option<&str>, opacity: f32) {
    let resolved = icon_svg::resolve(name);
    let accent = match accent {
        Some(name) => icon_svg::accent_index(name).or(resolved.accent),
        None => None,
    };
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, accent, crate::theme::vision());
    paint_resolved(p, r, resolved, look, opacity, Finish::Artwork, Mirror::None);
}

/// Shared destructive-action artwork: red shaded faces, normal outlines and cutouts.
/// Unlike a single-colour mask, this also tints a master's passive faces through
/// the constraint ramp without flattening the lid, handle or other details.
pub(crate) fn paint_danger(p: &Painter, r: Rect, name: &str, opacity: f32) {
    let resolved = icon_svg::resolve(name);
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, icon_svg::accent_index("constraint"), crate::theme::vision());
    paint_resolved(p, r, resolved, look, opacity, Finish::Danger, Mirror::None);
}

fn paint_resolved(p: &Painter, r: Rect, resolved: icon_svg::Resolved, look: Look, opacity: f32, finish: Finish, mirror: Mirror) {
    paint_tinted(p, r, resolved, look, opacity, finish, mirror, None);
}

#[allow(clippy::too_many_arguments)]
fn paint_tinted(p: &Painter, r: Rect, resolved: icon_svg::Resolved, look: Look, opacity: f32, finish: Finish, mirror: Mirror, tint: Option<Color32>) {
    let ctx = p.ctx();
    let ppp = ctx.pixels_per_point().max(0.5);
    let side = r.width().min(r.height());
    let px = (side * ppp).round().clamp(8.0, 512.0) as u32;
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::new("sc_icon_cache"), Cache::default).clone());
    let Ok(mut entries) = cache.0.lock() else { return };
    let palette = crate::theme::Tokens::get().icon_palette;
    if entries.palette != palette {
        entries.textures.clear();
        entries.palette = palette;
    }
    let map = &mut entries.textures;
    let key = (resolved.master, look, px, finish);
    if let std::collections::hash_map::Entry::Vacant(slot) = map.entry(key) {
        let Some(doc) = icon_svg::doc_for(resolved.master, px) else { return };
        let pm = if finish == Finish::Danger { icon_svg::render_tinted(doc, look, px) } else { icon_svg::render(doc, look, px) };
        let Some(pm) = pm else { return };
        let mut image = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data());
        if matches!(finish, Finish::WaveAccent | Finish::WaveNeutral) {
            for pixel in &mut image.pixels {
                *pixel = wave_part(*pixel, finish == Finish::WaveAccent);
            }
        }
        if let Finish::Ink(ink) = finish {
            for pixel in &mut image.pixels {
                *pixel = ink.gamma_multiply(f32::from(pixel.a()) / 255.0);
            }
        }
        slot.insert(ctx.load_texture(format!("icon:{}:{px}", resolved.master), image, TextureOptions::LINEAR));
    }
    let Some(tex) = map.get(&key) else { return };
    // Snap the image to whole physical pixels so it stays crisp.
    let size = px as f32 / ppp;
    let min = pos2(((r.center().x - size / 2.0) * ppp).round() / ppp, ((r.center().y - size / 2.0) * ppp).round() / ppp);
    let tint = tint.unwrap_or_else(|| Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8));
    let uv = match mirror {
        Mirror::None => Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Mirror::Vertical => Rect::from_min_max(pos2(0.0, 1.0), pos2(1.0, 0.0)),
        Mirror::Horizontal => Rect::from_min_max(pos2(1.0, 0.0), pos2(0.0, 1.0)),
    };
    p.image(tex.id(), Rect::from_min_size(min, egui::vec2(size, size)), uv, tint);
}

// RGB wave: ribbon icons (and optionally button blocks) take a hue from their own screen x plus
// time, so one wave runs across the whole window however the ribbon is laid out or scrolled.
const WAVE_FLAGS: &str = "sc_rgb_wave";

/// How much of a pixel is accent rather than white or grey, by its saturation.
fn wave_share(pixel: Color32) -> f32 {
    let [r, g, b, _] = pixel.to_srgba_unmultiplied();
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max == 0 {
        return 0.0;
    }
    let saturation = f32::from(max - min) / f32::from(max);
    ((saturation - 0.12) / 0.2).clamp(0.0, 1.0)
}

/// A pixel's accent (as its brightness, for a tint to multiply) or the rest of it. Together they
/// make up the pixel; brightness and not luminance, as a blue's luminance is dark whatever its shade.
fn wave_part(pixel: Color32, accent: bool) -> Color32 {
    let share = wave_share(pixel);
    if !accent {
        return pixel.gamma_multiply(1.0 - share);
    }
    let [r, g, b, a] = pixel.to_srgba_unmultiplied();
    let v = r.max(g).max(b);
    Color32::from_rgba_unmultiplied(v, v, v, a).gamma_multiply(share)
}

/// The wave's shape: how long a colour cycle is on screen and how fast it travels, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Wave {
    pub length: f32,
    pub speed: f32,
}

impl Default for Wave {
    fn default() -> Wave {
        Wave { length: 2400.0, speed: 150.0 }
    }
}

impl Wave {
    /// The hue's phase at screen x 0 and time (s), 0 to 1.
    fn phase(self, time: f64) -> f32 {
        (time * f64::from(self.speed) / f64::from(self.length.max(1.0))).rem_euclid(1.0) as f32
    }

    /// The wave's colour at screen x (points) and time (s), at `opacity`.
    pub(crate) fn tint(self, x: f32, time: f64, opacity: f32) -> Color32 {
        let hue = (x / self.length.max(1.0) - self.phase(time)).rem_euclid(1.0);
        let [r, g, b] = egui::ecolor::Hsva::new(hue, WAVE_SATURATION, 1.0, 1.0).to_srgb();
        Color32::from_rgb(r, g, b).gamma_multiply(opacity.clamp(0.0, 1.0))
    }
}

const WAVE_SATURATION: f32 = 0.65;
/// How strongly the wave shows over a panel.
const WAVE_PANEL_ALPHA: f32 = 0.22;

/// Set by the ribbon each frame: the wave on icons, on button blocks. Asks for ~30 fps while on.
pub(crate) fn set_wave(ctx: &egui::Context, icons: bool, blocks: bool, wave: Wave) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(WAVE_FLAGS), (icons, blocks, wave)));
    if icons || blocks {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

fn wave_flags(ctx: &egui::Context) -> (bool, bool, Wave) {
    ctx.data(|d| d.get_temp(egui::Id::new(WAVE_FLAGS))).unwrap_or_default()
}

/// The logo's faces with the wave on icons: the wave's colour at `x` in its three shades.
fn wave_logo(ctx: &egui::Context, x: f32) -> Option<[Color32; 3]> {
    let (icons, _, wave) = wave_flags(ctx);
    icons.then(|| crate::theme::logo_shades(wave.tint(x, ctx.input(|i| i.time), 1.0)))
}

/// One colour cycle of the wave, to repeat across a panel.
fn wave_texture(ctx: &egui::Context) -> egui::TextureId {
    let id = egui::Id::new("sc_rgb_wave_texture");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return tex.id();
    }
    let pixels = (0..256)
        .map(|i| {
            let [r, g, b] = egui::ecolor::Hsva::new(i as f32 / 256.0, WAVE_SATURATION, 1.0, 1.0).to_srgb();
            Color32::from_rgb(r, g, b)
        })
        .collect();
    let image = egui::ColorImage { size: [256, 1], source_size: egui::vec2(256.0, 1.0), pixels };
    let options = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..TextureOptions::LINEAR };
    let tex = ctx.load_texture("rgb-wave", image, options);
    let handle = tex.id();
    ctx.data_mut(|d| d.insert_temp(id, tex));
    handle
}

/// With `rgb_wave_panels`: the wave over the frame's panel fills, as a gradient by screen position
/// that keeps each panel's rounded corners. Before glass, which finds the fills in these shapes.
pub(crate) fn wave_panels(prefs: &crate::prefs::Prefs, ctx: &egui::Context) {
    use egui::epaint::{RectShape, Shape};
    if !prefs.rgb_wave_panels {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    let t = crate::theme::Tokens::get();
    let hosts = [t.panel, t.panel_header, t.toolbar, t.timeline, t.dialog_bg, t.dialog_body, t.app_bar, t.overlay];
    let wave = Wave { length: prefs.rgb_wave_length.max(1.0), speed: prefs.rgb_wave_speed };
    let phase = wave.phase(ctx.input(|i| i.time));
    let texture = wave_texture(ctx);
    let screen = ctx.content_rect();
    let mut layers = vec![egui::LayerId::background()];
    layers.extend(ctx.memory(|m| m.layer_ids().filter(|l| *l != egui::LayerId::background()).collect::<Vec<_>>()));
    let over = |r: &RectShape| {
        let u = |x: f32| x / wave.length - phase;
        let uv = Rect::from_min_max(pos2(u(r.rect.left()), 0.0), pos2(u(r.rect.right()), 1.0));
        Shape::Rect(RectShape::filled(r.rect, r.corner_radius, Color32::WHITE.gamma_multiply(WAVE_PANEL_ALPHA)).with_texture(texture, uv))
    };
    ctx.graphics_mut(|g| {
        for layer in layers {
            let Some(list) = g.get_mut(layer) else { continue };
            let mut done: Vec<Rect> = Vec::new();
            for i in 0..list.next_idx().0 {
                list.mutate_shape(egui::layers::ShapeIdx(i), |clipped| {
                    let mut wanted = |r: &RectShape| {
                        // Once for what is nested: a header inside its panel doesn't deepen it.
                        let take = hosts.contains(&r.fill)
                            && r.rect.width() >= 24.0
                            && r.rect.height() >= 20.0
                            && r.rect.area() <= screen.area() * 0.6
                            && !done.iter().any(|d| d.contains_rect(r.rect));
                        if take {
                            done.push(r.rect);
                        }
                        take
                    };
                    match &mut clipped.shape {
                        Shape::Rect(r) if wanted(r) => {
                            let tinted = over(r);
                            let fill = Shape::Rect(r.clone());
                            clipped.shape = Shape::Vec(vec![fill, tinted]);
                        }
                        Shape::Vec(parts) => {
                            let found = parts.iter().position(|p| matches!(p, Shape::Rect(r) if hosts.contains(&r.fill)));
                            if let Some(j) = found
                                && let Shape::Rect(r) = &parts[j]
                                && wanted(r)
                            {
                                let tinted = over(r);
                                parts.insert(j + 1, tinted);
                            }
                        }
                        _ => {}
                    }
                });
            }
        }
    });
}

/// A ribbon command icon: [`paint_icon`], or with the wave on, the artwork with the wave in place
/// of its accent.
pub(crate) fn paint_ribbon_icon(p: &Painter, r: Rect, name: &str, opacity: f32) {
    let ctx = p.ctx();
    let (icons, _, wave) = wave_flags(ctx);
    if !icons {
        return paint_icon(p, r, name, opacity);
    }
    let resolved = icon_svg::resolve(name);
    let look = Look::themed(ctx.global_style().visuals.dark_mode, resolved.accent, crate::theme::vision());
    let tint = wave.tint(r.center().x, ctx.input(|i| i.time), opacity);
    paint_tinted(p, r, resolved, look, opacity, Finish::WaveNeutral, Mirror::None, None);
    paint_tinted(p, r, resolved, look, opacity, Finish::WaveAccent, Mirror::None, Some(tint));
}

/// A ribbon button's block in the wave's colour at low alpha (`rgb_wave_background`).
pub(crate) fn paint_ribbon_block(p: &Painter, r: Rect, radius: u8, enabled: bool) {
    let ctx = p.ctx();
    let (_, blocks, wave) = wave_flags(ctx);
    if enabled && blocks {
        p.rect_filled(r, radius, wave.tint(r.center().x, ctx.input(|i| i.time), 0.18));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{self, ColorVision};

    /// The wave's hue comes from the painted x and the time only: the same icon at two x values
    /// differs, two icons at one x agree, time moves it, and off is the normal white tint.
    #[test]
    fn rgb_wave_tint_follows_screen_x_and_time_and_off_is_normal_artwork() {
        let ctx = crate::fonts::test_context();
        let tints = |on: bool, time: f64| {
            let mut input = egui::RawInput::default();
            input.time = Some(time);
            let out = ctx.run_ui(input, |ui| {
                set_wave(ui.ctx(), on, false, Wave { length: 600.0, speed: 150.0 });
                let at = |x: f32, y: f32| Rect::from_min_size(egui::pos2(x, y), egui::vec2(32.0, 32.0));
                paint_ribbon_icon(ui.painter(), at(10.0, 0.0), "extrude", 1.0);
                paint_ribbon_icon(ui.painter(), at(310.0, 0.0), "extrude", 1.0);
                paint_ribbon_icon(ui.painter(), at(10.0, 90.0), "save", 1.0);
            });
            out.shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Mesh(m) => m.vertices.first().map(|v| v.color),
                    _ => None,
                })
                // With the wave on, each icon is its neutral part and its tinted accent.
                .filter(|c| !on || *c != Color32::WHITE)
                .collect::<Vec<_>>()
        };
        let off = tints(false, 0.0);
        assert!(off.len() >= 3 && off.iter().all(|c| *c == Color32::WHITE), "off: plain artwork {off:?}");
        let (a, b) = (tints(true, 0.0), tints(true, 1.0));
        assert!(a.len() >= 3 && a[0] != a[1], "one icon at two x values differs");
        assert_eq!(a[0], a[2], "two icons at one x agree");
        assert_ne!(a[0], b[0], "time moves the wave");
        let wave = Wave { length: 600.0, speed: 150.0 };
        assert_eq!(wave.tint(10.0, 0.0, 1.0), wave.tint(10.0 + 600.0, 0.0, 1.0), "one cycle per length");
        assert_eq!(wave.tint(10.0, 2.0, 1.0), wave.tint(310.0, 0.0, 1.0), "it travels at its speed");
        assert!(wave.tint(10.0, 0.0, 0.35).a() < 120, "a disabled icon stays dim");
        let ctx = egui::Context::default();
        set_wave(&ctx, false, false, wave);
        assert_eq!(wave_logo(&ctx, 10.0), None, "off: the logo keeps its own colours");
        set_wave(&ctx, true, false, wave);
        assert_eq!(wave_logo(&ctx, 10.0).map(|faces| faces[1]), Some(wave.tint(10.0, ctx.input(|i| i.time), 1.0)));
    }

    /// An accent keeps its shading as brightness, two shades stay apart, and white stays neutral.
    #[test]
    fn the_wave_replaces_the_accent_and_keeps_the_detail() {
        let (light, dark) = (Color32::from_rgb(90, 170, 255), Color32::from_rgb(20, 70, 160));
        let v = |c: Color32| wave_part(c, true).to_srgba_unmultiplied()[0];
        assert!(v(light) > 200 && v(light) - v(dark) > 60, "shades apart: {} {}", v(light), v(dark));
        assert_eq!(wave_part(light, false).a(), 0, "an accent pixel has no neutral part");
        let white = Color32::from_rgb(235, 235, 235);
        assert_eq!(wave_part(white, true).a(), 0, "white takes no tint");
        assert_eq!(wave_part(white, false), white);
    }

    #[test]
    fn theme_icon_palettes_change_uploaded_pixels_and_reuse_only_matching_textures() {
        use serde_json::json;
        let ctx = crate::fonts::test_context();
        let dark = theme::is_dark();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut previous_alpha = None;
        let mut previous_texture = None;
        for (stroke, expected) in [
            ("#994477", Color32::from_rgb(0x99, 0x44, 0x77)),
            ("#226699", Color32::from_rgb(0x22, 0x66, 0x99)),
            ("#994477", Color32::from_rgb(0x99, 0x44, 0x77)),
        ] {
            let file = crate::theme_files::from_value(&json!({"format":crate::theme_files::FORMAT,
                "name":"Switch","base":if dark {"dark"} else {"light"},
                "icon_palette":{"solid":["#c0d0e0","#668899",stroke]}}))
            .unwrap();
            let _restore = theme::scoped_tokens(crate::theme_files::resolve(&file));
            let draw = |ui: &mut egui::Ui| {
                let r = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0));
                paint_icon(ui.painter(), r, "extrude", 1.0);
                // A forced-dark monochrome app-bar icon must not evict the theme palette.
                paint_app_bar(ui.painter(), r.translate(egui::vec2(70.0, 0.0)), "save", 1.0);
            };
            let mut out = ctx.run_ui(egui::RawInput::default(), draw);
            let (id, image) = out
                .textures_delta
                .set
                .iter()
                .flat_map(|(id, deltas)| deltas.iter().map(move |d| (id, &d.image)))
                .find_map(|(id, data)| match data {
                    egui::ImageData::Color(image) if image.size == [64, 64] && image.pixels.contains(&expected) => Some((*id, image)),
                    _ => None,
                })
                .expect("the new theme's actual SVG texture");
            let alpha: Vec<_> = image.pixels.iter().map(|p| p.a()).collect();
            if let Some(old) = &previous_alpha {
                assert_eq!(&alpha, old, "theme colours preserve artwork and cutouts");
            }
            if let Some(old) = previous_texture {
                assert_ne!(id, old, "switching a same-base theme uploads a new texture");
            }
            previous_alpha = Some(alpha);
            previous_texture = Some(id);
            out.textures_delta.clear();
            let mut steady = ctx.run_ui(egui::RawInput::default(), draw);
            assert!(
                steady.textures_delta.set.values().flatten().all(|d| match &d.image {
                    egui::ImageData::Color(i) => i.size != [64, 64],
                }),
                "unchanged palette reuses cached textures"
            );
            steady.textures_delta.clear();
        }
    }

    #[test]
    fn assistant_pixels_use_the_application_bar_text_token() {
        let previous = theme::is_dark();
        for dark in [false, true] {
            let ctx = crate::fonts::test_context();
            theme::set_dark(dark);
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_app_bar(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(32.0, 32.0)), "assistant", 1.0);
            });
            let ink = theme::Tokens::get().app_bar_text;
            assert!(out.textures_delta.set.iter().any(|(_, deltas)| deltas.iter().any(|delta| match &delta.image {
                egui::ImageData::Color(image) => image.pixels.contains(&ink),
            })));
            out.textures_delta.clear();
        }
        theme::set_dark(previous);
    }

    #[test]
    fn actual_uploaded_icon_pixels_follow_semantics_and_vision_cache_changes() {
        let ctx = crate::fonts::test_context();
        let previous = theme::vision();
        for dark in [false, true] {
            for vision in [ColorVision::Off, ColorVision::Protan, ColorVision::Tritan] {
                theme::set_vision(vision);
                // The line types: Normal in the base blue, and Construction, Centerline and Infinite
                // in the construction accent, as the sketch canvas draws them.
                for icon in
                    ["line", "c_parallel", "normal", "construction", "centerline", "infinite", "surface_extrude", "mesh_export", "contact_new"]
                {
                    let expected = icon_svg::primary(icon, dark, vision);
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        let r = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0));
                        paint_on(ui.painter(), r, icon, Some(dark), 1.0);
                    });
                    assert!(
                        output.textures_delta.set.iter().any(|(_, deltas)| deltas.iter().any(|delta| match &delta.image {
                            egui::ImageData::Color(image) => image.pixels.contains(&expected),
                        })),
                        "{icon}, {dark}, {vision:?}: actual texture misses its semantic stroke or reused a stale cache entry"
                    );
                    if icon == "contact_new" {
                        let image = output
                            .textures_delta
                            .set
                            .values()
                            .flatten()
                            .find_map(|delta| match &delta.image {
                                egui::ImageData::Color(image) if image.size == [64, 64] => Some(image),
                                _ => None,
                            })
                            .unwrap();
                        assert_eq!(image.pixels.get(48 * 64 + 48), Some(&Color32::TRANSPARENT), "uploaded plus must stay transparent");
                    }
                    // This headless test inspects uploads instead of sending them
                    // to a GPU; acknowledge the deltas before they are dropped.
                    output.textures_delta.clear();
                }
            }
        }
        theme::set_vision(previous);
    }

    #[test]
    fn per_call_accents_preserve_roles_and_share_the_existing_cache() {
        let previous = theme::vision();
        for dark in [false, true] {
            for vision in ColorVision::ALL {
                theme::set_vision(vision);
                let ctx = crate::fonts::test_context();
                ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                let mut textures = HashMap::new();
                let mut ids = HashMap::new();
                let mut silhouette = None;
                for accent in [
                    Some("construction"),
                    Some("constraint"),
                    None,
                    Some("sketch"),
                    Some("surface"),
                    Some("form"),
                    Some("mesh"),
                    Some("unknown"),
                    Some(""),
                    None,
                ] {
                    let expected_accent = match accent {
                        Some("unknown" | "") => icon_svg::resolve("surface_extrude").accent,
                        Some(name) => icon_svg::accent_index(name),
                        None => None,
                    };
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        paint_accented(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0)), "surface_extrude", accent, 0.42);
                    });
                    let texture = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Mesh(mesh) => {
                                assert!(mesh.vertices.iter().all(|vertex| vertex.color == Color32::from_white_alpha(107)));
                                Some(mesh.texture_id)
                            }
                            _ => None,
                        })
                        .unwrap();
                    for (id, deltas) in &output.textures_delta.set {
                        for delta in deltas {
                            let egui::ImageData::Color(image) = &delta.image;
                            if delta.pos.is_none() {
                                textures.insert(*id, image.clone());
                            }
                        }
                    }
                    if let Some(previous_id) = ids.insert(expected_accent, texture) {
                        assert_eq!(texture, previous_id, "repeat and unknown-name fallback reuse the same accent texture");
                        assert!(!output.textures_delta.set.contains_key(&texture));
                    }
                    let image = textures.get(&texture).unwrap();
                    let look = Look::themed(dark, expected_accent, vision);
                    for role in ["ink", "passive-2", "passive-3", "active-1", "active-2", "active-3"] {
                        assert!(image.pixels.contains(&icon_svg::role_colour(role, look)), "{accent:?}/{role}, {dark}/{vision:?}");
                    }
                    let alpha: Vec<_> = image.pixels.iter().map(|pixel| pixel.a()).collect();
                    if let Some(previous) = &silhouette {
                        assert_eq!(&alpha, previous, "a ramp override must preserve all geometry and knockouts");
                    } else {
                        silhouette = Some(alpha);
                    }
                    output.textures_delta.clear();
                }
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0)), "surface_extrude", 1.0);
                });
                let texture = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    ids.get(&icon_svg::resolve("surface_extrude").accent),
                    Some(&texture),
                    "the old painter retains semantic selection and shares the cache"
                );
                output.textures_delta.clear();
            }
        }
        theme::set_vision(previous);
    }

    #[test]
    fn danger_and_accent_only_textures_do_not_share_a_cache_entry() {
        for dark in [false, true] {
            let ctx = crate::fonts::test_context();
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            let mut textures = HashMap::new();
            for danger in [false, true, false, true] {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 20.0));
                    if danger {
                        paint_danger(ui.painter(), rect, "delete", 1.0);
                    } else {
                        paint_accented(ui.painter(), rect, "delete", Some("constraint"), 1.0);
                    }
                });
                let id = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                        _ => None,
                    })
                    .unwrap();
                if let Some(previous) = textures.insert(danger, id) {
                    assert_eq!(id, previous, "repeated danger painting reuses its texture");
                    assert!(!output.textures_delta.set.contains_key(&id));
                }
                output.textures_delta.clear();
            }
            assert_ne!(textures[&false], textures[&true], "matching Look values must not mix ordinary gray bins with shaded red bins");
        }
    }
}
