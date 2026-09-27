//! The `ClockBroker` (ADR-006): one owner for `t_sample ↔ t_musical ↔ t_wall`.
//!
//! The musical half is the kernel's piecewise [`Clock`] — the broker adds what a *stream* needs
//! around it:
//!
//! * **the position** — where the stream is (`sample_pos`), advanced per block, jumped by
//!   `locate`. `t_sample` is the master: nothing here can move it except the driver.
//! * **tempo changes anchored at the position** — [`ClockBroker::set_tempo`] turns "120 → 140"
//!   into a map segment anchored at `(sample_pos, tick_at(sample_pos))` with a ramp in samples,
//!   so the caller never computes an anchor (and cannot get one wrong).
//! * **the wall half** — [`WallEstimator`]: `observe_wall` pairs the current position with a
//!   monotonic microsecond reading; the estimator reports drift in ppm (windowed ratio,
//!   smoothed) and answers `wall_at_sample`. The wall clock is NOT deterministic and never
//!   enters a render or a journal replay — it is diagnostics and the future external-sync PLL's
//!   input (Phase 4). Observations that would move the wall map backwards are REFUSED and
//!   counted, never applied: a non-monotone OS clock is a fact about the machine, not a licence
//!   to corrupt the map.

use sparq_kernel::clock::Clock;

/// The wall-clock side: a smoothed rate estimator between `(sample, wall_us)` observations.
///
/// Deliberately simple and honest about its window: the instantaneous estimate is the ratio
/// over everything since the anchor, the published ppm is a one-pole smoothing of it
/// (α = 0.1), and the anchor slides every [`WallEstimator::REANCHOR_EVERY`] observations so a
/// long session's estimate tracks slow machine changes instead of averaging them away.
#[derive(Clone, Debug)]
struct WallEstimator {
    anchor: Option<(u64, u64)>, // (sample, wall_us)
    ppm: f64,
    n_obs: u64,
    since_anchor: u64,
    us_per_sample: f64,
    refusals: u64,
}

impl WallEstimator {
    /// Observations between anchor slides.
    const REANCHOR_EVERY: u64 = 1024;
    /// One-pole coefficient for the published estimate.
    const ALPHA: f64 = 0.1;

    fn new(sample_rate: u32) -> Self {
        Self {
            anchor: None,
            ppm: 0.0,
            n_obs: 0,
            since_anchor: 0,
            us_per_sample: 1_000_000.0 / f64::from(sample_rate),
            refusals: 0,
        }
    }

    fn observe(&mut self, sample: u64, wall_us: u64) {
        let Some((anchor_s, anchor_w)) = self.anchor else {
            self.anchor = Some((sample, wall_us));
            self.since_anchor = 0;
            return;
        };
        if sample <= anchor_s {
            // No span (or a locate moved us back): no information, and refusing beats dividing.
            self.refusals += 1;
            return;
        }
        let expected = (sample - anchor_s) as f64 * self.us_per_sample;
        if wall_us < anchor_w {
            // The machine's monotonic clock just wasn't. Count it, re-anchor, keep the estimate:
            // one bad reading is the OS; the map never sees it.
            self.refusals += 1;
            self.anchor = Some((sample, wall_us));
            self.since_anchor = 0;
            return;
        }
        let actual = (wall_us - anchor_w) as f64;
        let inst_ppm = (actual / expected - 1.0) * 1e6;
        self.ppm =
            if self.n_obs == 0 { inst_ppm } else { self.ppm + (inst_ppm - self.ppm) * Self::ALPHA };
        self.n_obs += 1;
        self.since_anchor += 1;
        if self.since_anchor >= Self::REANCHOR_EVERY {
            self.anchor = Some((sample, wall_us));
            self.since_anchor = 0;
        }
    }

    fn wall_at_sample(&self, sample: u64) -> Option<u64> {
        let (anchor_s, anchor_w) = self.anchor?;
        let scaled =
            (sample as f64 - anchor_s as f64) * self.us_per_sample * (1.0 + self.ppm * 1e-6);
        let w = anchor_w as f64 + scaled;
        if w < 0.0 || !w.is_finite() {
            return None;
        }
        Some(w.round() as u64)
    }

