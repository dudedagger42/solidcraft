//! Add-ons prototype, phase 1: the smallest workbench plugin. It declares one workspace,
//! "Airflow study", with a Study ribbon tab (Run: Solve; Results: Clear) and a Jobs panel that the
//! workspace shows while it is entered. Solve adds a made-up job row; no solver runs yet (the
//! FieldCraft sidecar comes with `engine@0.1`, phase 2).

wit_bindgen::generate!({ path: "wit", world: "panel-plugin" });

use craft::plugin::types::{Datum, FieldFailure, PluginError, Value as Cell};
use craft::plugin::ui::{
    self, ButtonSpec, Column, Declarations, DesignKind, DockZone, Layout, Node, PanelSpec, RibbonGroup, RibbonTab, SelectionMode, TableSpec, UiEvent,
    Widget, WorkspaceSpec,
};
use serde_json::json;
use std::cell::RefCell;

thread_local! {
    /// The jobs of this run of the plugin: (name, residual).
    static JOBS: RefCell<Vec<(String, f64)>> = const { RefCell::new(Vec::new()) };
}

struct Plugin;

impl exports::craft::plugin::entry::Guest for Plugin {
    fn execute(entry: String, _params: String) -> Result<String, PluginError> {
        match entry.as_str() {
            // The workspace's on_enter entry: show what there is.
            "enter" => {}
            "solve" => JOBS.with_borrow_mut(|jobs| {
                let n = jobs.len() + 1;
                jobs.push((format!("Study {n}"), 1e-3 / n as f64));
            }),
            "clear" => JOBS.with_borrow_mut(Vec::clear),
            _ => return Err(invalid("entry", format!("no entry `{entry}`"))),
        }
        show()?;
        Ok(json!({"jobs": JOBS.with_borrow(Vec::len)}).to_string())
    }
}

/// Replace the panel's rows: one cell per path, "/jobs/<row>/<column>".
fn show() -> Result<(), PluginError> {
    let cells: Vec<Datum> = JOBS.with_borrow(|jobs| {
        jobs.iter()
            .enumerate()
            .flat_map(|(row, (name, residual))| {
                [
                    Datum { path: format!("/jobs/{row}/id"), value: Cell::Text(format!("job{row}")) },
                    Datum { path: format!("/jobs/{row}/name"), value: Cell::Text(name.clone()) },
                    Datum { path: format!("/jobs/{row}/residual"), value: Cell::Number(*residual) },
                ]
            })
            .collect()
    });
    ui::clear_data("jobs")?;
    ui::set_data("jobs", &cells)
}

impl exports::craft::plugin::ui_provider::Guest for Plugin {
    /// The same declarations as `runtime.ui` in plugin.json.
    fn describe() -> Declarations {
        let column = |key: &str, label: &str| Column { key: key.into(), label: label.into(), unit: None };
        let table = TableSpec {
            bind: "/jobs".into(),
            row_id: "id".into(),
            columns: vec![column("name", "Study"), column("residual", "Residual")],
            select: SelectionMode::Single,
            on_select: None,
            on_edit: None,
        };
        let nodes = vec![
            Node { widget: Widget::Group(vec![1, 2]) },
            Node { widget: Widget::Toolbar(vec![ButtonSpec { label: "Solve".into(), icon: None, entry: "solve".into() }]) },
            Node { widget: Widget::Table(table) },
        ];
        let panel =
            PanelSpec { id: "jobs".into(), title: "Jobs".into(), zones: vec![DockZone::Right, DockZone::Bottom], layout: Layout { root: 0, nodes } };
        let group = |name: &str, entries: &[&str]| RibbonGroup { name: name.into(), entries: entries.iter().map(|e| e.to_string()).collect() };
        let workspace = WorkspaceSpec {
            id: "airflow".into(),
            name: "Airflow study".into(),
            icon: "ws_design".into(),
            applies_to: vec![DesignKind::Part, DesignKind::Assembly],
            ribbon: vec![RibbonTab { name: "Study".into(), groups: vec![group("Run", &["solve"]), group("Results", &["clear"])] }],
            panels: vec![(DockZone::Right, vec!["jobs".into()])],
            builtin_panels: vec!["browser".into()],
            on_enter: Some("enter".into()),
            on_exit: None,
        };
        Declarations { ui_version: 1, panels: vec![panel], workspaces: vec![workspace] }
    }

    fn on_event(_event: UiEvent) -> Result<(), PluginError> {
        Ok(())
    }

    fn on_enter(_workspace: String) -> Result<(), PluginError> {
        Ok(())
    }

    fn on_exit(_workspace: String) -> Result<(), PluginError> {
        Ok(())
    }
}

fn invalid(field: &str, reason: impl ToString) -> PluginError {
    PluginError::InvalidInput(FieldFailure { field: field.into(), reason: reason.to_string() })
}

export!(Plugin);
