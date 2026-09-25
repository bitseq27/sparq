//! `fx/bitcrush` — bit-depth and sample-rate reduction.
//!
//! Two independent degradations, both musical:
//!
//! * **Depth reduction** quantises the sample to `bits` bits. The quantisation step is what gives
//!   the granular, digital texture; at 1–4 bits it becomes a waveform in its own right.
//! * **Rate reduction** holds each sample for `div` input samples (zero-order hold), which adds
//!   images at multiples of the reduced rate.
//!
//! Both generate broadband aliasing *by design*, so unlike every other node in this crate the
//! aliasing test asserts that it is present, and the optional anti-alias filter is provided for the
//! cases where you want the texture without the fizz. Plan §9.1 makes anti-aliasing mandatory for
//! synthesis; an effect whose entire purpose is aliasing is the documented exception, and the
//! exception is a switch the user can see.

/// Bit-depth / sample-rate reduction effect.
#[derive(Clone, Copy, Debug)]
pub struct BitCrush {
    /// Bit depth, `1..=24`. 24 is effectively transparent.
    pub bits: u32,
    /// Integer sample-rate divider, `1..=512`. 1 means no rate reduction.
    pub div: u32,
    /// Add rectangular dither (±½ LSB) before quantising, which trades noise for linearity.
    pub dither: bool,
    /// Apply a one-pole lowpass after crushing to tame the images.
    pub anti_alias: bool,
    /// Anti-alias coefficient, derived from `aa_amount` (`0..=1`).
    aa_amount: f32,
    /// Output amplitude, linear.
    pub amp: f32,
    /// Dither PRNG state — separate from any synth noise so crushing is independently reproducible.
    rng: u64,
    /// Zero-order hold state.
    held: f32,
    hold_counter: u32,
    /// One-pole state for the anti-alias filter.
    aa_state: f32,
    aa_coeff: f32,
    sample_rate: u32,
}

impl Default for BitCrush {
    fn default() -> Self {
        Self::new(12, 1)
    }
}

impl BitCrush {
    /// A crusher at `bits` bits and rate divider `div`.
    #[must_use]
    pub fn new(bits: u32, div: u32) -> Self {
        Self {
            bits: bits.clamp(1, 24),
            div: div.clamp(1, 512),
            dither: false,
            anti_alias: false,
            aa_amount: 0.0,
            amp: 1.0,
            rng: 0x2545_F491_4F6C_DD1D,
            held: 0.0,
            hold_counter: 0,
            aa_state: 0.0,
            aa_coeff: 1.0,
            sample_rate: 0,
        }
    }

