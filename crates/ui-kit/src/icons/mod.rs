//! Icons: the kit draws them; the host supplies them.
//!
//! The kit ships no artwork. A host keeps its icons in a `static` [`IconSet`] and [`install`]s it
//! once, before the first frame (and in every test context that draws icons). Every painter here
//! draws from that set, in the set's colour roles for the current theme, accent and colour vision;
//! colours never come from arbitrary caller tints. A name the set does not know draws the set's
//! placeholder master; with no set installed nothing is drawn. A host that draws another host's
//! icons plus a few of its own installs [`IconSet::extended`] over that host's data.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, Painter, Rect, TextureHandle, TextureOptions, pos2};

pub mod glyphs;
mod svg;

pub use svg::{
    Doc, IconData, IconExtension, IconSet, Look, MAX_ACCENTS, MAX_ROLES, PLACEHOLDER, Palette, Resolved, audit, render, render_tinted,
    set_theme_palette, theme_palette, theme_palette_state,
};

static INSTALLED: OnceLock<&'static IconSet> = OnceLock::new();

/// No artwork: what the painters draw from before a set is installed.
static NO_ICONS: IconSet = IconSet::new(IconData::EMPTY);

/// Install the host's icons, once per process, before the first frame. Installing the same set
/// again does nothing; a different set is ignored and logged.
pub fn install(set: &'static IconSet) {
    if let Err(ignored) = INSTALLED.set(set)
        && let Some(installed) = INSTALLED.get()
        && !std::ptr::eq(*installed, ignored)
    {
        log::warn!("an icon set is already installed: a second, different set is ignored");
    }
}

/// The installed set, or an empty one before [`install`].
pub fn installed() -> &'static IconSet {
    INSTALLED.get().copied().unwrap_or(&NO_ICONS)
}

/// [`IconSet::resolve`] in the installed set.
pub fn resolve(name: &str) -> Resolved {
    installed().resolve(name)
}

/// [`IconSet::doc_for`] in the installed set.
pub fn doc_for(master: &str, px: u32) -> Option<&'static Doc> {
    installed().doc_for(master, px)
}

/// [`IconSet::accent_index`] in the installed set.
pub fn accent_index(name: &str) -> Option<usize> {
    installed().accent_index(name)
}

/// [`IconSet::primary`] in the installed set.
pub fn primary(name: &str, dark: bool, vision: solidcraft_theme::ColorVision) -> Color32 {
    installed().primary(name, dark, vision)
}

/// [`IconSet::role_colour`] in the installed set.
pub fn role_colour(name: &str, look: Look) -> Color32 {
    installed().role_colour(name, look)
}

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
pub fn clear_texture_cache(ctx: &egui::Context) {
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::unique("sc_icon_cache"), Cache::default).clone());
    if let Ok(mut entries) = cache.0.lock() {
        entries.textures.clear();
    }
}

#[derive(Default)]
struct CacheEntries {
    palette: Option<Palette>,
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
pub fn paint_app_bar(p: &Painter, r: Rect, name: &str, opacity: f32) {
    paint_with_ink(p, r, name, Some(true), opacity, Some(solidcraft_theme::Tokens::get().app_bar_text), Mirror::None);
}

/// Paint a single-colour icon in `ink`.
pub fn paint_ink(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::None);
}

/// Paint a single-colour icon in `ink`, mirrored top to bottom (a chevron pointing up).
pub fn paint_ink_flipped(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::Vertical);
}

/// Paint a single-colour icon mirrored left to right, reusing the original master.
pub fn paint_ink_mirrored(p: &Painter, r: Rect, name: &str, ink: Color32) {
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
    let resolved = resolve(name);
    let look = Look::themed(dark.unwrap_or_else(|| ctx.global_style().visuals.dark_mode), resolved.accent, solidcraft_theme::vision());
    paint_resolved(p, r, resolved, look, opacity, ink.map_or(Finish::Artwork, Finish::Ink), mirror);
}

/// Draw with a named palette ramp, preserving outlines, passive faces and transparency.
/// `None` explicitly selects base blue; an unknown name retains the icon's own accent.
/// Existing painters continue to select the icon's semantic accent.
pub fn paint_accented(p: &Painter, r: Rect, name: &str, accent: Option<&str>, opacity: f32) {
    let resolved = resolve(name);
    let accent = match accent {
        Some(name) => accent_index(name).or(resolved.accent),
        None => None,
    };
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, accent, solidcraft_theme::vision());
    paint_resolved(p, r, resolved, look, opacity, Finish::Artwork, Mirror::None);
}

/// Shared destructive-action artwork: shaded faces in the set's danger ramp, normal outlines and cutouts.
/// Unlike a single-colour mask, this also tints a master's passive faces through
/// that ramp without flattening the lid, handle or other details.
pub fn paint_danger(p: &Painter, r: Rect, name: &str, opacity: f32) {
    let set = installed();
    let resolved = set.resolve(name);
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, set.accent_index(set.data().danger_accent), solidcraft_theme::vision());
    paint_resolved(p, r, resolved, look, opacity, Finish::Danger, Mirror::None);
}

