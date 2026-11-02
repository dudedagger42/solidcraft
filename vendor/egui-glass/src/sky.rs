//! SolidCraft: a picture behind the app's view, drawn window-sized into a texture of its own. The
//! view shows that texture and the scene backdrop copies it, so glass sees the same picture on
//! every panel. A panorama turns with the camera; a flat picture stays put; rain runs down a
//! frosted window in front of a flat picture.

use egui::TextureId;
use egui_wgpu::{wgpu, RenderState};

use crate::mipgen::{MipGen, Pyramid};
use crate::renderer::GlassResources;

/// What part of the picture the window shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SkyView {
    /// The part of a flat picture between two texture coordinates.
    Flat { min: [f32; 2], max: [f32; 2] },
    /// A wrap-around picture (equirectangular, Z up) seen along `forward`. `right` and `up` are
    /// as long as half the window's width and height are at a distance of one.
    Round { right: [f32; 3], up: [f32; 3], forward: [f32; 3] },
    /// A flat picture behind a frosted window with rain on it, `time` seconds in.
    Rain { min: [f32; 2], max: [f32; 2], time: f32 },
}

const SHADER: &str = r"
struct Params { right: vec4<f32>, up: vec4<f32>, forward: vec4<f32>, flat_uv: vec4<f32>, lod: f32, round: f32, pad: vec2<f32> }
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> p: Params;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) ndc: vec2<f32> }
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: VOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}
fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}
struct Wet { bend: vec2<f32>, drop: f32, trail: f32 }
// One sheet of drops running down the glass. `q` is the window with its height as one, y down.
fn running(q: vec2<f32>, t: f32, scale: f32, seed: f32) -> Wet {
    var st = q * scale * vec2<f32>(1.0, 0.5);
    let column = floor(st.x);
    st.y -= t * 0.05 * (0.6 + hash(vec2<f32>(column, seed))) + hash(vec2<f32>(column, seed + 3.1));
    let cell = floor(st);
    let f = fract(st) - 0.5;
    let r = hash(cell + seed);
    let r2 = hash(cell + seed + 7.7);
    var out: Wet;
    out.bend = vec2<f32>(0.0);
    out.drop = 0.0;
    out.trail = 0.0;
    if r2 < 0.25 {
        return out; // Not every cell has a drop.
    }
    // It clings, then lets go and runs: most of the way is in the end of its life.
    let life = fract(t * (0.07 + 0.09 * r) + r * 17.0);
    let run = life * life * life;
    let y = mix(-0.34, 0.34, run);
    let x = (r2 - 0.625) * 0.8 + sin(f.y * 11.0 + r * 40.0) * 0.035 * (1.0 - 0.5 * r);
    let shown = smoothstep(0.0, 0.04, life) * (1.0 - smoothstep(0.93, 1.0, life));
    let radius = 0.09 + 0.07 * r;
    // Round on the glass, as the cell is twice as tall as wide, and a little narrower above.
    var d = (f - vec2<f32>(x, y)) * vec2<f32>(1.0, 2.0);
    d.x *= 1.0 + clamp(-d.y / radius, 0.0, 1.0) * 0.45;
    let inside = (1.0 - smoothstep(radius * 0.72, radius, length(d))) * shown;
    // What it wiped on the way down, with beads left along it.
    let wide = radius * 0.62;
    let across = 1.0 - smoothstep(wide * 0.45, wide, abs(f.x - x));
    let behind = smoothstep(-0.42, -0.2, f.y) * (1.0 - smoothstep(y - 0.02, y + 0.02, f.y));
    let trail = across * behind * shown;
    let beads = fract(f.y * 13.0 + r * 9.0) - 0.5;
    let bead = vec2<f32>((f.x - x) * 13.0 * 0.5, beads);
    let beaded = (1.0 - smoothstep(0.1, 0.24, length(bead))) * trail * step(0.45, hash(vec2<f32>(floor(f.y * 13.0 + r * 9.0), r)));
    out.drop = max(inside, beaded * 0.8);
    out.bend = d / radius * inside + bead * 0.5 * beaded;
    out.trail = trail;
    return out;
}
// Drops that land and stay: each one comes at once and slowly dries.
fn landing(q: vec2<f32>, t: f32, scale: f32) -> Wet {
    let st = q * scale;
    let cell = floor(st);
    let f = fract(st) - 0.5;
    let r = hash(cell + 91.0);
    let r2 = hash(cell + 13.0);
    let life = fract(t * (0.05 + 0.12 * r2) + r * 31.0);
    let at = (vec2<f32>(r, r2) - 0.5) * 0.6;
    let radius = (0.07 + 0.21 * fract(r * 7.3)) * step(0.62, fract(r2 * 5.1));
    let d = f - at;
    let shown = smoothstep(0.0, 0.015, life) * (1.0 - life) * (1.0 - life);
    // It lands as a ring that closes fast.
    let ring = (1.0 - smoothstep(0.0, 0.06, abs(length(d) - life * 3.0 * radius - radius))) * (1.0 - smoothstep(0.0, 0.05, life));
    var out: Wet;
    out.drop = (1.0 - smoothstep(radius * 0.65, radius, length(d))) * shown;
    out.bend = d / max(radius, 0.001) * out.drop;
    out.trail = ring * 0.5;
    return out;
}
fn frosted(uv: vec2<f32>, lod: f32, spread: f32, grain: f32) -> vec3<f32> {
    // A few taps turned by a per-pixel angle: the fine grain of frosted glass, and no blocks from
    // the wide levels of the picture.
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 7; i++) {
        let a = f32(i) * 2.399963 + grain * 6.2831853;
        let at = sqrt((f32(i) + 0.5) / 7.0) * spread;
        sum += textureSampleLevel(src, samp, uv + vec2<f32>(cos(a), sin(a) * p.pad.y) * at, lod).rgb;
    }
    return sum / 7.0;
}
fn rain(uv: vec2<f32>, window: vec2<f32>) -> vec3<f32> {
    let t = p.pad.x;
    let q = window * vec2<f32>(p.pad.y, 1.0);
    let a = running(q, t, 7.0, 1.0);
    let b = running(q + vec2<f32>(0.37, 0.11), t * 0.8, 12.0, 5.0);
    let c = landing(q, t, 26.0);
    let e = landing(q + 0.29, t * 0.7, 48.0);
    let drop = clamp(max(max(a.drop, b.drop), max(c.drop, e.drop)), 0.0, 1.0);
    let trail = clamp(max(a.trail, b.trail) + c.trail * 0.4, 0.0, 1.0);
    let bend = a.bend + b.bend * 0.8 + c.bend * 0.45 + e.bend * 0.3;
    // Each drop is a lens: it shows the city upside down and small, and nearly sharp.
    let see = uv - bend * vec2<f32>(0.12 / p.pad.y, 0.12);
    let clear = max(trail * 0.72, drop);
    let lod = mix(5.6, 1.1, clear);
    let spread = mix(0.024, 0.0012, clear);
    let grain = hash(floor(window * vec2<f32>(p.right.w, p.up.w)));
    var c3 = frosted(see, lod, spread, grain);
    // The fog on the glass, gone where the water has wiped it.
    let fog = vec3<f32>(0.2, 0.22, 0.28) + c3 * 0.3;
    c3 = mix(c3 * 0.85 + fog * 0.62, c3 * 1.04 + vec3<f32>(0.05, 0.055, 0.07), clear);
    // Dark at the drop's rim, a light high in it.
    let len = length(bend);
    c3 *= 1.0 - smoothstep(0.6, 1.0, len) * 0.14 * drop;
    let shine = 1.0 - smoothstep(0.0, 0.35, length(bend + vec2<f32>(0.3, 0.42)));
    c3 += vec3<f32>(0.34, 0.36, 0.42) * shine * drop;
    return c3;
}
@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let window = vec2<f32>(in.ndc.x, -in.ndc.y) * 0.5 + 0.5;
    var uv = p.flat_uv.xy + window * (p.flat_uv.zw - p.flat_uv.xy);
    if p.round > 1.5 {
        return vec4<f32>(rain(uv, window), 1.0);
    }
    if p.round > 0.5 {
        let ray = normalize(p.forward.xyz + p.right.xyz * in.ndc.x + p.up.xyz * in.ndc.y);
        uv = vec2<f32>(atan2(ray.y, ray.x) / 6.2831853 + 0.5, 0.5 - asin(clamp(ray.z, -1.0, 1.0)) / 3.1415927);
    }
    // A level set by hand: the wrap of `u` would otherwise pick a tiny one along the seam.
    return vec4<f32>(textureSampleLevel(src, samp, uv, p.lod).rgb, 1.0);
}
";

