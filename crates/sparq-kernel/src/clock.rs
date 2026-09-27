//! The three-clock model's musical half (ADR-006): `t_sample` is master, `t_musical` is derived.
//!
//! Phase 0 shipped the constant-tempo map. **WO-009 makes it the piecewise map ADR-006 rule 2
//! demands**: the timeline is a list of segments, each anchored at an exact `(sample, tick)` pair
//! with a tempo that GLIDES linearly from the rate it inherited to its target over a declared
//! ramp — a piecewise-linear map with a *smoothed derivative*, so a tempo change never produces a
//! discontinuity in either direction:
//!
//! * **position** is continuous by anchoring: a new segment starts at the exact tick the map
//!   already had at its sample, so `tick_at_sample` never jumps;
//! * **derivative** is continuous by inheritance: a new segment's ramp starts at the EFFECTIVE
//!   ticks-per-sample at its anchor — mid-ramp changes glide from the current rate instead of
//!   stepping to the new segment's.
//!
//! Inside a ramp the tick integral is quadratic in closed form (`∫ tps₀ + (tps₁−tps₀)·d/r`), so
//! `tick_at_sample` stays O(log n) exact float arithmetic — f64 holds integer precision to 2^53
//! ticks, ~149 years at 960 PPQN and 138 bpm. `sample_at_tick` (the event-scheduling direction,
//! where ADR-006 rule 7 demands ±0 samples) is an **exact integer bisection** on the monotone
//! map with nearest-sample rounding — inversion by search, not by float algebra, because a
//! rounding rule that disagrees with `tick_at_sample`'s would put events one sample off and
//! nobody would hear it until a journal replay diverged. A single-segment constant clock takes
//! the v0 closed-form path bit-for-bit (the phase-b goldens were rendered through it).
//!
//! `t_wall` is not here: the wall side of the broker (drift estimation) lives in
//! `sparq-music::broker`, because the kernel must not know what a wall clock is for — this type
//! is pure, deterministic map arithmetic, which is what makes replay reproducible (ADR-007).

/// One tempo segment: from `start_sample` (at `start_tick`), the rate glides linearly from
/// `tps_from` to `tps_to` over `ramp` samples, then holds. `ramp == 0` is a constant segment
/// (`tps_from == tps_to` by construction).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    start_sample: u64,
    start_tick: f64,
    /// Ticks per sample at the segment start.
    tps_from: f64,
    /// Ticks per sample after the ramp.
    tps_to: f64,
    /// Ramp length in samples; 0 = constant.
    ramp: u64,
    /// `start_tick + ∫ ramp` — precomputed so the post-ramp branch is one multiply.
    tick_at_ramp_end: f64,
}

impl Segment {
    fn new(start_sample: u64, start_tick: f64, tps_from: f64, tps_to: f64, ramp: u64) -> Self {
        let r = ramp as f64;
        let tick_at_ramp_end = if ramp == 0 {
            start_tick
        } else {
            // ∫₀ʳ tps₀ + (tps₁−tps₀)·x/r dx = tps₀·r + (tps₁−tps₀)·r/2
            start_tick + tps_from * r + (tps_to - tps_from) * r / 2.0
        };
        Self { start_sample, start_tick, tps_from, tps_to, ramp, tick_at_ramp_end }
    }

    /// Tick position at absolute sample `s` (which must be ≥ `start_sample` for the integral to
    /// mean what it says; the saturating sub keeps a mis-call monotone rather than negative).
    fn ticks_at(&self, s: u64) -> f64 {
        let d = s.saturating_sub(self.start_sample) as f64;
        if self.ramp == 0 {
            return self.start_tick + d * self.tps_to;
        }
        let r = self.ramp as f64;
        if d <= r {
            // ∫₀ᵈ tps₀ + (tps₁−tps₀)·x/r dx
            self.start_tick + self.tps_from * d + (self.tps_to - self.tps_from) * d * d / (2.0 * r)
        } else {
            self.tick_at_ramp_end + self.tps_to * (d - r)
        }
    }

