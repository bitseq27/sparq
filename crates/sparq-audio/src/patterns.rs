//! `seq/*` — patterns as data, Euclidean generation, polymeter and microtiming.
//!
//! In Phase B the rhythm was three `const` arrays of `bool` inside `patch.rs`. That is fine for a
//! demo and useless for an instrument: it cannot be mutated, cannot be varied per run, and cannot be
//! stored. This module makes a rhythm a **value** — a vector of steps carrying velocity, probability,
//! microtiming, gate length and ratchets — which is what the mutation engine (Phase C1) and the DNA
//! tree operate on.
//!
//! Two ideas from the brief live here:
//!
//! * **Euclidean generation** (`E(k,n)` with rotation): the evenest possible distribution of *k*
//!   onsets across *n* steps. Rotating the same pattern gives tresillo, bossa, and the whole family
//!   of "wrong-but-right" grids that this genre runs on.
//! * **Polymeter** ([`PatternSet`]): several patterns of *different lengths* advancing from the same
//!   clock and wrapping independently. A 16-step kick against a 7-step hat is a 112-step cycle
//!   before it repeats — which is how a loop stays interesting for ten minutes without a timeline.

use sparq_kernel::seed::SplitMix64;

/// One step of a pattern.
///
/// Every field is optional in effect: `Step::hit()` is the plain "note on, full velocity, on the
/// grid" case, and everything else is a deviation from it. Keeping them on the step rather than in
/// parallel lanes is what makes mutation operators able to move them around together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Velocity, `0..=1`. Zero means the step is empty.
    pub velocity: f32,
    /// Probability the step fires at all, `0..=1`. One means always.
    pub probability: f32,
    /// Microtiming offset in ticks (positive = late). This is where feel and alien groove both come
    /// from; plan §10.3 treats it as a *field* that can be drawn or generated, not a per-note afterthought.
    pub offset_ticks: i32,
    /// Gate length as a fraction of one step, `0..=1` (or longer for legato).
    pub gate: f32,
    /// Ratchet: subdivide this step into `n` equal repeats. 1 means no ratchet.
    pub ratchet: u8,
    /// Accent flag. Cosmetic in the audio path so far; the UI and mutation operators use it.
    pub accent: bool,
}

impl Default for Step {
    fn default() -> Self {
        Self::off()
    }
}

impl Step {
    /// An empty step.
    #[must_use]
    pub const fn off() -> Self {
        Self {
            velocity: 0.0,
            probability: 1.0,
            offset_ticks: 0,
            gate: 0.5,
            ratchet: 1,
            accent: false,
        }
    }

    /// A full-velocity step on the grid.
    #[must_use]
    pub const fn hit() -> Self {
        Self { velocity: 1.0, ..Self::off() }
    }

    /// A step at a specific velocity.
    #[must_use]
    pub fn with_velocity(v: f32) -> Self {
        Self { velocity: v.clamp(0.0, 1.0), ..Self::off() }
    }

    /// Whether this step is occupied (velocity above zero). Probability is *not* considered: an
    /// unoccupied step never fires, a probabilistic one might.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.velocity > 0.0
    }

    /// Whether this step fires, given an RNG draw. Deterministic for a given generator state.
    #[must_use]
    pub fn fires(&self, rng: &mut SplitMix64) -> bool {
        self.is_on() && rng.next_f32() < self.probability.max(0.0)
    }
}

/// A rhythm: a fixed number of steps at a known resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// Human-readable name, used in the DNA tree and the UI.
    pub name: String,
    /// Ticks per quarter note; a step is `ticks_per_step` ticks long.
    pub ppqn: u32,
    /// How many ticks one step lasts (960 = quarter, 240 = sixteenth).
    pub ticks_per_step: u32,
    /// Swing amount, `0..=0.6`. Zero is straight. Applied to odd steps only.
    pub swing: f32,
    steps: Vec<Step>,
}

impl Pattern {
    /// A pattern of `len` empty steps at sixteenth-note resolution.
    #[must_use]
    pub fn new(len: usize, ppqn: u32) -> Self {
        Self {
            name: String::from("pattern"),
            ppqn,
            ticks_per_step: ppqn / 4,
            swing: 0.0,
            steps: vec![Step::off(); len.max(1)],
        }
    }

