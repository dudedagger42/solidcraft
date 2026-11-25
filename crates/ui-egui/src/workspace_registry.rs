//! Workspaces as data (add-ons prototype, phase 1). The switcher, its menu, `ui.workspace` and
//! the ribbon read one list: the three built-in workspaces, then each workspace a turned-on plugin
//! declares (`Declarations.workspaces`, RFC 0015), keyed `plugin:<plugin id>/<workspace id>`.
//!
//! A plugin workspace shows its own ribbon tabs in place of the built-in ones. Its tabs take the
//! fixed ids [`PLUGIN_TABS`] (the declaration allows six), so the ribbon's tab code keeps its
//! `&'static str` ids; their labels are the plugin's tab names. Each group's entries are the
//! plugin's own commands, shown as its ribbon buttons are. Entering runs the workspace's
//! `on_enter` entry and leaving runs `on_exit`, as ordinary plugin commands.

use solidcraft_engine::plugin::ui::{DesignKind, RibbonTab, WorkspaceSpec};

use crate::SolveApp;

/// The prefix of a plugin workspace's key.
pub(crate) const PLUGIN_PREFIX: &str = "plugin:";

/// The ribbon tab ids a plugin workspace's tabs take, in order.
pub(crate) const PLUGIN_TABS: [&str; 6] = ["PLUGIN 1", "PLUGIN 2", "PLUGIN 3", "PLUGIN 4", "PLUGIN 5", "PLUGIN 6"];

/// Where a workspace comes from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Source {
    Builtin,
    Plugin { plugin: String, plugin_name: String, approval: String, spec: Box<WorkspaceSpec> },
}

/// One entry of the switcher's list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    /// `design`, `drawing`, `render` or `plugin:<plugin id>/<workspace id>`.
    pub(crate) key: String,
    /// The name shown: a built-in's message id, or the plugin's own text.
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) source: Source,
}

impl Entry {
    pub(crate) fn plugin(&self) -> Option<(&str, &WorkspaceSpec)> {
        match &self.source {
            Source::Plugin { plugin, spec, .. } => Some((plugin.as_str(), spec.as_ref())),
            Source::Builtin => None,
        }
    }
}

/// `plugin:<plugin id>/<workspace id>`.
pub(crate) fn plugin_key(plugin: &str, workspace: &str) -> String {
    format!("{PLUGIN_PREFIX}{plugin}/{workspace}")
}

/// The plugin id and workspace id of a plugin workspace's key.
pub(crate) fn split_key(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix(PLUGIN_PREFIX)?.split_once('/').filter(|(plugin, ws)| !plugin.is_empty() && !ws.is_empty())
}

/// Whether a workspace that applies to `kinds` is offered for the design open now. An empty list
/// applies everywhere. The prototype tells part and assembly designs apart from nothing yet, so a
/// design (not a sheet) is both; a drawing is a sheet view.
fn applies(kinds: &[DesignKind], sheet: bool) -> bool {
    kinds.is_empty()
        || kinds.iter().any(|kind| match kind {
            DesignKind::Drawing => sheet,
            DesignKind::Part | DesignKind::Assembly | DesignKind::Sketch => !sheet,
        })
}

/// The built-ins, in the switcher's order (Design, Drawing, Render).
fn builtins() -> Vec<Entry> {
    [
        ("design", "ui.workspace.design", "ws_design"),
        ("drawing", "ui.workspace.drawing", "ws_drawing"),
        ("render", "ui.workspace.render", "ws_render"),
    ]
    .into_iter()
    .map(|(key, name, icon)| Entry { key: key.into(), name: name.into(), icon: icon.into(), source: Source::Builtin })
    .collect()
}

/// The plugin workspaces of `packages` (plugin id, its name, approval, declared workspaces) that
/// apply to the design open now, in plugin id order then declaration order.
pub(crate) fn plugin_entries<'a>(packages: impl IntoIterator<Item = (&'a str, &'a str, &'a str, &'a [WorkspaceSpec])>, sheet: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    for (plugin, plugin_name, approval, specs) in packages {
        for spec in specs.iter().take(2).filter(|spec| applies(&spec.applies_to, sheet)) {
            out.push(Entry {
                key: plugin_key(plugin, &spec.id),
                name: spec.name.clone(),
                icon: spec.icon.clone(),
                source: Source::Plugin {
                    plugin: plugin.into(),
                    plugin_name: plugin_name.into(),
                    approval: approval.into(),
                    spec: Box::new(spec.clone()),
                },
            });
        }
    }
    out
}

