// Your Picture: the chosen picture behind the model. A picture about twice as wide as it is tall
// is a 360 picture around the view (equirectangular, Z up) and turns with the camera. Any other
// picture covers the view flat, keeping its aspect, and does not turn.

const TAU: f32 = 6.2831853;
const PI: f32 = 3.1415927;

fn background(px: ScPixel) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(picture, 0));
    let aspect = dims.x / max(dims.y, 1.0);
    let view_aspect = sc.size.x / max(sc.size.y, 1.0);
    if (abs(aspect - 2.0) < 0.1) {
        let ray = px.ray;
        let uv = vec2<f32>(atan2(ray.y, ray.x) / TAU + 0.5, 0.5 - asin(clamp(ray.z, -1.0, 1.0)) / PI);
        // The picture spans PI radians over its height; the view shows 2 atan(|up|) of them.
        let view = 2.0 * atan(length(sc.up.xyz));
        let level = max(log2(dims.y / PI * view / max(sc.size.y, 1.0)), 0.0);
        return vec4<f32>(textureSampleLevel(picture, sc_clamp, uv, level).rgb, 1.0);
    }
    // Cover the view, cropping the longer side of the picture.
    var fit = vec2<f32>(1.0);
    if (view_aspect > aspect) {
        fit.y = aspect / view_aspect;
    } else {
        fit.x = view_aspect / aspect;
    }
    let uv = (px.uv - 0.5) * fit + 0.5;
    let level = max(log2(max(dims.x * fit.x / sc.size.x, dims.y * fit.y / sc.size.y)), 0.0);
    return vec4<f32>(textureSampleLevel(picture, sc_clamp, uv, level).rgb, 1.0);
}
