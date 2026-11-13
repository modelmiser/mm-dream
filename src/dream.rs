//! mm-dream — GPU-accelerated kaleidoscope widget via WGSL fragment shader.

use std::cell::Cell;

use cosmic::iced::{Color, Length, Rectangle};
use cosmic::iced_widget::shader;

use crate::pipeline::{DreamPrimitive, Uniforms};

// ── Palette ──────────────────────────────────────────────────────────

const V40: [Color; 16] = [
    Color::from_rgb(0.000, 0.000, 0.000), //  0  Black
    Color::from_rgb(1.000, 1.000, 1.000), //  1  White
    Color::from_rgb(0.533, 0.000, 0.000), //  2  Red
    Color::from_rgb(0.667, 1.000, 0.933), //  3  Cyan
    Color::from_rgb(0.800, 0.267, 0.800), //  4  Purple
    Color::from_rgb(0.000, 0.800, 0.333), //  5  Green
    Color::from_rgb(0.000, 0.000, 0.667), //  6  Blue
    Color::from_rgb(0.933, 0.933, 0.467), //  7  Yellow
    Color::from_rgb(0.867, 0.533, 0.333), //  8  Orange
    Color::from_rgb(0.400, 0.267, 0.000), //  9  Brown
    Color::from_rgb(1.000, 0.467, 0.467), // 10  Light Red
    Color::from_rgb(0.200, 0.200, 0.200), // 11  Dark Grey
    Color::from_rgb(0.467, 0.467, 0.467), // 12  Medium Grey
    Color::from_rgb(0.667, 1.000, 0.400), // 13  Light Green
    Color::from_rgb(0.000, 0.533, 1.000), // 14  Light Blue
    Color::from_rgb(0.733, 0.733, 0.733), // 15  Light Grey
];

const RAINBOW: [Color; 10] = [
    V40[2], V40[8], V40[7], V40[5], V40[6], // R O Y G B
    V40[6], V40[5], V40[7], V40[8], V40[2], // B G Y O R (reflected)
];
const SCANLINE_PX: f32 = 2.0;
const NUM_BARS: usize = 10;

/// Bar modes 0..3 move the raster bars; mode 4 is the field scene (0.2.0), which has no bars.
pub const FIELD_MODE: u32 = 4;
/// Total combo states: the snake through the 4×4 bar grid, then the field scene.
pub const NUM_COMBOS: usize = 17;

/// Snake pattern: each step changes only ONE of (bar, rotozoom).
/// Bars cycle forward on even rows, backward on odd rows.
pub const SNAKE: [(u32, u32); NUM_COMBOS] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (1, 3),
    (1, 2),
    (1, 1),
    (1, 0),
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (3, 3),
    (3, 2),
    (3, 1),
    (3, 0),
    // 0.2.0: the field, unfolded. The snake ended on fold "none" and still does, so the closing
    // (last scene -> still -> zoom out) stays the mirror of the opening.
    (4, 0),
];

// ── Natural constants (original mm-dream vocabulary) ────────────────
//
// These irrational ratios were chosen because their mutual
// incommensurability prevents visual repetition — bars never
// return to the same configuration. The specific values were
// vibe-tuned: φ felt right for amplitude, 1/e for coupling,
// 1/τ for drift speeds. The math works because they're all
// irrational and mutually non-resonant, not because of any
// deeper physical meaning.

const PHI: f32 = 1.618_033_9;          // golden ratio
const FRAC_1_PHI: f32 = 0.618_033_9;   // 1/φ
const FRAC_1_E: f32 = 0.367_879_4;     // 1/e
const FRAC_1_TAU: f32 = 0.159_154_9;   // 1/τ (= 1/2π)