    fn reset(&mut self) {
        self.anchor = None;
        self.ppm = 0.0;
        self.n_obs = 0;
        self.since_anchor = 0;
        // refusals deliberately survive: they are the machine's record, not the window's.
    }
}

/// The three-clock broker: the kernel [`Clock`] plus the stream position and the wall side.
///
/// One per stream (the ADR says *a single ClockBroker*), owned by whatever drives the blocks —
/// the offline render loop, the HAL pump, or a [`crate::transport::Transport`] (which wraps one
/// and is the usual way to hold it).
#[derive(Clone, Debug)]
pub struct ClockBroker {
    clock: Clock,
    sample_pos: u64,
    wall: WallEstimator,
}

impl ClockBroker {
    /// A broker at constant tempo, positioned at sample zero.
    #[must_use]
    pub fn new(sample_rate: u32, bpm: f64, ppqn: u32) -> Self {
        Self {
            clock: Clock::new(sample_rate, bpm, ppqn),
            sample_pos: 0,
            wall: WallEstimator::new(sample_rate),
        }
    }

    /// The underlying map — the executor's `set_clock` takes a clone of this.
    #[must_use]
    pub const fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Where the stream is, in samples (the master clock's reading).
    #[must_use]
    pub const fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Ticks per quarter note.
    #[must_use]
    pub const fn ppqn(&self) -> u32 {
        self.clock.ppqn
    }

    /// Device sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.clock.sample_rate()
    }

    /// Advance the stream by `frames`; returns the new position. The driver calls this exactly
    /// once per rendered block — the position is the stream's, not the patch's.
    pub fn advance(&mut self, frames: usize) -> u64 {
        self.sample_pos += frames as u64;
        self.sample_pos
    }

    /// Jump the stream position (offline seek). The wall map re-anchors: observations across a
    /// locate compare two different streams, and an estimator that cannot tell is worse than
    /// one that restarts.
    pub fn locate(&mut self, sample: u64) {
        self.sample_pos = sample;
        self.wall.reset();
    }

    /// Tick position at an absolute sample (the map, f64).
    #[must_use]
    pub fn tick_at_sample_f64(&self, sample: u64) -> f64 {
        self.clock.tick_at_sample_f64(sample)
    }

    /// Tick position at an absolute sample, rounded.
    #[must_use]
    pub fn tick_at_sample(&self, sample: u64) -> u64 {
        self.clock.tick_at_sample(sample)
    }

    /// The sample at which a tick occurs — ±0 (ADR-006 rule 7; the kernel bisects).
    #[must_use]
    pub fn sample_at_tick(&self, tick: u64) -> u64 {
        self.clock.sample_at_tick(tick)
    }

    /// The effective tempo at the current position (mid-ramp, this is between the targets).
    #[must_use]
    pub fn bpm_now(&self) -> f64 {
        self.clock.effective_bpm_at(self.sample_pos)
    }

    /// Anchor a tempo change at the CURRENT position, gliding over `ramp_ms`.
    ///
    /// Returns `false` (map untouched) when the tempo is not a positive finite number — the
    /// kernel refuses, and the broker passes the refusal through rather than clamping: a tempo
    /// of zero is not slow, it is a mistake, and mistakes get answers, not silence.
    ///
    /// The ramp is the smoothed-derivative policy: `ramp_ms = 0` is a step (position still
    /// continuous — the map can never jump), and the transport's default is non-zero because
    /// tempo-synced DSP hears a step as a zipper.
    pub fn set_tempo(&mut self, bpm: f64, ramp_ms: f64) -> bool {
        let ramp_samples = if ramp_ms <= 0.0 {
            0
        } else {
            (f64::from(self.clock.sample_rate()) * ramp_ms / 1000.0).round() as u64
        };
        self.clock.set_tempo(bpm, self.sample_pos, ramp_samples)
    }

    /// Pair the current position with a monotonic wall reading (µs). Control-side; the audio
    /// path may not read a wall clock (clippy's RT denials), so the PUMP feeds this between
    /// blocks — which is also exactly the cadence the drift estimator wants.
    pub fn observe_wall(&mut self, wall_us: u64) {
        self.wall.observe(self.sample_pos, wall_us);
    }

