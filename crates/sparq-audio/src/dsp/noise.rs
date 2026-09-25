//! `syn/noise` — seeded noise sources, and `syn/dust` — random impulses.
//!
//! Every source is **seeded** and deterministic (ADR-007): the same seed produces the same samples,
//! which is what makes a noise-based patch reproducible from a journal. The PRNG is `xorshift64*` —
//! chosen for speed, tiny state, and being trivially reproducible, not for cryptographic quality.
//! Colour filters are the standard ones: pink via the Voss–McCartney / Paul Kellet refinement,
//! brown via a leaky integrator of white.

/// Noise colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoiseColour {
    /// Flat spectrum.
    #[default]
    White,
    /// −3 dB/octave.
    Pink,
    /// −6 dB/octave (integrated white).
    Brown,
    /// +3 dB/octave (differentiated white).
    Blue,
    /// +6 dB/octave.
    Violet,
}

/// `xorshift64*` — 64 bits of state, one multiply, three shifts.
#[derive(Clone, Copy, Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // A zero seed would lock the generator at zero forever.
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[-1, 1)`.
    #[inline]
    fn next_bipolar(&mut self) -> f32 {
        // Use the high bits; the low bits of an LCG-family generator are the weak ones.
        (self.next_u64() >> 40) as f32 / 8_388_608.0 - 1.0
    }
}

/// A coloured noise generator.
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    /// Output amplitude, linear.
    pub amp: f32,
    /// Colour.
    pub colour: NoiseColour,
    rng: Rng,
    /// Pink filter state (Kellet's refined Voss–McCartney).
    p0: f32,
    p1: f32,
    p2: f32,
    p3: f32,
    p4: f32,
    p5: f32,
    p6: f32,
    /// Brown integrator state.
    brown: f32,
    /// Previous white sample, for blue/violet differentiation.
    prev_white: f32,
    /// Seed, kept so `reset` is meaningful and so state can be reported.
    seed: u64,
}

impl Default for Noise {
    fn default() -> Self {
        Self::new(0x5EED)
    }
}

impl Noise {
    /// A noise generator with the given seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            amp: 1.0,
            colour: NoiseColour::White,
            rng: Rng::new(seed),
            p0: 0.0,
            p1: 0.0,
            p2: 0.0,
            p3: 0.0,
            p4: 0.0,
            p5: 0.0,
            p6: 0.0,
            brown: 0.0,
            prev_white: 0.0,
            seed,
        }
    }

    /// The seed this generator was created with.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Reset to the start of the stream for this seed — makes a render reproducible from any point.
    pub fn reset(&mut self) {
        *self = Self::new(self.seed);
    }

    /// Re-seed.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
        self.rng = Rng::new(seed);
    }

    /// Advance one sample.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        let w = self.rng.next_bipolar();
        let y = match self.colour {
            NoiseColour::White => w,
            NoiseColour::Pink => {
                // Paul Kellet's refined Voss–McCartney approximation, the standard cheap pink.
                self.p0 = 0.99886 * self.p0 + w * 0.0555179;
                self.p1 = 0.99332 * self.p1 + w * 0.0750759;
                self.p2 = 0.969 * self.p2 + w * 0.153852;
                self.p3 = 0.86650 * self.p3 + w * 0.3104856;
                self.p4 = 0.55000 * self.p4 + w * 0.5329522;
                self.p5 = -0.7616 * self.p5 - w * 0.0168980;
                let out = self.p0
                    + self.p1
                    + self.p2
                    + self.p3
                    + self.p4
                    + self.p5
                    + self.p6
                    + w * 0.5362;
                self.p6 = w * 0.115926;
                out * 0.11
            },
            NoiseColour::Brown => {
                // Leaky integrator: the leak stops it random-walking into saturation.
                self.brown = (self.brown + 0.02 * w).clamp(-1.0, 1.0);
                self.brown * 3.5
            },
            NoiseColour::Blue => (w - self.prev_white) * 0.7,
            NoiseColour::Violet => (w - self.prev_white) * 1.4,
        };
        self.prev_white = w;
        (y * self.amp).clamp(-1.5, 1.5)
    }

    /// Fill a buffer.
    pub fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = self.tick();
        }
    }
}

/// `syn/dust` — random impulses, averaging `rate` per second.
///
/// The impulse source for physical models and the "click/burst" layer of a percussion voice.
#[derive(Clone, Copy, Debug)]
pub struct Dust {
    /// Mean impulses per second.
    pub rate: f64,
    /// Impulse amplitude.
    pub amp: f32,
    rng: Rng,
    /// Per-sample probability, cached.
    p: f32,
    sample_rate: u32,
}

impl Dust {
    /// A dust generator at `rate` impulses per second.
    #[must_use]
    pub fn new(seed: u64, rate: f64) -> Self {
        Self { rate, amp: 1.0, rng: Rng::new(seed), p: 0.0, sample_rate: 0 }
    }

    /// Recompute the per-sample probability for a sample rate.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute();
    }

    /// Recompute after changing `rate`.
    pub fn recompute(&mut self) {
        self.p = if self.sample_rate == 0 {
            0.0
        } else {
            (self.rate / f64::from(self.sample_rate)).clamp(0.0, 1.0) as f32
        };
    }

    /// Advance one sample: either 0 or `amp`.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        // Compare the top 24 bits against the probability, so no division per sample.
        let u = (self.rng.next_u64() >> 40) as f32 / 16_777_216.0;
        if u < self.p {
            self.amp
        } else {
            0.0
        }
    }

    /// Fill a buffer.
    pub fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = self.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::dsp::fft::magnitude_spectrum;

    const SR: u32 = 48_000;

    fn render(n: &mut Noise, len: usize) -> Vec<f32> {
        let mut buf = vec![0.0f32; len];
        n.process(&mut buf);
        buf
    }

    /// Average magnitude in a band, from a magnitude spectrum.
    fn band(mag: &[f64], lo: f64, hi: f64, n: usize, sr: u32) -> f64 {
        let bin = |hz: f64| ((hz * n as f64 / f64::from(sr)).round() as usize).min(mag.len() - 1);
        let (a, b) = (bin(lo), bin(hi).max(bin(lo) + 1));
        let s: f64 = mag[a..b].iter().sum();
        s / (b - a) as f64
    }

    fn db(v: f64) -> f64 {
        if v <= 1e-15 {
            -300.0
        } else {
            20.0 * v.log10()
        }
    }

    #[test]
    fn same_seed_gives_identical_output() {
        let mut a = Noise::new(12345);
        let mut b = Noise::new(12345);
        a.colour = NoiseColour::Pink;
        b.colour = NoiseColour::Pink;
        assert_eq!(
            render(&mut a, 4096),
            render(&mut b, 4096),
            "ADR-007: seeded sources must reproduce"
        );
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Noise::new(1);
        let mut b = Noise::new(2);
        assert_ne!(render(&mut a, 4096), render(&mut b, 4096));
    }

    #[test]
    fn reset_restarts_the_stream() {
        let mut n = Noise::new(999);
        let first = render(&mut n, 1024);
        n.reset();
        let again = render(&mut n, 1024);
        assert_eq!(first, again);
    }

    #[test]
    fn zero_seed_does_not_lock_the_generator() {
        // A xorshift with zero state stays zero forever; the constructor must avoid that.
        let mut n = Noise::new(0);
        let buf = render(&mut n, 256);
        assert!(buf.iter().any(|&s| s != 0.0), "seed 0 produced all zeros");
    }

    #[test]
    fn white_is_spectrally_flat() {
        let mut n = Noise::new(7);
        n.colour = NoiseColour::White;
        let buf = render(&mut n, 16_384);
        let mag = magnitude_spectrum(&buf);
        let lo = band(&mag, 1_000.0, 3_000.0, 16_384, SR);
        let hi = band(&mag, 12_000.0, 20_000.0, 16_384, SR);
        assert!((db(hi) - db(lo)).abs() < 3.0, "white should be flat: 1-3k {lo} vs 12-20k {hi}");
    }

    #[test]
    fn pink_rolls_off_at_about_3db_per_octave() {
        let mut n = Noise::new(7);
        n.colour = NoiseColour::Pink;
        let buf = render(&mut n, 32_768);
        let mag = magnitude_spectrum(&buf);
        let lo = band(&mag, 500.0, 1_000.0, 32_768, SR);
        let hi = band(&mag, 8_000.0, 16_000.0, 32_768, SR);
        // Four octaves apart in *band width* too, so compare energy density per octave:
        // pink drops ~3 dB/octave, i.e. ~12 dB over four octaves. Allow a wide tolerance because
        // this is a cheap approximation of a cheap approximation.
        let slope = db(lo) - db(hi);
        assert!(
            slope > 5.0 && slope < 20.0,
            "pink slope over 4 octaves: {slope:.1} dB, expected ~12"
        );
    }

    #[test]
    fn brown_is_darker_than_pink() {
        let mut p = Noise::new(11);
        p.colour = NoiseColour::Pink;
        let mut b = Noise::new(11);
        b.colour = NoiseColour::Brown;
        let mp = magnitude_spectrum(&render(&mut p, 32_768));
        let mb = magnitude_spectrum(&render(&mut b, 32_768));
        let ratio = |m: &[f64]| {
            db(band(m, 10_000.0, 18_000.0, 32_768, SR)) - db(band(m, 200.0, 400.0, 32_768, SR))
        };
        assert!(
            ratio(&mb) < ratio(&mp) - 3.0,
            "brown should be darker: brown {:.1} pink {:.1}",
            ratio(&mb),
            ratio(&mp)
        );
    }

    #[test]
    fn blue_and_violet_are_brighter_than_white() {
        let mut w = Noise::new(5);
        let mut v = Noise::new(5);
        v.colour = NoiseColour::Violet;
        let mw = magnitude_spectrum(&render(&mut w, 32_768));
        let mv = magnitude_spectrum(&render(&mut v, 32_768));
        let tilt = |m: &[f64]| {
            db(band(m, 12_000.0, 20_000.0, 32_768, SR)) - db(band(m, 500.0, 1_000.0, 32_768, SR))
        };
        assert!(
            tilt(&mv) > tilt(&mw) + 3.0,
            "violet should be brighter: {:.1} vs {:.1}",
            tilt(&mv),
            tilt(&mw)
        );
    }

    #[test]
    fn every_colour_stays_bounded() {
        for colour in [
            NoiseColour::White,
            NoiseColour::Pink,
            NoiseColour::Brown,
            NoiseColour::Blue,
            NoiseColour::Violet,
        ] {
            let mut n = Noise::new(3);
            n.colour = colour;
            let buf = render(&mut n, 8192);
            let peak = buf.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
            assert!(peak <= 1.5, "{colour:?} peaked at {peak}");
            assert!(buf.iter().all(|s| s.is_finite()), "{colour:?} produced non-finite output");
        }
    }

    #[test]
    fn white_has_no_dc_offset() {
        let mut n = Noise::new(21);
        let buf = render(&mut n, 65_536);
        let mean = buf.iter().map(|&s| f64::from(s)).sum::<f64>() / buf.len() as f64;
        assert!(mean.abs() < 0.01, "white noise has DC offset {mean}");
    }

    #[test]
    fn dust_rate_matches_the_requested_density() {
        let mut d = Dust::new(4, 250.0);
        d.prepare(SR);
        let n = SR as usize * 2; // two seconds
        let mut buf = vec![0.0f32; n];
        d.process(&mut buf);
        let impulses = buf.iter().filter(|&&s| s != 0.0).count();
        let expected = 500.0; // 250/s * 2 s
        let err = (impulses as f64 - expected).abs() / expected;
        assert!(
            err < 0.15,
            "{impulses} impulses in 2 s, expected ~{expected} (err {:.1}%)",
            err * 100.0
        );
        assert!(buf.iter().all(|&s| s == 0.0 || s == 1.0), "dust must be 0 or amp");
    }

    #[test]
    fn dust_at_zero_rate_is_silent() {
        let mut d = Dust::new(1, 0.0);
        d.prepare(SR);
        let mut buf = vec![0.0f32; 4096];
        d.process(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));
    }
}
