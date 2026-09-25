//! `util/delay` — a feedback delay line with damping and dry/wet mixing.
//!
//! The buffer is allocated in [`DelayLine::prepare`] and never grown in `process`, per plan §5.2.
//! Read position is fractional and linearly interpolated, so delay time can be modulated at audio
//! rate without the zipper noise an integer tap produces.
//!
//! Feedback is taken *after* the damping filter, which is what makes a long tail darken the way a
//! real space does rather than ringing forever at one brightness.
//!
//! **Not yet here** (WO-014 / Phase 2): a feedback-delay *network* for reverb, modulation of the
//! delay time by an LFO as a first-class parameter, multitap, and tempo-sync derivation from the
//! transport clock. The tempo-sync maths lives in the patch layer for now so this node stays a pure
//! time-domain processor with no clock dependency.

/// A feedback delay line.
#[derive(Clone, Debug)]
pub struct DelayLine {
    /// Delay time in seconds.
    time: f64,
    /// Feedback amount, `0..=0.99`. Clamped below 1 so the tail always terminates.
    pub feedback: f32,
    /// Damping: one-pole lowpass coefficient applied in the feedback path, `0..=1` (0 = dark, 1 = bright).
    pub damp: f32,
    /// Dry/wet mix, `0..=1` (0 = fully dry, 1 = fully wet).
    pub mix: f32,
    /// Output amplitude.
    pub amp: f32,
    buf: Vec<f32>,
    /// Write index.
    write: usize,
    capacity: usize,
    sample_rate: u32,
    /// Cached fractional read position in samples.
    read_pos: f64,
    /// Damping filter state.
    damp_state: f32,
    /// Cached damping coefficient.
    damp_coeff: f32,
    /// Longest delay this instance can represent, in seconds.
    max_time: f64,
}

impl DelayLine {
    /// A delay line with a maximum delay time of `max_time` seconds.
    ///
    /// The maximum is fixed at construction because it determines the buffer size, and the buffer
    /// must not be reallocated on the audio path.
    #[must_use]
    pub fn new(max_time: f64) -> Self {
        Self {
            time: 0.25,
            feedback: 0.3,
            damp: 0.5,
            mix: 0.5,
            amp: 1.0,
            buf: Vec::new(),
            write: 0,
            capacity: 0,
            sample_rate: 0,
            read_pos: 0.0,
            damp_state: 0.0,
            damp_coeff: 1.0,
            max_time: max_time.max(0.001),
        }
    }

