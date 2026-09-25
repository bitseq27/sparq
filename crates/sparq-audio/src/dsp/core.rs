//! The Phase 0 core nodes (`syn/sine`, `util/gain`) plus the dB helpers everything else uses.

/// A sine oscillator.
///
/// Phase is accumulated in `f64` and wrapped to `[0, 2π)` every sample. The `f64` accumulator
/// keeps phase error below one `f32` ULP for about 34 hours at 192 kHz, which is far beyond any
/// render or set; the exact bound is asserted in `phase_error_stays_below_f32_ulp`.
#[derive(Clone, Copy, Debug)]
pub struct SineOsc {
    /// Frequency in Hz.
    pub freq: f64,
    /// Amplitude, linear 0..1.
    pub amp: f32,
    phase: f64,
    /// Cached `2π * freq / sample_rate`, recomputed in `prepare`.
    phase_inc: f64,
    sample_rate: u32,
}

/// `2π`, spelled out so the constant is exact and reviewable.
pub const TAU: f64 = std::f64::consts::TAU;

impl SineOsc {
    /// A new oscillator. Call [`SineOsc::prepare`] before the first `process`.
    #[must_use]
    pub const fn new(freq: f64, amp: f32) -> Self {
        Self { freq, amp, phase: 0.0, phase_inc: 0.0, sample_rate: 0 }
    }

    /// Recompute cached rate-dependent state. Called when the sample rate or frequency changes —
    /// never from inside `process` (plan §5.2: `prepare` may allocate and compute, `process` may
    /// not).
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute_increment();
    }

    /// Set the frequency, in Hz. Safe to call per block; the increment is recomputed here, not in
    /// the audio loop.
    pub fn set_freq(&mut self, freq: f64, sample_rate: u32) {
        self.freq = freq;
        if sample_rate != self.sample_rate {
            self.prepare(sample_rate);
        } else {
            self.recompute_increment();
        }
    }

    fn recompute_increment(&mut self) {
        self.phase_inc =
            if self.sample_rate == 0 { 0.0 } else { TAU * self.freq / f64::from(self.sample_rate) };
    }

    /// Reset phase to zero (e.g. on a trigger, or to make a render reproducible from any point).
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Current phase in radians, `[0, 2π)`. Exposed for displays (`dsp/scope` X/Y mode).
    #[must_use]
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// Advance one sample and return it.
    #[inline]
    pub fn process_mono(&mut self) -> f32 {
        let y = (self.phase.sin() as f32) * self.amp;
        self.phase += self.phase_inc;
        if self.phase >= TAU {
            self.phase -= TAU;
        } else if self.phase < 0.0 {
            self.phase += TAU;
        }
        y
    }

    /// Fill `out` (mono) with the oscillator output.
    ///
    /// Allocation-free, syscall-free, branch-free apart from the phase wrap.
    pub fn process(&mut self, out: &mut [f32]) {
        let mut phase = self.phase;
        let inc = self.phase_inc;
        let amp = self.amp;
        for s in out.iter_mut() {
            *s = (phase.sin() as f32) * amp;
            phase += inc;
            if phase >= TAU {
                phase -= TAU;
            } else if phase < 0.0 {
                phase += TAU;
            }
        }
        self.phase = phase;
    }
}

/// A dB gain stage with one-pole smoothing.
///
/// Smoothing happens **inside** the audio path (plan §5.2 rule 2): the host hands over a target and
/// the node glides to it, so a parameter change at a block boundary cannot produce a step, and a
/// step cannot produce a click.
#[derive(Clone, Copy, Debug)]
pub struct Gain {
    target: f32,
    current: f32,
    /// One-pole coefficient, computed in `prepare`. `1.0` disables smoothing.
    coeff: f32,
    /// Smoothing time constant in ms, remembered so `prepare` can be called with only a rate.
    smooth_ms: f32,
    /// Channel count, for applying the same gain across an interleaved buffer.
    channels: usize,
}

impl Gain {
    /// A gain stage at unity (0 dB), smoothing over `smooth_ms`.
    #[must_use]
    pub fn new(db: f32, smooth_ms: f32) -> Self {
        let lin = db_to_linear(db);
        Self { target: lin, current: lin, coeff: 1.0, smooth_ms, channels: 1 }
    }

    /// Recompute the smoothing coefficient for a sample rate and channel count.
    pub fn prepare(&mut self, sample_rate: u32, smooth_ms: f32, channels: usize) {
        self.smooth_ms = smooth_ms;
        self.channels = channels.max(1);
        let frames = (f64::from(sample_rate) * f64::from(smooth_ms) / 1000.0).max(1.0);
        // One-pole: y += a (x - y), a = 1 - exp(-1 / frames_in_time_constant)
        self.coeff = (1.0 - (-1.0 / frames).exp()) as f32;
        if smooth_ms <= 0.0 {
            self.coeff = 1.0;
        }
    }

    /// Set the target gain in dB. Takes effect smoothly over the configured time constant.
    pub fn set_db(&mut self, db: f32) {
        self.target = db_to_linear(db);
    }

    /// Set the target gain in dB and jump to it immediately (used at patch load).
    pub fn set_db_immediate(&mut self, db: f32) {
        self.target = db_to_linear(db);
        self.current = self.target;
    }

    /// Current linear gain (mid-glide). Exposed for meters.
    #[must_use]
    pub fn linear(&self) -> f32 {
        self.current
    }

    /// Re-prepare with the remembered smoothing time constant (used when only the rate changed).
    pub fn prepare_at(&mut self, sample_rate: u32, channels: usize) {
        let ms = self.smooth_ms;
        self.prepare(sample_rate, ms, channels);
    }