    /// Effective ticks per sample at `s` (the map's derivative — what "smoothed" means here).
    fn tps_at(&self, s: u64) -> f64 {
        let d = s.saturating_sub(self.start_sample) as f64;
        if self.ramp == 0 {
            return self.tps_to;
        }
        let r = self.ramp as f64;
        if d <= r {
            self.tps_from + (self.tps_to - self.tps_from) * (d / r)
        } else {
            self.tps_to
        }
    }
}

/// Musical clock derived from the sample clock: a piecewise tempo map (see the module header).
///
/// Constructed once per stream; cheap to clone. All arithmetic is f64 over integer sample
/// indices so the mapping is exact and reproducible (ADR-007). **WO-009 surface note:** the type
/// was `Copy` while it was a five-field constant map; segments make it `Clone` — every consumer
/// in the tree held it by value-once-and-reference-thereafter (verified when the change landed),
/// and `BlockContext::advance` always took `&Clock`.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    sample_rate: u32,
    /// Ticks per quarter note (960 by default).
    pub ppqn: u32,
    /// The CURRENT TARGET tempo: the last segment's `tps_to`, in bpm. Mid-ramp, the effective
    /// tempo is between the previous target and this one — [`Clock::effective_bpm_at`] reads it.
    pub bpm: f64,
    /// The map. Invariant: `start_sample` strictly increases (equal anchors REPLACE), every
    /// segment's `start_tick` equals the map's value at its `start_sample` (position continuity),
    /// and every `tps_from` equals the previous segment's effective tps there (derivative
    /// continuity). `set_tempo` is the only way in, and it maintains all three.
    segments: Vec<Segment>,
}

/// Ticks per sample for a tempo.
fn tps_of(bpm: f64, ppqn: u32, sample_rate: u32) -> f64 {
    bpm * f64::from(ppqn) / (60.0 * f64::from(sample_rate))
}

impl Clock {
    /// A clock at constant tempo starting at sample zero, tick zero.
    #[must_use]
    pub fn new(sample_rate: u32, bpm: f64, ppqn: u32) -> Self {
        let tps = tps_of(bpm, ppqn, sample_rate);
        Self { sample_rate, ppqn, bpm, segments: vec![Segment::new(0, 0.0, tps, tps, 0)] }
    }

    /// Device sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Seconds per tick at the current target tempo.
    #[must_use]
    pub fn seconds_per_tick(&self) -> f64 {
        let tps = tps_of(self.bpm, self.ppqn, self.sample_rate);
        if tps == 0.0 {
            f64::INFINITY
        } else {
            1.0 / tps
        }
    }

    /// Samples per beat at the current target tempo.
    #[must_use]
    pub fn samples_per_beat(&self) -> f64 {
        60.0 * f64::from(self.sample_rate) / self.bpm
    }

    /// The segment covering `sample` (the last one starting at or before it).
    fn segment_at(&self, sample: u64) -> &Segment {
        let idx = self.segments.partition_point(|s| s.start_sample <= sample);
        &self.segments[idx.saturating_sub(1)]
    }

    /// Tick position at an absolute sample index, exact f64 (the map itself).
    #[must_use]
    pub fn tick_at_sample_f64(&self, sample: u64) -> f64 {
        self.segment_at(sample).ticks_at(sample)
    }

    /// Tick position at an absolute sample index, rounded to the nearest tick.
    ///
    /// Exact at every anchor; the fractional part is accumulated in `f64`, which holds integer
    /// precision to 2^53 ticks — about 149 years at 960 PPQN and 138 bpm.
    #[must_use]
    pub fn tick_at_sample(&self, sample: u64) -> u64 {
        let t = self.tick_at_sample_f64(sample);
        if t <= 0.0 {
            0
        } else {
            t.round() as u64
        }
    }