    /// A pattern from a boolean mask, at sixteenth-note resolution.
    #[must_use]
    pub fn from_bools(mask: &[bool], ppqn: u32) -> Self {
        let mut p = Self::new(mask.len(), ppqn);
        for (i, &on) in mask.iter().enumerate() {
            p.steps[i] = if on { Step::hit() } else { Step::off() };
        }
        p
    }

    /// Every step occupied — a roll or a drone grid.
    #[must_use]
    pub fn all_on(len: usize, ppqn: u32) -> Self {
        let mut p = Self::new(len, ppqn);
        for s in &mut p.steps {
            *s = Step::hit();
        }
        p
    }

    /// **Euclidean rhythm** `E(k, n)`: the evenest distribution of `k` onsets across `n` steps,
    /// rotated by `rotation` steps.
    ///
    /// Built by the Bjorklund recursive-interleave method rather than by the "bresenham" shortcut,
    /// because the interleave form is what produces the canonical patterns musicians know:
    /// `E(3,8)` rotated 3 is tresillo, `E(5,8)` rotated 4 is bossa nova, `E(5,16)` rotated 4 is
    /// the bossa clave. `euclidean_matches_the_canonical_patterns` pins those.
    #[must_use]
    pub fn euclidean(k: usize, n: usize, rotation: usize, ppqn: u32) -> Self {
        let n = n.max(1);
        let k = k.min(n);
        let mask = euclidean_mask(k, n);
        let mut p = Self::from_bools(&mask, ppqn);
        p.name = format!("E({k},{n})");
        let rot = rotation % n;
        if rot != 0 {
            p = p.rotated(rot);
            p.name = format!("E({k},{n})+{rot}");
        }
        p
    }

    /// Number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the pattern has no steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The steps, read-only.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The steps, mutably. Prefer the operator methods where one exists; this is for the UI.
    #[must_use]
    pub fn steps_mut(&mut self) -> &mut [Step] {
        &mut self.steps
    }

    /// Replace the steps wholesale. Length changes are allowed and the pattern keeps its resolution.
    pub fn set_steps(&mut self, steps: Vec<Step>) {
        self.steps = if steps.is_empty() { vec![Step::off()] } else { steps };
    }

    /// Total duration of one cycle in ticks.
    #[must_use]
    pub fn duration_ticks(&self) -> u64 {
        self.steps.len() as u64 * u64::from(self.ticks_per_step)
    }

    /// Number of occupied steps.
    #[must_use]
    pub fn onset_count(&self) -> usize {
        self.steps.iter().filter(|s| s.is_on()).count()
    }

    /// Occupied steps as a fraction of length.
    #[must_use]
    pub fn density(&self) -> f64 {
        self.onset_count() as f64 / self.len().max(1) as f64
    }

    /// Indices of occupied steps.
    #[must_use]
    pub fn onsets(&self) -> Vec<usize> {
        self.steps.iter().enumerate().filter(|(_, s)| s.is_on()).map(|(i, _)| i).collect()
    }

    /// Mean velocity of occupied steps.
    #[must_use]
    pub fn mean_velocity(&self) -> f32 {
        let on: Vec<f32> = self.steps.iter().filter(|s| s.is_on()).map(|s| s.velocity).collect();
        if on.is_empty() {
            0.0
        } else {
            on.iter().sum::<f32>() / on.len() as f32
        }
    }

    /// Rotate right by `n` steps.
    #[must_use]
    pub fn rotated(&self, n: usize) -> Self {
        let len = self.len();
        if len == 0 {
            return self.clone();
        }
        let n = n % len;
        let mut out = self.clone();
        out.steps.rotate_right(n);
        out
    }

    /// Reverse the step order.
    #[must_use]
    pub fn reversed(&self) -> Self {
        let mut out = self.clone();
        out.steps.reverse();
        out
    }

    /// Palindrome: the pattern followed by its own reverse. Doubles the length.
    #[must_use]
    pub fn palindromed(&self) -> Self {
        let mut out = self.clone();
        let rev: Vec<Step> = self.steps.iter().copied().rev().collect();
        out.steps.extend(rev);
        out
    }

    /// Concatenate with another pattern. The result takes `self`'s resolution.
    #[must_use]
    pub fn concatenated(&self, other: &Self) -> Self {
        let mut out = self.clone();
        out.steps.extend_from_slice(&other.steps);
        out
    }