/// Every workspace the switcher offers now.
pub(crate) fn registry(app: &SolveApp) -> Vec<Entry> {
    let mut out = builtins();
    if cfg!(target_arch = "wasm32") {
        return out;
    }
    let sheet = crate::drawing_workspace::active(app) || crate::workspace::sheet_scope(app);
    let packages = app
        .session
        .plugin_state
        .panels
        .iter()
        .filter(|(id, _)| crate::plugins::enabled(app, id))
        .map(|(id, package)| (id.as_str(), package.name.as_str(), package.approval.as_str(), package.declarations.workspaces.as_slice()));
    out.extend(plugin_entries(packages, sheet));
    out
}

/// The plugin workspace shown now, while its plugin still offers it.
pub(crate) fn active_plugin(app: &SolveApp) -> Option<Entry> {
    let key = app.plugin_workspace.as_deref()?;
    registry(app).into_iter().find(|entry| entry.key == key && entry.plugin().is_some())
}

/// The key of the workspace shown now.
pub(crate) fn current(app: &SolveApp) -> String {
    if let Some(entry) = active_plugin(app) {
        entry.key
    } else if crate::drawing_workspace::active(app) {
        "drawing".into()
    } else if crate::render_workspace::active(app) {
        "render".into()
    } else {
        "design".into()
    }
}

/// The ribbon tab behind the fixed id `tab` while a plugin workspace is shown.
pub(crate) fn plugin_tab(app: &SolveApp, tab: &str) -> Option<RibbonTab> {
    let index = PLUGIN_TABS.iter().position(|id| *id == tab)?;
    let entry = active_plugin(app)?;
    entry.plugin()?.1.ribbon.get(index).cloned()
}

/// The ribbon tab ids of the plugin workspace shown now; none outside one.
pub(crate) fn plugin_tabs(app: &SolveApp) -> Option<Vec<&'static str>> {
    let entry = active_plugin(app)?;
    let count = entry.plugin()?.1.ribbon.len().min(PLUGIN_TABS.len());
    Some(PLUGIN_TABS[..count].to_vec())
}

/// Run a plugin workspace's `on_enter` or `on_exit` entry, when it declares one.
fn run_hook(app: &mut SolveApp, entry: &Entry, enter: bool) {
    let Source::Plugin { plugin, approval, spec, .. } = &entry.source else { return };
    let Some(hook) = (if enter { spec.on_enter.as_ref() } else { spec.on_exit.as_ref() }) else { return };
    let params = serde_json::json!({"approval": approval, "params": {}});
    if let Err(reason) = crate::plugin_menu::run_panel(app, format!("{plugin}.{hook}"), params) {
        app.set_status(reason, true);
    }
}

/// Leave the plugin workspace shown now, if any: its `on_exit` runs and the built-in ribbon
/// returns.
pub(crate) fn leave_plugin(app: &mut SolveApp) {
    if let Some(entry) = active_plugin(app) {
        run_hook(app, &entry, false);
    }
    if app.plugin_workspace.take().is_some() {
        app.ui.tab = "MODEL".into();
    }
}

