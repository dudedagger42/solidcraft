// 360 galaxy: a wrap-around (equirectangular, Z up) picture behind the model that turns with the
// view and stays put when you zoom or pan. Ported from the all-in-one look (all/looks, a4faf814f).

const TAU: f32 = 6.2831853;
const PI: f32 = 3.1415927;

fn background(px: ScPixel) -> vec4<f32> {
    let ray = px.ray;
    let uv = vec2<f32>(atan2(ray.y, ray.x) / TAU + 0.5, 0.5 - asin(clamp(ray.z, -1.0, 1.0)) / PI);
    // The level by hand: the wrap of u at the seam would make the GPU pick a tiny one there.
    // The picture spans PI radians over its height; the view shows 2 atan(|up|) over sc.size.y.
    let texels = f32(textureDimensions(sky, 0).y);
    let view = 2.0 * atan(length(sc.up.xyz));
    let lod = max(log2(texels / PI * view / max(sc.size.y, 1.0)), 0.0);
    return vec4<f32>(textureSampleLevel(sky, sc_repeat, uv, lod).rgb, 1.0);
}