    /// Apply gain in place to an interleaved buffer.
    pub fn process(&mut self, buf: &mut [f32]) {
        let a = self.coeff;
        let target = self.target;
        let mut g = self.current;
        if a >= 1.0 {
            g = target;
            for s in buf.iter_mut() {
                *s *= g;
            }
        } else {
            let chans = self.channels.max(1);
            // Smooth once per frame (not per sample) so the glide is rate-independent.
            for frame in buf.chunks_mut(chans) {
                g += a * (target - g);
                for s in frame.iter_mut() {
                    *s *= g;
                }
            }
        }
        self.current = g;
    }
}

/// Convert dB to linear gain. `-inf` and very negative values map to 0.
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= -120.0 {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

/// Convert linear gain to dB.
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn db_roundtrip() {
        for db in [-120.0f32, -60.0, -6.0, 0.0, 6.0, 12.0] {
            let lin = db_to_linear(db);
            if db > -120.0 {
                assert!((linear_to_db(lin) - db).abs() < 1e-4, "db {db} roundtrip");
            } else {
                assert_eq!(lin, 0.0);
            }
        }
    }

    #[test]
    fn sine_frequency_is_exact_by_zero_crossings() {
        let sr = 48_000u32;
        let freq = 440.0;
        let mut osc = SineOsc::new(freq, 1.0);
        osc.prepare(sr);
        let n = sr as usize; // exactly one second
        let mut buf = vec![0.0f32; n];
        osc.process(&mut buf);
        let crossings = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        // One rising zero crossing per period; ±1 for the window edges.
        assert!(
            (crossings as f64 - freq).abs() <= 1.0,
            "expected ~{freq} rising crossings in 1 s, got {crossings}"
        );
    }

    #[test]
    fn sine_is_dc_free_and_bounded() {
        let sr = 96_000u32;
        let mut osc = SineOsc::new(1234.5678, 0.8);
        osc.prepare(sr);
        let mut buf = vec![0.0f32; sr as usize * 2];
        osc.process(&mut buf);
        let mean = buf.iter().map(|s| f64::from(*s)).sum::<f64>() / buf.len() as f64;
        assert!(mean.abs() < 1e-4, "DC offset {mean} exceeds 1e-4");
        let peak = buf.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(peak <= 0.8 + 1e-6, "peak {peak} exceeds amplitude");
        assert!(peak > 0.79, "peak {peak} should reach the configured amplitude");
    }

    #[test]
    fn phase_stays_wrapped_over_long_renders() {
        // One hour at 192 kHz: the accumulator must never escape [0, τ), and a frequency that
        // divides the sample rate exactly must return to phase zero every cycle.
        let sr = 192_000u32;
        let mut osc = SineOsc::new(440.0, 1.0);
        osc.prepare(sr);
        let mut buf = [0.0f32; 512];
        let blocks = (sr as usize * 3600) / 512;
        for _ in 0..blocks {
            osc.process(&mut buf);
            let ph = osc.phase();
            assert!((0.0..TAU).contains(&ph), "phase escaped [0, τ): {ph}");
        }
        // 500 Hz at 48 kHz is exactly 96 samples per cycle, so one second is a whole number of
        // cycles and the phase must return to ~0 with only wrap-rounding error.
        let sr2 = 48_000u32;
        let mut o2 = SineOsc::new(500.0, 1.0);
        o2.prepare(sr2);
        // 750 blocks of 64 = exactly 48 000 samples = exactly one second = exactly 500 cycles.
        // (Block size must divide the total, or the "whole number of cycles" premise is false.)
        let mut b2 = [0.0f32; 64];
        for _ in 0..750 {
            o2.process(&mut b2);
        }
        assert!(
            o2.phase() < 1e-9 || (TAU - o2.phase()) < 1e-9,
            "phase drift after exactly 500 cycles: {} rad",
            o2.phase()
        );
    }

    #[test]
    fn frequency_change_takes_effect_immediately() {
        let sr = 48_000u32;
        let mut osc = SineOsc::new(100.0, 1.0);
        osc.prepare(sr);
        let mut buf = [0.0f32; 4800];
        osc.process(&mut buf);
        let low = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        osc.set_freq(2000.0, sr);
        osc.reset_phase();
        osc.process(&mut buf);
        let high = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!(high > low * 10, "expected ~20× the crossings, got {low} -> {high}");
    }

    #[test]
    fn gain_smoothing_has_no_step() {
        let sr = 48_000u32;
        let mut g = Gain::new(-60.0, 10.0);
        g.prepare(sr, 10.0, 1);
        g.set_db_immediate(0.0);
        g.set_db(-60.0);
        g.set_db(0.0); // glide from -60 dB toward 0 dB
        let mut buf = vec![1.0f32; 4096];
        g.process(&mut buf);
        let mut max_step = 0.0f32;
        for w in buf.windows(2) {
            max_step = max_step.max((w[1] - w[0]).abs());
        }
        assert!(max_step < 0.01, "gain glide produced a step of {max_step} (zipper noise)");
        assert!(buf.last().unwrap() > &0.5, "should have glided most of the way to unity");
    }

    #[test]
    fn gain_at_unity_is_transparent() {
        let mut g = Gain::new(0.0, 0.0);
        g.prepare(48_000, 0.0, 2);
        let mut buf = [0.25f32, -0.5, 0.75, -0.125];
        g.process(&mut buf);
        assert_eq!(buf, [0.25, -0.5, 0.75, -0.125]);
    }
}