// ── Tuning parameters (what the constants actually control) ─────────
//
// Wave mode
const WAVE_SPREAD_MIN: f32 = FRAC_1_TAU * FRAC_1_TAU;  // tightest bar grouping (~0.025)
const WAVE_SPREAD_MAX: f32 = FRAC_1_PHI;                // loosest bar grouping (~0.618)
const WAVE_DISPLACEMENT: f32 = FRAC_1_E;                // base wave push-pull strength
const WAVE_COUPLING: f32 = FRAC_1_E;                    // how bar distance scales displacement
const WAVE_PHASE_OFFSET: f32 = FRAC_1_PHI;              // phase stagger between bars
const JITTER_INTENSITY: f32 = FRAC_1_TAU;               // per-bar wobble amplitude
const JITTER_FREQ_SPREAD: f32 = FRAC_1_PHI;             // how bar index spreads wobble frequency
const JITTER_FREQ_OFFSET: f32 = FRAC_1_E;               // base wobble frequency offset
const WAVE_BAR_WIDTH_MAX: f32 = PHI + 1.0;              // widest bars (breath fully exhaled)
const WAVE_BAR_WIDTH_BREATH: f32 = 2.0;                 // breath contracts width by this much
//
// Plasma mode
const PLASMA_FREQ_STEP: f32 = FRAC_1_PHI;               // frequency spread across bars
const PLASMA_SECONDARY_FREQ: f32 = FRAC_1_E;            // secondary oscillation base freq
const PLASMA_SECONDARY_STEP: f32 = FRAC_1_TAU;          // secondary freq spread per bar
const PLASMA_PHASE_SPREAD: f32 = FRAC_1_PHI;            // phase offset between bars (× τ)
const PLASMA_PRIMARY_AMP: f32 = FRAC_1_PHI;             // main oscillation strength
const PLASMA_SECONDARY_AMP: f32 = FRAC_1_E;             // secondary oscillation strength
const PLASMA_BAR_WIDTH: f32 = PHI;                       // constant bar width
//
// Bounce mode
const BOUNCE_SPEED: f32 = PHI;                           // bounce frequency multiplier
const BOUNCE_PHASE_SPREAD: f32 = FRAC_1_PHI;            // stagger between bars (× τ)
const BOUNCE_AMPLITUDE: f32 = FRAC_1_PHI;               // snap displacement
const BOUNCE_BAR_WIDTH: f32 = FRAC_1_PHI;               // narrow bars
//
// Cascade mode
const CASCADE_SPEED: f32 = FRAC_1_PHI;                  // propagation rate
const CASCADE_PHASE_SPREAD: f32 = FRAC_1_PHI;           // phase delay per bar distance (× τ)
const CASCADE_AMPLITUDE: f32 = FRAC_1_PHI;              // wave displacement
const CASCADE_BAR_WIDTH_BASE: f32 = PHI;                 // widest (breath exhaled)
const CASCADE_BAR_WIDTH_BREATH: f32 = FRAC_1_PHI;       // breath shrinks by this much
//
// Kaleidoscope drift speeds (rad/s, all sub-perceptual)
const DRIFT_2FOLD: f32 = FRAC_1_TAU;                    // ~0.16 rad/s bilateral mirror
const DRIFT_5FOLD: f32 = FRAC_1_TAU * FRAC_1_PHI;      // ~0.10 rad/s pentagonal
const DRIFT_10FOLD: f32 = FRAC_1_TAU * FRAC_1_E;       // ~0.06 rad/s decagonal
const ZOOM_PULSE_DEPTH: f32 = 0.3;                      // 10-fold zoom oscillation amplitude
const ZOOM_PULSE_RATE: f32 = FRAC_1_E;                  // zoom oscillation speed
//
// CRT simulation
const CRT_BARREL: f32 = FRAC_1_TAU * FRAC_1_TAU;       // barrel distortion curvature
//
// Static fallback bar width
const STATIC_BAR_WIDTH: f32 = PHI;

// ── Transition state machine ─────────────────────────────────────────

#[derive(Copy, Clone)]
struct Transition {
    current: u32,
    previous: u32,
    progress: f32,
}

impl Transition {
    fn new(mode: u32) -> Self {
        Self {
            current: mode,
            previous: mode,
            progress: 1.0,
        }
    }
    fn start(&mut self, new: u32) {
        self.previous = self.current;
        self.current = new;
        self.progress = 0.0;
    }
    fn advance(&mut self, step: f32) {
        if self.progress < 1.0 {
            self.progress = (self.progress + step).min(1.0);
        }
    }
    fn active(&self) -> bool {
        self.progress < 1.0
    }
}

// ── Public API ───────────────────────────────────────────────────────

pub fn v40_bars(time: f32) -> V40Bars {
    V40Bars {
        time,
        blend: 1.0,
        speed: 1.0 / 3.0,
        zoom_t: 0.0,
        bar: Transition::new(0),
        rz: Transition::new(0),
        auto_cycle: true,
        width: Length::Fill,
        height: Length::Fill,
        prev_positions: Cell::new([0.0; 10]),
        flux: Cell::new(0.0),
        flags: 0,
    }
}

