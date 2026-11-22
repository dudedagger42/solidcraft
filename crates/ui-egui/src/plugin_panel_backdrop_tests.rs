use egui_wgpu::wgpu;
use solidcraft_engine::plugin::shader::{self, Resource, ShaderKind, ShaderParam};

fn examples() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-examples")
}

/// Every resource the compiled shader binds is in the hook's layout at the same binding, of the
/// same kind and size, and the layout asks for nothing the shader lacks.
pub(super) fn binds_to_the_hook(compiled: &shader::Compiled, textures: usize) {
    let layout = crate::blur::panel_hook_layout(textures);
    assert_eq!(compiled.bindings.len(), layout.len(), "{:?}", compiled.bindings);
    for (binding, resource) in &compiled.bindings {
        let entry = layout.iter().find(|e| e.binding == *binding).unwrap_or_else(|| panic!("binding {binding} is not in the hook's layout"));
        let matches = match (resource, entry.ty) {
            (Resource::Uniform(size), wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, min_binding_size, .. }) => {
                min_binding_size.map(u64::from) == Some(*size)
            }
            (Resource::Sampler, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)) => true,
            (
                Resource::Texture,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            ) => true,
            _ => false,
        };
        assert!(matches, "binding {binding}: the shader has {resource:?}, the hook lays out {:?}", entry.ty);
    }
}

/// Loads a plugin folder's panel backdrop as the app does and checks it against the hook.
pub(super) fn loads_and_binds(folder: &std::path::Path, key: &str) -> super::Loaded {
    let loaded = super::load(folder.to_string_lossy().into_owned(), key.into(), 4096).unwrap_or_else(|e| panic!("{key}: {e}"));
    let manifest = crate::plugin_shaders::manifest(&folder.to_string_lossy()).unwrap();
    let id = key.split_once('/').unwrap().1;
    let declared = manifest.shaders.iter().find(|s| s.id == id && s.kind == ShaderKind::PanelBackdrop).unwrap();
    let source = std::fs::read_to_string(folder.join(&declared.source)).unwrap();
    binds_to_the_hook(&shader::compile(declared, &source).unwrap(), declared.textures.len());
    loaded
}

#[test]
fn the_hooks_uniforms_are_the_sizes_the_prelude_declares() {
    assert_eq!(crate::blur::PANEL_HOOK_GLOBAL_BYTES, shader::PANEL_HOST_UNIFORM_BYTES);
    assert_eq!(crate::blur::PANEL_HOOK_PANEL_BYTES, shader::PANEL_RECT_UNIFORM_BYTES);
    assert_eq!(crate::blur::PANEL_HOOK_FIRST_TEXTURE, shader::PANEL_FIRST_TEXTURE_BINDING);
    let tokens = solidcraft_theme::Tokens::get();
    let mut params = [0.0; shader::MAX_PARAMS];
    params[5] = 0.25;
    let bytes = super::host_uniforms([800.0, 400.0], 1.5, 2.0, 14.0, true, &params, &tokens);
    assert_eq!(bytes.len(), shader::PANEL_HOST_UNIFORM_BYTES);
    let f: Vec<f32> = bytes.chunks(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    assert_eq!(f[..4], [800.0, 400.0, 1.5, 2.0]);
    assert_eq!((f[13], f[14]), (14.0, 1.0), "blur and the target's sRGB flag");
    assert_eq!(f[16 + 5], 0.25, "param 5 is params[1].y");
}

#[test]
fn the_frost_example_loads_and_binds_to_the_hook() {
    let loaded = loads_and_binds(&examples().join("frost"), "frost/frost");
    assert!(loaded.wgsl.contains("fn panel") && !loaded.animated);
    assert_eq!((loaded.blur, loaded.params.len()), (16.0, 1));
}

#[test]
fn card_settings_reach_the_shader_in_range_and_default_when_unset() {
    let param = |id: &str, default: f32| ShaderParam { id: id.into(), name: id.into(), min: 0.0, max: 10.0, default };
    let params = [param("a", 1.0), param("b", 2.0), param("c", 3.0)];
    let stored: std::collections::BTreeMap<String, f32> =
        [("p/s/a".to_string(), 4.0), ("p/s/c".to_string(), 99.0), ("other/s/b".to_string(), 7.0)].into_iter().collect();
    let values = super::param_values("p/s", &params, &stored);
    assert_eq!(values[..4], [4.0, 2.0, 10.0, 0.0]);
}

#[test]
fn no_panel_backdrop_chosen_leaves_the_blur_without_a_hook() {
    let mut h = crate::scenario::Harness::new();
    let ctx = egui::Context::default();
    super::prepare(&mut h.app, &ctx);
    assert!(!h.app.blur.panel_hook_active());
    assert!(h.app.plugin_panel_backdrop.loading.is_none() && h.app.plugin_panel_backdrop.active.is_none());
}

#[test]
fn a_panel_backdrop_stops_when_its_plugin_is_turned_off_and_comes_back_when_on() {
    let mut h = crate::scenario::Harness::new();
    h.app.preferences.panel_backdrop = "frost/frost".into();
    h.app.plugin_panel_backdrop.loading = Some(("frost/frost".into(), super::Pending::default()));
    assert!(super::allowed(&mut h.app, "frost/frost"));
    h.app.preferences.disabled_plugins.insert("frost".into());
    assert!(!super::allowed(&mut h.app, "frost/frost"), "a turned-off plugin's backdrop must not draw");
    assert!(h.app.plugin_panel_backdrop.loading.is_none(), "its pending load is dropped");
    assert_eq!(h.app.plugin_panel_backdrop.failed.as_deref(), Some("frost/frost"));
    h.app.preferences.disabled_plugins.remove("frost");
    assert!(super::allowed(&mut h.app, "frost/frost"));
    assert_eq!(h.app.plugin_panel_backdrop.failed, None, "tried again");
}

#[test]
fn removing_a_plugin_forgets_its_panel_backdrop() {
    let mut h = crate::scenario::Harness::new();
    h.app.plugin_panel_backdrop.loading = Some(("frost/frost".into(), super::Pending::default()));
    super::forget(&mut h.app, "other");
    assert!(h.app.plugin_panel_backdrop.loading.is_some());
    super::forget(&mut h.app, "frost");
    assert!(h.app.plugin_panel_backdrop.loading.is_none());
}