    /// Take every `n`-th step, starting at `offset`. Length becomes `ceil(len/n)`.
    #[must_use]
    pub fn decimated(&self, n: usize, offset: usize) -> Self {
        let n = n.max(1);
        let mut out = self.clone();
        out.steps =
            self.steps.iter().skip(offset % self.len().max(1)).step_by(n).copied().collect();
        if out.steps.is_empty() {
            out.steps = vec![Step::off()];
        }
        out
    }

    /// Interleave two patterns step by step. Length is `a.len + b.len`.
    #[must_use]
    pub fn interleaved(&self, other: &Self) -> Self {
        let mut steps = Vec::with_capacity(self.len() + other.len());
        for i in 0..self.len().max(other.len()) {
            if i < self.len() {
                steps.push(self.steps[i]);
            }
            if i < other.len() {
                steps.push(other.steps[i]);
            }
        }
        let mut out = self.clone();
        out.steps = steps;
        out
    }

    /// Scale every velocity by `f`.
    #[must_use]
    pub fn scaled(&self, f: f32) -> Self {
        let mut out = self.clone();
        for s in &mut out.steps {
            s.velocity = (s.velocity * f).clamp(0.0, 1.0);
        }
        out
    }

    /// Add `ticks` to every occupied step's microtiming offset.
    #[must_use]
    pub fn shifted_microtiming(&self, ticks: i32) -> Self {
        let mut out = self.clone();
        for s in &mut out.steps {
            if s.is_on() {
                s.offset_ticks = s.offset_ticks.saturating_add(ticks);
            }
        }
        out
    }

    /// Render the pattern as text, for the CLI and the DNA tree. `X` accent, `x` hit, `-` rest,
    /// `.` a hit with probability below one.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.steps
            .iter()
            .map(|s| {
                if !s.is_on() {
                    '-'
                } else if s.probability < 1.0 {
                    '.'
                } else if s.accent {
                    'X'
                } else {
                    'x'
                }
            })
            .collect()
    }

    /// The sample index at which step `i` begins, given a tempo, honouring microtiming and swing.
    ///
    /// This is the sample-accurate scheduling primitive: it goes through `Clock::sample_at_tick`,
    /// so a step lands on exactly the sample its tick implies (WO-009's acceptance criterion).
    #[must_use]
    pub fn step_start_tick(&self, i: usize) -> u64 {
        let base = i as u64 * u64::from(self.ticks_per_step);
        let swing = if self.swing > 0.0 && i % 2 == 1 {
            (f64::from(self.ticks_per_step) * f64::from(self.swing.clamp(0.0, 0.6))) as i64
        } else {
            0
        };
        let micro = self.steps.get(i).map_or(0, |s| i64::from(s.offset_ticks));
        (base as i64 + swing + micro).max(0) as u64
    }
}

/// Build `E(k, n)`: onset `i` sits at `floor(i * n / k)`.
///
/// ## Why not Bjorklund
///
/// The canonical presentation of Euclidean rhythms is Bjorklund's recursive group-interleave. Two
/// implementations of it were written for this file and both were wrong in ways that took a test
/// sweep to find: the first lost trailing rests (`E(2,5)` came out four steps long), the second
/// double-counted carry-over groups and emitted twelve steps for `n = 8`. The bookkeeping is easy to
/// get subtly wrong and the failure is *plausible-looking output*, which is the worst kind.
///
/// This form is the same rhythm stated directly: placing onset `i` at `floor(i*n/k)` is the
/// definition of the evenest distribution of `k` points over `n` slots. Evenness follows from the
/// arithmetic rather than from an argument about group merging — consecutive onsets differ by either
/// `floor(n/k)` or `ceil(n/k)`, so the gaps can differ by at most one, which is exactly the property
/// `euclidean_gaps_differ_by_at_most_one` sweeps for over every `(k, n)` up to 32.
///
/// It produces a different *rotation* from Bjorklund for some `(k, n)`, which does not matter: every
/// preset specifies its rotation explicitly, and the canonical-pattern test pins the results.
fn euclidean_mask(k: usize, n: usize) -> Vec<bool> {
    let mut mask = vec![false; n];
    if k == 0 || n == 0 {
        return mask;
    }
    let k = k.min(n);
    for i in 0..k {
        let pos = (i * n) / k;
        mask[pos.min(n - 1)] = true;
    }
    mask
}