    /// Recompute rate-dependent coefficients.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute();
    }

    /// Recompute after changing `bits`, `div` or `aa_amount`.
    pub fn recompute(&mut self) {
        self.bits = self.bits.clamp(1, 24);
        self.div = self.div.clamp(1, 512);
        // One-pole lowpass: aa_amount 0 -> transparent, 1 -> very dark.
        let cutoff = 20_000.0 * (1.0 - self.aa_amount.clamp(0.0, 1.0) * 0.97);
        let sr = self.sample_rate.max(1) as f32;
        let rc = 1.0 / (std::f32::consts::TAU * cutoff);
        let dt = 1.0 / sr;
        self.aa_coeff = dt / (rc + dt);
    }

    /// Set the anti-alias amount, `0..=1`.
    pub fn set_anti_alias(&mut self, amount: f32) {
        self.aa_amount = amount.clamp(0.0, 1.0);
        self.anti_alias = self.aa_amount > 0.0;
        self.recompute();
    }

    /// Set the dither seed.
    pub fn set_seed(&mut self, seed: u64) {
        self.rng = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
    }

    /// Reset the hold and filter state.
    pub fn reset(&mut self) {
        self.held = 0.0;
        self.hold_counter = 0;
        self.aa_state = 0.0;
    }

    #[inline]
    fn next_dither(&mut self) -> f32 {
        // xorshift64*; cheap and reproducible.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        // Already in [-1, 1); the caller scales it to ±0.5 LSB.
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / 8_388_608.0 - 1.0
    }

    /// Process one sample.
    #[inline]
    pub fn tick(&mut self, input: f32) -> f32 {
        // Sample-rate reduction: zero-order hold.
        let sample = if self.div > 1 {
            if self.hold_counter == 0 {
                self.held = input;
            }
            self.hold_counter = (self.hold_counter + 1) % self.div;
            self.held
        } else {
            input
        };

        // Bit-depth reduction.
        let bits = self.bits.max(1);
        let levels = (1u64 << bits) as f32;
        let step = 2.0 / levels; // full scale is [-1, 1]
        let mut x = sample.clamp(-1.0, 1.0);
        if self.dither {
            x += self.next_dither() * step * 0.5;
        }
        // Round-to-nearest in units of `step`, then back.
        let q = (x / step).round() * step;
        let mut y = q.clamp(-1.0, 1.0);

        if self.anti_alias {
            self.aa_state += self.aa_coeff * (y - self.aa_state);
            y = self.aa_state;
        }

        y * self.amp
    }

    /// Process a buffer in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for s in buf.iter_mut() {
            *s = self.tick(*s);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::dsp::fft::{bin_for_hz, magnitude_spectrum};

    const SR: u32 = 48_000;

    fn sine(f0: f64, n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                ((2.0 * std::f64::consts::PI * f0 * i as f64 / f64::from(SR)).sin() as f32) * amp
            })
            .collect()
    }

    fn db(v: f64) -> f64 {
        if v <= 1e-15 {
            -300.0
        } else {
            20.0 * v.log10()
        }
    }

    #[test]
    fn high_bit_depth_is_transparent() {
        let mut c = BitCrush::new(24, 1);
        c.prepare(SR);
        let x = sine(1000.0, 4096, 0.9);
        let mut y = x.clone();
        c.process(&mut y);
        let max_err = x.iter().zip(y.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(max_err < 1e-6, "24-bit crush should be transparent, max error {max_err}");
    }

    #[test]
    fn low_bit_depth_produces_exactly_the_expected_number_of_levels() {
        for bits in [1u32, 2, 3, 4, 8] {
            let mut c = BitCrush::new(bits, 1);
            c.prepare(SR);
            let x = sine(97.0, SR as usize, 0.99); // a slow sweep through the whole range
            let mut y = x.clone();
            c.process(&mut y);
            let levels: std::collections::BTreeSet<i32> =
                y.iter().map(|&v| v.to_bits() as i32).collect();
            let expected = (1usize << bits) + 1; // inclusive of both endpoints
            assert!(
                levels.len() <= expected + 1,
                "{bits} bits produced {} distinct output values, expected <= {}",
                levels.len(),
                expected + 1
            );
            assert!(
                levels.len() >= bits as usize,
                "{bits} bits produced only {} levels",
                levels.len()
            );
        }
    }

    #[test]
    fn one_bit_becomes_a_square_wave() {
        let mut c = BitCrush::new(1, 1);
        c.prepare(SR);
        let x = sine(1000.0, 4096, 0.9);
        let mut y = x.clone();
        c.process(&mut y);
        // Only two distinct magnitudes should appear.
        let mut mags: Vec<f32> = y.iter().map(|v| v.abs()).collect();
        mags.sort_by(|a, b| a.partial_cmp(b).unwrap());
        mags.dedup();
        assert!(mags.len() <= 2, "1-bit crush should give two levels, got {:?}", mags);
    }

    #[test]
    fn rate_reduction_holds_samples() {
        let div = 8u32;
        let mut c = BitCrush::new(24, div);
        c.prepare(SR);
        let x = sine(500.0, 1024, 0.9);
        let mut y = x.clone();
        c.process(&mut y);
        // Every run of `div` output samples must be identical.
        for chunk in y.chunks_exact(div as usize) {
            assert!(chunk.windows(2).all(|w| w[0] == w[1]), "zero-order hold broke: {chunk:?}");
        }
    }

    #[test]
    fn rate_reduction_creates_the_expected_images() {
        // At div = 8 the effective rate is 6 kHz, so images appear around multiples of 6 kHz.
        let div = 8u32;
        let mut c = BitCrush::new(24, div);
        c.prepare(SR);
        let n = 16_384;
        let x = sine(1000.0, n, 0.9);
        let mut y = x.clone();
        c.process(&mut y);
        let mag = magnitude_spectrum(&y);
        let reduced = f64::from(SR) / f64::from(div);
        let image = mag[bin_for_hz(reduced - 1000.0, n, SR)];
        let fund = mag[bin_for_hz(1000.0, n, SR)];
        assert!(
            db(image) - db(fund) > -30.0,
            "expected a strong image near {:.0} Hz, got {:.1} dB rel fundamental",
            reduced - 1000.0,
            db(image) - db(fund)
        );
    }

    #[test]
    fn crushing_aliases_by_design_and_the_test_knows_it() {
        // Plan §9.1 forbids aliasing in *synthesis*. This node's purpose is aliasing, so the
        // assertion is the opposite of every other spectral test in the crate — and that inversion
        // is the point of writing it down.
        let mut c = BitCrush::new(4, 1);
        c.prepare(SR);
        let n = 16_384;
        let x = sine(1000.0, n, 0.9);
        let mut y = x.clone();
        c.process(&mut y);
        let mag = magnitude_spectrum(&y);
        let fund = mag[bin_for_hz(1000.0, n, SR)];
        let hi: f64 = mag[bin_for_hz(10_000.0, n, SR)..].iter().fold(0.0, |a, &v| a.max(v));
        assert!(
            db(hi) - db(fund) > -40.0,
            "4-bit crush should spray broadband energy; measured {:.1} dB rel fundamental",
            db(hi) - db(fund)
        );
    }

    #[test]
    fn anti_alias_reduces_the_high_frequency_spray() {
        let n = 16_384;
        let x = sine(1000.0, n, 0.9);
        let measure = |aa: f32| {
            let mut c = BitCrush::new(4, 4);
            c.prepare(SR);
            c.set_anti_alias(aa);
            let mut y = x.clone();
            c.process(&mut y);
            let mag = magnitude_spectrum(&y);
            let fund = mag[bin_for_hz(1000.0, n, SR)];
            let hi: f64 = mag[bin_for_hz(14_000.0, n, SR)..].iter().fold(0.0, |a, &v| a.max(v));
            db(hi) - db(fund)
        };
        let dry = measure(0.0);
        let filtered = measure(0.9);
        assert!(
            filtered < dry - 6.0,
            "anti-alias should darken the spray: {dry:.1} -> {filtered:.1} dB"
        );
    }

    #[test]
    fn dither_changes_the_output_but_not_the_level() {
        let x = sine(1000.0, 8192, 0.5);
        let run = |dither: bool| {
            let mut c = BitCrush::new(6, 1);
            c.dither = dither;
            c.set_seed(1234);
            c.prepare(SR);
            let mut y = x.clone();
            c.process(&mut y);
            y
        };
        let a = run(false);
        let b = run(true);
        assert_ne!(a, b, "dither should change the quantised output");
        let rms = |v: &[f32]| {
            (v.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>() / v.len() as f64).sqrt()
        };
        assert!(
            (rms(&a) - rms(&b)).abs() < 0.02,
            "dither should not change the level much: {} vs {}",
            rms(&a),
            rms(&b)
        );
    }

    #[test]
    fn dither_is_reproducible_from_the_seed() {
        let x = sine(1000.0, 4096, 0.5);
        let run = || {
            let mut c = BitCrush::new(5, 1);
            c.dither = true;
            c.set_seed(4242);
            c.prepare(SR);
            let mut y = x.clone();
            c.process(&mut y);
            y
        };
        assert_eq!(run(), run(), "seeded dither must be deterministic (ADR-007)");
    }

    #[test]
    fn output_stays_bounded_for_extreme_settings() {
        for bits in [1u32, 2, 24] {
            for div in [1u32, 3, 512] {
                let mut c = BitCrush::new(bits, div);
                c.prepare(SR);
                let x = sine(3_000.0, 4096, 1.0);
                let mut y = x.clone();
                c.process(&mut y);
                let peak = y.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
                assert!(peak <= 1.5, "bits={bits} div={div} peaked at {peak}");
                assert!(y.iter().all(|s| s.is_finite()));
            }
        }
    }

    #[test]
    fn silence_in_silence_out() {
        let mut c = BitCrush::new(4, 8);
        c.prepare(SR);
        let mut buf = [0.0f32; 1024];
        c.process(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));
    }
}
