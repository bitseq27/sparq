//! The three-clock model (ADR-006): `t_sample` is master, `t_musical` and `t_wall` are derived.
//!
//! Phase 0 implements the sample↔musical mapping at **constant tempo**, which is all WO-005 needs.
//! The piecewise-linear tempo map with a smoothed derivative (so tempo automation never
//! discontinues) is WO-009 and will replace [`Clock::tick_at_sample`] with a segment lookup while
//! keeping this type's public surface.

/// Musical clock derived from the sample clock.
///
/// Constructed once per stream; cheap to copy. All arithmetic is integer where possible so that
/// the mapping is exact and reproducible (ADR-007).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    sample_rate: u32,
    /// Ticks per quarter note (960 by default).
    pub ppqn: u32,
    /// Tempo in beats per minute.
    pub bpm: f64,
    /// Ticks accumulated before `sample_offset_origin` (for metric modulation later).
    tick_origin: u64,
    sample_origin: u64,
    /// Ticks per sample, cached: `bpm * ppqn / (60 * sample_rate)`.
    ticks_per_sample: f64,
}

impl Clock {
    /// A clock at constant tempo starting at sample zero, tick zero.
    #[must_use]
    pub fn new(sample_rate: u32, bpm: f64, ppqn: u32) -> Self {
        let ticks_per_sample = bpm * f64::from(ppqn) / (60.0 * f64::from(sample_rate));
        Self { sample_rate, ppqn, bpm, tick_origin: 0, sample_origin: 0, ticks_per_sample }
    }

    /// Device sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Seconds per tick.
    #[must_use]
    pub fn seconds_per_tick(&self) -> f64 {
        if self.ticks_per_sample == 0.0 {
            f64::INFINITY
        } else {
            1.0 / self.ticks_per_sample
        }
    }

    /// Samples per beat at the current tempo.
    #[must_use]
    pub fn samples_per_beat(&self) -> f64 {
        60.0 * f64::from(self.sample_rate) / self.bpm
    }

    /// Tick position at an absolute sample index.
    ///
    /// Exact at the origin; the fractional part is accumulated in `f64`, which holds integer
    /// precision to 2^53 ticks — about 149 years at 960 PPQN and 138 bpm.
    #[must_use]
    pub fn tick_at_sample(&self, sample: u64) -> u64 {
        let delta = sample.saturating_sub(self.sample_origin) as f64;
        self.tick_origin + (delta * self.ticks_per_sample).round() as u64
    }

    /// Sample index at which a tick occurs (rounded to the nearest sample).
    #[must_use]
    pub fn sample_at_tick(&self, tick: u64) -> u64 {
        if self.ticks_per_sample == 0.0 {
            return self.sample_origin;
        }
        let delta = tick.saturating_sub(self.tick_origin) as f64;
        self.sample_origin + (delta / self.ticks_per_sample).round() as u64
    }

    /// `bar.beat.tick` view of a tick position, given a beats-per-bar count.
    #[must_use]
    pub fn bar_beat_tick(&self, tick: u64, beats_per_bar: u64) -> (u64, u64, u64) {
        let ticks_per_beat = u64::from(self.ppqn);
        let beats = tick / ticks_per_beat;
        let remainder = tick % ticks_per_beat;
        let bars = beats / beats_per_bar.max(1);
        (bars + 1, beats % beats_per_bar.max(1) + 1, remainder)
    }

    /// A clock re-anchored at a sample/tick pair — the primitive metric modulation will use.
    #[must_use]
    pub fn reanchored(&self, sample: u64, tick: u64, bpm: f64) -> Self {
        let mut next = *self;
        next.sample_origin = sample;
        next.tick_origin = tick;
        next.bpm = bpm;
        next.ticks_per_sample = bpm * f64::from(self.ppqn) / (60.0 * f64::from(self.sample_rate));
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_beat_at_120bpm_48k_is_24000_samples() {
        let c = Clock::new(48_000, 120.0, 960);
        assert!((c.samples_per_beat() - 24_000.0).abs() < 1e-9);
        assert_eq!(c.sample_at_tick(960), 24_000);
    }

    #[test]
    fn tick_and_sample_are_inverse_at_beat_boundaries() {
        let c = Clock::new(96_000, 138.42, 960);
        for beat in 0..64u64 {
            let tick = beat * 960;
            let sample = c.sample_at_tick(tick);
            let back = c.tick_at_sample(sample);
            assert!(
                back.abs_diff(tick) <= 1,
                "beat {beat}: tick {tick} -> sample {sample} -> tick {back}"
            );
        }
    }

    #[test]
    fn bar_beat_tick_view_is_one_based() {
        let c = Clock::new(48_000, 120.0, 960);
        assert_eq!(c.bar_beat_tick(0, 4), (1, 1, 0));
        assert_eq!(c.bar_beat_tick(960, 4), (1, 2, 0));
        assert_eq!(c.bar_beat_tick(4 * 960, 4), (2, 1, 0));
        assert_eq!(c.bar_beat_tick(960 + 240, 4), (1, 2, 240));
    }

    #[test]
    fn reanchor_preserves_position() {
        let c = Clock::new(48_000, 120.0, 960);
        let at = 48_000u64;
        let tick = c.tick_at_sample(at);
        let r = c.reanchored(at, tick, 120.0);
        assert_eq!(r.tick_at_sample(at), tick);
        assert_eq!(r.tick_at_sample(at + 24_000), tick + 960);
    }
}
