//! `env/ad` and `env/adsr` — envelope generators.
//!
//! Output is normalised `0..=1` so a caller can scale it into anything: amplitude (unipolar),
//! filter cutoff (bipolar via a depth), or pitch in semitones. Envelopes are *state machines driven
//! by triggers*, not curves evaluated from a clock, which is what makes them sample-accurate and
//! free-running at any tempo (plan §10.2).

/// Which segment an envelope is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvSegment {
    /// Idle at zero.
    #[default]
    Off,
    /// Rising toward the peak.
    Attack,
    /// Falling from the peak toward the sustain level.
    Decay,
    /// Holding the sustain level.
    Sustain,
    /// Falling from wherever it was toward zero.
    Release,
}

/// Segment shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvCurve {
    /// Linear ramp.
    Linear,
    /// One-pole exponential toward the target — the analogue-like default.
    #[default]
    Exp,
}

/// Per-sample coefficient for an exponential segment that **arrives** at its target in `time_ms`.
///
/// `a = 1 - exp(-1 / (T/5))`: five time constants is the standard "done" point, so the segment is
/// musically `time_ms` long rather than having a time constant of `time_ms` (which would take ~5×
/// longer than the panel says — the first implementation made exactly that mistake, and
/// `ad_returns_to_zero_after_decay` caught it).
fn exp_coeff(time_ms: f32, sample_rate: u32) -> f32 {
    if time_ms <= 0.0 {
        return 1.0;
    }
    let frames = (f64::from(sample_rate) * f64::from(time_ms) / 1000.0 / 5.0).max(1.0);
    (1.0 - (-1.0 / frames).exp()) as f32
}

/// Per-sample increment for a linear segment covering `distance` in `time_ms`.
fn lin_step(time_ms: f32, sample_rate: u32, distance: f32) -> f32 {
    if time_ms <= 0.0 {
        return distance;
    }
    let frames = (f64::from(sample_rate) * f64::from(time_ms) / 1000.0).max(1.0);
    distance / frames as f32
}

/// Attack–Decay envelope (no sustain): the percussion workhorse.
///
/// `trigger()` starts a new cycle from the current level, so retriggers at any rate are smooth
/// rather than clicking — essential for the roll/ratchet patterns this project is for.
#[derive(Clone, Copy, Debug)]
pub struct AdEnv {
    /// Attack time in ms.
    pub attack_ms: f32,
    /// Decay time in ms.
    pub decay_ms: f32,
    /// Segment shape.
    pub curve: EnvCurve,
    /// Peak level, `0..=1`.
    pub peak: f32,
    sample_rate: u32,
    level: f32,
    segment: EnvSegment,
    a: f32,
    d: f32,
    /// Linear increment, computed when a segment is entered (it depends on the start level).
    lin: f32,
}

impl Default for AdEnv {
    fn default() -> Self {
        Self::new(2.0, 200.0)
    }
}

impl AdEnv {
    /// An AD envelope with the given attack and decay in ms.
    #[must_use]
    pub fn new(attack_ms: f32, decay_ms: f32) -> Self {
        Self {
            attack_ms,
            decay_ms,
            curve: EnvCurve::Exp,
            peak: 1.0,
            sample_rate: 0,
            level: 0.0,
            segment: EnvSegment::Off,
            a: 1.0,
            d: 1.0,
            lin: 0.0,
        }
    }

    /// Recompute coefficients for a sample rate.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute();
    }

    /// Recompute after changing the times or curve.
    pub fn recompute(&mut self) {
        let sr = self.sample_rate;
        if sr == 0 {
            return;
        }
        self.a = exp_coeff(self.attack_ms, sr);
        self.d = exp_coeff(self.decay_ms, sr);
    }

    /// Start a new cycle from the current level.
    pub fn trigger(&mut self) {
        self.segment = EnvSegment::Attack;
        self.lin = lin_step(self.attack_ms, self.sample_rate, self.peak - self.level);
    }

    /// Force the envelope to zero immediately (a choke, not a release).
    pub fn reset(&mut self) {
        self.level = 0.0;
        self.segment = EnvSegment::Off;
    }

    /// Current level, `0..=1`.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Current segment, for displays.
    #[must_use]
    pub fn segment(&self) -> EnvSegment {
        self.segment
    }

    /// Whether the envelope has finished and returned to idle.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.segment == EnvSegment::Off
    }

    /// Advance one sample.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.segment {
            EnvSegment::Off => {},
            EnvSegment::Attack => {
                if self.curve == EnvCurve::Linear {
                    self.level = (self.level + self.lin).min(self.peak);
                } else {
                    self.level += self.a * (self.peak - self.level);
                }
                if self.level >= self.peak - 1e-5 {
                    self.level = self.peak;
                    self.segment = EnvSegment::Decay;
                    self.lin = -lin_step(self.decay_ms, self.sample_rate, self.peak);
                }
            },
            EnvSegment::Decay => {
                if self.curve == EnvCurve::Linear {
                    self.level = (self.level + self.lin).max(0.0);
                } else {
                    self.level += self.d * (0.0 - self.level);
                }
                if self.level <= 1e-5 {
                    self.level = 0.0;
                    self.segment = EnvSegment::Off;
                }
            },
            EnvSegment::Sustain | EnvSegment::Release => self.segment = EnvSegment::Off,
        }
        self.level
    }

    /// Fill a buffer.
    pub fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = self.tick();
        }
    }
}