pub struct V40Bars {
    time: f32,
    blend: f32,
    speed: f32,
    zoom_t: f32,
    bar: Transition,
    rz: Transition,
    auto_cycle: bool,
    width: Length,
    height: Length,
    // Flux tracking — interior mutability because draw() takes &self
    prev_positions: Cell<[f32; 10]>,
    flux: Cell<f32>,
    /// Shader flags: bit 1 CRT off.
    flags: u32,
}

fn smoothstep_f(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl V40Bars {
    /// Frame-to-frame visual change rate (updated each draw call).
    /// Normalized: bar movement / bar_width × fold_count.
    pub fn flux(&self) -> f32 {
        self.flux.get()
    }

    pub fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }

    pub fn set_time(&mut self, t: f32) {
        self.time = t;
    }

    pub fn set_blend(&mut self, b: f32) {
        self.blend = b.clamp(0.0, 1.0);
    }

    pub fn set_zoom_t(&mut self, z: f32) {
        self.zoom_t = z.clamp(0.0, 1.0);
    }

    pub fn reset_modes(&mut self) {
        self.bar = Transition::new(0);
        self.rz = Transition::new(0);
    }

    pub fn set_mode(&mut self, m: u32) {
        self.bar.current = m;
    }

    pub fn start_rz_transition(&mut self, new_mode: u32) {
        self.rz.start(new_mode);
    }

    pub fn auto_cycle(&self) -> bool {
        self.auto_cycle
    }

    pub fn transitioning(&self) -> bool {
        self.bar.active() || self.rz.active()
    }

    pub fn step_to(&mut self, bar: u32, rz: u32) {
        if bar != self.bar.current {
            self.bar.start(bar);
        }
        if rz != self.rz.current {
            self.rz.start(rz);
        }
    }

    pub fn advance_all_transitions(&mut self, step: f32) {
        self.bar.advance(step);
        self.rz.advance(step);
    }

    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }

    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
}

// ── Shader Program ──────────────────────────────────────────────────

impl<Message> shader::Program<Message> for V40Bars {
    type State = ();
    type Primitive = DreamPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: cosmic::iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let t = self.time * self.speed;
        let n = (bounds.height / SCANLINE_PX).max(1.0);

        let (pos_curr, half_curr) = compute_bars(t, n as usize, self.blend, self.bar.current);

        let (positions, bar_half) = if self.bar.active() {
            let (pos_prev, half_prev) = compute_bars(t, n as usize, self.blend, self.bar.previous);
            let s = smoothstep_f(self.bar.progress);
            let mut blended = [0.0_f32; 10];
            for i in 0..NUM_BARS {
                blended[i] = pos_prev[i] + (pos_curr[i] - pos_prev[i]) * s;
            }
            let half = half_prev + (half_curr - half_prev) * s;
            (blended, half)
        } else {
            (pos_curr, half_curr)
        };

        let mut bar_colors = [[0.0_f32; 4]; 10];
        for (i, c) in RAINBOW.iter().enumerate() {
            bar_colors[i] = Uniforms::pack_color(*c);
        }

        let (rz_curr_a, rz_curr_z, rz_curr_f) = compute_rotozoom(t, self.rz.current);
        let (fold_angle, fold_zoom, fold_count, prev_fold_count, fold_blend, prev_fold_angle) =
            if self.rz.active() {
                let (rz_prev_a, rz_prev_z, rz_prev_f) = compute_rotozoom(t, self.rz.previous);
                let s = smoothstep_f(self.rz.progress);
                (
                    rz_curr_a,
                    rz_prev_z + (rz_curr_z - rz_prev_z) * s,
                    rz_curr_f,
                    rz_prev_f,
                    s,
                    rz_prev_a, // prev mode's own axis angle
                )
            } else {
                (rz_curr_a, rz_curr_z, rz_curr_f, rz_curr_f, 1.0, rz_curr_a)
            };

        // Measure flux — raw frame-to-frame bar movement in scanline units.
        // NOT normalized by bar_half (that flattens mode differences — wide bars
        // move more but normalizing cancels it out). Raw delta preserves the
        // amplitude differences between modes (plasma 3× > wave 1×).
        let prev = self.prev_positions.get();
        let bar_flux: f32 = positions
            .iter()
            .zip(prev.iter())
            .map(|(c, p)| (c - p).abs())
            .sum::<f32>();
        self.flux.set(bar_flux);
        self.prev_positions.set(positions);

