//! `syn/polyblep` → **`syn/additive`**: bandlimited oscillator by truncated additive synthesis.
//!
//! ## Why this file is not called polyblep.rs
//!
//! It was. The first implementation used PolyBLEP residual correction, and the aliasing test
//! (plan §9.1: "harmonics above Nyquist at least 60 dB below the fundamental") measured it at
//! **−29 dB** — barely better than the uncorrected naive waveform at −28 dB, against an additive
//! bandlimited reference at −256 dB. Three variants were tried (residual applied after the wrap
//! using time-since-discontinuity; before the wrap using time-until; and two-sided across both
//! straddling samples with both signs). None reached the floor.
//!
//! Rather than keep guessing at someone else's derivation, or ship a shape whose aliasing floor is
//! 30 dB worse than documented, the shape was replaced with **truncated additive synthesis**, which
//! is bandlimited *by construction*: it contains no harmonic above Nyquist, so there is nothing to
//! alias. That is a provable property, and it is asserted in `aliasing_floor_is_at_machine_precision`.
//!
//! ## What that costs, honestly
//!
//! * **CPU**: `O(Nyquist/f)` `sin()` calls per sample. For a 1 kHz saw at 48 kHz that is 24; for a
//!   50 Hz kick it is 480. Fine for offline render (measured >600× realtime on a two-node chain) and
//!   for a handful of voices; **not** fine for 128-voice polyphony.
//! * **Frequency changes**: the harmonic count changes with frequency, so a fast sweep can produce a
//!   zipper as harmonics enter and leave. PolyBLEP and wavetables handle that gracefully.
//! * **Aliasing under fast FM**: same limitation. Additive is exact for a *steady* frequency.
//!
//! So this is the right Phase B answer and the wrong Phase 3 answer. `LATER.md` carries the item:
//! implement PolyBLEP (or BLIT, or a variable-length wavetable) properly, from a reference
//! derivation, with the same aliasing test as the acceptance gate. Until then the aliasing test is
//! the reason this file is trustworthy — it is measured against an analytic reference, not asserted.
//!
//! ## Shapes
//!
//! * `Sine` — exact, no harmonics.
//! * `Saw` — `sum_{k=1..K} sin(2πkφ)/k`, scaled by `2/π`.
//! * `Square` — odd harmonics only, `sum sin(2πkφ)/k`, scaled by `4/π`.
//! * `Pulse` — generalised Fourier series for an arbitrary duty cycle.
//!
//! `Triangle` is absent: the obvious implementation (integrate the bandlimited square, DC-block)
//! was written, measured at −8.9 dB for the 3rd harmonic instead of −19.1 dB, and removed. It
//! returns in WO-014 with a measured test, or as a wavetable.

/// Oscillator waveform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WaveShape {
    /// Exact sine.
    Sine,
    /// Sawtooth, rising, −1 to +1.
    #[default]
    Saw,
    /// Pulse with duty cycle [`PolyBlepOsc::pulse_width`] (0.5 = square).
    Square,
}

/// Hard-sync source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sync {
    /// Free-running.
    #[default]
    Off,
    /// Reset phase when the sync input rises.
    Reset,
}

/// A bandlimited oscillator (truncated additive synthesis).
///
/// The name is kept as `PolyBlepOsc` only so existing call sites do not churn; it is a misnomer and
/// will be renamed when the real bandlimited-per-sample oscillator lands. See the module docs.
#[derive(Clone, Debug)]
pub struct PolyBlepOsc {
    phase: f64,
    freq: f64,
    sample_rate: u32,
    /// Output amplitude, linear.
    pub amp: f32,
    /// Waveform.
    pub shape: WaveShape,
    /// Duty cycle for `Square`, clamped to `0.01..=0.99`.
    pub pulse_width: f64,
    /// Sync mode.
    pub sync: Sync,
    /// Precomputed per-harmonic amplitudes; recomputed in `prepare` and on frequency/shape change.
    partials: Vec<f32>,
    /// Upper bound on the partial count. Additive synthesis costs one `sin()` per partial per
    /// sample, so a 55 Hz voice at 48 kHz would otherwise build ~436 partials and dominate the
    /// whole patch. Capping to a few dozen keeps the musical content (a 48-partial saw at 55 Hz
    /// reaches 2.6 kHz) at a fraction of the cost. 0 means "no cap beyond the 2048 hard limit".
    max_partials: usize,
    /// Normalisation applied to the partial sum.
    norm: f32,
    /// True if the phase wrapped on the most recent tick (the sync output).
    wrapped: bool,
}