/// Attack–Decay–Sustain–Release envelope.
///
/// `gate(true)` opens, `gate(false)` releases. Re-gating during release resumes from the current
/// level, which is what makes legato playing behave.
#[derive(Clone, Copy, Debug)]
pub struct AdsrEnv {
    /// Attack time in ms.
    pub attack_ms: f32,
    /// Decay time in ms.
    pub decay_ms: f32,
    /// Sustain level, `0..=1`.
    pub sustain: f32,
    /// Release time in ms.
    pub release_ms: f32,
    /// Segment shape.
    pub curve: EnvCurve,
    sample_rate: u32,
    level: f32,
    segment: EnvSegment,
    a: f32,
    d: f32,
    r: f32,
    /// Linear increment for the current segment.
    lin: f32,
}

impl Default for AdsrEnv {
    fn default() -> Self {
        Self::new(5.0, 120.0, 0.7, 250.0)
    }
}

impl AdsrEnv {
    /// An ADSR envelope with times in ms and a sustain level `0..=1`.
    #[must_use]
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32) -> Self {
        Self {
            attack_ms,
            decay_ms,
            sustain: sustain.clamp(0.0, 1.0),
            release_ms,
            curve: EnvCurve::Exp,
            sample_rate: 0,
            level: 0.0,
            segment: EnvSegment::Off,
            a: 1.0,
            d: 1.0,
            r: 1.0,
            lin: 0.0,
        }
    }

    /// Recompute coefficients for a sample rate.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.recompute();
    }

    /// Recompute after changing times, sustain or curve.
    pub fn recompute(&mut self) {
        let sr = self.sample_rate;
        if sr == 0 {
            return;
        }
        self.a = exp_coeff(self.attack_ms, sr);
        self.d = exp_coeff(self.decay_ms, sr);
        self.r = exp_coeff(self.release_ms, sr);
    }

    /// Open or close the gate.
    pub fn gate(&mut self, on: bool) {
        if on {
            self.segment = EnvSegment::Attack;
            self.lin = lin_step(self.attack_ms, self.sample_rate, 1.0 - self.level);
        } else {
            self.segment = EnvSegment::Release;
            self.lin = -lin_step(self.release_ms, self.sample_rate, self.level);
        }
    }

    /// Force to zero immediately.
    pub fn reset(&mut self) {
        self.level = 0.0;
        self.segment = EnvSegment::Off;
    }

    /// Current level, `0..=1`.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Current segment, for displays.
    #[must_use]
    pub fn segment(&self) -> EnvSegment {
        self.segment
    }

    /// Advance one sample.
    ///
    /// Exponential segments aim slightly *past* their target so the one-pole actually crosses the
    /// arrival threshold; without the overshoot an exponential segment asymptotes forever and the
    /// envelope never leaves the segment (that was the `adsr_release_reaches_zero` failure).
    #[inline]
    pub fn tick(&mut self) -> f32 {
        let linear = self.curve == EnvCurve::Linear;
        match self.segment {
            EnvSegment::Off => {},
            EnvSegment::Attack => {
                if linear {
                    self.level = (self.level + self.lin).min(1.0);
                } else {
                    self.level += self.a * (1.02 - self.level);
                }
                if self.level >= 1.0 - 1e-4 {
                    self.level = 1.0;
                    self.segment = EnvSegment::Decay;
                    self.lin = -lin_step(self.decay_ms, self.sample_rate, 1.0 - self.sustain);
                }
            },
            EnvSegment::Decay => {
                if linear {
                    self.level = (self.level + self.lin).max(self.sustain);
                } else {
                    self.level += self.d * (self.sustain - 0.001 - self.level);
                }
                if self.level <= self.sustain + 1e-4 {
                    self.level = self.sustain;
                    self.segment = EnvSegment::Sustain;
                }
            },
            EnvSegment::Sustain => self.level = self.sustain,
            EnvSegment::Release => {
                if linear {
                    self.level = (self.level + self.lin).max(0.0);
                } else {
                    self.level += self.r * (-0.001 - self.level);
                }
                if self.level <= 1e-5 {
                    self.level = 0.0;
                    self.segment = EnvSegment::Off;
                }
            },
        }
        self.level
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

    const SR: u32 = 48_000;

    fn ms_to_samples(ms: f32) -> usize {
        (f64::from(SR) * f64::from(ms) / 1000.0) as usize
    }

    #[test]
    fn ad_reaches_peak_in_the_attack_time() {
        let mut e = AdEnv::new(10.0, 200.0);
        e.curve = EnvCurve::Linear;
        e.prepare(SR);
        e.trigger();
        let mut peak_at: Option<usize> = None;
        for i in 0..ms_to_samples(50.0) {
            if e.tick() >= 1.0 - 1e-4 {
                peak_at = Some(i);
                break;
            }
        }
        let expected = ms_to_samples(10.0);
        let got = peak_at.unwrap_or(usize::MAX);
        assert!(
            (got as i64 - expected as i64).abs() <= 3,
            "peak at sample {got}, expected ~{expected}"
        );
    }

    #[test]
    fn ad_returns_to_zero_after_decay() {
        let mut e = AdEnv::new(2.0, 100.0);
        e.prepare(SR);
        e.trigger();
        let mut last = 0.0f32;
        for _ in 0..ms_to_samples(400.0) {
            last = e.tick();
        }
        assert!(last < 1e-4, "envelope had not finished: {last}");
        assert!(e.is_idle(), "segment should be Off, got {:?}", e.segment());
    }

    #[test]
    fn ad_retrigger_from_mid_decay_is_smooth() {
        let mut e = AdEnv::new(5.0, 200.0);
        e.prepare(SR);
        e.trigger();
        for _ in 0..ms_to_samples(80.0) {
            e.tick();
        }
        let before = e.level();
        assert!(before > 0.05 && before < 0.95, "expected a mid-decay level, got {before}");
        e.trigger();
        let mut max_step = 0.0f32;
        let mut prev = before;
        for _ in 0..ms_to_samples(10.0) {
            let v = e.tick();
            max_step = max_step.max((v - prev).abs());
            prev = v;
        }
        assert!(max_step < 0.02, "retrigger produced a step of {max_step} (a click)");
    }

    #[test]
    fn adsr_holds_the_sustain_level() {
        let mut e = AdsrEnv::new(5.0, 50.0, 0.6, 100.0);
        e.prepare(SR);
        e.gate(true);
        let mut level = 0.0f32;
        for _ in 0..ms_to_samples(400.0) {
            level = e.tick();
        }
        assert!((level - 0.6).abs() < 0.01, "sustain should hold 0.6, got {level}");
        assert_eq!(e.segment(), EnvSegment::Sustain);
    }

    #[test]
    fn adsr_release_reaches_zero() {
        let mut e = AdsrEnv::new(5.0, 50.0, 0.8, 80.0);
        e.prepare(SR);
        e.gate(true);
        for _ in 0..ms_to_samples(300.0) {
            e.tick();
        }
        e.gate(false);
        let mut last = 1.0f32;
        for _ in 0..ms_to_samples(400.0) {
            last = e.tick();
        }
        assert!(last < 1e-4, "release did not finish: {last}");
        assert_eq!(e.segment(), EnvSegment::Off);
    }

    #[test]
    fn adsr_regate_during_release_resumes() {
        let mut e = AdsrEnv::new(5.0, 50.0, 0.8, 200.0);
        e.prepare(SR);
        e.gate(true);
        for _ in 0..ms_to_samples(300.0) {
            e.tick();
        }
        e.gate(false);
        for _ in 0..ms_to_samples(100.0) {
            e.tick();
        }
        let mid = e.level();
        assert!(mid > 0.05 && mid < 0.8, "expected a mid-release level, got {mid}");
        e.gate(true);
        for _ in 0..ms_to_samples(300.0) {
            e.tick();
        }
        assert!(
            (e.level() - 0.8).abs() < 0.02,
            "re-gate should return to sustain, got {}",
            e.level()
        );
    }

    #[test]
    fn envelope_output_is_bounded_and_non_negative() {
        let mut e = AdsrEnv::new(1.0, 10.0, 1.0, 10.0);
        e.prepare(SR);
        e.gate(true);
        for i in 0..ms_to_samples(200.0) {
            let v = e.tick();
            assert!((0.0..=1.05).contains(&v), "level {v} out of range at sample {i}");
            if i % 997 == 0 {
                e.gate(false);
            } else if i % 1009 == 0 {
                e.gate(true);
            }
        }
    }

    #[test]
    fn linear_and_exp_curves_differ() {
        let mut lin = AdEnv::new(20.0, 200.0);
        lin.curve = EnvCurve::Linear;
        lin.prepare(SR);
        lin.trigger();
        let mut exp = AdEnv::new(20.0, 200.0);
        exp.curve = EnvCurve::Exp;
        exp.prepare(SR);
        exp.trigger();
        // Halfway through a linear attack the level is 0.5; an exponential one-pole is above that.
        let half = ms_to_samples(10.0);
        let mut lv = 0.0;
        let mut ev = 0.0;
        for _ in 0..half {
            lv = lin.tick();
            ev = exp.tick();
        }
        assert!((lv - 0.5).abs() < 0.05, "linear midpoint {lv}");
        assert!(ev > lv + 0.1, "exp midpoint {ev} should lead linear {lv}");
    }

    #[test]
    fn envelopes_are_deterministic() {
        let run = || {
            let mut e = AdsrEnv::new(3.0, 40.0, 0.55, 90.0);
            e.prepare(SR);
            e.gate(true);
            let mut out = Vec::with_capacity(ms_to_samples(300.0));
            for i in 0..ms_to_samples(300.0) {
                if i == ms_to_samples(150.0) {
                    e.gate(false);
                }
                out.push(e.tick());
            }
            out
        };
        assert_eq!(run(), run());
    }
}