struct Target {
    size: [u32; 2],
    /// Gamma bytes as they are, for the pass and for the glass's copy.
    plain: wgpu::TextureView,
    id: TextureId,
    drew: Option<SkyView>,
}

pub(crate) struct SkyResources {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    image: Option<(Pyramid, wgpu::BindGroup)>,
    target: Option<Target>,
}

impl SkyResources {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("egui_glass_sky"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui_glass_sky"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("egui_glass_sky"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui_glass_sky"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("egui_glass_sky"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui_glass_sky"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SkyResources { pipeline, layout, sampler, uniform, image: None, target: None }
    }

    /// The window-sized picture as gamma bytes, for the glass's copy.
    pub(crate) fn view(&self) -> Option<wgpu::TextureView> {
        self.target.as_ref().filter(|t| t.drew.is_some()).map(|t| t.plain.clone())
    }
}

/// Uploads the picture (RGBA, `size` pixels). Call [`crate::init`] first.
///
/// # Panics
///
/// Panics if [`crate::init`] has not been called, or `rgba` isn't `size` pixels.
pub fn set_sky_image(render_state: &RenderState, size: [u32; 2], rgba: &[u8]) {
    assert_eq!(rgba.len(), size[0] as usize * size[1] as usize * 4, "the sky's pixels");
    let device = &render_state.device;
    let mut renderer = render_state.renderer.write();
    let mipgen: std::sync::Arc<MipGen> = renderer.callback_resources.get::<GlassResources>().map(|g| g.mipgen.clone()).expect("egui_glass::init must be called first");
    if renderer.callback_resources.get::<SkyResources>().is_none() {
        renderer.callback_resources.insert(SkyResources::new(device));
    }
    let pyramid = mipgen.create(device, size[0], size[1], wgpu::TextureUsages::COPY_DST);
    render_state.queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &pyramid.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * size[0]), rows_per_image: Some(size[1]) },
        wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("egui_glass_sky") });
    mipgen.generate(&mut encoder, &pyramid);
    render_state.queue.submit([encoder.finish()]);
    let sky = renderer.callback_resources.get_mut::<SkyResources>().expect("inserted above");
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("egui_glass_sky"),
        layout: &sky.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&pyramid.sample_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sky.sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: sky.uniform.as_entire_binding() },
        ],
    });
    sky.image = Some((pyramid, bind));
    if let Some(target) = &mut sky.target {
        target.drew = None;
    }
}

