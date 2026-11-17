//! Installing a plugin from a `.zip`: checked in memory, then unpacked into a hidden folder beside
//! the installed plugins and renamed into `plugins/<id>/`. A plugin already installed under that
//! id is replaced only when the caller says so.

use super::{MAX_COMPONENT_BYTES, Manifest, parse_manifest};
use craft_plugin::HostError;
use solidcraft_io::safe_zip;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// The largest `.zip` read, and the most it may unpack to.
pub const MAX_ZIP_BYTES: usize = 64 << 20;
pub const LIMITS: safe_zip::Limits =
    safe_zip::Limits { max_entries: 256, max_entry_bytes: MAX_COMPONENT_BYTES as u64, max_total_bytes: MAX_ZIP_BYTES as u64 };

/// Why a `.zip` was not installed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error(transparent)]
    Archive(#[from] safe_zip::Error),
    #[error("the .zip has no plugin.json")]
    MissingManifest,
    #[error("the .zip must hold one plugin, at its top or in one folder")]
    NotOnePlugin,
    #[error("the .zip has no {0}, which plugin.json names")]
    MissingModule(String),
    /// The module is missing and the .zip holds Rust sources: most likely the "Source code"
    /// archive that GitHub lists beside a release, not the plugin's own .zip (#16736).
    #[error(
        "the .zip has no {0}, which plugin.json names: it holds the plugin's source code. Install the plugin's own .zip from its release instead"
    )]
    SourceOnly(String),
    #[error("{0}")]
    Manifest(HostError),
    #[error("“{name}” {version} is already installed; confirm to replace it")]
    AlreadyInstalled { id: String, name: String, version: String },
    #[error("installing plugins needs the desktop app")]
    NoPluginsFolder,
    #[error("could not install the plugin: {0}")]
    Io(String),
}

/// A checked plugin, still in memory.
#[derive(Debug)]
pub struct Unpacked {
    pub manifest: Manifest,
    /// Hash of the exact manifest and component that will be published.
    pub approval: String,
    /// SHA-256 of the whole `.zip` as read, so an install can insist on exactly the archive that was
    /// reviewed: `approval` covers only the manifest and component, not the plugin's other files.
    pub archive: String,
    /// Paths relative to the plugin's folder.
    pub files: Vec<safe_zip::File>,
}

/// Files a desktop zip tool adds that are not part of the plugin.
fn is_junk(path: &str) -> bool {
    path.starts_with("__MACOSX/") || path.rsplit('/').next().is_some_and(|name| name == ".DS_Store" || name == "Thumbs.db")
}

/// Checks `zip`: one plugin (`plugin.json` and its module) at the top or in one top-level folder,
/// a manifest the host accepts, safe paths and sizes.
pub fn unpack(zip: &[u8]) -> Result<Unpacked, InstallError> {
    let files: Vec<_> = safe_zip::read(zip, LIMITS)?.into_iter().filter(|f| !is_junk(&f.path)).collect();
    let manifests: Vec<&str> =
        files.iter().filter(|f| f.path == "plugin.json" || f.path.ends_with("/plugin.json")).map(|f| f.path.as_str()).collect();
    let prefix = match manifests.as_slice() {
        [] => return Err(InstallError::MissingManifest),
        ["plugin.json"] => "",
        [one] => match one.strip_suffix("plugin.json") {
            Some(folder) if folder.matches('/').count() == 1 => folder,
            _ => return Err(InstallError::NotOnePlugin),
        },
        _ => return Err(InstallError::NotOnePlugin),
    }
    .to_string();
    // Everything belongs to that one plugin.
    if files.iter().any(|f| !f.path.starts_with(&prefix)) {
        return Err(InstallError::NotOnePlugin);
    }
    let files: Vec<safe_zip::File> = files.into_iter().map(|f| safe_zip::File { path: f.path[prefix.len()..].to_string(), bytes: f.bytes }).collect();
    let text = files.iter().find(|f| f.path == "plugin.json").map(|f| f.bytes.as_slice()).ok_or(InstallError::MissingManifest)?;
    // The folder it installs to is its id: the parser checks the id itself.
    let id = serde_json::from_slice::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("id").and_then(serde_json::Value::as_str).map(str::to_string))
        .unwrap_or_default();
    let manifest = parse_manifest(text, &id).map_err(|e| match e {
        crate::EngineError::Plugin(e) => InstallError::Manifest(e),
        e => InstallError::Manifest(HostError::InvalidManifest { field: "manifest".into(), reason: e.to_string() }),
    })?;
    if let Some(runtime) = &manifest.runtime
        && !files.iter().any(|f| f.path == runtime.module)
    {
        let source = files.iter().any(|f| f.path == "Cargo.toml" || f.path.ends_with(".rs"));
        let module = runtime.module.clone();
        return Err(if source { InstallError::SourceOnly(module) } else { InstallError::MissingModule(module) });
    }
    let component = if let Some(runtime) = &manifest.runtime {
        files
            .iter()
            .find(|f| f.path == runtime.module)
            .map(|f| f.bytes.as_slice())
            .ok_or_else(|| InstallError::MissingModule(runtime.module.clone()))?
    } else {
        &[]
    };
    let approval = super::package_hash(text, component);
    let archive = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(zip));
    Ok(Unpacked { manifest, approval, archive, files })
}

/// The installed plugin of this id: its name and version, if its folder holds a readable manifest.
#[cfg(not(target_arch = "wasm32"))]
pub fn installed(plugins: &Path, id: &str) -> Option<(String, String)> {
    let folder = plugins.join(id);
    if !folder.exists() {
        return None;
    }
    let text = std::fs::read(folder.join("plugin.json")).ok().filter(|t| t.len() <= super::MAX_MANIFEST_BYTES);
    Some(match text.and_then(|t| parse_manifest(&t, id).ok()) {
        Some(m) => (m.name, m.version),
        None => (id.to_string(), String::new()),
    })
}