    /// Sample index at which a tick occurs — the event-scheduling direction, where ADR-006
    /// rule 7 holds the line at ±0 samples.
    ///
    /// A single constant segment (the v0 shape, and what the phase-b goldens rendered through)
    /// takes the closed-form path bit-for-bit. Anything piecewise is an exact integer bisection
    /// for the nearest sample: the smallest `s` with `tick_f(s) ≥ t`, then whichever of `s`/`s−1`
    /// is closer (ties round UP, matching `f64::round`'s half-away-from-zero in the forward
    /// direction, so the two directions agree at every boundary).
    #[must_use]
    pub fn sample_at_tick(&self, tick: u64) -> u64 {
        let t = tick as f64;
        let first = &self.segments[0];
        if t <= first.start_tick {
            return first.start_sample;
        }
        // The constant-map fast path: one segment, no ramp — the v0 closed form, bit-identical.
        if self.segments.len() == 1 && first.ramp == 0 && first.tps_to > 0.0 {
            let delta = t - first.start_tick;
            return first.start_sample + (delta / first.tps_to).round() as u64;
        }
        // Find the segment whose tick range contains t (start_tick increases with the map).
        let si = self.segments.partition_point(|s| s.start_tick <= t).saturating_sub(1);
        let seg = &self.segments[si];
        let lo = seg.start_sample;
        // Upper bound: generous — the slowest rate in this segment, plus the next anchor if any.
        let tps_lo = seg.tps_from.min(seg.tps_to).max(1e-12);
        let mut hi = lo + ((t - seg.start_tick) / tps_lo).ceil() as u64 + 2;
        if let Some(next) = self.segments.get(si + 1) {
            // t is below the next segment's start_tick only if the search is exact; cap anyway.
            hi = hi.min(next.start_sample.saturating_add(1));
        }
        // Smallest s in [lo, hi] with ticks_at(s) >= t.
        let (mut a, mut b) = (lo, hi);
        while a < b {
            let m = a + (b - a) / 2;
            if seg.ticks_at(m) >= t {
                b = m;
            } else {
                a = m + 1;
            }
        }
        // Nearest of {a-1, a}, ties up (a-1 only when it exists and is strictly closer).
        if a > lo {
            let below = t - seg.ticks_at(a - 1);
            let above = seg.ticks_at(a) - t;
            if below < above {
                return a - 1;
            }
        }
        a
    }

    /// The effective tempo at a sample — mid-ramp, this is between the two targets (the
    /// derivative of the map, in musical units).
    #[must_use]
    pub fn effective_bpm_at(&self, sample: u64) -> f64 {
        let tps = self.segment_at(sample).tps_at(sample);
        tps * 60.0 * f64::from(self.sample_rate) / f64::from(self.ppqn)
    }

    /// Anchor a tempo change at `at_sample`, gliding to `bpm` over `ramp_samples`.
    ///
    /// Returns `false` (changing nothing) when the anchor is in the past — a segment whose start
    /// precedes the last one's would make the map ambiguous, and a stale tempo message must not
    /// rewrite history — or when `bpm` is not a positive finite number. An anchor EQUAL to the
    /// last segment's start REPLACES it (scrubbing a tempo control must not stack zero-length
    /// segments). Allocation happens here, on the control thread, never on the audio path.
    pub fn set_tempo(&mut self, bpm: f64, at_sample: u64, ramp_samples: u64) -> bool {
        if bpm <= 0.0 || !bpm.is_finite() {
            // NaN fails both halves — refused either way
            return false;
        }
        let last_start = self.segments.last().map_or(0, |s| s.start_sample);
        if at_sample < last_start {
            return false;
        }
        let anchor_tick = self.tick_at_sample_f64(at_sample);
        let tps_from = self.segment_at(at_sample).tps_at(at_sample);
        let tps_to = tps_of(bpm, self.ppqn, self.sample_rate);
        let seg = Segment::new(at_sample, anchor_tick, tps_from, tps_to, ramp_samples);
        if at_sample == last_start {
            if let Some(last) = self.segments.last_mut() {
                *last = seg;
            }
        } else {
            self.segments.push(seg);
        }
        self.bpm = bpm;
        true
    }