    /// The wall time the estimator places at `sample`; `None` before the first observation.
    #[must_use]
    pub fn wall_at_sample(&self, sample: u64) -> Option<u64> {
        self.wall.wall_at_sample(sample)
    }

    /// The smoothed drift estimate in ppm: positive means the wall clock runs FASTER than the
    /// sample clock (the device is slow, or the wall is fast — the distinction is the machine's,
    /// and the journal records which clock said what).
    #[must_use]
    pub fn drift_ppm(&self) -> f64 {
        self.wall.ppm
    }

    /// Wall observations refused (non-monotone readings or degenerate spans) — counted, never
    /// silently dropped, and never applied to the map.
    #[must_use]
    pub fn wall_refusals(&self) -> u64 {
        self.wall.refusals
    }

    /// How many wall observations the estimator has accepted.
    #[must_use]
    pub fn wall_observations(&self) -> u64 {
        self.wall.n_obs
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_broker_anchors_tempo_changes_at_the_position() {
        let mut b = ClockBroker::new(48_000, 120.0, 960);
        b.advance(24_000); // one beat in
        let tick_here = b.tick_at_sample(24_000);
        assert_eq!(tick_here, 960);
        assert!(b.set_tempo(60.0, 0.0));
        // Position continuity at the anchor: the tick at 24000 is what it always was.
        assert_eq!(b.tick_at_sample(24_000), tick_here);
        // Rate halved after it: one more beat now takes twice as long.
        assert_eq!(b.tick_at_sample(24_000 + 48_000), tick_here + 960);
        assert!((b.bpm_now() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn bad_tempos_refuse_and_leave_no_trace() {
        let mut b = ClockBroker::new(48_000, 120.0, 960);
        b.advance(100);
        let before = b.clock().clone();
        assert!(!b.set_tempo(0.0, 10.0));
        assert!(!b.set_tempo(-120.0, 10.0));
        assert!(!b.set_tempo(f64::INFINITY, 10.0));
        assert_eq!(b.clock(), &before, "a refusal moves nothing");
    }

    #[test]
    fn the_drift_estimator_converges_on_a_synthetic_1000ppm_skew() {
        let mut b = ClockBroker::new(48_000, 120.0, 960);
        let us_per_sample = 1_000_000.0 / 48_000.0;
        // The wall runs 1000 ppm FAST relative to the sample clock.
        for i in 0..400u64 {
            b.advance(64);
            let wall = ((b.sample_pos() as f64) * us_per_sample * 1.001).round() as u64;
            let _ = i;
            b.observe_wall(wall);
        }
        assert!(
            (b.drift_ppm() - 1000.0).abs() < 2.0,
            "estimator says {} ppm against a synthetic 1000",
            b.drift_ppm()
        );
        // The wall map is monotone and lands near the synthetic truth.
        let w1 = b.wall_at_sample(100_000).unwrap();
        let w2 = b.wall_at_sample(200_000).unwrap();
        assert!(w2 > w1, "monotone");
        let expect = (100_000.0 * us_per_sample * 1.001) as i64;
        assert!((w2 as i64 - w1 as i64 - expect).abs() < 100, "span within 100 µs of truth");
    }

    #[test]
    fn a_backwards_wall_reading_is_refused_counted_and_never_applied() {
        let mut b = ClockBroker::new(48_000, 120.0, 960);
        b.advance(64);
        b.observe_wall(1_000_000);
        b.advance(64);
        b.observe_wall(999_000); // the machine lied
        assert_eq!(b.wall_refusals(), 1, "counted");
        assert_eq!(b.wall_observations(), 0, "and not folded into the estimate");
        b.advance(64);
        b.observe_wall(1_002_000);
        assert!(b.wall_at_sample(b.sample_pos()).is_some(), "the map recovered from the re-anchor");
    }

    #[test]
    fn locate_reanchors_the_wall_side_and_moves_only_the_position() {
        let mut b = ClockBroker::new(48_000, 120.0, 960);
        b.advance(48_000);
        b.observe_wall(1_000_000);
        b.locate(0);
        assert_eq!(b.sample_pos(), 0);
        assert!(b.wall_at_sample(0).is_none(), "the wall map restarts at a locate");
        assert_eq!(b.tick_at_sample(24_000), 960, "the musical map is position-independent");
    }
}