/// Writes `plugin` to `plugins/<id>/`. An installed plugin of that id is replaced only when
/// `replace`; the old folder is kept until the new one is in place.
#[cfg(not(target_arch = "wasm32"))]
pub fn install(plugin: &Unpacked, plugins: &Path, replace: bool) -> Result<PathBuf, InstallError> {
    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    let id = &plugin.manifest.id;
    let target = plugins.join(id);
    if !replace && let Some((name, version)) = installed(plugins, id) {
        return Err(InstallError::AlreadyInstalled { id: id.clone(), name, version });
    }
    // Preflight logical destinations before the staging folder writes or a replacement deletes.
    // Checking only the plugin folder would bypass a scoped caller's hidden-file restrictions.
    for path in std::iter::once(target.clone()).chain(plugin.files.iter().map(|file| target.join(&file.path))) {
        solidcraft_io::vfs::check_new(&path.to_string_lossy()).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    if target.exists() {
        super::package::check_remove_tree(&target).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    std::fs::create_dir_all(plugins).map_err(io)?;
    // A hidden name: the plugins list skips it while it is written.
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let staging = plugins.join(format!(".install-{id}-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&staging).map_err(io)?;
    let written = (|| -> std::io::Result<()> {
        for file in &plugin.files {
            let path = staging.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::OpenOptions::new().write(true).create_new(true).open(&path).and_then(|mut f| std::io::Write::write_all(&mut f, &file.bytes))?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io(e));
    }
    let old = target.exists().then(|| plugins.join(format!(".replaced-{id}-{}-{stamp}", std::process::id())));
    if let Some(old) = &old
        && let Err(e) = std::fs::rename(&target, old)
    {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io(e));
    }
    if let Err(e) = std::fs::rename(&staging, &target) {
        if let Some(old) = &old {
            let _ = std::fs::rename(old, &target);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io(e));
    }
    if let Some(old) = old {
        let _ = std::fs::remove_dir_all(old);
    }
    Ok(target)
}

/// What [`install_bundled`] did.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Bundled {
    /// Plugins installed or brought up to the bundled version.
    pub added: Vec<String>,
    /// Zips left alone, by file name, and why.
    pub skipped: Vec<(String, String)>,
}

/// Installs each `.zip` in `bundle` (the `plugins` folder shipped beside the app) once, and later
/// brings it up to a newer bundled version if the user has not changed or removed it. A plugin
/// with code is left for the user to review and approve.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_bundled(bundle: &Path, plugins: &Path) -> Bundled {
    use std::collections::BTreeMap;
    let mut out = Bundled::default();
    let Ok(entries) = std::fs::read_dir(bundle) else {
        return out;
    };
    let mut zips: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip"))).collect();
    zips.sort();
    // `.bundled` lists `id version` for each bundled plugin as last installed, or `id -` for one the user had.
    let record = plugins.join(".bundled");
    let read_record = || -> BTreeMap<String, String> {
        std::fs::read_to_string(&record)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (id, version) = l.trim().split_once(' ').unwrap_or((l.trim(), ""));
                (!id.is_empty()).then(|| (id.to_string(), version.to_string()))
            })
            .collect()
    };
    let mut seen = read_record();
    let mut changed = BTreeMap::new();
    for zip in zips {
        let name = zip.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let plugin = match std::fs::read(&zip) {
            Ok(bytes) if bytes.len() > MAX_ZIP_BYTES => Err("the .zip is too large".to_string()),
            Ok(bytes) => unpack(&bytes).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        let plugin = match plugin {
            Ok(plugin) if plugin.manifest.runtime.is_some() => {
                out.skipped.push((name, "it runs code, so it is installed only from Install plugin…".into()));
                continue;
            }
            Ok(plugin) => plugin,
            Err(e) => {
                out.skipped.push((name, e));
                continue;
            }
        };
        let (id, version) = (plugin.manifest.id.clone(), plugin.manifest.version.clone());
        let replace = match (seen.get(&id).cloned(), installed(plugins, &id)) {
            (None, None) => false,
            // The user's own copy: `-` never matches a version, so it is never replaced.
            (None, Some(_)) => {
                changed.insert(id.clone(), "-".to_string());
                seen.insert(id, "-".to_string());
                continue;
            }
            (Some(was), Some((_, have))) if was == have && newer(&version, &was) => true,
            _ => continue,
        };
        match install(&plugin, plugins, replace) {
            Ok(_) => {
                out.added.push(id.clone());
                changed.insert(id.clone(), version.clone());
                seen.insert(id, version);
            }
            Err(InstallError::AlreadyInstalled { version: have, .. }) => {
                changed.insert(id.clone(), have.clone());
                seen.insert(id, have);
            }
            Err(e) => out.skipped.push((name, e.to_string())),
        }
    }
    if !changed.is_empty() {
        // Another instance may have written the record since it was read: keep its entries too.
        let mut merged = read_record();
        merged.extend(changed);
        let text: String = merged.iter().map(|(id, version)| format!("{id} {version}\n")).collect();
        let temp = plugins.join(format!(".bundled-{}", std::process::id()));
        if std::fs::write(&temp, text).and_then(|()| std::fs::rename(&temp, &record)).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }
    out
}

/// `a` is a higher dotted version than `b`; false when either has a part that is not a number.
#[cfg(not(target_arch = "wasm32"))]
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split(['.', '-', '+']).take(3).map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>();
    matches!((parts(a), parts(b)), (Some(a), Some(b)) if a > b)
}