    /// Allocate the buffer for a sample rate and recompute coefficients. **Must** be called before
    /// the first `tick`; until then the line outputs the dry signal only.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        let n = ((self.max_time * f64::from(sample_rate)).ceil() as usize).max(2) + 2;
        if self.buf.len() != n {
            self.buf = vec![0.0; n];
        }
        self.capacity = n;
        self.write = 0;
        self.recompute();
    }

    /// Recompute the cached read position and damping coefficient.
    pub fn recompute(&mut self) {
        if self.sample_rate == 0 || self.capacity == 0 {
            return;
        }
        self.time = self.time.clamp(0.0, self.max_time);
        self.read_pos = self.time * f64::from(self.sample_rate);
        // damp = 1 -> transparent, damp = 0 -> one-pole at about 400 Hz.
        let cutoff = 400.0 + (self.damp.clamp(0.0, 1.0) as f64) * 16_000.0;
        let rc = 1.0 / (std::f64::consts::TAU * cutoff);
        let dt = 1.0 / f64::from(self.sample_rate);
        self.damp_coeff = (dt / (rc + dt)) as f32;
    }

    /// Set the delay time in seconds.
    pub fn set_time(&mut self, seconds: f64) {
        self.time = seconds.max(0.0);
        self.recompute();
    }

    /// Set the delay time from a tempo: `beats` at `bpm`, divided by `divisor`.
    ///
    /// Provided here rather than in the patch layer so a delay can be tempo-synced without the node
    /// needing a clock: the caller supplies the bpm.
    pub fn set_tempo_sync(&mut self, bpm: f64, beats: f64, divisor: f64) {
        if bpm <= 0.0 || divisor <= 0.0 {
            return;
        }
        let seconds = (60.0 / bpm) * beats / divisor;
        self.set_time(seconds);
    }

    /// Current delay time in seconds.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Buffer size in bytes, for the memory diagnostics panel.
    #[must_use]
    pub fn buffer_bytes(&self) -> usize {
        self.buf.len() * std::mem::size_of::<f32>()
    }

    /// Clear the buffer (scene change, patch load).
    pub fn reset(&mut self) {
        for s in self.buf.iter_mut() {
            *s = 0.0;
        }
        self.damp_state = 0.0;
        self.write = 0;
    }

    /// Process one sample.
    #[inline]
    pub fn tick(&mut self, input: f32) -> f32 {
        if self.capacity == 0 {
            return input;
        }
        // Fractional read with linear interpolation.
        let pos = self.read_pos;
        let idx = pos.floor();
        let frac = (pos - idx) as f32;
        let cap = self.capacity;
        // The read position trails the write position by `pos` samples.
        let i1 = (self.write + cap).wrapping_sub(idx as usize % cap) % cap;
        let i0 = (i1 + 1) % cap;
        let s1 = self.buf[i1];
        let s0 = self.buf[i0];
        let delayed = s1 + frac * (s0 - s1);

        // Write the input plus the damped feedback.
        self.damp_state += self.damp_coeff * (delayed - self.damp_state);
        let fb = self.damp_state * self.feedback.clamp(0.0, 0.99);
        self.buf[self.write] = input + fb;
        self.write = (self.write + 1) % cap;

        let mix = self.mix.clamp(0.0, 1.0);
        (input * (1.0 - mix) + delayed * mix) * self.amp
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

    const SR: u32 = 48_000;

    fn impulse() -> Vec<f32> {
        let mut v = vec![0.0f32; SR as usize];
        v[0] = 1.0;
        v
    }

    /// Indices whose magnitude exceeds `threshold`.
    fn peaks_above(buf: &[f32], threshold: f32) -> Vec<usize> {
        buf.iter().enumerate().filter(|(_, &v)| v.abs() > threshold).map(|(i, _)| i).collect()
    }

    #[test]
    fn the_first_echo_arrives_at_the_configured_delay() {
        let mut d = DelayLine::new(1.0);
        d.feedback = 0.0;
        d.mix = 1.0;
        d.prepare(SR);
        d.set_time(0.125);
        let mut buf = impulse();
        d.process(&mut buf);
        let p = peaks_above(&buf, 0.5);
        // mix = 1 means fully wet: there is NO dry component, so the impulse must appear only at
        // the delay time. (The first version of this test asserted a dry peak too, and was wrong.)
        let expected = (0.125 * f64::from(SR)) as usize;
        assert!(!p.is_empty(), "no echo appeared at all");
        assert!(
            p.iter().any(|&i| i.abs_diff(expected) <= 2),
            "echo should land at sample {expected}, peaks were at {p:?}"
        );
        assert!(
            !p.iter().any(|&i| i < expected - 2),
            "energy appeared before the delay time: {p:?}"
        );
    }

    #[test]
    fn feedback_produces_a_decaying_series_of_echoes() {
        let mut d = DelayLine::new(1.0);
        d.feedback = 0.5;
        d.mix = 1.0;
        d.damp = 1.0; // transparent, so the decay is purely from feedback
        d.prepare(SR);
        d.set_time(0.020);
        let mut buf = impulse();
        d.process(&mut buf);
        let step = (0.020 * f64::from(SR)) as usize;
        let mut amps = Vec::new();
        for k in 1..5 {
            let centre = step * k;
            let window = &buf[centre.saturating_sub(2)..(centre + 3).min(buf.len())];
            amps.push(window.iter().fold(0.0f32, |a, &v| a.max(v.abs())));
        }
        for w in amps.windows(2) {
            assert!(w[1] < w[0] * 0.9, "echo series should decay: {amps:?}");
            assert!(w[1] > w[0] * 0.2, "decay is far steeper than feedback=0.5 implies: {amps:?}");
        }
    }

    #[test]
    fn feedback_is_clamped_so_the_tail_always_terminates() {
        let mut d = DelayLine::new(0.5);
        d.feedback = 5.0; // nonsense value: must be clamped, not allowed to run away
        d.mix = 1.0;
        d.prepare(SR);
        d.set_time(0.010);
        let mut buf = impulse();
        d.process(&mut buf);
        let peak = buf.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(peak < 10.0, "runaway feedback: peak {peak}");
        assert!(buf.iter().all(|s| s.is_finite()));
        // The very end must be quiet: the tail terminated.
        let tail = &buf[buf.len() - 1024..];
        let tail_peak = tail.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(tail_peak < 0.05, "tail did not terminate: {tail_peak}");
    }

    #[test]
    fn damping_darkens_later_echoes() {
        let measure = |damp: f32| {
            let mut d = DelayLine::new(1.0);
            d.feedback = 0.7;
            d.mix = 1.0;
            d.damp = damp;
            d.prepare(SR);
            d.set_time(0.020);
            // Excite with a bright burst rather than an impulse so brightness is measurable.
            let mut buf: Vec<f32> = (0..SR as usize)
                .map(|i| if i < 200 { ((i as f32 * 1.7).sin()) * 0.8 } else { 0.0 })
                .collect();
            d.process(&mut buf);
            // High-frequency energy in the last third vs the first third.
            let hf = |sl: &[f32]| {
                sl.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / sl.len() as f32
            };
            let n = buf.len();
            hf(&buf[2 * n / 3..]) / hf(&buf[..n / 3]).max(1e-9)
        };
        let bright = measure(1.0);
        let dark = measure(0.0);
        assert!(
            dark < bright * 0.6,
            "damping should darken the tail: bright {bright}, dark {dark}"
        );
    }

    #[test]
    fn mix_zero_is_transparent() {
        let mut d = DelayLine::new(1.0);
        d.mix = 0.0;
        d.prepare(SR);
        let x: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let mut y = x.clone();
        d.process(&mut y);
        let err = x.iter().zip(y.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(err < 1e-6, "mix=0 should pass the dry signal unchanged, max error {err}");
    }

    #[test]
    fn tempo_sync_computes_the_right_time() {
        let mut d = DelayLine::new(2.0);
        d.prepare(SR);
        d.set_tempo_sync(120.0, 1.0, 4.0); // one sixteenth at 120 bpm = 0.125 s
        assert!((d.time() - 0.125).abs() < 1e-9, "got {}", d.time());
        d.set_tempo_sync(138.42, 1.0, 3.0); // one triplet eighth
        let expected = (60.0 / 138.42) / 3.0;
        assert!((d.time() - expected).abs() < 1e-9, "got {} expected {expected}", d.time());
        d.set_tempo_sync(0.0, 1.0, 4.0); // must not divide by zero
        assert!(d.time().is_finite());
    }

    #[test]
    fn fractional_delay_time_does_not_click() {
        let mut d = DelayLine::new(1.0);
        d.feedback = 0.0;
        d.mix = 1.0;
        d.prepare(SR);
        // A delay time that is not a whole number of samples: interpolation must handle it.
        d.set_time(0.00123456);
        let mut prev = 0.0f32;
        let mut max_step = 0.0f32;
        for i in 0..4096 {
            let x = (i as f32 * 0.03).sin() * 0.5;
            let y = d.tick(x);
            max_step = max_step.max((y - prev).abs());
            prev = y;
        }
        assert!(max_step < 0.2, "fractional delay produced a step of {max_step}");
    }

    #[test]
    fn buffer_is_not_reallocated_during_process() {
        let mut d = DelayLine::new(1.0);
        d.prepare(SR);
        let ptr_before = d.buf.as_ptr();
        let len_before = d.buf.len();
        d.set_time(0.5);
        let mut buf = vec![0.0f32; 4096];
        d.process(&mut buf);
        d.set_time(0.9);
        d.process(&mut buf);
        assert_eq!(d.buf.as_ptr(), ptr_before, "process must not reallocate (plan §5.2)");
        assert_eq!(d.buf.len(), len_before);
        assert!(d.buffer_bytes() > 0);
    }

    #[test]
    fn reset_clears_the_tail() {
        let mut d = DelayLine::new(1.0);
        d.feedback = 0.8;
        d.mix = 1.0;
        d.prepare(SR);
        d.set_time(0.01);
        let mut buf = impulse();
        for _ in 0..200 {
            d.process(&mut buf);
        }
        d.reset();
        let mut after = vec![0.0f32; 4096];
        d.process(&mut after);
        assert!(after.iter().all(|&s| s == 0.0), "reset left audio in the buffer");
    }

    #[test]
    fn delay_is_deterministic() {
        let run = || {
            let mut d = DelayLine::new(1.0);
            d.feedback = 0.4;
            d.damp = 0.6;
            d.prepare(SR);
            d.set_time(0.037);
            let mut buf: Vec<f32> = (0..8192).map(|i| (i as f32 * 0.011).sin() * 0.4).collect();
            d.process(&mut buf);
            buf
        };
        assert_eq!(run(), run());
    }
}