fn paint_resolved(p: &Painter, r: Rect, resolved: Resolved, look: Look, opacity: f32, finish: Finish, mirror: Mirror) {
    paint_tinted(p, r, resolved, look, opacity, finish, mirror, None);
}

#[allow(clippy::too_many_arguments)]
fn paint_tinted(p: &Painter, r: Rect, resolved: Resolved, look: Look, opacity: f32, finish: Finish, mirror: Mirror, tint: Option<Color32>) {
    let ctx = p.ctx();
    let ppp = ctx.pixels_per_point().max(0.5);
    let side = r.width().min(r.height());
    let px = (side * ppp).round().clamp(8.0, 512.0) as u32;
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::unique("sc_icon_cache"), Cache::default).clone());
    let Ok(mut entries) = cache.0.lock() else { return };
    let palette = theme_palette(solidcraft_theme::is_dark());
    if entries.palette != palette {
        entries.textures.clear();
        entries.palette = palette;
    }
    let map = &mut entries.textures;
    let key = (resolved.master, look, px, finish);
    if let std::collections::hash_map::Entry::Vacant(slot) = map.entry(key) {
        let Some(doc) = doc_for(resolved.master, px) else { return };
        let pm = if finish == Finish::Danger { render_tinted(doc, look, px) } else { render(doc, look, px) };
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
pub fn wave_part(pixel: Color32, accent: bool) -> Color32 {
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
pub struct Wave {
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
    pub fn phase(self, time: f64) -> f32 {
        (time * f64::from(self.speed) / f64::from(self.length.max(1.0))).rem_euclid(1.0) as f32
    }

    /// The wave's colour at screen x (points) and time (s), at `opacity`.
    pub fn tint(self, x: f32, time: f64, opacity: f32) -> Color32 {
        let hue = (x / self.length.max(1.0) - self.phase(time)).rem_euclid(1.0);
        let [r, g, b] = egui::ecolor::Hsva::new(hue, WAVE_SATURATION, 1.0, 1.0).to_srgb();
        Color32::from_rgb(r, g, b).gamma_multiply(opacity.clamp(0.0, 1.0))
    }
}

const WAVE_SATURATION: f32 = 0.65;
/// How strongly the wave shows over a panel.
pub const WAVE_PANEL_ALPHA: f32 = 0.22;

/// Set by the ribbon each frame: the wave on icons, on button blocks. Asks for ~30 fps while on.
pub fn set_wave(ctx: &egui::Context, icons: bool, blocks: bool, wave: Wave) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(WAVE_FLAGS), (icons, blocks, wave)));
    if icons || blocks {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

pub fn wave_flags(ctx: &egui::Context) -> (bool, bool, Wave) {
    ctx.data(|d| d.get_temp(egui::Id::unique(WAVE_FLAGS))).unwrap_or_default()
}

/// The logo's faces with the wave on icons: the wave's colour at `x` in its three shades.
pub fn wave_logo(ctx: &egui::Context, x: f32) -> Option<[Color32; 3]> {
    let (icons, _, wave) = wave_flags(ctx);
    icons.then(|| solidcraft_theme::logo_shades(wave.tint(x, ctx.input(|i| i.time), 1.0)))
}

/// One colour cycle of the wave, to repeat across a panel.
pub fn wave_texture(ctx: &egui::Context) -> egui::TextureId {
    let id = egui::Id::unique("sc_rgb_wave_texture");
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

/// A ribbon command icon: [`paint_icon`], or with the wave on, the artwork with the wave in place
/// of its accent.
pub fn paint_ribbon_icon(p: &Painter, r: Rect, name: &str, opacity: f32) {
    let ctx = p.ctx();
    let (icons, _, wave) = wave_flags(ctx);
    if !icons {
        return paint_icon(p, r, name, opacity);
    }
    let resolved = resolve(name);
    let look = Look::themed(ctx.global_style().visuals.dark_mode, resolved.accent, solidcraft_theme::vision());
    let tint = wave.tint(r.center().x, ctx.input(|i| i.time), opacity);
    paint_tinted(p, r, resolved, look, opacity, Finish::WaveNeutral, Mirror::None, None);
    paint_tinted(p, r, resolved, look, opacity, Finish::WaveAccent, Mirror::None, Some(tint));
}

/// A ribbon button's block in the wave's colour at low alpha (`rgb_wave_background`).
pub fn paint_ribbon_block(p: &Painter, r: Rect, radius: u8, enabled: bool) {
    let ctx = p.ctx();
    let (_, blocks, wave) = wave_flags(ctx);
    if enabled && blocks {
        p.rect_filled(r, radius, wave.tint(r.center().x, ctx.input(|i| i.time), 0.18));
    }
}