        // The field scene crossfades with the bars over the same transition that blends bar modes.
        let is_field = |m: u32| if m == FIELD_MODE { 1.0_f32 } else { 0.0 };
        let field_mix = if self.bar.active() {
            let s = smoothstep_f(self.bar.progress);
            is_field(self.bar.previous) + (is_field(self.bar.current) - is_field(self.bar.previous)) * s
        } else {
            is_field(self.bar.current)
        };

        DreamPrimitive {
            uniforms: Uniforms {
                bar_pos: Uniforms::pack_positions(&positions),
                bar_colors,
                bar_half,
                blend: self.blend,
                time: t,
                scale: 1.0, // overwritten in prepare() with viewport scale factor
                n,
                bounds_w: bounds.width,
                bounds_h: bounds.height,
                k_crt: CRT_BARREL,
                fold_angle,
                fold_zoom,
                fold_count,
                prev_fold_count,
                fold_blend,
                prev_fold_angle,
                zoom_t: self.zoom_t,
                flags: self.flags as f32,
                field_mix,
                _pad: [0.0; 3],
            },
        }
    }
}

// ── Motion math (CPU — runs once per frame) ─────────────────────────
//
// Each mode has its own CHARACTER:
//   Wave    — oceanic, slow breath, bars coupled, wide + overlapping
//   Plasma  — chaotic, bars cross freely, large sweeps, constant width
//   Bounce  — percussive, sharp snappy motion, narrow bars, clear separation
//   Cascade — fluid, traveling wave, smooth propagation, medium width