    /// How many tempo segments the map holds (diagnostics; the map is the journal's truth).
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
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

    /// A clock re-anchored at a sample/tick pair, continuing at `bpm` — the primitive metric
    /// modulation and transport locate use. Segments after `at_sample` are dropped (the future is
    /// being redefined); the history before it stays, so `tick_at_sample` of an older sample is
    /// unchanged — a journal replay of the PAST still agrees.
    #[must_use]
    pub fn reanchored(&self, sample: u64, tick: u64, bpm: f64) -> Self {
        let mut next = self.clone();
        next.segments.retain(|s| s.start_sample <= sample);
        if next.segments.is_empty() {
            next.segments.push(Segment::new(0, 0.0, 0.0, 0.0, 0));
        }
        let tps = tps_of(bpm.max(1e-9), next.ppqn, next.sample_rate);
        let seg = Segment::new(sample, tick as f64, tps, tps, 0);
        if next.segments.last().is_some_and(|s| s.start_sample == sample) {
            if let Some(last) = next.segments.last_mut() {
                *last = seg;
            }
        } else {
            next.segments.push(seg);
        }
        next.bpm = bpm;
        next
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

    // ---------------------------------------------------- WO-009: the piecewise map

    #[test]
    fn a_tempo_change_never_moves_position_only_rate() {
        // ADR-006 rule 2, the position half: the tick at the anchor is the same from both
        // segments' arithmetic, and the map on either side runs at its own rate.
        let mut c = Clock::new(48_000, 120.0, 960);
        let at = 100_000u64;
        let before = c.tick_at_sample(at);
        assert!(c.set_tempo(180.0, at, 0));
        assert_eq!(c.tick_at_sample(at), before, "the anchor did not move the position");
        // 120 bpm @48k = 0.04 ticks/sample; 180 = 0.06.
        assert_eq!(c.tick_at_sample(at + 1_000) as i64 - before as i64, 60);
        assert_eq!(c.tick_at_sample(at) as i64 - c.tick_at_sample(at - 1_000) as i64, 40);
        assert!((c.effective_bpm_at(at - 1) - 120.0).abs() < 1e-9);
        assert!((c.effective_bpm_at(at + 1) - 180.0).abs() < 1e-9);
    }

    #[test]
    fn a_ramp_glides_the_derivative_linearly() {
        // The smoothed-derivative half: across the ramp, the per-sample tick delta interpolates
        // tps(120) → tps(240) LINEARLY — the second difference is zero inside the ramp and the
        // first difference is continuous at both ends.
        let mut c = Clock::new(48_000, 120.0, 960);
        let (at, ramp) = (48_000u64, 4_800u64); // 100 ms ramp
        assert!(c.set_tempo(240.0, at, ramp));
        let tps_lo = 120.0 * 960.0 / (60.0 * 48_000.0);
        let tps_hi = 240.0 * 960.0 / (60.0 * 48_000.0);
        let delta = |s: u64| c.tick_at_sample_f64(s + 1) - c.tick_at_sample_f64(s);
        assert!((delta(at - 100) - tps_lo).abs() < 1e-12, "pre-ramp runs at the old rate");
        assert!((delta(at + ramp + 100) - tps_hi).abs() < 1e-12, "post-ramp at the new");
        // Linearity — with the finite-difference midpoint term stated, not hand-waved: a
        // one-sample difference across a linear ramp is the rate at the sample MIDPOINT, i.e.
        // tps(s) + inc/2. Getting this wrong is how a correct map fails a sloppy test (this
        // test's first draft made exactly that error and the map was right).
        let inc = (tps_hi - tps_lo) / ramp as f64;
        for (frac, want) in [(0.25, 0.25), (0.5, 0.5), (0.75, 0.75)] {
            let s = at + (ramp as f64 * frac) as u64;
            let expect = tps_lo + (tps_hi - tps_lo) * want + inc / 2.0;
            assert!(
                (delta(s) - expect).abs() < 1e-9,
                "delta at frac {frac}: {} vs {expect}",
                delta(s)
            );
        }
        // Derivative continuity at the ramp entry: the instantaneous rate does not jump (the
        // ramp starts AT the inherited rate), so the boundary-straddling difference moves by
        // exactly HALF an increment, and the next one by a full increment.
        assert!((delta(at) - delta(at - 1) - inc / 2.0).abs() < 1e-12, "entry: half increment");
        assert!((delta(at + 1) - delta(at) - inc).abs() < 1e-12, "inside: full increments");
        // The exit mirrors the entry: the difference straddling the ramp end moves by the other
        // half increment, and beyond it the map is flat at the new rate.
        assert!(
            (delta(at + ramp) - delta(at + ramp - 1) - inc / 2.0).abs() < 1e-12,
            "exit: the other half increment"
        );
        assert!((delta(at + ramp + 1) - delta(at + ramp)).abs() < 1e-12, "beyond: flat");
    }

    #[test]
    fn a_mid_ramp_change_glines_from_the_current_rate_not_the_old_target() {
        // Derivative continuity where it is easiest to get wrong: changing tempo INSIDE a ramp
        // must start the new ramp at the rate the old ramp had reached, not at its origin.
        let mut c = Clock::new(48_000, 120.0, 960);
        let (at, ramp) = (10_000u64, 10_000u64);
        assert!(c.set_tempo(240.0, at, ramp));
        let mid = at + ramp / 2; // halfway: the rate is exactly 180 bpm's
        assert!((c.effective_bpm_at(mid) - 180.0).abs() < 1e-9);
        assert!(c.set_tempo(180.0, mid, 1_000)); // hold 180 from here, gliding
        let delta = |s: u64| c.tick_at_sample_f64(s + 1) - c.tick_at_sample_f64(s);
        let tps_mid = 180.0 * 960.0 / (60.0 * 48_000.0);
        assert!((delta(mid) - tps_mid).abs() < 1e-9, "the glide starts at the inherited rate");
        assert!((delta(mid + 2_000) - tps_mid).abs() < 1e-9, "and holds the held target");
        // The map itself never jumped: monotone across the whole edit history.
        let mut prev = 0.0f64;
        for s in (0..40_000u64).step_by(97) {
            let t = c.tick_at_sample_f64(s);
            assert!(t >= prev, "monotone at {s}");
            prev = t;
        }
    }

    #[test]
    fn stale_and_bad_tempo_requests_refuse_without_touching_the_map() {
        let mut c = Clock::new(48_000, 120.0, 960);
        assert!(c.set_tempo(140.0, 50_000, 100));
        let before = c.clone();
        assert!(!c.set_tempo(200.0, 49_999, 100), "a stale anchor cannot rewrite history");
        assert!(!c.set_tempo(0.0, 60_000, 100), "zero bpm is not a tempo");
        assert!(!c.set_tempo(f64::NAN, 60_000, 100), "and neither is NaN");
        assert_eq!(c, before, "refusals leave no trace (the graph's own rule, the clock's too)");
        // Scrubbing the same anchor replaces rather than stacking.
        assert!(c.set_tempo(150.0, 50_000, 100));
        assert!(c.set_tempo(160.0, 50_000, 100));
        assert_eq!(c.segment_count(), 2, "same-anchor edits replace; the map stays minimal");
        assert!((c.bpm - 160.0).abs() < 1e-12);
    }

    #[test]
    fn sample_at_tick_bisects_exactly_across_segments() {
        // ADR-006 rule 7's kernel half: with tempo changes in the map, the scheduling direction
        // lands on the nearest sample, and the two directions agree (round-trip within a tick,
        // exact wherever the map is not within half a tick of a boundary).
        let mut c = Clock::new(48_000, 120.0, 960);
        assert!(c.set_tempo(174.5, 30_000, 500));
        assert!(c.set_tempo(92.25, 90_000, 2_000));
        assert!(c.set_tempo(140.0, 150_000, 0));
        // A deterministic walk of ticks (no RNG in the kernel; the 1000-random-tick acceptance
        // lives in sparq-music's suite — this is the map's own invariant).
        let mut tick = 0u64;
        while tick < 12_000 {
            let s = c.sample_at_tick(tick);
            let back = c.tick_at_sample(s);
            assert!(back.abs_diff(tick) <= 1, "tick {tick} -> sample {s} -> tick {back}");
            // Nearest, not just close: the neighbours must not be closer.
            let d = |x: u64| (c.tick_at_sample_f64(x) - tick as f64).abs();
            assert!(
                d(s) <= d(s + 1) + 1e-9 && (s == 0 || d(s) <= d(s - 1) + 1e-9),
                "sample {s} is not the nearest for tick {tick}"
            );
            tick += 137; // coprime-ish stride across beats and segments
        }
    }

    #[test]
    fn the_constant_fast_path_is_the_v0_closed_form_bit_for_bit() {
        // The phase-b goldens rendered through the v0 arithmetic; the fast path must not drift
        // by one sample anywhere the old closed form did not.
        let c = Clock::new(48_000, 138.0, 960);
        let tps = 138.0 * 960.0 / (60.0 * 48_000.0);
        for tick in (0..40_000u64).step_by(31) {
            let closed = ((tick as f64) / tps).round() as u64;
            assert_eq!(c.sample_at_tick(tick), closed, "tick {tick} left the v0 path");
        }
    }

    #[test]
    fn the_map_is_monotone_and_continuous_across_a_worst_case_sweep() {
        // The continuity acceptance, kernel-side: a hostile edit pattern — a tempo change EVERY
        // 64 samples with alternating ramps — must keep the map strictly increasing and the
        // per-sample deltas inside the envelope the ramps declare.
        let mut c = Clock::new(48_000, 120.0, 960);
        let mut s = 0u64;
        let mut target = 0;
        while s < 64_000 {
            let bpm = [60.0, 200.0, 75.5, 182.3][target % 4];
            let ramp = if target % 2 == 0 { 64 } else { 0 };
            assert!(c.set_tempo(bpm, s, ramp));
            s += 64;
            target += 1;
        }
        let tps_max = 200.0 * 960.0 / (60.0 * 48_000.0);
        let tps_min = 60.0 * 960.0 / (60.0 * 48_000.0);
        let max_inc = (tps_max - tps_min) / 64.0;
        let mut prev = c.tick_at_sample_f64(0);
        let mut prev_delta = 0.0f64;
        for s in 1..=64_000u64 {
            let t = c.tick_at_sample_f64(s);
            let delta = t - prev;
            assert!(delta > 0.0, "strictly increasing at {s}");
            assert!(delta <= tps_max + 1e-12, "delta {delta} exceeds the fastest tempo's rate");
            assert!(delta >= tps_min - 1e-12, "delta {delta} under the slowest tempo's rate");
            // Derivative changes are bounded by the steepest ramp increment — EXCEPT in the
            // difference right AFTER a ramp-0 anchor, where a step is the DECLARED behaviour:
            // smoothing is the caller's ramp choice (the transport's default is non-zero), and
            // the kernel map's guarantee at a step is position continuity, which the
            // monotone+bounded checks above pin. (The difference covering [anchor−1, anchor]
            // ends AT the anchor, where the map is continuous by construction — the jump shows
            // up one sample later, in the difference that lies wholly inside the new segment.)
            let at_step_anchor = s % 64 == 1 && ((s - 1) / 64) % 2 == 1; // odd edits: ramp = 0
            if prev_delta > 0.0 && !at_step_anchor {
                assert!(
                    (delta - prev_delta).abs() <= max_inc + 1e-9,
                    "derivative jumped at {s}: {prev_delta} -> {delta}"
                );
            }
            prev = t;
            prev_delta = delta;
        }
    }
}