/// The mip level that keeps the picture from shimmering: how many picture pixels fall on one of
/// the window's.
fn level(view: &SkyView, image: [u32; 2], window: [u32; 2]) -> f32 {
    let per_pixel = match view {
        SkyView::Flat { min, max } => (max[0] - min[0]).abs() * image[0] as f32 / window[0].max(1) as f32,
        SkyView::Rain { .. } => 0.0,
        SkyView::Round { up, .. } => {
            let half = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
            image[0] as f32 / std::f32::consts::TAU * (2.0 * half / window[1].max(1) as f32)
        }
    };
    per_pixel.max(1.0).log2()
}

/// Draws the picture for a window of `size` pixels if that view isn't drawn yet, and returns the
/// texture to show it with. `None` before there is a picture.
pub fn draw_sky(render_state: &RenderState, size: [u32; 2], view: SkyView) -> Option<TextureId> {
    let device = &render_state.device;
    let [w, h] = [size[0].max(1), size[1].max(1)];
    let mut guard = render_state.renderer.write();
    let renderer = &mut *guard;
    let sky = renderer.callback_resources.get_mut::<SkyResources>()?;
    sky.image.as_ref()?;
    if sky.target.as_ref().is_none_or(|t| t.size != [w, h]) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui_glass_sky"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        let plain = texture.create_view(&wgpu::TextureViewDescriptor { format: Some(wgpu::TextureFormat::Rgba8Unorm), ..Default::default() });
        let srgb = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let old = sky.target.take();
        let id = renderer.register_native_texture(device, &srgb, wgpu::FilterMode::Linear);
        if let Some(old) = old {
            renderer.free_texture(&old.id);
        }
        let sky = renderer.callback_resources.get_mut::<SkyResources>()?;
        sky.target = Some(Target { size: [w, h], plain, id, drew: None });
    }
    let sky = renderer.callback_resources.get_mut::<SkyResources>()?;
    let (pyramid, bind) = sky.image.as_ref()?;
    let target = sky.target.as_mut()?;
    if target.drew == Some(view) {
        return Some(target.id);
    }
    let lod = level(&view, pyramid.size, [w, h]);
    let (right, up, forward, flat, round, time) = match view {
        SkyView::Flat { min, max } => ([0.0; 3], [0.0; 3], [0.0; 3], [min[0], min[1], max[0], max[1]], 0.0, 0.0),
        SkyView::Round { right, up, forward } => (right, up, forward, [0.0; 4], 1.0, 0.0),
        SkyView::Rain { min, max, time } => ([0.0; 3], [0.0; 3], [0.0; 3], [min[0], min[1], max[0], max[1]], 2.0, time),
    };
    let mut params = [0.0_f32; 24];
    params[..3].copy_from_slice(&right);
    params[4..7].copy_from_slice(&up);
    params[8..11].copy_from_slice(&forward);
    params[12..16].copy_from_slice(&flat);
    params[16] = lod;
    params[17] = round;
    params[18] = time;
    params[19] = w as f32 / h as f32;
    // The window's pixels, for the rain's grain.
    params[3] = w as f32;
    params[7] = h as f32;
    let bytes: Vec<u8> = params.iter().flat_map(|v| v.to_le_bytes()).collect();
    render_state.queue.write_buffer(&sky.uniform, 0, &bytes);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("egui_glass_sky") });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui_glass_sky"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.plain,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&sky.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }
    render_state.queue.submit([encoder.finish()]);
    target.drew = Some(view);
    Some(target.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_shown_smaller_takes_a_smaller_level() {
        let flat = |x: f32| SkyView::Flat { min: [0.0, 0.0], max: [x, 1.0] };
        assert_eq!(level(&flat(1.0), [1024, 512], [1024, 512]), 0.0);
        assert_eq!(level(&flat(1.0), [1024, 512], [256, 128]), 2.0);
        assert_eq!(level(&flat(0.5), [1024, 512], [2048, 1024]), 0.0, "never below the full picture");
    }
}
