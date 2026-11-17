//! Colour themes from plugins (capability `themes@0.1`, `docs/plugin-themes.md`).
//!
//! The theme files of the plugins that are on are read here and offered to the theme picker after
//! the built-in and imported themes, as `plugin:<plugin-id>/<theme-id>` in the `theme` preference.
//! The offer is read again only when a plugin is installed, removed or switched, never per frame.
//! When the chosen theme is no longer offered (its plugin is off or removed, or its file can't be
//! read), the default theme is turned on with one status line.

use crate::i18n::{format, tr};

use crate::SolveApp;

/// What the offer was read from: the switches and the installed packages. A change to either reads
/// the offer again.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) signature: Option<String>,
}

/// Reads the plugin themes again if the plugins changed since the last call.
pub(crate) fn sync(app: &mut SolveApp) {
    let signature = format!("{:?}|{:?}", app.preferences.disabled_plugins, app.session.plugin_state.packages);
    if app.plugin_themes.signature.as_deref() == Some(signature.as_str()) {
        return;
    }
    app.plugin_themes.signature = Some(signature);
    let dir = if cfg!(target_arch = "wasm32") { None } else { app.session.plugin_state.plugins_dir() };
    let (plugins, _) = crate::plugins::inventory(dir.as_deref(), app.session.plugin_state.packages.values());
    let mut offers = Vec::new();
    let mut failure = None;
    for plugin in plugins.iter().filter(|plugin| crate::plugins::enabled(app, &plugin.id)) {
        for (key, _, source) in &plugin.themes {
            match read(&plugin.path, source) {
                Ok(text) => offers.push((key.clone(), text)),
                Err(error) => failure = failure.or(Some(format!("{key}: {error}"))),
            }
        }
    }
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
}

/// A theme file's text, read whole if it is within its size limit.
fn read(folder: &str, source: &str) -> Result<String, String> {
    let path = std::path::Path::new(folder).join(source);
    let size = std::fs::metadata(&path).map_err(|e| format!("{source}: {e}"))?.len();
    if size > solidcraft_engine::plugin::theme::MAX_THEME_BYTES as u64 {
        return Err(format!("{source}: larger than {} KiB", solidcraft_engine::plugin::theme::MAX_THEME_BYTES / 1024));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("{source}: {e}"))
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

#[cfg(test)]
mod tests {
    use crate::scenario::Harness;

    const GLASS: &str = r##"{"format":"solidcraft-theme/1","name":"Solid Glass","base":"dark","colors":{"accent":"#44d62c"}}"##;

    /// A plugin `tints` with one theme, in a plugins folder the app reads.
    fn install(h: &mut Harness) -> std::path::PathBuf {
        let dir = h.dir.join("theme-plugins");
        let folder = dir.join("tints");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("plugin.json"),
            r#"{"id":"tints","name":"Tints","version":"1","capabilities":["themes@0.1"],"themes":[{"id":"solid-glass","source":"solid-glass.json"}]}"#,
        )
        .unwrap();
        std::fs::write(folder.join("solid-glass.json"), GLASS).unwrap();
        h.app.session.plugin_state.plugins_dir = Some(dir.clone());
        dir
    }

    fn offered(h: &Harness) -> bool {
        crate::theme_files::find("plugin:tints/solid-glass", &h.app.preferences.custom_themes).is_some()
    }

    /// The offer lists an installed plugin's theme, which applies its colours while chosen; turning
    /// the plugin off, or removing it, turns the default theme on with one status line, once.
    #[test]
    fn a_plugin_theme_is_listed_applies_and_falls_back_once_when_its_plugin_goes() {
        let mut h = Harness::new();
        let dir = install(&mut h);
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(offered(&h), "status {:?}, plugins {:?}", h.app.status, crate::plugins::inventory(h.app.session.plugin_state.plugins_dir().as_deref(), std::iter::empty()).0.iter().map(|p| (p.id.clone(), p.themes.len(), p.path.clone())).collect::<Vec<_>>());
        assert!(crate::theme_files::plugin_theme_choices().contains(&("plugin:tints/solid-glass".into(), "Solid Glass".into())));
        h.app.preferences.theme = "plugin:tints/solid-glass".into();
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert_eq!(h.app.preferences.theme, "plugin:tints/solid-glass", "an offered theme stays chosen");
        let accent = crate::theme_files::find("plugin:tints/solid-glass", &h.app.preferences.custom_themes).unwrap();
        assert_eq!(crate::theme_files::resolve(accent).accent, crate::theme_files::parse_color("#44d62c").unwrap());

        h.app.preferences.disabled_plugins.insert("tints".into());
        h.app.status = None;
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(!offered(&h));
        assert_eq!(h.app.preferences.theme, "dark");
        assert!(h.app.ui.dark);
        let (message, _, error) = h.app.status.clone().expect("one status line");
        assert!(error && message.contains("solid-glass"), "{message}");
        h.app.status = None;
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(h.app.status.is_none(), "not retried or repeated while nothing changes");

        h.app.preferences.disabled_plugins.remove("tints");
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(offered(&h), "back on the offer once its plugin is turned on");

        std::fs::remove_dir_all(dir.join("tints")).unwrap();
        h.app.preferences.theme = "plugin:tints/solid-glass".into();
        h.app.plugin_themes.signature = None;
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(!offered(&h), "a removed plugin's theme is gone");
        assert_eq!(h.app.preferences.theme, "dark");
    }

    /// A theme file that is no longer a theme leaves the offer and the picker's list alone.
    #[test]
    fn a_plugin_theme_whose_file_is_broken_is_not_offered() {
        let mut h = Harness::new();
        let dir = install(&mut h);
        std::fs::write(dir.join("tints").join("solid-glass.json"), "{").unwrap();
        crate::plugins::tick(&mut h.app, &h.ctx, true);
        assert!(!offered(&h));
        assert!(crate::theme_files::plugin_theme_choices().is_empty());
    }

    /// The install review names a package's themes by their ids.
    #[test]
    fn the_install_review_lists_a_plugins_themes() {
        let lines = crate::plugin_install::permission_lines(&serde_json::json!({
            "id": "tints", "themes": [{"id": "solid-glass", "source": "solid-glass.json"}]
        }));
        assert!(lines.iter().any(|line| line.contains("solid-glass")), "{lines:?}");
    }
}
