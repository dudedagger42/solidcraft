// RGB wave: one hue wave across the whole window, through the ribbon's icons, its button blocks
// and the panels, travelling to the right. Each region takes the hue at its own screen x, so the
// wave runs on however the ribbon is laid out.

// sc.settings.x: points from one colour to the same colour again (Wave length).
// sc.settings.y: points a second it travels (Speed).
const SATURATION: f32 = 0.65;
const BUTTON_ALPHA: f32 = 0.18;
const PANEL_ALPHA: f32 = 0.22;

fn hsv(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = (vec3<f32>(5.0, 3.0, 1.0) + h * 6.0) % vec3<f32>(6.0);
    return v - v * s * clamp(min(k, 4.0 - k), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The hue is picked on screen (sRGB) values; the host takes linear colour.
fn linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

fn tint(r: ScRegion) -> vec4<f32> {
    let hue = fract((r.position.x - sc.time * sc.settings.y) / sc.settings.x);
    let colour = linear(hsv(hue, SATURATION, 1.0));
    if r.kind == SC_ICON {
        return vec4<f32>(colour, 1.0);
    }
    if r.kind == SC_BUTTON {
        return vec4<f32>(colour, select(0.0, BUTTON_ALPHA, r.enabled >= 1.0));
    }
    return vec4<f32>(colour, PANEL_ALPHA);
}
