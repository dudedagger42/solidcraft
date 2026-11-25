# Add-ons prototype (fork only)

`airflow-demo` is the smallest workbench plugin for the add-ons prototype (proto/addons). It
declares one workspace, "Airflow study", for parts and assemblies:

- the workspace switcher lists it after Design, Drawing and Render;
- entering it shows its Study tab (Run: Solve; Results: Clear) in place of the built-in tabs and
  opens its Jobs panel, and runs its `enter` entry;
- entering Design again closes the panel and brings the built-in tabs back.

Solve only adds a made-up row: no solver runs until the `engine@0.1` sidecar (phase 2).

Build it as the plugin guide's examples are built (docs/book/src/dev/plugins.md): copy
`crates/craft-plugin/wit/plugin.wit` into `airflow-demo-src/wit/`, run
`cargo build --release --target wasm32-unknown-unknown` in `airflow-demo-src`, make the module a
component (`wasm-tools component new`, as `cargo xtask plugin-examples` does), and save it as
`airflow-demo/airflow-demo.wasm`. Then install the `airflow-demo` folder from Preferences →
Plugins. Not built yet: team3 is under a build hold.
