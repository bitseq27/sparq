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

    /// Linear magnitude `|H(e^{jω})|` at `freq_hz` for the CURRENT mode, from the live
    /// coefficients — the per-module curve contract's reference implementation (WO-012
    /// increment 5, D2). Display-only: pure, allocation-free, and audio-thread-forbidden by
    /// contract (never called from `tick`/`process`, so no golden can move).
    ///
    /// The exact discrete-time response, solved in the z-domain (`z = e^{jωT}`, `ω = 2πf/fs`)
    /// from the same `a1/a2/a3/k` the sample loop uses. With `I = 2/(z+1)` the trapezoidal
    /// integrator's frequency-domain gain and `Den = (1 − I(1−a3))(1 − a1 I) + a2² I²`:
    ///
    /// * `H_lp = a3 / Den` (the identity `a2² = a1·a3` collapses the general numerator —
    ///   `a2 = g·a1`, `a3 = g·a2`, so `a1·a3 = a2²`)
    /// * `H_bp = a2 (1 − I) / Den`
    /// * `H_hp = (1 − I)[(1 − a1 I) − k a2] / Den`
    /// * `H_notch = H_lp + H_hp`, `H_peak = H_lp − H_hp` — the mode's own sum/difference,
    ///   matching `tick`'s `Notch`/`Peak` arms exactly; `Morph` is the same weighted sum, in
    ///   complex.
    ///
    /// Evaluated in the `J = (z+1)/2 = 1/I` form (numerator and denominator both scaled by
    /// `J²`), which is algebraically identical and finite at the boundaries: DC (`J = 1`)
    /// gives `H_lp = 1`, `H_hp = H_bp = 0`; Nyquist (`J = 0`) gives `H_lp → 0`, `H_hp → 1` —
    /// the two points every response must get right, with no `1/0` anywhere on the grid.
    ///
    /// `drive` is included: `tick` applies it as a plain linear input gain, so the display and
    /// the sine sweep measure the same transfer. The sample loop's `±8` headroom clamp is a
    /// large-signal guard, not part of the linear response, and is deliberately NOT modelled.
    /// An unprepared filter (no coefficients — `tick` would output garbage) reads `0.0`, the
    /// display's honest floor; a non-finite ask or result sanitises to `0.0` the same way.
    #[must_use]
    pub fn magnitude_at(&self, freq_hz: f64) -> f64 {
        if !self.is_prepared() || !freq_hz.is_finite() {
            return 0.0;
        }
        let fs = f64::from(self.sample_rate);
        let theta = std::f64::consts::TAU * freq_hz.clamp(0.0, fs * 0.5) / fs;
        // J = (z+1)/2 = cos(θ/2)·e^{jθ/2}, as a plain (re, im) pair — no complex type, no alloc.
        let (sin_half, cos_half) = (theta * 0.5).sin_cos();
        let jr = cos_half * cos_half;
        let ji = cos_half * sin_half;

        let (a1, a2, a3, k) =
            (f64::from(self.a1), f64::from(self.a2), f64::from(self.a3), f64::from(self.k));
        // Complex helpers, inlined: mul/add/sub of (re, im) pairs and |·|.
        let mul =
            |(ar, ai): (f64, f64), (br, bi): (f64, f64)| (ar * br - ai * bi, ar * bi + ai * br);
        let add = |(ar, ai): (f64, f64), (br, bi): (f64, f64)| (ar + br, ai + bi);
        let sub = |(ar, ai): (f64, f64), (br, bi): (f64, f64)| (ar - br, ai - bi);
        let j = (jr, ji);
        let j2 = mul(j, j);

        // Den·J² = (J − (1−a3))(J − a1) + a2²
        let den = add(mul(sub(j, (1.0 - a3, 0.0)), sub(j, (a1, 0.0))), (a2 * a2, 0.0));
        let den_m2 = den.0 * den.0 + den.1 * den.1;
        if den_m2.is_nan() || den_m2 <= 0.0 {
            return 0.0; // a degenerate coefficient set reads at the floor, never NaN
        }
        // H·J² numerators (the J² scaling cancels between numerator and denominator).
        let h_lp = (a3 * j2.0, a3 * j2.1);
        let jm1 = sub(j, (1.0, 0.0));
        let h_bp = mul((a2, 0.0), mul(jm1, j));
        let h_hp = mul(jm1, sub(sub(j, mul((k * a2, 0.0), j)), (a1, 0.0)));
        let h_notch = add(h_lp, h_hp);
        let h = match self.mode {
            SvfMode::LowPass => h_lp,
            SvfMode::HighPass => h_hp,
            SvfMode::BandPass => h_bp,
            SvfMode::Notch => h_notch,
            SvfMode::Peak => sub(h_lp, h_hp),
            SvfMode::Morph => {
                let m = self.morph;
                add(
                    add(mul((f64::from(m[0]), 0.0), h_lp), mul((f64::from(m[1]), 0.0), h_bp)),
                    add(mul((f64::from(m[2]), 0.0), h_hp), mul((f64::from(m[3]), 0.0), h_notch)),
                )
            },
        };
        // |H| = |H·J²| / |Den·J²| — one square root per side, both real and non-negative.
        let num_m2 = h.0 * h.0 + h.1 * h.1;
        let mag = (num_m2 / den_m2).sqrt() * f64::from(self.drive);
        if mag.is_finite() {
            mag
        } else {
            0.0
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
        response_amp(f, freq, sr, seconds, 0.5)
    }

    /// [`response`] at a chosen input amplitude. The drift gate drives it QUIET (0.05): a
    /// resonant peak mode at 0.5 would exceed `tick`'s ±8 headroom clamp and the instrument
    /// would measure the ceiling instead of the filter.
    fn response_amp(f: &mut SvfFilter, freq: f64, sr: u32, seconds: f64, amp: f64) -> f64 {
        f.prepare(sr);
        f.reset();
        let n = (seconds * f64::from(sr)) as usize;
        let mut peak = 0.0f64;
        let mut phase = 0.0f64;
        let inc = 2.0 * std::f64::consts::PI * freq / f64::from(sr);
        // skip the first 20 % as transient
        let skip = n / 5;
        for i in 0..n {
            let y = f.tick((phase.sin() * amp) as f32);
            phase += inc;
            if phase > std::f64::consts::TAU {
                phase -= std::f64::consts::TAU;
            }
            if i > skip {
                peak = peak.max(f64::from(y.abs()));
            }
        }
        peak / amp
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

    // ------------------------------------------------ the curve contract's drift gate (inc 5)
    //
    // `magnitude_at` is the DISPLAY's copy of nothing: it is computed from the filter's own
    // cached coefficients, and this test pins it to the time-domain instrument above — the
    // analytic curve and the measured filter must agree, or the inspector plot is a lie with
    // axes on it. Tolerances are the plan of record's (WO012-INC5-PLAN.md D2): ≤ 0.5 dB where
    // the response is in its passband (|H| ≥ −20 dB), ≤ 1 dB elsewhere — the sweep's crude
    // peak measurement, not the math, is what the looser stopband band absorbs.

    #[test]
    fn magnitude_at_gets_the_two_boundary_points_every_response_must_get_right() {
        let sr = 48_000;
        for reso in [0.0f32, 0.7] {
            let mut f = SvfFilter::new(1000.0, reso);
            f.prepare(sr);
            // DC (J = 1): lp passes, hp and bp reject.
            f.mode = SvfMode::LowPass;
            assert!((f.magnitude_at(0.0) - 1.0).abs() < 1e-6, "lp at DC is unity");
            f.mode = SvfMode::HighPass;
            assert!(f.magnitude_at(0.0).abs() < 1e-6, "hp at DC is zero");
            f.mode = SvfMode::BandPass;
            assert!(f.magnitude_at(0.0).abs() < 1e-6, "bp at DC is zero");
            // Nyquist (J = 0): lp rejects, hp passes — computed, not extrapolated: the J-form
            // is finite exactly at fs/2, so the grid can include its own endpoint.
            f.mode = SvfMode::LowPass;
            assert!(f.magnitude_at(f64::from(sr) / 2.0).abs() < 1e-6, "lp at Nyquist is zero");
            f.mode = SvfMode::HighPass;
            assert!(
                (f.magnitude_at(f64::from(sr) / 2.0) - 1.0).abs() < 1e-6,
                "hp at Nyquist is unity"
            );
            // Notch is lp + hp at both ends; peak is lp − hp. Both follow from the arms.
            f.mode = SvfMode::Notch;
            assert!((f.magnitude_at(0.0) - 1.0).abs() < 1e-6, "notch at DC is unity");
            f.mode = SvfMode::Peak;
            assert!((f.magnitude_at(0.0) - 1.0).abs() < 1e-6, "peak at DC is unity");
        }
    }

    #[test]
    fn magnitude_at_matches_the_sine_sweep_across_all_five_modes() {
        let sr = 48_000;
        for &mode in
            &[SvfMode::LowPass, SvfMode::HighPass, SvfMode::BandPass, SvfMode::Notch, SvfMode::Peak]
        {
            for fc in [200.0f64, 1_000.0, 5_000.0] {
                for reso in [0.0f32, 0.5, 0.9] {
                    let mut f = SvfFilter::new(fc, reso);
                    f.mode = mode;
                    f.prepare(sr);
                    for &probe in &[0.25f64, 0.5, 1.0, 2.0, 4.0] {
                        let freq = fc * probe;
                        if freq >= f64::from(sr) * 0.45 {
                            continue; // keep the sweep instrument away from its own Nyquist
                        }
                        let analytic = db(f.magnitude_at(freq));
                        let measured = db(response_amp(&mut f, freq, sr, 0.6, 0.05));
                        if analytic >= -20.0 {
                            // Passband: the two must agree tightly — this is the band a user
                            // reads the curve in.
                            assert!(
                                (analytic - measured).abs() <= 0.5,
                                "{mode:?} fc={fc} r={reso} @ {freq} Hz: analytic {analytic:.2} dB \
                                 vs sweep {measured:.2} dB (tol 0.5 dB)"
                            );
                        } else if analytic >= -60.0 {
                            // Stopband, still above the display floor: the looser declared tol.
                            assert!(
                                (analytic - measured).abs() <= 1.0,
                                "{mode:?} fc={fc} r={reso} @ {freq} Hz: analytic {analytic:.2} dB \
                                 vs sweep {measured:.2} dB (tol 1.0 dB)"
                            );
                        } else {
                            // Below the display floor (the notch's own zero): the crude peak
                            // instrument reads its transient residue, not −180 dB. The honest
                            // shared claim at this depth is "well down", and the sweep proves it.
                            assert!(
                                measured <= -20.0,
                                "{mode:?} fc={fc} r={reso} @ {freq} Hz: analytic is below the \
                                 −60 dB floor but the sweep measured {measured:.2} dB"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn magnitude_at_is_the_linear_response_of_tick_including_drive_and_morph() {
        let sr = 48_000;
        // Drive is a plain input gain in `tick`, so the display curve must carry it too —
        // one transfer function, measured and plotted the same way.
        let mut f = SvfFilter::new(1_000.0, 0.3);
        f.prepare(sr);
        let base = f.magnitude_at(300.0);
        f.drive = 2.0;
        assert!(
            (f.magnitude_at(300.0) - 2.0 * base).abs() < 1e-9,
            "drive scales the response linearly"
        );
        f.drive = 1.0;
        // Morph is the same weighted sum the sample loop makes, in complex: at the default
        // weights it IS the lowpass, and lp+notch−hp mixes like the arms say.
        f.mode = SvfMode::Morph;
        f.morph = [1.0, 0.0, 0.0, 0.0];
        let lp = {
            f.mode = SvfMode::LowPass;
            let v = f.magnitude_at(2_500.0);
            f.mode = SvfMode::Morph;
            v
        };
        assert!((f.magnitude_at(2_500.0) - lp).abs() < 1e-9, "morph[lp] == lp");
        f.morph = [0.0, 1.0, 0.0, 0.0];
        let bp = {
            f.mode = SvfMode::BandPass;
            let v = f.magnitude_at(2_500.0);
            f.mode = SvfMode::Morph;
            v
        };
        assert!((f.magnitude_at(2_500.0) - bp).abs() < 1e-9, "morph[bp] == bp");
    }

    #[test]
    fn magnitude_at_sanitises_instead_of_propagating() {
        let mut f = SvfFilter::new(1_000.0, 0.3);
        assert_eq!(f.magnitude_at(500.0), 0.0, "unprepared: the honest floor, never garbage");
        f.prepare(48_000);
        assert_eq!(f.magnitude_at(f64::NAN), 0.0, "a NaN ask reads at the floor");
        assert_eq!(f.magnitude_at(f64::INFINITY), 0.0, "an infinite ask reads at the floor");
        // Below DC and above Nyquist clamp into the plotted range — the grid endpoint rule.
        let at_neg = f.magnitude_at(-50.0);
        assert!(at_neg.is_finite() && at_neg > 0.0, "negative frequencies clamp to DC");
        let over = f.magnitude_at(1.0e9);
        assert!(over.is_finite(), "above Nyquist clamps to fs/2, never NaN");
        assert_eq!(over, f.magnitude_at(24_000.0), "…exactly onto the Nyquist point");
    }
}
