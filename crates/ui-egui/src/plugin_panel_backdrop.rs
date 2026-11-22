//! Panel backdrops from plugins (capability `panel-backdrop@0.1`, `docs/plugin-panel-backdrop.md`).
//!
//! The plugin's WGSL is checked off the window thread and handed to the panel blur
//! (`ui_kit::blur`), which then draws every see-through panel's backdrop with it. Whatever fails
//! leaves the plain blur, with one line in the status bar.

use std::sync::{Arc, Mutex, PoisonError};

use solidcraft_engine::plugin::shader::{self, ShaderKind, ShaderParam};

use crate::SolveApp;

const UNREADABLE: &str = "prefs.panel_backdrop_failed";

struct Loaded {
    wgsl: String,
    animated: bool,
    blur: f32,
    params: Vec<ShaderParam>,
    textures: Vec<Vec<([u32; 2], Vec<u8>)>>,
}

type Pending = Arc<Mutex<Option<Result<Loaded, String>>>>;

struct Active {
    key: String,
    animated: bool,
    blur: f32,
    params: Vec<ShaderParam>,
}

#[derive(Default)]
pub(crate) struct State {
    render: Option<egui_wgpu::RenderState>,
    loading: Option<(String, Pending)>,
    active: Option<Active>,
    /// The key that failed, so it is not tried every frame; another choice clears it.
    failed: Option<String>,
    /// `failed` only because its plugin is off: turning it on again clears it.
    off: bool,
}

impl State {
    /// The window's GPU. A new device starts over (the blur drops its hook on install too).
    pub(crate) fn install(&mut self, rs: &egui_wgpu::RenderState) {
        *self = Self { render: Some(rs.clone()), ..Self::default() };
    }
}

