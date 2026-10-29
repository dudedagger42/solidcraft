//! SolidCraft: a picture the app already renders off screen (its 3D view) as the live backdrop,
//! copied into a mip pyramid each frame, so glass over the view refracts the model itself. Unlike
//! [`crate::LiveBackdrop`] nothing is laid out or drawn twice: one blit and the mip chain.

use std::sync::Arc;

use egui::{Rect, Shape};
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};

use crate::mipgen::{MipGen, Pyramid, FORMAT};
use crate::renderer::GlassResources;

/// Finds the app's picture among the renderer's resources: its view, and whether its format is
/// sRGB. `None` when there is no picture this frame (glass then shows the static backdrop).
pub type SceneSource = Arc<dyn Fn(&CallbackResources) -> Option<(wgpu::TextureView, bool)> + Send + Sync>;

const BLIT: &str = r"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: VOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}
@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(src, samp, in.uv).rgb, 1.0);
}
";

impl SceneResources {
    /// Records the picture's copy into the pyramid and its mip chain.
    fn record(&self, device: &wgpu::Device, view: &wgpu::TextureView, srgb: bool, encoder: &mut wgpu::CommandEncoder) {
        let Some((pyramid, plain)) = &self.target else { return };
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui_glass_scene"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_glass_scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if srgb { &pyramid.mip_views[0] } else { plain },
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(if srgb { &self.srgb } else { &self.plain });
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.mipgen.generate(encoder, pyramid);
    }
}

pub(crate) struct SceneResources {
    mipgen: Arc<MipGen>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Into the pyramid's sRGB view (an sRGB source) and its plain view (gamma bytes as they are).
    srgb: wgpu::RenderPipeline,
    plain: wgpu::RenderPipeline,
    target: Option<(Pyramid, wgpu::TextureView)>,
    frame: u64,
}

/// Sets up the copy. Call once, after [`crate::init`].
///
/// # Panics
///
/// Panics if [`crate::init`] has not been called on this renderer.
pub fn init_scene_backdrop(render_state: &RenderState) {
    let device = &render_state.device;
    let mut renderer = render_state.renderer.write();
    let mipgen = renderer.callback_resources.get::<GlassResources>().map(|g| g.mipgen.clone()).expect("egui_glass::init must be called first");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("egui_glass_scene"), source: wgpu::ShaderSource::Wgsl(BLIT.into()) });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("egui_glass_scene"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("egui_glass_scene"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipeline = |format| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui_glass_scene"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("egui_glass_scene"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let resources = SceneResources {
        mipgen,
        srgb: pipeline(FORMAT),
        plain: pipeline(wgpu::TextureFormat::Rgba8Unorm),
        layout,
        sampler,
        target: None,
        frame: 0,
    };
    renderer.callback_resources.insert(resources);
}

/// A shape that, placed before every glass shape of the frame, makes `source`'s picture the
/// backdrop of that glass, at `size` pixels (smaller than the picture is fine: it is blurred).
/// Map the backdrop to where the picture shows with [`crate::map_backdrop`].
pub fn scene_backdrop_shape(rect: Rect, size: [u32; 2], source: SceneSource) -> Shape {
    Shape::Callback(egui_wgpu::Callback::new_paint_callback(rect, SceneCallback { rect, size, source }))
}

struct SceneCallback {
    rect: Rect,
    size: [u32; 2],
    source: SceneSource,
}

impl CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some((view, srgb)) = (self.source)(resources) else { return Vec::new() };
        let Some(res) = resources.get_mut::<SceneResources>() else { return Vec::new() };
        let [w, h] = [self.size[0].max(1), self.size[1].max(1)];
        if res.target.as_ref().is_none_or(|(p, _)| p.size != [w, h]) {
            let pyramid = res.mipgen.create(device, w, h, wgpu::TextureUsages::empty());
            let plain = pyramid.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8Unorm),
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            });
            res.target = Some((pyramid, plain));
        }
        res.record(device, &view, srgb, encoder);
        let (pyramid, _) = res.target.as_ref().expect("created above");
        let (sample_view, mips) = (pyramid.sample_view.clone(), pyramid.mip_levels());
        // Glass blurs in screen pixels; the pyramid has fewer.
        let scale = w as f32 / (self.rect.width() * screen.pixels_per_point).max(1.0);
        res.frame += 1;
        // Live frames count down from the top so they never match `LiveBackdrop`'s.
        let frame = u64::MAX / 2 - res.frame;
        if let Some(glass) = resources.get_mut::<GlassResources>() {
            glass.set_live_backdrop(sample_view, mips, frame, scale);
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, _pass: &mut wgpu::RenderPass<'static>, _resources: &CallbackResources) {}
}