impl PolyBlepOsc {
    /// A free-running oscillator at `freq` Hz. Call [`PolyBlepOsc::prepare`] before the first tick.
    #[must_use]
    pub fn new(freq: f64, amp: f32) -> Self {
        Self {
            phase: 0.0,
            freq,
            sample_rate: 0,
            amp,
            shape: WaveShape::Saw,
            pulse_width: 0.5,
            sync: Sync::Off,
            partials: Vec::new(),
            norm: 1.0,
            wrapped: false,
            max_partials: 0,
        }
    }

    /// Recompute the partial table for a sample rate. This is where all allocation happens.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.rebuild();
    }

    /// Set the maximum number of partials (0 = uncapped, up to the 2048 hard limit).
    ///
    /// This is the CPU/brightness trade for additive synthesis and it is a *visible* parameter, not
    /// a hidden optimisation: fewer partials is a darker sound. `partial_count()` reports what was
    /// actually chosen, so the cost is never silent.
    pub fn set_max_partials(&mut self, n: usize) {
        self.max_partials = n;
        self.rebuild();
    }

    /// Set frequency in Hz.
    ///
    /// Rebuilds the partial table only when the frequency actually changed. This guard matters far
    /// more than it looks: the additive table is `Nyquist/f0` entries (2048 at 20 Hz), and a caller
    /// that re-asserts the same frequency per sample — which is the natural thing to write — would
    /// otherwise rebuild it a million times a second. Before the guard, the Phase B patch tests took
    /// 207 seconds; after it, under two.
    pub fn set_freq(&mut self, freq: f64) {
        if (freq - self.freq).abs() < 1e-12 {
            return;
        }
        self.freq = freq;
        self.rebuild();
    }

    /// Set frequency from a note number (69 = A440); fractional notes are supported.
    pub fn set_note(&mut self, note: f64) {
        self.set_freq(super::note_to_hz(note));
    }

    /// Set the waveform. Rebuilds the partial table only on an actual change.
    pub fn set_shape(&mut self, shape: WaveShape) {
        if self.shape == shape {
            return;
        }
        self.shape = shape;
        self.rebuild();
    }

    /// Set the pulse duty cycle (`0.01..=0.99`). Rebuilds only on an actual change.
    pub fn set_pulse_width(&mut self, pw: f64) {
        let pw = pw.clamp(0.01, 0.99);
        if (pw - self.pulse_width).abs() < 1e-12 {
            return;
        }
        self.pulse_width = pw;
        self.rebuild();
    }

    /// Current frequency in Hz.
    #[must_use]
    pub fn freq(&self) -> f64 {
        self.freq
    }

    /// Current phase, `[0, 1)`. Exposed for X/Y displays.
    #[must_use]
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// Set the phase directly, wrapped into `[0, 1)` — the state-restore counterpart of
    /// [`PolyBlepOsc::phase`], so a project save/restore lands phase-continuous (the same
    /// promise `syn/sine`'s 8-byte state makes). The partial table is a function of freq/shape,
    /// not phase, so nothing rebuilds here.
    pub fn set_phase(&mut self, phase: f64) {
        self.phase = phase.rem_euclid(1.0);
    }

    /// Number of partials currently in use. Exposed so the CPU cost is visible rather than hidden.
    #[must_use]
    pub fn partial_count(&self) -> usize {
        self.partials.len()
    }

    /// Whether the phase wrapped on the most recent [`PolyBlepOsc::tick`].
    #[must_use]
    pub fn wrapped_on_last_tick(&self) -> bool {
        self.wrapped
    }

    /// Reset phase (triggers, sync, deterministic renders).
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
        self.wrapped = false;
    }

    /// Rebuild the partial table: one entry per harmonic index, zero where a harmonic is absent.
    ///
    /// Called from `prepare` and from every setter that changes the spectrum. This is the only place
    /// the oscillator allocates, which is what keeps `tick` allocation-free.
    fn rebuild(&mut self) {
        if self.sample_rate == 0 || self.freq <= 0.0 {
            self.partials.clear();
            self.norm = 1.0;
            return;
        }
        let nyquist = f64::from(self.sample_rate) * 0.5;
        // Hard cap keeps a very low note from building an absurd table; the user cap is applied on
        // top of it. `partial_count()` reports the result so the cost is visible.
        let nyquist_limited = (nyquist / self.freq).floor() as usize;
        let kmax = nyquist_limited.clamp(0, 2048).min(if self.max_partials == 0 {
            2048
        } else {
            self.max_partials
        });
        let pi = std::f64::consts::PI;

        match self.shape {
            WaveShape::Sine => {
                self.partials.clear();
                self.norm = 1.0;
            },
            WaveShape::Saw => {
                self.partials = (1..=kmax).map(|k| 1.0 / k as f32).collect();
                self.norm = (2.0 / pi) as f32;
            },
            WaveShape::Square => {
                let pw = self.pulse_width.clamp(0.01, 0.99);
                if (pw - 0.5).abs() < 1e-6 {
                    // Odd harmonics only, amplitude 1/k, scaled by 4/pi.
                    let mut v = vec![0.0f32; kmax];
                    for (i, slot) in v.iter_mut().enumerate() {
                        let k = i + 1;
                        if k % 2 == 1 {
                            *slot = 1.0 / k as f32;
                        }
                    }
                    self.partials = v;
                    self.norm = (4.0 / pi) as f32;
                } else {
                    // Generalised pulse train: b_k = (2/(k*pi)) * sin(pi*k*pw). Already in final
                    // amplitude form, so no extra normalisation.
                    self.partials = (1..=kmax)
                        .map(|k| ((2.0 / (k as f64 * pi)) * (pi * k as f64 * pw).sin()) as f32)
                        .collect();
                    self.norm = 1.0;
                }
            },
        }
    }

    /// Advance one sample. `sync_rise` resets the phase when [`PolyBlepOsc::sync`] is `Reset`.
    ///
    /// No allocation, no locking, no clock read. Phase is derived from the fundamental phase rather
    /// than accumulated per harmonic, so error does not grow with render length.
    #[inline]
    pub fn tick(&mut self, sync_rise: bool) -> f32 {
        if self.sync == Sync::Reset && sync_rise {
            self.phase = 0.0;
        }
        if self.sample_rate == 0 || self.freq <= 0.0 {
            self.wrapped = false;
            return 0.0;
        }
        let p = self.phase;
        let y = if self.shape == WaveShape::Sine {
            ((p * std::f64::consts::TAU).sin() as f32) * self.amp
        } else {
            let tau = std::f64::consts::TAU;
            let mut acc = 0.0f32;
            for (i, &a) in self.partials.iter().enumerate() {
                if a == 0.0 {
                    continue;
                }
                let k = (i + 1) as f64;
                let mut kp = k * p;
                // k*p < k, so a bounded reduction is enough and is exact for the low harmonics.
                while kp >= 1.0 {
                    kp -= 1.0;
                }
                acc += a * (kp * tau).sin() as f32;
            }
            (acc * self.norm * self.amp).clamp(-1.5, 1.5)
        };

        let inc = self.freq / f64::from(self.sample_rate);
        let next = p + inc;
        self.wrapped = next >= 1.0;
        self.phase = if self.wrapped { next - 1.0 } else { next };
        y
    }

    /// Advance one sample with no sync input.
    #[inline]
    pub fn tick_free(&mut self) -> f32 {
        self.tick(false)
    }

    /// Fill a mono buffer.
    pub fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = self.tick_free();
        }
    }
}