#[allow(clippy::needless_range_loop)]
fn compute_bars(t: f32, n: usize, blend: f32, mode: u32) -> ([f32; 10], f32) {
    use std::f32::consts::TAU;

    let bar_half_base = (n as f32 / 16.0).max(4.0);
    let band_h = n as f32 / (NUM_BARS as f32 * 1.6);
    let total_h = band_h * NUM_BARS as f32;
    let home_start = (n as f32 - total_h) / 2.0;
    let center = home_start + NUM_BARS as f32 / 2.0 * band_h;
    let base_amp = ((n as f32 / 2.0 - bar_half_base) * 0.25).max(0.0);
    let mid = (NUM_BARS - 1) as f32 / 2.0;
    let omega = TAU / 2.0;

    let mut positions = [0.0_f32; 10];

    // Mode-specific amplitude, bar width, and motion
    let bar_half = match mode {
        0 => {
            // Wave — oceanic, slow breath, wide overlapping bars
            let breath = (t * omega).sin() * 0.5 + 0.5;
            let amp = base_amp;
            let spread = WAVE_SPREAD_MIN + breath * (WAVE_SPREAD_MAX - WAVE_SPREAD_MIN);
            let wave_freq = omega * 4.0 / 3.0;
            let jf = [
                omega * 11.0 / 3.0,
                omega * 3.0,
                omega * 7.0 / 3.0,
                omega * 5.0 / 3.0,
                omega,
                omega,
                omega * 5.0 / 3.0,
                omega * 7.0 / 3.0,
                omega * 3.0,
                omega * 11.0 / 3.0,
            ];
            for i in 0..NUM_BARS {
                let home_pos = home_start + i as f32 * band_h + band_h / 2.0;
                let dist = (i as f32 - mid).abs();
                let mirror = if (i as f32) < mid { -1.0 } else { 1.0 };
                let breathed = center + (home_pos - center) * spread;
                let wave = amp
                    * WAVE_DISPLACEMENT
                    * (dist * WAVE_COUPLING + WAVE_PHASE_OFFSET)
                    * mirror
                    * (t * wave_freq).sin();
                let jitter =
                    amp * JITTER_INTENSITY * (dist * JITTER_FREQ_SPREAD + JITTER_FREQ_OFFSET) * mirror * (jf[i] * t).sin();
                let anim = breathed + wave + jitter;
                positions[i] = anim + (home_pos - anim) * blend;
            }
            // Wide bars, breath-driven
            bar_half_base * (WAVE_BAR_WIDTH_MAX - breath * WAVE_BAR_WIDTH_BREATH)
        }

        1 => {
            // Plasma — chaotic, bars cross, large sweeps, constant width
            let amp = base_amp * 3.0; // 3x amplitude — bars sweep the viewport
            for i in 0..NUM_BARS {
                let home_pos = home_start + i as f32 * band_h + band_h / 2.0;
                // Two incommensurate frequencies per bar → complex paths
                let f1 = omega * (1.0 + i as f32 * PLASMA_FREQ_STEP);
                let f2 = omega * (PLASMA_SECONDARY_FREQ + i as f32 * PLASMA_SECONDARY_STEP);
                let phase = i as f32 * TAU * PLASMA_PHASE_SPREAD;
                let anim = center
                    + amp * PLASMA_PRIMARY_AMP * (t * f1 + phase).sin()
                    + amp * PLASMA_SECONDARY_AMP * (t * f2 - phase).sin();
                positions[i] = anim + (home_pos - anim) * blend;
            }
            // Constant medium-wide bars (no breath pulsing)
            bar_half_base * PLASMA_BAR_WIDTH
        }

        2 => {
            // Bounce — percussive, sharp motion, narrow separated bars
            let amp = base_amp * 2.5;
            let bounce_speed = omega * BOUNCE_SPEED;
            for i in 0..NUM_BARS {
                let home_pos = home_start + i as f32 * band_h + band_h / 2.0;
                let mirror = if (i as f32) < mid { -1.0_f32 } else { 1.0 };
                // Staggered phase — bars bounce in sequence
                let phase = i as f32 * BOUNCE_PHASE_SPREAD * TAU;
                let raw = (t * bounce_speed + phase).sin();
                // abs + quadratic: fast snap at bounce points, slow at peak
                let bounce = raw.abs();
                let sharp = bounce * bounce;
                let anim = home_pos + amp * BOUNCE_AMPLITUDE * mirror * sharp;
                positions[i] = anim + (home_pos - anim) * blend;
            }
            // Narrow bars — clear separation between bounces
            bar_half_base * BOUNCE_BAR_WIDTH
        }

        3 => {
            // Cascade — fluid traveling wave, smooth propagation
            let amp = base_amp * 2.0;
            let cascade_speed = omega * CASCADE_SPEED;
            // Slow gentle breath (half rate)
            let breath = (t * omega * 0.5).sin() * 0.5 + 0.5;
            for i in 0..NUM_BARS {
                let home_pos = home_start + i as f32 * band_h + band_h / 2.0;
                let dist = (i as f32 - mid).abs();
                let mirror = if (i as f32) < mid { -1.0_f32 } else { 1.0 };
                // Phase delay proportional to distance from center — visible propagation
                let phase = dist * CASCADE_PHASE_SPREAD * TAU;
                let wave = (t * cascade_speed - phase).sin();
                let anim = home_pos + amp * CASCADE_AMPLITUDE * mirror * wave;
                positions[i] = anim + (home_pos - anim) * blend;
            }
            // Medium bars with slow breath
            bar_half_base * (CASCADE_BAR_WIDTH_BASE - breath * CASCADE_BAR_WIDTH_BREATH)
        }

        _ => {
            // Static — just home positions
            for i in 0..NUM_BARS {
                positions[i] = home_start + i as f32 * band_h + band_h / 2.0;
            }
            bar_half_base * STATIC_BAR_WIDTH
        }
    };

    (positions, bar_half)
}

// ── Kaleidoscope modes — analytical N-fold coordinate folding ────────
//
// Instead of rotating fast and blurring, fold coordinates into a
// fundamental domain via polar angle modular arithmetic. Perfect
// N-fold symmetry, zero flicker, single sample per pixel.
// Axis drifts slowly for organic morphing.
//
// Mode 0: None    — identity (horizontal bars)
// Mode 1: 2-fold  — bilateral mirror, slow axis drift
// Mode 2: 5-fold  — pentagonal kaleidoscope (10 bars / 2)
// Mode 3: 10-fold — decagonal, one fold per bar, slow zoom pulse

/// Returns (fold_angle, zoom, fold_count).
fn compute_rotozoom(t: f32, mode: u32) -> (f32, f32, u32) {
    use std::f32::consts::TAU;
    let omega = TAU / 2.0;

    match mode {
        0 => (0.0, 1.0, 0),
        1 => (t * DRIFT_2FOLD, 1.0, 2),
        2 => (t * DRIFT_5FOLD, 1.0, 5),
        3 => (
            t * DRIFT_10FOLD,
            1.0 + ZOOM_PULSE_DEPTH * (t * omega * ZOOM_PULSE_RATE).sin(),
            10,
        ),
        _ => (0.0, 1.0, 0),
    }
}
