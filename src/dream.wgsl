// mm-dream fragment shader — analytical kaleidoscope + CRT simulation.

// ── Natural constants ────────────────────────────────────────────────
const PHI: f32 = 1.618033988;          // golden ratio
const FRAC_1_SQRT2: f32 = 0.707106781; // 1/√2
const PI: f32 = 3.141592654;
const TAU: f32 = 6.283185307;
const HALF_PI: f32 = 1.570796327;
const NUM_BARS: u32 = 10u;
const SCANLINE_PX: f32 = 2.0;

// ── Tuning parameters (shader-side) ─────────────────────────────────
const WARP_AMPLITUDE: f32 = FRAC_1_SQRT2;  // x-axis bar wobble during transitions
const RADIAL_MIRROR_CENTER: f32 = 0.4;     // fold_coords radial mirror threshold base
const RADIAL_MIRROR_DRIFT: f32 = 0.2;      // how much the mirror threshold oscillates
const RADIAL_MIRROR_SPEED: f32 = 0.3;      // oscillation rate of the radial threshold

struct Uniforms {
    bar_pos: array<vec4<f32>, 3>,
    bar_colors: array<vec4<f32>, 10>,
    bar_half: f32,
    blend: f32,
    time: f32,
    scale: f32,
    n: f32,
    bounds_w: f32,
    bounds_h: f32,
    k_crt: f32,
    fold_angle: f32,
    fold_zoom: f32,
    fold_count: u32,
    prev_fold_count: u32,
    fold_blend: f32,
    prev_fold_angle: f32,
    zoom_t: f32,
    flags: f32,     // bit 1: CRT post-process off
    field_mix: f32, // 0 = raster bars, 1 = the field scene; between: crossfade
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos: array<vec2<f32>, 6>;
    pos[0] = vec2(-1.0, -1.0);
    pos[1] = vec2( 1.0, -1.0);
    pos[2] = vec2( 1.0,  1.0);
    pos[3] = vec2(-1.0, -1.0);
    pos[4] = vec2( 1.0,  1.0);
    pos[5] = vec2(-1.0,  1.0);
    var uv: array<vec2<f32>, 6>;
    uv[0] = vec2(0.0, 1.0);
    uv[1] = vec2(1.0, 1.0);
    uv[2] = vec2(1.0, 0.0);
    uv[3] = vec2(0.0, 1.0);
    uv[4] = vec2(1.0, 0.0);
    uv[5] = vec2(0.0, 0.0);
    var out: VsOut;
    out.position = vec4(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

fn get_bar_pos(i: u32) -> f32 {
    return u.bar_pos[i / 4u][i % 4u];
}

// Sample bar pattern at given source coordinates. Returns pre-CRT linear color.
fn sample_bars(x_norm: f32, y_scan: f32, half: f32) -> vec3<f32> {
    let omega = PI;
    let mid = f32(NUM_BARS - 1u) / 2.0;
    let warp_time = u.time * omega * PHI;
    let warp_spatial = x_norm * TAU * PHI;

    var max_i: f32 = 0.0;
    var sum_w: f32 = 0.0;
    var mix_r: f32 = 0.0;
    var mix_g: f32 = 0.0;
    var mix_b: f32 = 0.0;

    for (var i = 0u; i < NUM_BARS; i = i + 1u) {
        let bar_y = get_bar_pos(i);
        let warp_phase = abs(f32(i) - mid) * TAU / f32(NUM_BARS);
        let warp = half * WARP_AMPLITUDE * (1.0 - u.blend)
            * sin(warp_spatial + warp_time - warp_phase);
        let warped_y = bar_y + warp;

        let dist = abs(y_scan - warped_y);
        if dist < half {
            let intensity = cos(dist / half * HALF_PI);
            let norm = 1.0 - dist / half;
            let bloom_f = norm * norm * norm;
            let color = u.bar_colors[i].rgb;
            let r = color.r + (1.0 - color.r) * bloom_f;
            let g = color.g + (1.0 - color.g) * bloom_f;
            let b = color.b + (1.0 - color.b) * bloom_f;

            mix_r += r * intensity;
            mix_g += g * intensity;
            mix_b += b * intensity;
            sum_w += intensity;
            max_i = max(max_i, intensity);
        }
    }

    if sum_w <= 0.0 {
        return vec3(0.0);
    }
    let inv = 1.0 / sum_w;
    return vec3(mix_r * inv * max_i, mix_g * inv * max_i, mix_b * inv * max_i);
}

// ── Field source (0.2.0) ─────────────────────────────────────────────
// One rotating, breathing lattice over a slow three-term plasma, through a three-phase sine palette shaded by the
// lattice. The float form of a 16-bit integer shader that runs on an FPGA tile array (one sine table of 128 steps;
// every motion a frame count times its own rate). The rates below are those integers; the unit of length is 1/120
// of the half-height, so the pattern is the same size at every resolution.
const FIELD_UNITS: f32 = 120.0;
const FIELD_FPS: f32 = 60.0;

fn fsin(q: f32) -> f32 {
    return 0.5 + 0.5 * sin(q * TAU / 128.0);
}

fn sample_field(p: vec2<f32>) -> vec3<f32> {
    let t = u.time * FIELD_FPS;
    let dx = p.x * FIELD_UNITS;
    let dy = p.y * FIELD_UNITS;
    // plasma: horizontal, vertical and radial terms at rates 128, 80, 104 (per 512 frames, one table step)
    let rr = (dx * dx + dy * dy) / 64.0;
    let pl = (fsin(dx + t * 0.25) + fsin(dy + t * 0.15625) + fsin(rr - t * 0.203125)) * 63.75;
    // lattice: rotation at rate 29, zoom breathing at rate 41 (turns per 65536 frames)
    let ang = t * (29.0 / 65536.0) * TAU;
    let zoom = (896.0 + 510.0 + 510.0 * sin(t * (41.0 / 65536.0) * TAU)) * (2040.0 / 16384.0);
    let c = zoom * cos(ang);
    let s = zoom * sin(ang);
    let lu = (dx * c - dy * s) / 128.0;
    let lv = (dx * s + dy * c) / 128.0;
    let lat = (fsin(lu * 0.25) + fsin(lv * 0.25)) * 127.5;
    let shade = (128.0 + lat * 0.5) / 256.0;
    let i = pl + lat;
    return vec3(fsin(i), fsin(i + 43.0), fsin(i + 85.0)) * shade;
}

// Apply kaleidoscope fold to square-space coordinates
fn fold_coords(sx: f32, sy: f32, fc: u32, angle: f32) -> vec2<f32> {
    if fc == 0u {
        return vec2(sx, sy);
    }
    let r = sqrt(sx * sx + sy * sy);
    let theta = atan2(sy, sx);
    let sector = TAU / f32(fc);
    let t_shifted = theta - angle;
    let t_in_sector = t_shifted - sector * floor(t_shifted / sector);
    var t_folded: f32;
    if t_in_sector > sector * 0.5 {
        t_folded = sector - t_in_sector;
    } else {
        t_folded = t_in_sector;
    }
    t_folded = t_folded + angle;

    // 2nd order: radial mirror → mandala rings
    let r_threshold = RADIAL_MIRROR_CENTER + RADIAL_MIRROR_DRIFT * sin(u.time * RADIAL_MIRROR_SPEED);
    var folded_r = abs(r - r_threshold) + r_threshold * 0.5;

    return vec2(folded_r * cos(t_folded), folded_r * sin(t_folded));
}

// Sample bars through fold + counter-rotation pipeline. Returns screen-blended H+V color.
fn sample_fold(sq: vec2<f32>, fc: u32, angle: f32, zoom: f32,
               aspect: f32, ica: f32, isa: f32) -> vec3<f32> {
    let f = fold_coords(sq.x, sq.y, fc, angle);
    // The field scene is sampled once, in square space (no aspect squeeze), with the intro rotation. It replaces
    // the bars (it is never screened over them: overlapping rainbows burn to white) and crossfades at scene changes.
    var field = vec3(0.0);
    if u.field_mix > 0.0 {
        let q = vec2(f.x * zoom, f.y * zoom);
        field = sample_field(vec2(q.x * ica - q.y * isa, q.x * isa + q.y * ica));
        if u.field_mix >= 1.0 {
            return field;
        }
    }
    let s = vec2(f.x * zoom / aspect, f.y * zoom);
    let sh = vec2(s.x * ica - s.y * isa, s.x * isa + s.y * ica);
    let sv = vec2(s.x * ica + s.y * isa, -s.x * isa + s.y * ica);
    let xh = (sh.x + 1.0) * 0.5;
    let yh = (sh.y + 1.0) * 0.5 * u.n;
    let ch = sample_bars(xh, yh, u.bar_half);
    let xv = (sv.x + 1.0) * 0.5;
    let yv = (sv.y + 1.0) * 0.5 * u.n;
    let cv = sample_bars(yv / u.n, xv * u.n, u.bar_half);
    let bars = vec3(1.0) - (vec3(1.0) - ch) * (vec3(1.0) - cv);
    return mix(bars, field, u.field_mix);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let crt = (u32(u.flags) & 2u) == 0u;
    let k = select(0.0, u.k_crt, crt);
    let raw_dx = in.uv.x * 2.0 - 1.0;
    let raw_dy = in.uv.y * 2.0 - 1.0;

    // Zoom + counter-rotation, driven by CPU zoom_t (0 = far, 1 = settled, pre-eased).
    var intro_zoom = mix(20.0, 1.0, u.zoom_t);
    let intro_angle = (1.0 - u.zoom_t) * TAU;
    let ica = cos(intro_angle); let isa = sin(intro_angle);

    // Zoom only (no rotation) — rotation applied per-pass after fold
    let dx = raw_dx / intro_zoom;
    let dy = raw_dy / intro_zoom;

    // Inverse barrel
    let r2 = raw_dx * raw_dx + raw_dy * raw_dy;  // vignette uses screen-space radius
    let aspect = u.bounds_w / u.bounds_h;

    // The barrel bends the coordinates before anything samples them, so its radius decides which symmetries survive.
    //  - An UNFOLDED scene is built in per-axis screen units (the H and V bar passes): its mirrors, the corner-to-
    //    corner one included, survive only if the bend uses the per-axis radius. That is what 0.1 did.
    //  - A FOLDED scene is built in square pixels: its sectors match only if the bend uses the true radius
    //    (0.1 used the per-axis radius there too: sectors off by up to 33 px at 4K).
    // So each fold count gets the radius its scene is built in; a fold crossfade mixes two exact pictures.
    // Both are scaled to bend the corners equally.
    let r2_true = (raw_dx * raw_dx * aspect * aspect + raw_dy * raw_dy) * 2.0 / (aspect * aspect + 1.0);
    let b_curr = 1.0 + k * select(r2_true, r2, u.fold_count == 0u);
    let b_prev = 1.0 + k * select(r2_true, r2, u.prev_fold_count == 0u);

    // Fold + zoom — sample both current and previous fold, crossfade
    let sq = vec2(dx * b_curr * aspect, dy * b_curr);

    let curr = sample_fold(sq, u.fold_count, u.fold_angle, u.fold_zoom, aspect, ica, isa);

    var color: vec3<f32>;
    if u.fold_blend < 1.0 {
        let sq_prev = vec2(dx * b_prev * aspect, dy * b_prev);
        let prev = sample_fold(sq_prev, u.prev_fold_count, u.prev_fold_angle, u.fold_zoom, aspect, ica, isa);
        color = mix(prev, curr, u.fold_blend);
    } else {
        color = curr;
    }

    if !crt {
        return vec4(clamp(color, vec3(0.0), vec3(1.0)), 1.0);
    }

    // ── CRT effects (display space, DPI-aware) ──

    // Scale pitches by display scale factor so CRT effects look consistent
    // across 1x, 1.75x, 2x, 3x displays.
    let sl_pitch = SCANLINE_PX * u.scale;
    let gr_pitch = 3.0 * u.scale;

    // Scanlines — smooth static gap, no interlace flicker
    let scanline = 0.82 + 0.18 * cos(in.position.y * PI / sl_pitch);
    color = color * scanline;

    // Phosphor grille — pixel-exact vertical cells via @builtin(position)
    let subpixel = fract(in.position.x / gr_pitch) * 3.0;
    let grille_r = 1.0 - 0.12 * smoothstep(0.0, 1.0, abs(subpixel - 0.5));
    let grille_g = 1.0 - 0.12 * smoothstep(0.0, 1.0, abs(subpixel - 1.5));
    let grille_b = 1.0 - 0.12 * smoothstep(0.0, 1.0, abs(subpixel - 2.5));
    color = vec3(color.r * grille_r, color.g * grille_g, color.b * grille_b);

    // Vignette
    let vignette = 1.0 - smoothstep(0.8, 1.4, sqrt(r2));
    color = color * vignette;

    return vec4(clamp(color, vec3(0.0), vec3(1.0)), 1.0);
}
