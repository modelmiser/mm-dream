//! The animation clock: integers, not accumulated f32 seconds.
//!
//! 0.1 kept `elapsed: f32`, added 0.016 each tick, wrapped it at 10,000 s (a visible jump every 2.8 hours) and
//! multiplied it by each motion's angular rate. An f32 second count loses phase precision as it grows, which
//! is why it had to be wrapped at all.
//!
//! Here time is a tick count (`u64` whole ticks plus a 32-bit fraction, because the flux limiter runs the clock
//! at a fractional speed), and every periodic motion asks for its PHASE: `ticks × rate mod 1 turn`, with the
//! rate in 0.64 fixed point. Wrapping `u64` multiplication is exactly "mod 1 turn", so a phase is as precise in
//! the ten-thousandth hour as in the first second, nothing accumulates, and nothing ever wraps visibly
//! (the tick counter itself wraps consistently after 2^64 ticks).

use std::f64::consts::TAU;

/// Seconds per tick (the app ticks every 16 ms).
pub const DT: f64 = 0.016;

/// Turns per tick, 0.64 fixed point.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rate(u64);

impl Rate {
    pub fn turns_per_tick(x: f64) -> Self {
        Rate((x.rem_euclid(1.0) * 18_446_744_073_709_551_616.0) as u64)
    }
    /// An angular rate in radians per second of clock time.
    pub fn rad_per_sec(w: f64) -> Self {
        Self::turns_per_tick(w * DT / TAU)
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    ticks: u64,
    frac: u32,
}

impl Clock {
    /// One tick of the app, at `speed` in 0..=1 of full rate.
    pub fn advance(&mut self, speed: f32) {
        let add = (f64::from(speed.clamp(0.0, 1.0)) * 4_294_967_296.0) as u64;
        let f = u64::from(self.frac) + add;
        self.ticks = self.ticks.wrapping_add(f >> 32);
        self.frac = f as u32;
    }

    /// The phase of a motion at `rate`, in turns, 0 <= phase < 1. Exact to 2^-24 at any age.
    pub fn turns(&self, r: Rate) -> f32 {
        let whole = self.ticks.wrapping_mul(r.0);
        let part = ((u128::from(self.frac) * u128::from(r.0)) >> 32) as u64;
        (whole.wrapping_add(part) >> 40) as f32 / 16_777_216.0
    }

    /// The same in radians, 0 <= phase < 2π: what `(t * w)` was, inside a sine.
    pub fn rad(&self, r: Rate) -> f32 {
        self.turns(r) * std::f32::consts::TAU
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ticks: u64) -> Clock {
        Clock { ticks, frac: 0 }
    }

    /// distance between two phases, in turns, on the circle
    fn circ(a: f64, b: f64) -> f64 {
        let d = (a - b).rem_euclid(1.0);
        d.min(1.0 - d)
    }

    #[test]
    fn phase_is_time_times_rate_while_f64_can_still_tell() {
        // against an f64 reference, for tick counts where f64 is still exact enough to be the reference
        for &w in &[0.3_f64, 1.0, std::f64::consts::PI * 1.618_033_988, 7.77, 0.0613] {
            let r = Rate::rad_per_sec(w);
            for &n in &[0_u64, 1, 62, 625, 62_500, 625_000, 5_000_000] {
                let want = (n as f64 * w * DT / TAU).rem_euclid(1.0);
                let got = f64::from(at(n).turns(r));
                assert!(circ(got, want) < 2e-6, "w {w} n {n}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn no_jump_where_the_old_clock_wrapped() {
        // 0.1 wrapped at 10,000 s = 625,000 ticks. One tick is one rate step, there as anywhere.
        let r = Rate::rad_per_sec(3.0);
        let step = 3.0 * DT / TAU;
        for n in [624_998_u64, 624_999, 625_000, 625_001] {
            let d = (f64::from(at(n + 1).turns(r)) - f64::from(at(n).turns(r))).rem_euclid(1.0);
            assert!((d - step).abs() < 2e-6, "tick {n}: step {d} vs {step}");
        }
    }

    #[test]
    fn as_precise_after_a_century_as_at_the_start() {
        // a rate of exactly 1/65536 turn per tick: the phase must be exactly (n mod 65536) / 65536, at any n
        let r = Rate::turns_per_tick(1.0 / 65536.0);
        for &n in &[65_535_u64, 65_536, 65_537, 200_000_000_000, u64::MAX - 3, u64::MAX] {
            let want = (n % 65_536) as f32 / 65_536.0;
            assert_eq!(at(n).turns(r), want, "tick {n}");
        }
    }

    #[test]
    fn fractional_speed_adds_up_exactly() {
        let mut half = Clock::default();
        let mut full = Clock::default();
        for _ in 0..1000 {
            half.advance(0.5);
            half.advance(0.5);
            full.advance(1.0);
        }
        assert_eq!(half, full);
        assert_eq!(full, at(1000));
    }

    #[test]
    fn negative_rates_run_backwards() {
        let f = Rate::turns_per_tick(0.125);
        let b = Rate::turns_per_tick(-0.125);
        assert_eq!(at(3).turns(f), 0.375);
        assert_eq!(at(3).turns(b), 0.625);
    }
}
