//! Colour themes from plugins (capability `themes@0.1`, `docs/plugin-themes.md`).
//!
//! The theme files of the plugins that are on are read here and offered to the theme picker after
//! the built-in and imported themes, as `plugin:<plugin-id>/<theme-id>` in the `theme` preference.
//! The offer is read again only when a plugin is installed, removed or switched, never per frame.
//! When the chosen theme is no longer offered (its plugin is off or removed, or its file can't be
//! read), the default theme is turned on with one status line.
//!
//! A theme file with `glass` (Liquid Craft) is a look, not a picker entry: it is on while its
//! plugin's switch is on, over whatever the picker chose, and its settings are on that plugin's
//! card. Of a plugin's glass themes, the one whose base matches `glass_dark` is used.

use solidcraft_ui_text::{format, tr};

use crate::SolveApp;

/// What the offer was read from: the switches and the installed packages. A change to either reads
/// the offer again.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) signature: Option<Signature>,
    /// The installed plugins with a glass theme, on or off: their cards carry the glass settings.
    glass_plugins: std::collections::BTreeSet<String>,
    /// The glass themes of the plugins that are on: the theme's key and whether its base is dark.
    looks: Vec<(String, bool)>,
}

/// The inputs of the offer, compared in place each frame without building a string.
pub(crate) struct Signature {
    dir: Option<std::path::PathBuf>,
    disabled: std::collections::BTreeSet<String>,
    packages: std::collections::BTreeMap<String, String>,
}

impl Signature {
    fn matches(&self, dir: &Option<std::path::PathBuf>, app: &SolveApp) -> bool {
        self.dir == *dir && self.disabled == app.preferences.disabled_plugins && self.packages == app.session.plugin_state.packages
    }
}

/// Reads the plugin themes again if the plugins changed since the last call.
pub(crate) fn sync(app: &mut SolveApp) {
    let dir = if cfg!(target_arch = "wasm32") { None } else { app.session.plugin_state.plugins_dir() };
    if app.plugin_themes.signature.as_ref().is_some_and(|signature| signature.matches(&dir, app)) {
        return;
    }
    app.plugin_themes.signature =
        Some(Signature { dir: dir.clone(), disabled: app.preferences.disabled_plugins.clone(), packages: app.session.plugin_state.packages.clone() });
    let (plugins, _) = crate::plugins::inventory(dir.as_deref(), app.session.plugin_state.packages.values());
    let kinds = plugins
        .iter()
        .filter(|plugin| crate::plugins::enabled(app, &plugin.id))
        .flat_map(|plugin| plugin.effects.iter().map(|(_, _, kind)| kind.clone()))
        .collect();
    crate::plugin_effects::set_active(app, kinds);
    let mut offers = Vec::new();
    let mut failure = None;
    let mut glass_plugins = std::collections::BTreeSet::new();
    let mut looks = Vec::new();
    for plugin in &plugins {
        let on = crate::plugins::enabled(app, &plugin.id);
        for (key, _, source) in &plugin.themes {
            match read(&plugin.path, source) {
                Ok(text) => {
                    if let Some(dark) = glass_base(&text) {
                        glass_plugins.insert(plugin.id.clone());
                        if on {
                            looks.push((key.clone(), dark));
                        }
                    }
                    if on {
                        offers.push((key.clone(), text));
                    }
                }
                Err(error) if on => failure = failure.or(Some(format!("{key}: {error}"))),
                Err(_) => {}
            }
        }
    }
    app.plugin_themes.glass_plugins = glass_plugins;
    app.plugin_themes.looks = looks;
    let errors = crate::theme_files::set_plugin_themes(offers);
    if let Some(error) = failure.or_else(|| errors.into_iter().next()) {
        app.set_status(error, true);
    }
    if let Some(id) = app.preferences.theme.strip_prefix("plugin:").map(str::to_owned)
        && crate::theme_files::find(&app.preferences.theme, &app.preferences.custom_themes).is_none()
    {
        let theme = id.rsplit('/').next().unwrap_or_default().to_owned();
        app.preferences.theme = "dark".into();
        app.ui.dark = true;
        app.set_status(format("prefs.plugin_theme_unavailable", &[("name", theme.as_str())]), true);
    }
    for (slot, default) in [(&mut app.preferences.theme_day, "light"), (&mut app.preferences.theme_night, "dark")] {
        if removed(slot, &plugins) {
            *slot = default.into();
        }
    }
}

/// Whether `theme` names a plugin theme of a plugin that is no longer installed.
fn removed(theme: &str, plugins: &[crate::plugins::Plugin]) -> bool {
    let Some(rest) = theme.strip_prefix("plugin:") else { return false };
    let id = rest.split('/').next().unwrap_or_default();
    !plugins.iter().any(|plugin| plugin.id == id)
}

/// A theme file's text, read whole if it is within its size limit.
fn read(folder: &str, source: &str) -> Result<String, String> {
    let path = std::path::Path::new(folder).join(source);
    let meta = std::fs::metadata(&path).map_err(|e| format!("{source}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{source}: not a file"));
    }
    if meta.len() > solidcraft_engine::plugin::theme::MAX_THEME_BYTES as u64 {
        return Err(format!("{source}: larger than {} KiB", solidcraft_engine::plugin::theme::MAX_THEME_BYTES / 1024));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("{source}: {e}"))
}

/// A theme file with glass: `Some(dark)`, whether its base is dark. `None`: no glass.
fn glass_base(text: &str) -> Option<bool> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    value.get("glass").is_some_and(serde_json::Value::is_object).then(|| value.get("base").and_then(serde_json::Value::as_str) == Some("dark"))
}

/// Whether an installed plugin offers a glass theme: its card carries the glass settings.
pub(crate) fn has_glass(app: &SolveApp, plugin: &str) -> bool {
    app.plugin_themes.glass_plugins.contains(plugin)
}

/// Whether that plugin offers a light and a dark glass theme, so its card offers the choice.
pub(crate) fn has_both_tones(app: &SolveApp, plugin: &str) -> bool {
    let prefix = format!("plugin:{plugin}/");
    let mut tones = app.plugin_themes.looks.iter().filter(|(key, _)| key.starts_with(&prefix)).map(|(_, dark)| *dark);
    tones.clone().any(|dark| dark) && tones.any(|dark| !dark)
}

/// The glass theme on now, if a plugin with one is on: the one matching `glass_dark`, else its first.
pub(crate) fn glass_look(app: &SolveApp) -> Option<&str> {
    let looks = &app.plugin_themes.looks;
    looks.iter().find(|(_, dark)| *dark == app.preferences.glass_dark).or(looks.first()).map(|(key, _)| key.as_str())
}

/// Whether `id` is a glass theme of a plugin that is on: the theme picker leaves it out.
pub(crate) fn is_look(app: &SolveApp, id: &str) -> bool {
    app.plugin_themes.looks.iter().any(|(key, _)| key == id)
}

/// A plugin's theme name, read from its file, or its id when the file doesn't say.
pub(crate) fn name(folder: &str, source: &str, id: &str) -> String {
    read(folder, source)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.get("name").and_then(serde_json::Value::as_str).map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned))
        .unwrap_or_else(|| id.to_owned())
}

/// The card's line naming a plugin's themes.
pub(crate) fn hint(names: &[&str]) -> String {
    tr("prefs.plugin_themes").replace("{names}", &names.join(", "))
}