/// Enter the plugin workspace `key`. It sits on the Design workspace: Render and Drawing are left
/// first, as entering Design does.
pub(crate) fn enter_plugin(app: &mut SolveApp, key: &str) -> Result<(), String> {
    let entry = registry(app).into_iter().find(|entry| entry.key == key && entry.plugin().is_some()).ok_or_else(|| {
        if split_key(key).is_some() {
            format!("no workspace `{key}`: its plugin is off or doesn't declare it")
        } else {
            format!("unknown workspace `{key}`")
        }
    })?;
    if app.plugin_workspace.as_deref() == Some(key) {
        return Ok(());
    }
    leave_plugin(app);
    crate::workspace::enter(app, "design")?;
    app.plugin_workspace = Some(entry.key.clone());
    app.ui.tab = PLUGIN_TABS[0].into();
    run_hook(app, &entry, true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_engine::plugin::ui::RibbonGroup;

    fn spec(id: &str, applies_to: Vec<DesignKind>) -> WorkspaceSpec {
        WorkspaceSpec {
            id: id.into(),
            name: format!("{id} name"),
            icon: "ws_design".into(),
            applies_to,
            ribbon: vec![RibbonTab { name: "Study".into(), groups: vec![RibbonGroup { name: "Run".into(), entries: vec!["solve".into()] }] }],
            panels: Vec::new(),
            builtin_panels: Vec::new(),
            on_enter: Some("enter".into()),
            on_exit: None,
        }
    }

    #[test]
    fn keys_round_trip_and_refuse_half_keys() {
        assert_eq!(plugin_key("fieldcraft", "airflow"), "plugin:fieldcraft/airflow");
        assert_eq!(split_key("plugin:fieldcraft/airflow"), Some(("fieldcraft", "airflow")));
        for bad in ["design", "plugin:", "plugin:fieldcraft", "plugin:/airflow", "plugin:fieldcraft/"] {
            assert_eq!(split_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn builtins_come_first_in_the_switchers_order() {
        let keys: Vec<_> = builtins().into_iter().map(|e| e.key).collect();
        assert_eq!(keys, ["design", "drawing", "render"]);
    }

    #[test]
    fn plugin_workspaces_follow_the_design_kind_and_cap_at_two() {
        let specs = [spec("airflow", vec![DesignKind::Part]), spec("sheets", vec![DesignKind::Drawing]), spec("any", Vec::new())];
        let listed =
            |sheet| -> Vec<String> { plugin_entries([("fieldcraft", "FieldCraft", "hash", &specs[..])], sheet).into_iter().map(|e| e.key).collect() };
        // Only the first two declarations count (the manifest allows two).
        assert_eq!(listed(false), ["plugin:fieldcraft/airflow"]);
        assert_eq!(listed(true), ["plugin:fieldcraft/sheets"]);
        let open = [spec("any", Vec::new())];
        let entries = plugin_entries([("fieldcraft", "FieldCraft", "hash", &open[..])], true);
        assert_eq!(entries.len(), 1, "an empty applies_to applies everywhere");
        let (plugin, ws) = entries[0].plugin().unwrap();
        assert_eq!((plugin, ws.id.as_str(), entries[0].name.as_str()), ("fieldcraft", "any", "any name"));
    }

    /// A plugin package that declares one workspace, installed as a loaded plugin's would be.
    fn install(app: &mut SolveApp, spec: WorkspaceSpec) {
        use solidcraft_engine::plugin::{PanelPackage, ui::Declarations};
        app.session.plugin_state.panels.insert(
            "fieldcraft".into(),
            PanelPackage {
                name: "FieldCraft".into(),
                approval: "hash".into(),
                declarations: Declarations { ui_version: 1, panels: Vec::new(), workspaces: vec![spec] },
                data: Default::default(),
                visible: Default::default(),
                visibility_revision: Default::default(),
            },
        );
    }

    /// Entering a plugin workspace through `ui.workspace` shows its tabs under its own names and
    /// the switcher's menu lists it as current; entering Design leaves it and Model's tabs return.
    #[test]
    fn a_plugin_workspace_enters_shows_its_tabs_and_leaves() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.frames(2);
        let mut airflow = spec("airflow", Vec::new());
        airflow.on_enter = None;
        install(&mut h.app, airflow);
        let key = "plugin:fieldcraft/airflow";
        let keys: Vec<_> = registry(&h.app).into_iter().map(|e| e.key).collect();
        assert_eq!(keys, ["design", "drawing", "render", key]);
        let reply = h.call("ui.workspace", serde_json::json!({"workspace": key}));
        assert_eq!(reply["result"]["workspace"], key, "{reply}");
        h.frames(3);
        assert_eq!(current(&h.app), key);
        assert_eq!(crate::workspace::visible_tabs(&h.app), ["PLUGIN 1"]);
        assert_eq!(h.app.ui.tab, "PLUGIN 1");
        assert_eq!(crate::toolbar::ribbon_label(&h.app, "PLUGIN 1"), "Study");
        assert!(crate::workspace::visible_tabs(&h.app).iter().all(|tab| *tab != "MODEL"), "the built-in tabs give way");
        let menu = crate::context_menu::items(&h.app, &crate::context_menu::Target::Workspace);
        let listed = menu.iter().find(|item| item.params.as_ref().is_some_and(|p| p["workspace"] == key)).expect("in the switcher's menu");
        assert_eq!(listed.label, "airflow name");
        assert_eq!(listed.tip.as_deref(), Some("FieldCraft"), "named for its plugin");
        let reply = h.call("ui.workspace", serde_json::json!({"workspace": "design"}));
        assert_eq!(reply["result"]["workspace"], "design", "{reply}");
        h.frames(3);
        assert!(h.app.plugin_workspace.is_none());
        assert_eq!(h.app.ui.tab, "MODEL");
        assert!(crate::workspace::visible_tabs(&h.app).contains(&"MODEL"));
        // The plugin turned off: its workspace is gone and the key is refused.
        h.app.session.plugin_state.panels.clear();
        let reply = h.call("ui.workspace", serde_json::json!({"workspace": key}));
        assert!(reply.get("error").is_some(), "{reply}");
    }

    #[test]
    fn a_harness_lists_only_the_builtins_and_refuses_unknown_keys() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.frames(2);
        let keys: Vec<_> = registry(&h.app).into_iter().map(|e| e.key).collect();
        assert_eq!(keys, ["design", "drawing", "render"]);
        assert_eq!(current(&h.app), "design");
        assert!(enter_plugin(&mut h.app, "plugin:nobody/none").is_err());
        assert!(h.app.plugin_workspace.is_none());
        assert!(plugin_tabs(&h.app).is_none());
    }
}