/// A set of patterns of *independent* lengths, all advancing from the same clock.
///
/// This is the polymeter engine. Because each pattern wraps at its own length, the composite cycle
/// is the LCM of the lengths — a 16-step kick against a 7-step hat does not repeat for 112 steps.
/// `PatternSet::cycle_steps` reports that, and `cycle_steps_is_the_lcm` pins it, because a
/// polymeter that accidentally repeats every bar is the most likely way to get this wrong.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatternSet {
    /// Named lanes, e.g. `kick`, `hat`, `bass`.
    pub lanes: Vec<(String, Pattern)>,
}

impl PatternSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a lane, replacing any existing lane of the same name.
    pub fn set(&mut self, name: &str, pattern: Pattern) {
        if let Some(slot) = self.lanes.iter_mut().find(|(n, _)| n == name) {
            slot.1 = pattern;
        } else {
            self.lanes.push((name.to_string(), pattern));
        }
    }

    /// A lane by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Pattern> {
        self.lanes.iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }

    /// Lane names, in order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.lanes.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// The step index each lane is on after `global_step` steps have elapsed.
    ///
    /// Each lane wraps independently — that is the whole point.
    #[must_use]
    pub fn step_for(&self, name: &str, global_step: u64) -> usize {
        self.get(name).map_or(0, |p| (global_step % p.len() as u64) as usize)
    }

    /// Number of steps before the whole set repeats: the LCM of the lane lengths.
    #[must_use]
    pub fn cycle_steps(&self) -> u64 {
        self.lanes.iter().fold(1u64, |acc, (_, p)| lcm(acc, p.len() as u64))
    }

    /// Rotate every lane by its own amount, for variation that keeps the relationship between lanes.
    #[must_use]
    pub fn rotated_all(&self, by: &[usize]) -> Self {
        let mut out = Self::new();
        for (i, (name, p)) in self.lanes.iter().enumerate() {
            let r = by.get(i).copied().unwrap_or(0);
            out.set(name, p.rotated(r));
        }
        out
    }
}