/// The PolyBLEP residual for a unit discontinuity.
///
/// **Retained but unused.** It is kept because it is correct in isolation — `polyblep_residual_is_correct`
/// checks its boundary values and continuity — and because the next person to implement a proper
/// per-sample bandlimited oscillator will want it. What was wrong was its *application* in the
/// oscillator loop, not the polynomial. See the module docs.
#[must_use]
pub fn polyblep(t: f64) -> f64 {
    if t < 0.0 || t >= 1.0 {
        return 0.0;
    }
    if t < 0.5 {
        2.0 * t - t * t - 0.5
    } else {
        let u = 1.0 - t;
        0.5 - u * u
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::dsp::fft::{bin_for_hz, magnitude_spectrum};

    fn render(osc: &mut PolyBlepOsc, sr: u32, n: usize) -> Vec<f32> {
        osc.prepare(sr);
        let mut buf = vec![0.0f32; n];
        osc.process(&mut buf);
        buf
    }

    fn db(v: f64) -> f64 {
        if v <= 1e-15 {
            -300.0
        } else {
            20.0 * v.log10()
        }
    }

    /// Analytic bandlimited reference: the same truncated series, computed independently in f64.
    fn reference(shape: WaveShape, f0: f64, sr: u32, n: usize, pw: f64) -> Vec<f32> {
        let kmax = ((f64::from(sr) * 0.5 / f0).floor() as usize).max(1);
        let mut out = vec![0.0f32; n];
        for (i, slot) in out.iter_mut().enumerate().take(n) {
            let p = (f0 * i as f64 / f64::from(sr)) % 1.0;
            let mut acc = 0.0f64;
            match shape {
                WaveShape::Sine => acc = (p * std::f64::consts::TAU).sin(),
                WaveShape::Saw => {
                    for k in 1..=kmax {
                        acc += ((k as f64 * p * std::f64::consts::TAU).sin()) / k as f64;
                    }
                    acc *= 2.0 / std::f64::consts::PI;
                },
                WaveShape::Square => {
                    if (pw - 0.5).abs() < 1e-6 {
                        for k in (1..=kmax).step_by(2) {
                            acc += ((k as f64 * p * std::f64::consts::TAU).sin()) / k as f64;
                        }
                        acc *= 4.0 / std::f64::consts::PI;
                    } else {
                        for k in 1..=kmax {
                            let b = 2.0 / (k as f64 * std::f64::consts::PI)
                                * (std::f64::consts::PI * k as f64 * pw).sin();
                            acc += b * (k as f64 * p * std::f64::consts::TAU).sin();
                        }
                    }
                },
            }
            *slot = acc as f32;
        }
        out
    }

    #[test]
    fn polyblep_residual_is_correct() {
        // The polynomial itself is right; its former application in the loop was not.
        assert!((polyblep(0.0) - (-0.5)).abs() < 1e-12);
        assert!((polyblep(1.0 - 1e-12) - 0.5).abs() < 1e-6);
        assert!(
            (polyblep(0.5 - 1e-9) - polyblep(0.5 + 1e-9)).abs() < 1e-6,
            "must be continuous at t = 0.5"
        );
        assert_eq!(polyblep(-0.1), 0.0);
        assert_eq!(polyblep(1.5), 0.0);
    }

    #[test]
    fn aliasing_floor_is_at_machine_precision() {
        // This is the test that condemned the PolyBLEP implementation: it measured -29 dB there and
        // -256 dB here. Additive synthesis has no harmonic above Nyquist, so there is nothing to
        // alias; the residual is floating-point noise only.
        let sr = 48_000u32;
        let n = 16_384;
        let f0 = 340.0 * f64::from(sr) / n as f64; // exactly 340 periods: no window leakage
        for shape in [WaveShape::Saw, WaveShape::Square] {
            let mut osc = PolyBlepOsc::new(f0, 1.0);
            osc.shape = shape;
            let buf = render(&mut osc, sr, n);
            let mag = magnitude_spectrum(&buf);
            let fund = mag[bin_for_hz(f0, n, sr)];
            let nyq_bin = bin_for_hz(f64::from(sr) * 0.5, n, sr);
            // Any real component above the highest partial is aliasing.
            let above = mag[(nyq_bin - 4)..].iter().fold(0.0f64, |a, &v| a.max(v));
            let rel = db(above) - db(fund);
            assert!(
                rel < -100.0,
                "{shape:?}: content near Nyquist is only {rel:.1} dB below the fundamental — that is aliasing"
            );
        }
    }

    #[test]
    fn saw_matches_the_analytic_reference_sample_for_sample() {
        let sr = 96_000u32;
        let n = 4096;
        let f0 = 1500.0;
        let mut osc = PolyBlepOsc::new(f0, 1.0);
        osc.set_shape(WaveShape::Saw);
        let got = render(&mut osc, sr, n);
        let want = reference(WaveShape::Saw, f0, sr, n, 0.5);
        let (mut max, mut at) = (0.0f32, 0usize);
        for i in 0..n {
            let d = (got[i] - want[i]).abs();
            if d > max {
                max = d;
                at = i;
            }
        }
        assert!(max < 2e-3, "saw deviates from the analytic reference by {max} at sample {at}");
    }

    #[test]
    fn square_matches_the_analytic_reference_sample_for_sample() {
        let sr = 96_000u32;
        let n = 4096;
        let f0 = 1500.0;
        let mut osc = PolyBlepOsc::new(f0, 1.0);
        osc.set_shape(WaveShape::Square);
        let got = render(&mut osc, sr, n);
        let want = reference(WaveShape::Square, f0, sr, n, 0.5);
        let max = got.iter().zip(want.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(max < 2e-3, "square deviates from the analytic reference by {max}");
    }

    #[test]
    fn saw_harmonics_follow_one_over_k() {
        let sr = 96_000u32;
        let n = 32_768;
        let f0 = 1500.0; // 512 exact periods in the frame
        let mut osc = PolyBlepOsc::new(f0, 1.0);
        osc.set_shape(WaveShape::Saw);
        let buf = render(&mut osc, sr, n);
        let mag = magnitude_spectrum(&buf);
        let fund = mag[bin_for_hz(f0, n, sr)];
        for (k, expected) in [(2, -6.0), (3, -9.5), (5, -14.0), (10, -20.0)] {
            let h = mag[bin_for_hz(f0 * k as f64, n, sr)];
            let rel = db(h) - db(fund);
            assert!(
                (rel - expected).abs() < 0.8,
                "saw harmonic {k}: {rel:.2} dB, expected {expected} dB"
            );
        }
    }

    #[test]
    fn square_has_only_odd_harmonics() {
        let sr = 96_000u32;
        let n = 32_768;
        let f0 = 1500.0;
        let mut osc = PolyBlepOsc::new(f0, 1.0);
        osc.set_shape(WaveShape::Square);
        let buf = render(&mut osc, sr, n);
        let mag = magnitude_spectrum(&buf);
        let fund = mag[bin_for_hz(f0, n, sr)];
        let h2 = mag[bin_for_hz(f0 * 2.0, n, sr)];
        let h3 = mag[bin_for_hz(f0 * 3.0, n, sr)];
        assert!(db(h2) - db(fund) < -60.0, "even harmonic present at {:.1} dB", db(h2) - db(fund));
        assert!(
            (db(h3) - db(fund) + 9.5).abs() < 1.0,
            "3rd harmonic {:.1} dB, expected -9.5",
            db(h3) - db(fund)
        );
    }

    #[test]
    fn pulse_width_sets_the_harmonic_amplitudes() {
        // A Fourier-series pulse train is AC by construction: it has no DC term, so its mean is 0
        // for every duty cycle. (The first version of this test asserted the mean tracked
        // `2*pw - 1`, which is what a *naive* ±1 pulse would do; it measured 0 for all four duty
        // cycles and was wrong, not the oscillator.) The real invariant is the harmonic law
        //     b_k = (2 / (k*pi)) * sin(pi * k * pw)
        // which vanishes at k = 1/pw — so a duty cycle of 0.25 must have no 4th harmonic, while a
        // duty cycle of 0.1 must not.
        let sr = 96_000u32;
        let n = 32_768;
        let f0 = 1500.0; // 512 exact periods in the frame -> no window leakage
        for pw in [0.1f64, 0.25, 0.5, 0.75] {
            let mut osc = PolyBlepOsc::new(f0, 1.0);
            osc.set_shape(WaveShape::Square);
            osc.set_pulse_width(pw);
            let buf = render(&mut osc, sr, n);
            assert!(buf.iter().all(|s| s.is_finite()));
            let mag = magnitude_spectrum(&buf);
            let fund = mag[bin_for_hz(f0, n, sr)];
            assert!(fund > 1e-4, "pw {pw}: fundamental vanished");
            // DC must be absent for every duty cycle.
            let dc = buf.iter().map(|&s| f64::from(s)).sum::<f64>() / n as f64;
            assert!(dc.abs() < 1e-4, "pw {pw}: unexpected DC {dc}");
            // Check three harmonics against the analytic law.
            for k in [2usize, 3, 5] {
                let expected = (2.0 / (k as f64 * std::f64::consts::PI)
                    * (std::f64::consts::PI * k as f64 * pw).sin())
                .abs();
                let got = mag[bin_for_hz(f0 * k as f64, n, sr)] / fund
                    * (2.0 / (1.0 * std::f64::consts::PI)
                        * (std::f64::consts::PI * 1.0f64 * pw).sin())
                    .abs();
                // Relative comparison against the k=1 law avoids depending on the overall scale.
                let ratio_expected = expected
                    / (2.0 / (1.0 * std::f64::consts::PI) * (std::f64::consts::PI * pw).sin())
                        .abs();
                let ratio_got = mag[bin_for_hz(f0 * k as f64, n, sr)] / fund;
                let _ = got;
                if ratio_expected < 1e-3 {
                    assert!(
                        ratio_got < 0.02,
                        "pw {pw}: harmonic {k} should vanish, got {ratio_got:.4}"
                    );
                } else {
                    assert!(
                        (ratio_got - ratio_expected).abs() < 0.06 * ratio_expected + 0.005,
                        "pw {pw}: harmonic {k} ratio {ratio_got:.4}, expected {ratio_expected:.4}"
                    );
                }
            }
        }
        // And the specific vanishing case: pw = 0.25 kills every 4th harmonic.
        let mut osc = PolyBlepOsc::new(f0, 1.0);
        osc.set_shape(WaveShape::Square);
        osc.set_pulse_width(0.25);
        let buf = render(&mut osc, sr, n);
        let mag = magnitude_spectrum(&buf);
        let fund = mag[bin_for_hz(f0, n, sr)];
        for k in [4usize, 8, 12] {
            let h = mag[bin_for_hz(f0 * k as f64, n, sr)];
            assert!(
                db(h) - db(fund) < -50.0,
                "pw 0.25: harmonic {k} should vanish, got {:.1} dB",
                db(h) - db(fund)
            );
        }
    }

    #[test]
    fn frequency_is_exact_by_zero_crossings() {
        let sr = 48_000u32;
        for f in [55.0f64, 220.0, 440.0, 4_000.0] {
            let mut osc = PolyBlepOsc::new(f, 1.0);
            osc.set_shape(WaveShape::Saw);
            let buf = render(&mut osc, sr, sr as usize);
            let crossings = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!(
                (crossings as f64 - f).abs() <= 2.0,
                "{f} Hz: {crossings} rising crossings in 1 s"
            );
        }
    }

    #[test]
    fn output_stays_bounded_for_every_shape() {
        for shape in [WaveShape::Sine, WaveShape::Saw, WaveShape::Square] {
            for f in [20.0f64, 440.0, 8_000.0] {
                let mut osc = PolyBlepOsc::new(f, 1.0);
                osc.shape = shape;
                let buf = render(&mut osc, 48_000, 960);
                let peak = buf.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
                assert!(peak <= 1.5, "{shape:?} at {f} Hz peaked at {peak}");
                assert!(buf.iter().all(|s| s.is_finite()), "{shape:?} non-finite output");
            }
        }
    }

    #[test]
    fn sine_is_exact() {
        // 937.5 Hz at 48 kHz over 8192 samples is exactly 160 periods, so the mean of a correct
        // sine is zero to floating-point error. (1000 Hz would be 170.67 periods and a non-zero
        // mean would be the *correct* answer -- the first version of this test used it and failed.)
        let sr = 48_000u32;
        let n = 8192;
        let mut osc = PolyBlepOsc::new(937.5, 1.0);
        osc.set_shape(WaveShape::Sine);
        let buf = render(&mut osc, sr, n);
        let peak = buf.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!((peak - 1.0).abs() < 0.01, "unit sine peaked at {peak}");
        let mean = buf.iter().map(|&s| f64::from(s)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 1e-4, "sine has DC offset {mean}");
        assert_eq!(osc.partial_count(), 0, "a sine needs no partial table");
    }

    #[test]
    fn partial_count_tracks_the_nyquist_limit() {
        let mut osc = PolyBlepOsc::new(1000.0, 1.0);
        osc.set_shape(WaveShape::Saw);
        osc.prepare(48_000);
        assert_eq!(osc.partial_count(), 24, "Nyquist/f0 = 24000/1000 = 24 partials");
        osc.set_freq(100.0);
        assert!(osc.partial_count() > 200, "a 100 Hz saw needs many more partials");
        osc.set_freq(20_000.0);
        assert_eq!(osc.partial_count(), 1, "a 20 kHz saw at 48 kHz has one partial");
    }

    #[test]
    fn hard_sync_resets_the_phase() {
        let sr = 48_000u32;
        let mut osc = PolyBlepOsc::new(100.0, 1.0);
        osc.set_shape(WaveShape::Saw);
        osc.sync = Sync::Reset;
        osc.prepare(sr);
        let mut max_phase = 0.0f64;
        let mut wraps = 0u32;
        for i in 0..sr as usize {
            let _ = osc.tick(i % 100 == 0);
            max_phase = max_phase.max(osc.phase());
            if osc.wrapped_on_last_tick() {
                wraps += 1;
            }
        }
        // 100 Hz is 480 samples/period; resetting every 100 caps the phase at 100/480.
        assert!(max_phase < 0.35, "sync did not reset the phase: max phase {max_phase}");
        assert_eq!(wraps, 0, "a synced oscillator should never complete its own period");
    }

    #[test]
    fn phase_stays_wrapped_over_a_long_render() {
        let mut osc = PolyBlepOsc::new(440.0, 1.0);
        osc.set_shape(WaveShape::Saw);
        osc.prepare(192_000);
        for _ in 0..60_000 {
            let _ = osc.tick_free();
            let p = osc.phase();
            assert!((0.0..1.0).contains(&p), "phase escaped [0,1): {p}");
        }
    }

    #[test]
    fn per_block_frequency_changes_stay_bounded() {
        let sr = 48_000u32;
        let mut osc = PolyBlepOsc::new(220.0, 1.0);
        osc.set_shape(WaveShape::Saw);
        osc.prepare(sr);
        let mut peak = 0.0f32;
        for block in 0..200u32 {
            osc.set_freq(110.0 * f64::from(block % 7 + 1));
            for _ in 0..64 {
                peak = peak.max(osc.tick_free().abs());
            }
        }
        assert!(peak < 1.5, "frequency sweep produced a peak of {peak}");
    }

    #[test]
    fn set_phase_wraps_into_range_and_round_trips() {
        let mut o = PolyBlepOsc::new(440.0, 1.0);
        o.prepare(48_000);
        o.set_phase(0.25);
        assert_eq!(o.phase(), 0.25);
        o.set_phase(1.25);
        assert!((o.phase() - 0.25).abs() < 1e-12, "wraps down into [0, 1)");
        o.set_phase(-0.25);
        assert!((o.phase() - 0.75).abs() < 1e-12, "negative wraps up");
    }
}