/// Reads and checks the chosen panel backdrop off the window thread.
fn load(folder: String, key: String, limit: u32) -> Result<Loaded, String> {
    let manifest = crate::plugin_shaders::manifest(&folder)?;
    let shader_id = key.split_once('/').map(|(_, s)| s).unwrap_or_default();
    let shader = manifest
        .shaders
        .iter()
        .find(|s| s.id == shader_id && s.kind == ShaderKind::PanelBackdrop)
        .ok_or_else(|| format!("{key} is not a panel backdrop of the plugin any more"))?;
    let dir = std::path::Path::new(&folder);
    let read = |file: &str, max: usize| -> Result<Vec<u8>, String> {
        let path = dir.join(file);
        let size = std::fs::metadata(&path).map_err(|e| format!("{file}: {e}"))?.len();
        if size > max as u64 {
            return Err(format!("{file}: larger than {} KiB", max / 1024));
        }
        std::fs::read(&path).map_err(|e| format!("{file}: {e}"))
    };
    let source = String::from_utf8(read(&shader.source, shader::MAX_SOURCE_BYTES)?).map_err(|_| format!("{} is not UTF-8 text", shader.source))?;
    let compiled = shader::compile(shader, &source).map_err(|e| e.to_string())?;
    let side = limit.min(shader::MAX_TEXTURE_SIDE);
    let textures = shader
        .textures
        .iter()
        .map(|t| {
            read(&t.file, shader::MAX_TEXTURE_BYTES)
                .and_then(|bytes| crate::plugin_shaders::mips(&bytes, side).map_err(|e| format!("{}: {e}", t.file)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Loaded { wgsl: compiled.wgsl, animated: shader.animated, blur: shader.blur_radius(), params: shader.params.clone(), textures })
}

/// Before the window's frame begins: loads, swaps or drops the chosen panel backdrop and writes
/// this frame's uniform. Nothing chosen: the blur has no hook and panels draw as without plugins.
pub(crate) fn prepare(app: &mut SolveApp, ctx: &egui::Context) {
    if ctx.viewport_id() != egui::ViewportId::ROOT || app.plugin_panel_backdrop.render.is_none() {
        return;
    }
    let key = app.preferences.panel_backdrop.clone();
    if !allowed(app, &key) {
        return;
    }
    let state = &mut app.plugin_panel_backdrop;
    if state.failed.as_ref().is_some_and(|failed| *failed != key) {
        state.failed = None;
    }
    if state.active.as_ref().is_some_and(|a| a.key != key) || (key.is_empty() && app.blur.panel_hook_active()) {
        drop_hook(app);
    }
    let state = &mut app.plugin_panel_backdrop;
    if key.is_empty() || state.failed.as_deref() == Some(key.as_str()) {
        return;
    }
    let Some(rs) = state.render.clone() else { return };
    if rs.device_lost.is_lost() {
        // The shader may be why the driver gave up: off before the app restarts.
        app.plugin_panel_backdrop.failed = Some(key.clone());
        app.preferences.panel_backdrop.clear();
        drop_hook(app);
        log::warn!("graphics device lost while the panel backdrop {key} was drawn; it is turned off");
        return;
    }
    if state.active.is_none() {
        if state.loading.as_ref().is_none_or(|(k, _)| *k != key) {
            let Some(folder) = crate::plugin_shaders::folder_of(app, &key) else {
                missing(app, &key);
                return;
            };
            let pending: Pending = Arc::default();
            let (slot, repaint, k, limit) = (pending.clone(), ctx.clone(), key.clone(), rs.device.limits().max_texture_dimension_2d);
            std::thread::spawn(move || {
                let result = load(folder, k, limit);
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                repaint.request_repaint();
            });
            app.plugin_panel_backdrop.loading = Some((key, pending));
            return;
        }
        let done = state.loading.as_ref().and_then(|(_, p)| p.lock().unwrap_or_else(PoisonError::into_inner).take());
        let Some(done) = done else { return };
        state.loading = None;
        let built = done.and_then(|loaded| {
            let active = Active { key: key.clone(), animated: loaded.animated, blur: loaded.blur, params: loaded.params };
            let hook = crate::blur::PanelHook { wgsl: loaded.wgsl, textures: loaded.textures, blur: loaded.blur };
            app.blur.set_panel_hook(Some(hook)).map(|()| active)
        });
        match built {
            Ok(active) => app.plugin_panel_backdrop.active = Some(active),
            Err(error) => {
                app.plugin_panel_backdrop.failed = Some(key.clone());
                let message = format!("{} {key}: {error}", solidcraft_ui_text::tr(UNREADABLE));
                log::warn!("{message}");
                app.set_status(message, true);
                return;
            }
        }
    }
    let Some(active) = &app.plugin_panel_backdrop.active else { return };
    let time = if active.animated {
        let focused = ctx.input(|i| i.focused);
        ctx.request_repaint_after(std::time::Duration::from_millis(if focused { 33 } else { 100 }));
        (ctx.input(|i| i.time) % 3600.0) as f32
    } else {
        0.0
    };
    let ppp = ctx.pixels_per_point();
    let size = ctx.content_rect().size() * ppp;
    let values = param_values(&active.key, &active.params, &app.preferences.panel_backdrop_params);
    let bytes = host_uniforms([size.x, size.y], time, ppp, active.blur, app.blur.target_srgb(), &values, &solidcraft_theme::Tokens::get());
    app.blur.set_panel_hook_globals(&bytes);
}

fn drop_hook(app: &mut SolveApp) {
    app.plugin_panel_backdrop.active = None;
    if let Err(error) = app.blur.set_panel_hook(None) {
        log::warn!("panel backdrop: {error}");
    }
}

/// A plugin turned off after its backdrop was set, or while it loads, drops it, saying so once.
fn allowed(app: &mut SolveApp, key: &str) -> bool {
    if key.is_empty() {
        return true;
    }
    let plugin = key.split_once('/').map_or(key, |(id, _)| id);
    let enabled = crate::plugins::enabled(app, plugin);
    let state = &mut app.plugin_panel_backdrop;
    if state.failed.as_deref() == Some(key) {
        if !(state.off && enabled) {
            return true;
        }
        state.failed = None;
    }
    state.off = false;
    if enabled {
        return true;
    }
    forget(app, plugin);
    missing(app, key);
    app.plugin_panel_backdrop.off = true;
    false
}

/// Drops `plugin`'s panel backdrop, set or loading, so it is read again from its folder when next chosen.
pub(crate) fn forget(app: &mut SolveApp, plugin: &str) {
    let owns = |key: &str| key.split_once('/').is_some_and(|(id, _)| id == plugin);
    if app.plugin_panel_backdrop.active.as_ref().is_some_and(|a| owns(&a.key)) {
        drop_hook(app);
    }
    let state = &mut app.plugin_panel_backdrop;
    if state.loading.as_ref().is_some_and(|(key, _)| owns(key)) {
        state.loading = None;
    }
}

fn missing(app: &mut SolveApp, key: &str) {
    app.plugin_panel_backdrop.failed = Some(key.to_owned());
    let message = format!("{} {key}: {}", solidcraft_ui_text::tr(UNREADABLE), solidcraft_ui_text::tr("prefs.background_shader_missing"));
    log::warn!("{message}");
    app.set_status(message, true);
}

/// The card's settings in manifest order, each within its range; a missing one at its default.
fn param_values(key: &str, params: &[ShaderParam], stored: &std::collections::BTreeMap<String, f32>) -> [f32; shader::MAX_PARAMS] {
    let mut values = [0.0; shader::MAX_PARAMS];
    for (value, param) in values.iter_mut().zip(params) {
        *value = stored.get(&format!("{key}/{}", param.id)).copied().unwrap_or(param.default).clamp(param.min, param.max);
    }
    values
}

/// Linear-light RGBA of a theme colour, alpha 1.
fn linear(c: egui::Color32) -> [f32; 4] {
    let l = egui::Rgba::from(c);
    [l.r(), l.g(), l.b(), 1.0]
}

/// `ScHost` as a panel backdrop reads it (`shader::PANEL_PRELUDE`).
fn host_uniforms(
    size: [f32; 2],
    time: f32,
    scale: f32,
    blur: f32,
    srgb: bool,
    params: &[f32; shader::MAX_PARAMS],
    t: &solidcraft_theme::Tokens,
) -> Vec<u8> {
    let mut f: Vec<f32> = vec![size[0], size[1], time, scale];
    f.extend(linear(t.accent));
    f.extend(linear(t.text));
    let dark = solidcraft_theme::is_dark();
    f.extend([if dark { 1.0 } else { 0.0 }, blur, if srgb { 1.0 } else { 0.0 }, 0.0]);
    f.extend(params);
    debug_assert_eq!(f.len() * 4, shader::PANEL_HOST_UNIFORM_BYTES);
    f.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[cfg(test)]
#[path = "plugin_panel_backdrop_tests.rs"]
mod tests;
