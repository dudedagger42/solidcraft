use super::*;
use serde_json::json;

/// Throwaway (stack-on-0.46.1 trial): install each package under tests/trial0461 into its own
/// empty plugins folder and print the verdict. It never fails; the log is the result.
#[test]
fn trial0461_install_verdicts() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial0461");
    let root = solidcraft_io::vfs::temp_dir().join(format!("trial0461-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut zips: Vec<_> = std::fs::read_dir(&fixtures)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "zip"))
        .collect();
    zips.sort();
    for source in zips {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let zip = root.join(&name);
        std::fs::copy(&source, &zip).unwrap();
        let mut s = Session::default();
        s.plugin_state.plugins_dir = Some(root.join(format!("installed-{name}")));
        match s.execute("plugin.install", &json!({"path": zip.to_string_lossy(), "replace": true})) {
            Ok(v) => println!("TRIAL0461 {name}: INSTALLED {v}"),
            Err(e) => println!("TRIAL0461 {name}: REFUSED {e:?}"),
        }
    }
}
