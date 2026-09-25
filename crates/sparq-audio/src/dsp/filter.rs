//! `flt/svf` — state-variable filter, zero-delay feedback (trapezoidal SVF).
//!
//! Topology per Välimäki & Huovilainen / the Cytomic "SVF" derivation, which is what most modern
//! analogue-modelling filters use: two trapezoidal integrators in a loop solved in closed form, so
//! there is no unit-delay in the feedback path and the cutoff can be modulated per sample without
//! the phase error and instability a naive SVF shows.
//!
//! Quality bar (plan §9.1): coefficients are computed in `f64` in `prepare`/`set_cutoff`, the
//! sample loop is `f32`, and `g` is clamped so the filter cannot be driven unstable by an extreme
//! cutoff + resonance combination.

/// Filter response.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SvfMode {
    /// Lowpass, 12 dB/oct.
    #[default]
    LowPass,
    /// Highpass, 12 dB/oct.
    HighPass,
    /// Bandpass, 6 dB/oct either side.
    BandPass,
    /// Notch (LP + HP).
    Notch,
    /// Peak (LP − HP).
    Peak,
    /// All four outputs summed with the given weights — used by modules that expose the SVF as a
    /// morphable response rather than a switch.
    Morph,
}

/// A zero-delay-feedback state-variable filter.
#[derive(Clone, Copy, Debug)]
pub struct SvfFilter {
    /// Integrator states.
    s1: f32,
    s2: f32,
    /// Cached coefficients (recomputed on cutoff/resonance/rate change, never per sample).
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    /// Response selector.
    pub mode: SvfMode,
    /// Morph weights `[lp, bp, hp, notch]`, used only in [`SvfMode::Morph`].
    pub morph: [f32; 4],
    /// Cutoff in Hz. Stored, not derived from the coefficients: deriving it caused a real bug
    /// where `new()` ran before the sample rate was known, the coefficients came out at the 1 Hz
    /// clamp, and every filter in the crate silently became a 1 Hz lowpass.
    cutoff: f64,
    /// Normalised resonance `0..=1`, stored for the same reason.
    resonance: f32,
    sample_rate: u32,
    /// Drive applied before the integrators; >1 pushes the filter into nonlinearity in later
    /// phases (there is no saturation here yet, so it is a plain gain and is documented as such).
    pub drive: f32,
}

impl Default for SvfFilter {
    fn default() -> Self {
        Self::new(1000.0, 0.707)
    }
}

impl SvfFilter {
    /// A filter at `cutoff` Hz with normalised resonance `resonance` in `0..=1`.
    ///
    /// `resonance` maps to Q as `Q = 0.5 + 9.5 * r`, so 0 is flat (Q ≈ 0.71 at the low end is
    /// avoided deliberately: Q 0.5 gives a genuinely flat Butterworth-ish response) and 1.0 is a
    /// pronounced but still stable peak. Call [`SvfFilter::prepare`] before the first `process`.
    #[must_use]
    pub fn new(cutoff: f64, resonance: f32) -> Self {
        Self {
            s1: 0.0,
            s2: 0.0,
            g: 0.0,
            k: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            mode: SvfMode::LowPass,
            morph: [1.0, 0.0, 0.0, 0.0],
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            sample_rate: 0,
            drive: 1.0,
        }
    }

