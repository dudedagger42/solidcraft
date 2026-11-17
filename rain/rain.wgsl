// Rain on a frosted window over a city at night, simulated on the GPU so the drops remember. The
// state is a grid of cells (96 across, 48 down, the manifest's "state"), each holding at most one
// drop. A cell's texel is (x offset, y offset, radius, trail): the offsets are from the cell's
// centre, in cell widths across and cell heights down; the radius is in cell heights; the trail is
// how wiped the glass is, 1 just after a drop went through and fading over about 7 s.
//
// Each frame a drop at rest dries. A drop big enough to slide moves down by its speed, and once
// its centre passes the bottom of its cell it is handed to the cell below, which reads its upper
// neighbour for that. Two drops that meet in a cell merge, their areas adding up. Empty cells
// catch new drops at random. The rules follow the CPU simulation of the all-in-one (all/looks,
// vendor/egui-glass/src/rain.rs) in cells; the grid is coarser, so the drops are smaller.

// Sizes in cell heights, for the 48-cell grid.
const STICK: f32 = 0.36;
const MIN_R: f32 = 0.03;
const MAX_R: f32 = 0.5;
// Radius lost per frame at rest: a small bead lasts about 30 s.
const DRY: f32 = 0.00025;
// Chance per empty cell per frame of a new drop, four times as much for the first 900 frames so
// the glass fills in quickly.
const LAND: f32 = 0.0003;
// Frames for a wiped trail to fog over again: 7 s at 30 frames a second.
const REFOG: f32 = 210.0;

const TAU: f32 = 6.2831853;

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

// A random number for a cell, this frame and a salt for each use. The frame wraps so the hash
// keeps its precision.
fn rnd(cell: vec2<f32>, tick: f32, salt: f32) -> f32 {
    let t = tick - 1024.0 * floor(tick / 1024.0);
    return hash(vec2<f32>(cell.x * 1.37 + salt * 7.31, cell.y * 0.73 + t * 3.11 + salt));
}

// How far a drop wanders sideways in one frame, in cell widths.
fn sway(cell: vec2<f32>, tick: f32) -> f32 {
    return (rnd(cell, tick, 0.0) - 0.5) * 0.06;
}

// How far a sliding drop goes in one frame, in cell heights: the CPU's pace at 30 frames a
// second, a little faster for a heavier drop.
fn speed(r: f32, rows: f32) -> f32 {
    return clamp((0.13 + 32.0 * max(r / rows - 0.0055, 0.0)) * rows / 30.0, 0.1, 0.5);
}

// The drop (x, y, radius) of the cell `cell` one frame on. A drop that slides past the bottom of
// its cell comes back with y past 0.5, for the cell below to take; a drop at rest dries, and a
// radius of 0 is no drop.
fn advance(d: vec3<f32>, cell: vec2<f32>, tick: f32, rows: f32) -> vec3<f32> {
    if d.z < STICK {
        let r = d.z - DRY;
        return vec3<f32>(d.xy, select(0.0, r, r >= MIN_R));
    }
    return vec3<f32>(clamp(d.x + sway(cell, tick), -0.4, 0.4), d.y + speed(d.z, rows), d.z);
}

// Two drops in one cell become one: the areas add up, and the heavier drop keeps its place.
fn merged(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    let heavy = select(a, b, b.z > a.z);
    return vec3<f32>(heavy.xy, min(sqrt(a.z * a.z + b.z * b.z), MAX_R));
}

fn state(px: ScStatePixel) -> vec4<f32> {
    let rows = sc.state_size.y;
    let tick = sc.frame;
    let at = vec2<i32>(px.texel);
    let cell = vec2<f32>(px.texel);
    let own = textureLoad(sc_state_prev, at, 0);
    var above = vec4<f32>(0.0);
    if at.y > 0 {
        above = textureLoad(sc_state_prev, at + vec2<i32>(0, -1), 0);
    }
    // The drop here, if it is still here after this frame.
    var drop = vec3<f32>(0.0);
    let kept = advance(own.xyz, cell, tick, rows);
    if kept.z > 0.0 && kept.y <= 0.5 {
        drop = kept;
    }
    // The drop that comes down from the cell above, if it slides past the boundary.
    let came = advance(above.xyz, vec2<f32>(cell.x, cell.y - 1.0), tick, rows);
    let arrives = came.z > 0.0 && came.y > 0.5;
    if arrives {
        let moved = vec3<f32>(came.x, came.y - 1.0, came.z);
        drop = select(moved, merged(drop, moved), drop.z > 0.0);
    }
    if drop.z <= 0.0 {
        let landing = LAND * select(1.0, 4.0, sc.frame < 900.0);
        if rnd(cell, tick, 1.0) < landing {
            let size = rnd(cell, tick, 3.0);
            let r = select(mix(0.1, 0.3, size), mix(0.35, MAX_R, size), rnd(cell, tick, 2.0) < 0.035);
            drop = vec3<f32>((rnd(cell, tick, 4.0) - 0.5) * 0.8, (rnd(cell, tick, 5.0) - 0.5) * 0.8, r);
        }
    }
    if drop.z <= 0.0 {
        drop = vec3<f32>(0.0);
    }
    // A sliding drop wipes the glass it passes; the trail fogs over again after REFOG frames.
    var trail = max(own.w - 1.0 / REFOG, 0.0);
    if own.z >= STICK || arrives {
        trail = 1.0;
    }
    return vec4<f32>(drop.xy, drop.z, trail);
}

