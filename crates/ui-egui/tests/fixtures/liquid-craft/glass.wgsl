// Liquid Craft glass: a lens at the rim that bends what is behind the panel, frost that grows
// towards the rim, a trace of colour fringing, a specular rim, a dark contour and a hairline ring.
// Tone and tint are worked on screen values, as the original glass was tuned there.

// normalize(-0.45, -1.0): light from the top left (window y points down).
const LIGHT: vec2<f32> = vec2<f32>(-0.41036, -0.91192);
const CHROMA: f32 = 0.04;
const SPECULAR: f32 = 0.35;
const BORDER: f32 = 0.3;
// How far the full edge bend reaches inwards: 1 bends the whole lens band.
const REACH: f32 = 1.0;

fn to_screen(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
}

fn to_light(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Sharp backdrop at p, frosted by f: mixed towards the blurred image and, as f grows, a wider
// ring of blurred taps around q.
fn frosted(p: vec2<f32>, q: vec2<f32>, f: f32) -> vec3<f32> {
    let sharp = sc_backdrop(p).rgb;
    if f < 0.01 {
        return sharp;
    }
    let r = sc.blur * 0.6 * f;
    var soft = sc_blurred(q).rgb * 2.0;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976 + 0.5235988;
        soft += sc_blurred(q + r * vec2<f32>(cos(a), sin(a))).rgb;
    }
    // Plain mixing shows the sharp image as a ghost, so it fades out early.
    return mix(sharp, soft / 8.0, clamp(f * 1.5, 0.0, 1.0));
}

fn panel(px: ScPanel) -> vec4<f32> {
    let frost = sc.settings.x;
    let see_through = sc.settings.y;
    let bend = sc.settings.z;
    let accent_tint = sc.settings.w;
    let dark = sc.dark;

    let s = max(sc.scale, 0.5);
    let d = px.edge;
    let n = px.normal;
    let lens_width = clamp(px.radius / s * 1.5, 16.0, 36.0) * s;
    let refraction = bend * s;

    let t = clamp((d + lens_width) / lens_width, 0.0, 1.0);
    let lens = t * t * (3.0 - 2.0 * t);
    let magnify = min(refraction / lens_width, 0.75) * max(-d, 0.0);
    let disp = n * lens * mix(magnify, refraction, REACH);

    let band = lens_width * 3.0;
    let rim_frost = smoothstep(0.0, 1.0, clamp((d + band) / band, 0.0, 1.0));
    let f = frost * mix(0.65, 1.35, rim_frost) * (1.0 + 0.2 * px.floating);

    let p = px.position + disp;
    // sc_blurred is only valid about two blur radii past the panel; keep the soft taps inside that.
    let q = p - n * max(d + dot(disp, n) - 1.5 * sc.blur, 0.0);
    var col: vec3<f32>;
    if lens > 0.001 {
        let k = disp * CHROMA;
        col = vec3<f32>(frosted(p + k, q + k, f).r, frosted(p, q, f).g, frosted(p - k, q - k, f).b);
    } else {
        col = frosted(p, q, f);
    }

    var g = to_screen(col);
    let lum = dot(g, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturation = mix(1.05, 0.9, dark);
    let brightness = mix(0.985, 0.86, dark) * (1.0 + 0.04 * px.hovered);
    g = mix(vec3<f32>(lum), g, saturation) * brightness;
    let tint = mix(vec3<f32>(1.0), to_screen(sc.accent.rgb), accent_tint);
    let tint_amount = mix(mix(80.0, 24.0, dark), mix(26.0, 34.0, dark), accent_tint) / 255.0;
    g = mix(g, tint, tint_amount);
    g = mix(g, mix(vec3<f32>(0.96), vec3<f32>(0.16), dark), (1.0 - see_through) * 0.5);

    let ndl = dot(n, LIGHT);
    let lit = max(ndl, 0.0);
    let unlit = max(-ndl, 0.0);
    // Highlights gather where the edge curves; straight edges get only a faint line.
    let curved = 4.0 * n.x * n.x * n.y * n.y;
    let specular = SPECULAR + 0.1 * px.hovered;
    g += vec3<f32>(specular * pow(t, 10.0) * (0.3 + 0.7 * curved) * (lit + 0.6 * unlit));
    let border = BORDER + 0.1 * px.focused;
    g *= 1.0 - 0.6 * border * pow(t, 8.0) * (0.6 + 0.4 * (1.0 - lit));
    let ring = 1.0 - smoothstep(0.0, 1.2 * s, abs(d + 0.7 * s));
    g = mix(g, vec3<f32>(1.0), border * ring * (0.32 + 0.4 * lit + 0.18 * unlit));

    return vec4<f32>(to_light(clamp(g, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