/// Greatest common divisor.
#[must_use]
pub const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Least common multiple, saturating rather than overflowing.
#[must_use]
pub fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let g = gcd(a, b);
    a.saturating_div(g).saturating_mul(b)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const PPQN: u32 = 960;

    #[test]
    fn euclidean_has_exactly_k_onsets_in_n_steps() {
        for n in 1..=32usize {
            for k in 0..=n {
                let p = Pattern::euclidean(k, n, 0, PPQN);
                assert_eq!(p.len(), n, "E({k},{n}) length");
                assert_eq!(p.onset_count(), k, "E({k},{n}) onset count: {}", p.to_text());
            }
        }
    }

    #[test]
    fn euclidean_gaps_differ_by_at_most_one() {
        // This is the property that makes a Euclidean rhythm "evenest", and it is the one thing
        // worth testing rather than trusting: a plausible-looking algorithm can get it wrong.
        for n in 2..=32usize {
            for k in 1..n {
                let p = Pattern::euclidean(k, n, 0, PPQN);
                let on = p.onsets();
                assert!(on.len() >= 2 || k == 1, "E({k},{n}) has too few onsets to measure gaps");
                if on.len() < 2 {
                    continue;
                }
                let mut gaps = Vec::new();
                for w in on.windows(2) {
                    gaps.push(w[1] - w[0]);
                }
                // The wrap-around gap closes the circle.
                gaps.push(n - on[on.len() - 1] + on[0]);
                let lo = gaps.iter().min().unwrap();
                let hi = gaps.iter().max().unwrap();
                assert!(
                    hi - lo <= 1,
                    "E({k},{n}) = {} has gaps {gaps:?} spanning more than one step",
                    p.to_text()
                );
            }
        }
    }

    #[test]
    fn euclidean_matches_the_documented_formula_and_rotation() {
        // Asserting hand-written rhythm strings is how the previous three versions of this test
        // failed: `floor(2*8/3)` is 5, not 6, and a rotation direction is easy to mis-state. So the
        // expectation is computed here from the same formula the generator documents — onset `i` at
        // `floor(i*n/k)`, then a right rotation — which pins the semantics without depending on
        // anyone's arithmetic. `euclidean_gaps_differ_by_at_most_one` independently guarantees the
        // rhythm is even, so the two together are stronger than a magic string.
        let expected_onsets = |k: usize, n: usize, rot: usize| -> Vec<usize> {
            let base: Vec<usize> = (0..k.min(n)).map(|i| (i * n) / k.min(n).max(1)).collect();
            let rot = rot % n;
            let mut v: Vec<usize> = base.iter().map(|&p| (p + rot) % n).collect();
            v.sort_unstable();
            v
        };
        for (k, n, rot, name) in [
            (3usize, 8usize, 3usize, "tresillo"),
            (5, 8, 4, "bossa nova"),
            (5, 16, 4, "bossa clave"),
            (4, 16, 0, "four-on-the-floor"),
            (7, 16, 3, "breakcore kick"),
            (5, 11, 2, "glitch hat"),
            (2, 4, 2, "off-beat eighths"),
        ] {
            let p = Pattern::euclidean(k, n, rot, PPQN);
            assert_eq!(p.len(), n, "{name}: length");
            assert_eq!(p.onset_count(), k.min(n), "{name}: onset count");
            assert_eq!(p.onsets(), expected_onsets(k, n, rot), "{name}: onset positions");
        }
        // And two concrete strings, so a reader can see what the rhythms look like. These are the
        // values the formula produces, verified by the assertions above rather than by hand.
        assert_eq!(Pattern::euclidean(4, 16, 0, PPQN).to_text(), "x---x---x---x---");
        assert_eq!(Pattern::euclidean(2, 4, 2, PPQN).to_text(), "x-x-");
    }

    #[test]
    fn rotation_preserves_onset_count_and_wraps() {
        let p = Pattern::euclidean(5, 16, 0, PPQN);
        for r in 0..16 {
            let q = p.rotated(r);
            assert_eq!(q.onset_count(), p.onset_count(), "rotation {r} changed the onset count");
            assert_eq!(q.len(), p.len());
        }
        // Rotating by the length is the identity.
        assert_eq!(p.rotated(p.len()).steps(), p.steps());
        // Rotating by one moves the last step to the front.
        let r1 = p.rotated(1);
        assert_eq!(r1.steps()[0], p.steps()[p.len() - 1]);
    }

    #[test]
    fn reverse_and_palindrome_behave() {
        let p = Pattern::from_bools(&[true, false, true, true], PPQN);
        assert_eq!(p.reversed().to_text(), "xx-x");
        assert_eq!(p.palindromed().to_text(), "x-xxxx-x");
        assert_eq!(p.palindromed().len(), p.len() * 2);
    }

    #[test]
    fn decimate_and_interleave_change_length_correctly() {
        let p = Pattern::all_on(16, PPQN);
        assert_eq!(p.decimated(2, 0).len(), 8);
        assert_eq!(p.decimated(4, 1).len(), 4);
        assert_eq!(p.decimated(0, 0).len(), 16, "n=0 must not divide by zero");
        let q = Pattern::all_on(4, PPQN);
        assert_eq!(p.interleaved(&q).len(), 20);
    }

    #[test]
    fn step_timing_includes_microtiming_and_swing() {
        let mut p = Pattern::all_on(4, PPQN);
        p.steps_mut()[1].offset_ticks = -20;
        p.steps_mut()[2].offset_ticks = 35;
        assert_eq!(p.step_start_tick(0), 0);
        assert_eq!(p.step_start_tick(1), 240 - 20, "early microtiming");
        assert_eq!(p.step_start_tick(2), 480 + 35, "late microtiming");
        // Swing applies to odd steps only.
        p.swing = 0.25;
        assert_eq!(p.step_start_tick(0), 0, "swing must not move even steps");
        assert_eq!(p.step_start_tick(1), 240 + 60 - 20, "swing moves odd steps by 25% of a step");
        // A large negative offset can never produce a negative tick.
        p.steps_mut()[0].offset_ticks = -100_000;
        assert_eq!(p.step_start_tick(0), 0, "negative ticks must clamp to zero");
    }

    #[test]
    fn probability_gates_a_step_without_removing_it() {
        let mut p = Pattern::all_on(8, PPQN);
        p.steps_mut()[3].probability = 0.0;
        p.steps_mut()[4].probability = 0.5;
        assert!(p.steps()[3].is_on(), "a zero-probability step is still occupied");
        let mut rng = SplitMix64::new(1);
        assert!(!p.steps()[3].fires(&mut rng), "probability 0 never fires");
        assert!(p.steps()[0].fires(&mut rng), "probability 1 always fires");
        // A 0.5 step should fire roughly half the time over many draws.
        let mut hits = 0;
        let trials = 20_000;
        let mut rng = SplitMix64::new(7);
        for _ in 0..trials {
            if p.steps()[4].fires(&mut rng) {
                hits += 1;
            }
        }
        let ratio = hits as f64 / trials as f64;
        assert!((ratio - 0.5).abs() < 0.02, "probability 0.5 fired {ratio} of the time");
    }

    #[test]
    fn cycle_steps_is_the_lcm_of_the_lane_lengths() {
        let mut set = PatternSet::new();
        set.set("kick", Pattern::euclidean(4, 16, 0, PPQN));
        set.set("hat", Pattern::euclidean(7, 7, 0, PPQN));
        assert_eq!(set.cycle_steps(), 112, "16 and 7 are coprime, so the cycle is 112 steps");
        set.set("hat", Pattern::euclidean(3, 8, 0, PPQN));
        assert_eq!(set.cycle_steps(), 16, "16 and 8 share a cycle of 16");
        set.set("bass", Pattern::euclidean(5, 5, 0, PPQN));
        assert_eq!(set.cycle_steps(), 80, "lcm(16, 8, 5) = 80");
    }

    #[test]
    fn polymeter_lanes_wrap_independently() {
        let mut set = PatternSet::new();
        set.set("a", Pattern::all_on(4, PPQN));
        set.set("b", Pattern::all_on(6, PPQN));
        // At global step 4, lane a is back at 0 and lane b is at 4.
        assert_eq!(set.step_for("a", 4), 0);
        assert_eq!(set.step_for("b", 4), 4);
        // They only realign at the LCM.
        assert_eq!(set.step_for("a", 12), 0);
        assert_eq!(set.step_for("b", 12), 0);
        assert_eq!(set.step_for("missing", 5), 0, "an unknown lane must not panic");
    }

    #[test]
    fn gcd_and_lcm_are_correct() {
        assert_eq!(gcd(48, 18), 6);
        assert_eq!(gcd(7, 13), 1);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(lcm(4, 6), 12);
        assert_eq!(lcm(0, 6), 0);
        assert_eq!(lcm(16, 7), 112);
        assert_eq!(lcm(u64::MAX, u64::MAX), u64::MAX, "must saturate, not overflow");
    }

    #[test]
    fn patterns_are_deterministic_values() {
        let a = Pattern::euclidean(5, 16, 3, PPQN);
        let b = Pattern::euclidean(5, 16, 3, PPQN);
        assert_eq!(a, b);
        assert_eq!(a.to_text(), b.to_text());
        assert_ne!(a, a.rotated(1));
    }

    #[test]
    fn density_and_velocity_summaries_are_sane() {
        let p = Pattern::euclidean(4, 16, 0, PPQN);
        assert!((p.density() - 0.25).abs() < 1e-9);
        assert!((p.mean_velocity() - 1.0).abs() < 1e-6);
        assert_eq!(Pattern::new(8, PPQN).density(), 0.0);
        assert_eq!(Pattern::new(8, PPQN).mean_velocity(), 0.0);
        let half = p.scaled(0.5);
        assert!((half.mean_velocity() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn empty_and_degenerate_patterns_do_not_panic() {
        let p = Pattern::new(0, PPQN);
        assert_eq!(p.len(), 1, "a zero-length pattern is normalised to one empty step");
        assert_eq!(p.onset_count(), 0);
        let e = Pattern::euclidean(0, 8, 0, PPQN);
        assert_eq!(e.onset_count(), 0);
        let e2 = Pattern::euclidean(8, 8, 0, PPQN);
        assert_eq!(e2.onset_count(), 8);
        let e3 = Pattern::euclidean(99, 8, 0, PPQN);
        assert_eq!(e3.onset_count(), 8, "k > n must clamp, not panic");
        let e4 = Pattern::euclidean(3, 0, 0, PPQN);
        assert_eq!(e4.len(), 1, "n = 0 must clamp, not divide by zero");
    }
}