// The trail of one cell, the grid's edges clamped.
fn wiped_at(n: vec2<i32>) -> f32 {
    let c = clamp(n, vec2<i32>(0), vec2<i32>(sc.state_size) - 1);
    return textureLoad(sc_state, c, 0).w;
}

// The shading below was tuned on screen values, so it works on gamma colour and turns back at the end.
fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
}
fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Frosted glass: a few taps turned by a per-pixel angle, from a blurry level of the picture.
fn frosted(uv: vec2<f32>, lod: f32, spread: f32, grain: f32, aspect: f32) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 7; i++) {
        let a = f32(i) * 2.399963 + grain * TAU;
        let at = sqrt((f32(i) + 0.5) / 7.0) * spread;
        sum += to_gamma(textureSampleLevel(city, sc_clamp, uv + vec2<f32>(cos(a) / aspect, sin(a)) * at, lod).rgb);
    }
    return sum / 7.0;
}

fn background(px: ScPixel) -> vec4<f32> {
    let aspect = sc.size.x / max(sc.size.y, 1.0);
    // The city covers the view from its centre.
    let picture = vec2<f32>(textureDimensions(city, 0));
    let cover = max(sc.size.x / picture.x, sc.size.y / picture.y);
    let half = sc.size / cover / picture * 0.5;
    let uv = vec2<f32>(0.5) - half + px.uv * half * 2.0;
    // The glass in window heights, y down, and the grid's cells in it.
    let q = px.uv * vec2<f32>(aspect, 1.0);
    let grid = sc.state_size;
    let cw = aspect / grid.x;
    let ch = 1.0 / grid.y;
    let top = vec2<i32>(grid) - 1;
    let at = clamp(vec2<i32>(px.uv * grid), vec2<i32>(0), top);
    // The drops in this cell and its eight neighbours.
    var drop = 0.0;
    var bend = vec2<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let n = clamp(at + vec2<i32>(i, j), vec2<i32>(0), top);
            let d = textureLoad(sc_state, n, 0);
            if d.z <= 0.0 {
                continue;
            }
            let centre = vec2<f32>((f32(n.x) + 0.5 + d.x) * cw, (f32(n.y) + 0.5 + d.y) * ch);
            let radius = d.z * ch;
            let v = q - centre;
            let len = length(v);
            if len < radius {
                let inside = 1.0 - smoothstep(radius * 0.72, radius, len);
                if inside > drop {
                    drop = inside;
                    bend = v / radius * inside;
                }
            }
        }
    }
    // The wiped glass, bilinear between the cells' centres.
    let f = px.uv * grid - 0.5;
    let base = vec2<i32>(floor(f));
    let t = f - floor(f);
    let wiped = mix(
        mix(wiped_at(base), wiped_at(base + vec2<i32>(1, 0)), t.x),
        mix(wiped_at(base + vec2<i32>(0, 1)), wiped_at(base + vec2<i32>(1, 1)), t.x),
        t.y,
    );
    // Each drop is a lens: it shows the city small and nearly sharp.
    let see = uv - bend * vec2<f32>(0.2 / aspect, 0.2);
    let clear = max(wiped * 0.72, drop);
    let lod = mix(5.6, 0.6, clear);
    let spread = mix(0.024, 0.0008, clear);
    let grain = hash(floor(px.position));
    var c3 = frosted(see, lod, spread, grain, aspect);
    // The fog on the glass, gone where the water wiped it.
    let fog = vec3<f32>(0.2, 0.22, 0.28) + c3 * 0.3;
    c3 = mix(c3 * 0.85 + fog * 0.62, c3 * 1.04 + vec3<f32>(0.05, 0.055, 0.07), clear);
    // A clear lens, a little brighter than the fog: a bright crescent low in it, a dark top rim,
    // and a small light high up.
    let len = length(bend);
    c3 += c3 * 0.35 * drop + vec3<f32>(0.035, 0.04, 0.05) * drop;
    let rim = smoothstep(0.55, 1.0, len) * drop;
    c3 *= 1.0 - rim * clamp(-bend.y / max(len, 1e-3), 0.0, 1.0) * 0.45;
    let crescent = smoothstep(0.45, 0.95, len) * clamp(bend.y / max(len, 1e-3), 0.0, 1.0);
    c3 += vec3<f32>(0.5, 0.48, 0.44) * crescent * crescent * drop * 0.5;
    let shine = 1.0 - smoothstep(0.0, 0.25, length(bend + vec2<f32>(0.25, 0.4)));
    c3 += vec3<f32>(0.5, 0.52, 0.58) * shine * drop;
    return vec4<f32>(to_linear(c3), 1.0);
}