    /// Recompute coefficients for a sample rate. **Must** be called before the first `tick`; until
    /// then the filter has no coefficients and outputs zeros.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute();
    }

    /// Set cutoff in Hz. Cheap enough to call per block; the `tan` is the only real cost.
    pub fn set_cutoff(&mut self, cutoff: f64) {
        self.cutoff = cutoff;
        self.recompute();
    }

    /// Set normalised resonance (`0..=1`).
    pub fn set_resonance(&mut self, resonance: f32) {
        self.resonance = resonance.clamp(0.0, 1.0);
        self.recompute();
    }

    /// Current cutoff in Hz.
    #[must_use]
    pub fn cutoff_hz(&self) -> f64 {
        self.cutoff
    }

    /// Current normalised resonance.
    #[must_use]
    pub fn resonance(&self) -> f32 {
        self.resonance
    }

    /// Whether coefficients have been computed (i.e. `prepare` has run).
    #[must_use]
    pub fn is_prepared(&self) -> bool {
        self.sample_rate != 0 && self.g > 0.0
    }

    fn recompute(&mut self) {
        let sample_rate = self.sample_rate;
        if sample_rate == 0 {
            return;
        }
        // Clamp so g stays below 1: at g >= 1 the trapezoidal SVF denominator degenerates and the
        // filter loses stability margin. Nyquist/2 is a musically sensible ceiling anyway.
        let fc = self.cutoff.clamp(1.0, f64::from(sample_rate) * 0.4999);
        let g = (std::f64::consts::PI * fc / f64::from(sample_rate)).tan() as f32;
        let g = g.clamp(1e-6, 0.999);
        let q = 0.5 + 9.5 * f64::from(self.resonance.clamp(0.0, 1.0));
        let k = (1.0 / q) as f32;
        self.g = g;
        self.k = k;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    /// Reset the integrator state (e.g. on patch load or a scene change).
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// Filter one sample and return the response selected by [`SvfFilter::mode`].
    #[inline]
    pub fn tick(&mut self, input: f32) -> f32 {
        debug_assert!(self.is_prepared(), "SvfFilter::tick before prepare(); output will be wrong");
        let v = input * self.drive;
        let a1 = self.a1;
        let a2 = self.a2;
        let a3 = self.a3;
        let k = self.k;

        let v3 = v - self.s2;
        let v1 = a1 * self.s1 + a2 * v3;
        let v2 = self.s2 + a2 * self.s1 + a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;

        let lp = v2;
        let bp = v1;
        let hp = v3 - k * v1;
        match self.mode {
            SvfMode::LowPass => lp,
            SvfMode::HighPass => hp,
            SvfMode::BandPass => bp,
            SvfMode::Notch => lp + hp,
            SvfMode::Peak => lp - hp,
            SvfMode::Morph => {
                let m = self.morph;
                let notch = lp + hp;
                m[0] * lp + m[1] * bp + m[2] * hp + m[3] * notch
            },
        }
        .clamp(-8.0, 8.0) // headroom guard: resonance can ring above full scale
    }

    /// Filter a buffer in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for s in buf.iter_mut() {
            *s = self.tick(*s);
        }
    }

    /// All four responses for one sample, as `[lp, bp, hp, notch]`. Useful for displays and for
    /// modules that want to morph without changing `mode`.
    #[inline]
    pub fn tick_all(&mut self, input: f32) -> [f32; 4] {
        debug_assert!(self.is_prepared(), "SvfFilter::tick_all before prepare()");
        let v = input * self.drive;
        let (a1, a2, a3, k) = (self.a1, self.a2, self.a3, self.k);
        let v3 = v - self.s2;
        let v1 = a1 * self.s1 + a2 * v3;
        let v2 = self.s2 + a2 * self.s1 + a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        let lp = v2;
        let bp = v1;
        let hp = v3 - k * v1;
        [lp, bp, hp, lp + hp]
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Amplitude response at a given frequency, measured by pushing a sine through and reading the
    /// steady-state peak. Deliberately crude and dependency-free: it is a sanity instrument, not a
    /// reference analyser.
    fn response(f: &mut SvfFilter, freq: f64, sr: u32, seconds: f64) -> f64 {
        f.prepare(sr);
        f.reset();
        let n = (seconds * f64::from(sr)) as usize;
        let mut peak = 0.0f64;
        let mut phase = 0.0f64;
        let inc = 2.0 * std::f64::consts::PI * freq / f64::from(sr);
        // skip the first 20 % as transient
        let skip = n / 5;
        for i in 0..n {
            let y = f.tick((phase.sin() * 0.5) as f32);
            phase += inc;
            if phase > std::f64::consts::TAU {
                phase -= std::f64::consts::TAU;
            }
            if i > skip {
                peak = peak.max(f64::from(y.abs()));
            }
        }
        peak / 0.5
    }

    fn db(v: f64) -> f64 {
        if v <= 1e-9 {
            -180.0
        } else {
            20.0 * v.log10()
        }
    }

    #[test]
    fn new_before_prepare_is_not_silently_a_1hz_lowpass() {
        // Regression: `new()` used to compute coefficients with sample_rate = 0, land on the 1 Hz
        // clamp, and every filter in the crate became a 1 Hz lowpass that passed nothing.
        let mut f = SvfFilter::new(1000.0, 0.0);
        assert!(!f.is_prepared(), "coefficients must not exist before prepare()");
        f.prepare(48_000);
        assert!(f.is_prepared());
        assert!(
            (f.cutoff_hz() - 1000.0).abs() < 1e-9,
            "cutoff survived prepare: {}",
            f.cutoff_hz()
        );
        let r = response(&mut f, 100.0, 48_000, 0.2);
        assert!(db(r) > -3.0, "a 1 kHz lowpass must pass 100 Hz, got {:.1} dB", db(r));
    }

    #[test]
    fn cutoff_survives_a_rate_change() {
        let mut f = SvfFilter::new(2500.0, 0.4);
        f.prepare(48_000);
        assert!((f.cutoff_hz() - 2500.0).abs() < 1e-9);
        f.prepare(96_000);
        assert!((f.cutoff_hz() - 2500.0).abs() < 1e-9, "rate change must not move the cutoff");
        let r = response(&mut f, 2500.0, 96_000, 0.3);
        assert!(r > 0.2, "filter should be near unity at its own cutoff, got {r}");
    }

    #[test]
    fn lowpass_passes_low_and_attenuates_high() {
        let sr = 48_000;
        let mut f = SvfFilter::new(1000.0, 0.0);
        f.mode = SvfMode::LowPass;
        let low = response(&mut f, 100.0, sr, 0.2);
        let high = response(&mut f, 10_000.0, sr, 0.2);
        assert!(db(low) > -3.0, "100 Hz should pass, got {:.1} dB", db(low));
        assert!(db(high) < -18.0, "10 kHz should be well down, got {:.1} dB", db(high));
    }

    #[test]
    fn highpass_is_the_complement() {
        let sr = 48_000;
        let mut f = SvfFilter::new(1000.0, 0.0);
        f.mode = SvfMode::HighPass;
        let low = response(&mut f, 100.0, sr, 0.2);
        let high = response(&mut f, 10_000.0, sr, 0.2);
        assert!(db(low) < -18.0, "100 Hz should be rejected, got {:.1} dB", db(low));
        assert!(db(high) > -3.0, "10 kHz should pass, got {:.1} dB", db(high));
    }

    #[test]
    fn bandpass_peaks_near_cutoff() {
        let sr = 48_000;
        let mut f = SvfFilter::new(2000.0, 0.6);
        f.mode = SvfMode::BandPass;
        let at = response(&mut f, 2000.0, sr, 0.3);
        let below = response(&mut f, 300.0, sr, 0.3);
        let above = response(&mut f, 12_000.0, sr, 0.3);
        assert!(at > below * 1.5, "bp at cutoff {at} should exceed 300 Hz {below}");
        assert!(at > above * 1.5, "bp at cutoff {at} should exceed 12 kHz {above}");
    }

    #[test]
    fn notch_cancels_at_cutoff() {
        let sr = 48_000;
        let mut f = SvfFilter::new(1000.0, 0.0);
        f.mode = SvfMode::Notch;
        let at = response(&mut f, 1000.0, sr, 0.4);
        let away = response(&mut f, 100.0, sr, 0.4);
        assert!(at < away * 0.35, "notch at cutoff {at} should dip vs 100 Hz {away}");
    }

    #[test]
    fn resonance_raises_the_peak() {
        let sr = 48_000;
        let mut flat = SvfFilter::new(1000.0, 0.0);
        let mut reso = SvfFilter::new(1000.0, 0.85);
        let a = response(&mut flat, 1000.0, sr, 0.3);
        let b = response(&mut reso, 1000.0, sr, 0.3);
        assert!(b > a * 1.5, "resonance should boost the cutoff region: {a} -> {b}");
    }

    #[test]
    fn cutoff_readback_matches_what_was_set() {
        let sr = 96_000;
        for fc in [20.0f64, 120.0, 1000.0, 12_345.6, 40_000.0] {
            let mut f = SvfFilter::new(fc, 0.3);
            f.prepare(sr);
            let got = f.cutoff_hz();
            let err = (got - fc).abs() / fc;
            assert!(err < 0.02, "set {fc} Hz, read back {got} Hz (err {:.3}%)", err * 100.0);
        }
    }

    #[test]
    fn extreme_settings_stay_stable_and_bounded() {
        let sr = 48_000;
        for fc in [1.0f64, 5.0, 23_999.0] {
            for reso in [0.0f32, 0.5, 1.0] {
                let mut f = SvfFilter::new(fc, reso);
                f.prepare(sr);
                let mut x = 0.0f64;
                let mut peak = 0.0f32;
                for _ in 0..sr {
                    x += 0.013;
                    let y = f.tick((x.sin() * 0.9) as f32);
                    assert!(y.is_finite(), "non-finite output at fc={fc} r={reso}");
                    peak = peak.max(y.abs());
                }
                assert!(peak < 8.0, "output ran away at fc={fc} r={reso}: peak {peak}");
            }
        }
    }

    #[test]
    fn silence_in_silence_out() {
        let mut f = SvfFilter::new(800.0, 0.7);
        f.prepare(48_000);
        let mut buf = [0.0f32; 4096];
        f.process(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn per_sample_cutoff_modulation_does_not_blow_up() {
        // The whole point of ZDF: modulating the cutoff at audio rate must not destabilise.
        let sr = 48_000;
        let mut f = SvfFilter::new(500.0, 0.9);
        f.prepare(sr);
        let mut peak = 0.0f32;
        let mut m = 0.0f64;
        for i in 0..sr * 2 {
            m += 0.05;
            let fc = 400.0 + 6000.0 * (m.sin() * 0.5 + 0.5);
            if i % 8 == 0 {
                f.set_cutoff(fc);
            }
            let y = f.tick((m * 3.0).sin() as f32 * 0.4);
            assert!(y.is_finite());
            peak = peak.max(y.abs());
        }
        assert!(peak < 8.0, "audio-rate cutoff modulation ran away: peak {peak}");
    }
}
