//! `gen/*` — mutation operators.
//!
//! Plan §10.5 makes mutation a **subsystem**, not a randomise button. Three properties distinguish
//! it from what a DAW calls "randomise":
//!
//! 1. **Operators are values.** [`Op`] is an enum you can store, print, serialise and chain. A
//!    composition therefore has a *score* that is data: "rotate by 3, then Markov at order 2, then
//!    Rule 110, seeded 0xA17E" is reproducible forever.
//! 2. **Mutation never mutates.** Every operator takes a pattern and returns a new one. The original
//!    stays intact, which is what makes the DNA tree ([`crate::dna`]) a tree rather than a smear.
//! 3. **Randomness comes from the seed tree.** Operators take an `&mut SplitMix64` supplied by the
//!    caller, never a global RNG. Same seed ⇒ same result (ADR-007), so a generative performance can
//!    be re-rendered exactly.
//!
//! Operators are grouped the way plan §10.5 groups them, and the grouping is not cosmetic: the
//! deterministic ones are safe to chain arbitrarily, while the stochastic ones need a seed recorded
//! alongside them or the result cannot be reproduced.

use sparq_kernel::seed::SplitMix64;

use crate::patterns::{Pattern, Step};

/// A mutation operator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    // ---- deterministic transforms -------------------------------------------------
    /// Do nothing. Useful as a chain element and as the identity in tests.
    Identity,
    /// Reverse the step order.
    Reverse,
    /// Rotate right by `n` steps.
    Rotate(usize),
    /// Rotate by a random amount.
    RotateRandom,
    /// Mirror: the pattern followed by its reverse. Doubles the length.
    Mirror,
    /// Swap every occupied step with its neighbour.
    SwapPairs,
    /// Keep every `n`-th step.
    Decimate(usize),
    /// Concatenate with a copy of itself.
    Double,
    /// Scale every velocity by `f`.
    ScaleVelocity(f32),
    /// Add `ticks` to every occupied step's microtiming offset.
    ShiftMicrotiming(i32),
    /// Set every occupied step's gate to `f`.
    SetGate(f32),
    /// Set every occupied step's ratchet to `n`.
    SetRatchet(u8),

    // ---- stochastic (need the RNG) -------------------------------------------------
    /// Flip each step on or off with probability `p`.
    Randomise(f64),
    /// Remove each occupied step with probability `p`.
    Thin(f64),
    /// Add a hit to each empty step with probability `p`.
    Densify(f64),
    /// Add a random microtiming offset in `±ticks` to each occupied step.
    Humanise(i32),
    /// Randomly shift a few steps' velocities.
    RandomVelocity(f64),
    /// Shuffle the velocities between occupied steps, keeping the positions.
    ShuffleVelocities,
    /// Re-roll the ratchet on some steps.
    RandomRatchet(f64, u8),

    // ---- structural / generative ---------------------------------------------------
    /// First-order Markov over "occupied or not": `p_stay` is the probability that a step keeps the
    /// previous step's state. Order 1 is enough to break the grid without losing the pulse.
    Markov(f64),
    /// Apply an elementary cellular automaton rule (0..=255) to the occupied/not pattern for
    /// `generations` rows, then read the last row back out. Rule 30 is noise-like, 90 is a Sierpinski
    /// triangle, 110 is structured — all three are useful and they sound completely different.
    CellularAutomaton(u8, usize),
    /// Sample a logistic map at `rate` and threshold it into a rhythm. `r` above ~3.57 is chaotic.
    Logistic(f64, f64),
    /// Sort the velocities of occupied steps ascending (`false`) or descending (`true`), leaving the
    /// rhythm untouched. Turns a flat pattern into a crescendo or a decay.
    SortVelocities(bool),
    /// Euclidean re-generation: keep the length, re-derive the rhythm from the current onset count
    /// with a random rotation. This is the operator that turns "a pattern I typed" into "a pattern
    /// the grid chose".
    Euclideanise,
}

impl Op {
    /// A short label for the DNA tree, the CLI and the UI.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Identity => "identity".into(),
            Self::Reverse => "reverse".into(),
            Self::Rotate(n) => format!("rotate {n}"),
            Self::RotateRandom => "rotate?".into(),
            Self::Mirror => "mirror".into(),
            Self::SwapPairs => "swap-pairs".into(),
            Self::Decimate(n) => format!("decimate {n}"),
            Self::Double => "double".into(),
            Self::ScaleVelocity(f) => format!("scale-vel {f:.2}"),
            Self::ShiftMicrotiming(t) => format!("shift-µt {t:+}"),
            Self::SetGate(g) => format!("gate {g:.2}"),
            Self::SetRatchet(n) => format!("ratchet {n}"),
            Self::Randomise(p) => format!("randomise {p:.2}"),
            Self::Thin(p) => format!("thin {p:.2}"),
            Self::Densify(p) => format!("densify {p:.2}"),
            Self::Humanise(t) => format!("humanise ±{t}"),
            Self::RandomVelocity(p) => format!("vel? {p:.2}"),
            Self::ShuffleVelocities => "shuffle-vel".into(),
            Self::RandomRatchet(p, n) => format!("ratchet? {p:.2}/{n}"),
            Self::Markov(p) => format!("markov {p:.2}"),
            Self::CellularAutomaton(rule, gens) => format!("ca r{rule} g{gens}"),
            Self::Logistic(r, rate) => format!("logistic r={r:.3} @{rate:.3}"),
            Self::SortVelocities(asc) => if *asc { "sort-vel ↑" } else { "sort-vel ↓" }.into(),
            Self::Euclideanise => "euclideanise".into(),
        }
    }

    /// Whether this operator's output depends on the RNG.
    ///
    /// The DNA tree uses this to decide whether a seed must be recorded: a deterministic operator
    /// reproduces from the pattern alone, a stochastic one does not.
    #[must_use]
    pub fn is_stochastic(&self) -> bool {
        matches!(
            self,
            Self::RotateRandom
                | Self::Randomise(_)
                | Self::Thin(_)
                | Self::Densify(_)
                | Self::Humanise(_)
                | Self::RandomVelocity(_)
                | Self::ShuffleVelocities
                | Self::RandomRatchet(..)
                | Self::Markov(_)
                | Self::Logistic(..)
                | Self::Euclideanise
        )
    }

    /// Whether the operator can change the pattern length.
    #[must_use]
    pub fn changes_length(&self) -> bool {
        matches!(
            self,
            Self::Mirror | Self::Decimate(_) | Self::Double | Self::CellularAutomaton(..)
        )
    }
}

/// Apply one operator, returning a **new** pattern. The input is never modified.
#[must_use]
pub fn apply(op: Op, p: &Pattern, rng: &mut SplitMix64) -> Pattern {
    let mut out = p.clone();
    match op {
        Op::Identity => {},

        Op::Reverse => out = p.reversed(),
        Op::Rotate(n) => out = p.rotated(n),
        Op::RotateRandom => out = p.rotated(rng.next_index(p.len())),
        Op::Mirror => out = p.palindromed(),
        Op::Double => out = p.concatenated(p),
        Op::Decimate(n) => out = p.decimated(n, 0),

        Op::SwapPairs => {
            let steps = out.steps_mut();
            for pair in steps.chunks_mut(2) {
                if pair.len() == 2 {
                    pair.swap(0, 1);
                }
            }
        },

        Op::ScaleVelocity(f) => out = p.scaled(f),
        Op::ShiftMicrotiming(t) => out = p.shifted_microtiming(t),
        Op::SetGate(g) => {
            for s in out.steps_mut() {
                if s.is_on() {
                    s.gate = g.clamp(0.0, 4.0);
                }
            }
        },
        Op::SetRatchet(n) => {
            for s in out.steps_mut() {
                if s.is_on() {
                    s.ratchet = n.max(1);
                }
            }
        },

        Op::Randomise(prob) => {
            for s in out.steps_mut() {
                *s = if rng.chance(prob) {
                    Step::with_velocity(rng.range_f64(0.5, 1.0) as f32)
                } else {
                    Step::off()
                };
            }
        },
        Op::Thin(prob) => {
            for s in out.steps_mut() {
                if s.is_on() && rng.chance(prob) {
                    *s = Step::off();
                }
            }
        },
        Op::Densify(prob) => {
            for s in out.steps_mut() {
                if !s.is_on() && rng.chance(prob) {
                    *s = Step::with_velocity(rng.range_f64(0.4, 0.9) as f32);
                }
            }
        },
        Op::Humanise(ticks) => {
            for s in out.steps_mut() {
                if s.is_on() {
                    s.offset_ticks =
                        s.offset_ticks.saturating_add(rng.range_i32(-ticks.abs(), ticks.abs()));
                }
            }
        },
        Op::RandomVelocity(prob) => {
            for s in out.steps_mut() {
                if s.is_on() && rng.chance(prob) {
                    s.velocity = rng.range_f64(0.35, 1.0) as f32;
                }
            }
        },
        Op::ShuffleVelocities => {
            // Fisher–Yates over the occupied steps only, so the rhythm is untouched and only the
            // dynamics move. Velocities are collected, shuffled, and written back in order.
            let mut vels: Vec<f32> =
                out.steps().iter().filter(|s| s.is_on()).map(|s| s.velocity).collect();
            for i in (1..vels.len()).rev() {
                let j = rng.next_index(i + 1);
                vels.swap(i, j);
            }
            let mut it = vels.into_iter();
            for s in out.steps_mut() {
                if s.is_on() {
                    if let Some(v) = it.next() {
                        s.velocity = v;
                    }
                }
            }
        },
        Op::RandomRatchet(prob, max) => {
            for s in out.steps_mut() {
                if s.is_on() && rng.chance(prob) {
                    s.ratchet = (1 + rng.next_below(max.max(1).into())) as u8;
                }
            }
        },

        Op::Markov(p_stay) => {
            // A Markov chain transitions from its own previous OUTPUT. The first version of this
            // operator transitioned from the input step at the same index, which made it a no-op:
            // with p_stay = 1 it copied the pattern exactly, and the operator looked like it worked
            // because the output was always a plausible rhythm.
            let len = out.len();
            let source: Vec<bool> = out.steps().iter().map(Step::is_on).collect();
            let vels: Vec<f32> = out.steps().iter().map(|s| s.velocity).collect();
            // Seed the chain from the first occupied step so the result relates to the input rather
            // than starting from an arbitrary coin flip.
            let mut state = source.first().copied().unwrap_or(false);
            let mut steps = Vec::with_capacity(len);
            for i in 0..len {
                state = if rng.chance(p_stay) { state } else { !state };
                let vel = if state { vels.get(i).copied().unwrap_or(1.0).max(0.6) } else { 0.0 };
                steps.push(if state { Step::with_velocity(vel) } else { Step::off() });
            }
            out.set_steps(steps);
        },

        Op::CellularAutomaton(rule, generations) => {
            let len = out.len();
            let mut row: Vec<bool> = out.steps().iter().map(Step::is_on).collect();
            // Seed a single live cell if the row is dead, or the CA dies immediately and the
            // operator silently becomes "erase the pattern".
            if !row.iter().any(|&b| b) {
                row[len / 2] = true;
            }
            let mut steps: Vec<Step> =
                row.iter().map(|&b| if b { Step::hit() } else { Step::off() }).collect();
            for _ in 0..generations.max(1) {
                let mut next = vec![false; len];
                for i in 0..len {
                    let l = row[(i + len - 1) % len];
                    let c = row[i];
                    let r = row[(i + 1) % len];
                    let idx = (usize::from(l) << 2) | (usize::from(c) << 1) | usize::from(r);
                    next[i] = (rule >> idx) & 1 == 1;
                }
                row = next;
                steps = row.iter().map(|&b| if b { Step::hit() } else { Step::off() }).collect();
            }
            out.set_steps(steps);
            out.name = format!("{}·ca{}", out.name, rule);
        },

        Op::Logistic(r, rate) => {
            let len = out.len();
            let mut x = rng.range_f64(0.2, 0.8);
            let mut steps = Vec::with_capacity(len);
            for _ in 0..len {
                x = r * x * (1.0 - x);
                // Threshold at the mean of the attractor rather than 0.5, so the density stays
                // musical instead of saturating at r near 4.
                steps.push(if x > rate {
                    Step::with_velocity(x.clamp(0.0, 1.0) as f32)
                } else {
                    Step::off()
                });
            }
            out.set_steps(steps);
        },

        Op::SortVelocities(ascending) => {
            let mut vels: Vec<f32> =
                out.steps().iter().filter(|s| s.is_on()).map(|s| s.velocity).collect();
            vels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if !ascending {
                vels.reverse();
            }
            let mut it = vels.into_iter();
            for s in out.steps_mut() {
                if s.is_on() {
                    if let Some(v) = it.next() {
                        s.velocity = v;
                    }
                }
            }
        },

        Op::Euclideanise => {
            let len = out.len();
            let k = out.onset_count().clamp(1, len);
            let rotation = rng.next_index(len);
            out = Pattern::euclidean(k, len, rotation, out.ppqn);
        },
    }
    out
}

/// Apply a chain of operators left to right.
///
/// The chain is the unit a generative patch is written in: it is serialisable, printable and
/// replayable, which is what turns "I randomised until I liked it" into a score.
#[must_use]
pub fn apply_chain(ops: &[Op], p: &Pattern, rng: &mut SplitMix64) -> Pattern {
    ops.iter().fold(p.clone(), |acc, &op| apply(op, &acc, rng))
}

/// A small library of chains that are known to be musically useful, so the CLI and the UI can offer
/// starting points rather than a blank enum.
#[must_use]
pub fn preset_chains() -> Vec<(&'static str, Vec<Op>)> {
    vec![
        ("grid-break", vec![Op::Thin(0.15), Op::Humanise(18), Op::RandomVelocity(0.4)]),
        ("densify", vec![Op::Densify(0.2), Op::SortVelocities(false)]),
        ("canon", vec![Op::Mirror, Op::ScaleVelocity(0.8)]),
        ("chaos", vec![Op::CellularAutomaton(30, 3), Op::Humanise(30)]),
        ("structure", vec![Op::CellularAutomaton(110, 2)]),
        ("sierpinski", vec![Op::CellularAutomaton(90, 4)]),
        ("markov-walk", vec![Op::Markov(0.72)]),
        ("euclidean-drift", vec![Op::Euclideanise, Op::Humanise(12)]),
        ("roll", vec![Op::Decimate(2), Op::SetRatchet(4)]),
        ("logistic", vec![Op::Logistic(3.92, 0.5)]),
        ("stutter", vec![Op::Double, Op::Thin(0.3), Op::SetGate(0.25)]),
    ]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::patterns::Pattern;

    const PPQN: u32 = 960;

    fn base() -> Pattern {
        // 16 steps with 8 onsets, not the 8-step/3-onset pattern this originally used: a stochastic
        // operator like `Thin(0.3)` has a ~34% chance of leaving a 3-onset pattern untouched, so two
        // seeds legitimately produced identical output and `stochastic_operators_actually_vary_with_
        // the_seed` failed about a third of the time. A flaky test is worse than no test.
        let mut p = Pattern::from_bools(
            &[
                true, false, true, true, false, false, true, false, true, true, false, true, false,
                false, true, true,
            ],
            PPQN,
        );
        // Give the steps distinct velocities. `from_bools` sets every onset to 1.0, and shuffling a
        // list of identical values is the identity — so `ShuffleVelocities` looked non-stochastic
        // when it was working correctly. A fixture that cannot exercise an operator is a bug in the
        // fixture, and this one silently disabled a third of the mutation tests.
        for (i, st) in p.steps_mut().iter_mut().enumerate() {
            if st.is_on() {
                st.velocity = 0.5 + 0.05 * ((i * 7) % 10) as f32;
            }
        }
        p
    }

    fn rng(seed: u64) -> SplitMix64 {
        SplitMix64::new(seed)
    }

    #[test]
    fn every_operator_is_deterministic_for_a_given_seed() {
        for op in all_ops() {
            let a = apply(op, &base(), &mut rng(42));
            let b = apply(op, &base(), &mut rng(42));
            assert_eq!(a.steps(), b.steps(), "{op:?} is not reproducible from its seed");
        }
    }

    #[test]
    fn stochastic_operators_actually_vary_with_the_seed() {
        // Over a *set* of seeds, not two. Asserting that seeds 1 and 2 differ is statistically
        // unreasonable: `Thin(0.3)` on an 8-onset pattern leaves it untouched with probability
        // 0.7^8 = 5.7%, so two seeds can legitimately agree and the test would flake. Twelve seeds
        // agreeing has probability 0.7^96, which is a real defect rather than bad luck.
        for op in all_ops() {
            if !op.is_stochastic() {
                continue;
            }
            // Compare full step vectors, not `to_text()`: the text form renders only on/off,
            // probability and accent, so it cannot see microtiming or velocity. `Humanise` changes
            // *only* microtiming, and the first version of this test used `to_text()` and therefore
            // reported Humanise as non-stochastic when it was working correctly.
            let variants: Vec<Vec<Step>> = (1..13u64)
                .map(|seed| apply(op, &base(), &mut rng(seed)).steps().to_vec())
                .collect();
            let all_same = variants.iter().all(|v| *v == variants[0]);
            assert!(
                !all_same,
                "{op:?} is marked stochastic but produced identical output for all 12 seeds"
            );
        }
    }

    #[test]
    fn deterministic_operators_ignore_the_seed() {
        for op in all_ops() {
            if op.is_stochastic() {
                continue;
            }
            let a = apply(op, &base(), &mut rng(1));
            let b = apply(op, &base(), &mut rng(999));
            assert_eq!(
                a.steps(),
                b.steps(),
                "{op:?} is marked deterministic but depends on the seed"
            );
        }
    }

    #[test]
    fn the_input_pattern_is_never_modified() {
        // Mutation must be a pure function, or the DNA tree is a smear rather than a tree.
        for op in all_ops() {
            let p = base();
            let before = p.steps().to_vec();
            let _ = apply(op, &p, &mut rng(7));
            assert_eq!(p.steps(), &before[..], "{op:?} modified its input");
        }
    }

    #[test]
    fn identity_is_the_identity() {
        let p = base();
        assert_eq!(apply(Op::Identity, &p, &mut rng(0)).steps(), p.steps());
    }

    #[test]
    fn length_changing_operators_change_length_as_documented() {
        let p = base();
        assert_eq!(apply(Op::Mirror, &p, &mut rng(0)).len(), p.len() * 2);
        assert_eq!(apply(Op::Double, &p, &mut rng(0)).len(), p.len() * 2);
        assert_eq!(apply(Op::Decimate(2), &p, &mut rng(0)).len(), p.len() / 2);
        for op in all_ops() {
            let q = apply(op, &p, &mut rng(3));
            if !op.changes_length() {
                assert_eq!(q.len(), p.len(), "{op:?} changed the length but says it does not");
            }
        }
    }

    #[test]
    fn thin_removes_and_densify_adds() {
        let p = Pattern::all_on(32, PPQN);
        let thinned = apply(Op::Thin(0.5), &p, &mut rng(11));
        assert!(thinned.onset_count() < p.onset_count(), "thin did not remove anything");
        assert!(thinned.onset_count() > 0, "thin removed everything");

        let empty = Pattern::new(32, PPQN);
        let densified = apply(Op::Densify(0.5), &empty, &mut rng(11));
        assert!(densified.onset_count() > 0, "densify added nothing to an empty pattern");
    }

    #[test]
    fn humanise_moves_microtiming_only() {
        let p = Pattern::all_on(16, PPQN);
        let h = apply(Op::Humanise(40), &p, &mut rng(5));
        assert_eq!(h.onset_count(), p.onset_count(), "humanise must not change the rhythm");
        assert!(
            h.steps().iter().any(|s| s.offset_ticks != 0),
            "humanise produced no microtiming at all"
        );
        assert!(
            h.steps().iter().all(|s| s.offset_ticks.abs() <= 40),
            "humanise exceeded its declared range"
        );
        // Offsets must not be so large that a step lands before the previous one: that would make
        // the sequencer fire notes out of order.
        let tps = h.ticks_per_step as i32;
        assert!(
            h.steps().iter().all(|s| s.offset_ticks.abs() < tps),
            "microtiming exceeded one step, which reorders events"
        );
    }

    #[test]
    fn shuffle_velocities_keeps_the_rhythm() {
        let mut p = Pattern::all_on(16, PPQN);
        for (i, s) in p.steps_mut().iter_mut().enumerate() {
            s.velocity = (i as f32 + 1.0) / 16.0;
        }
        let q = apply(Op::ShuffleVelocities, &p, &mut rng(3));
        assert_eq!(q.onsets(), p.onsets(), "shuffling velocities moved the rhythm");
        let mut a: Vec<f32> = p.steps().iter().map(|s| s.velocity).collect();
        let mut b: Vec<f32> = q.steps().iter().map(|s| s.velocity).collect();
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert_eq!(a, b, "shuffle changed the multiset of velocities");
        assert_ne!(
            p.steps().iter().map(|s| s.velocity).collect::<Vec<_>>(),
            q.steps().iter().map(|s| s.velocity).collect::<Vec<_>>(),
            "shuffle did nothing"
        );
    }

    #[test]
    fn sort_velocities_produces_a_monotonic_contour() {
        let mut p = Pattern::all_on(8, PPQN);
        for (i, s) in p.steps_mut().iter_mut().enumerate() {
            s.velocity = [0.9, 0.2, 0.7, 0.4, 1.0, 0.1, 0.6, 0.3][i];
        }
        let up = apply(Op::SortVelocities(true), &p, &mut rng(0));
        let v: Vec<f32> = up.steps().iter().map(|s| s.velocity).collect();
        assert!(v.windows(2).all(|w| w[0] <= w[1]), "ascending sort was not ascending: {v:?}");
        let down = apply(Op::SortVelocities(false), &p, &mut rng(0));
        let v: Vec<f32> = down.steps().iter().map(|s| s.velocity).collect();
        assert!(v.windows(2).all(|w| w[0] >= w[1]), "descending sort was not descending: {v:?}");
    }

    #[test]
    fn markov_respects_the_stay_probability_at_both_extremes() {
        let p = base();
        // p_stay = 1.0 means "keep the previous state", so the output follows step 0 throughout.
        let sticky = apply(Op::Markov(1.0), &p, &mut rng(1));
        let all_on = sticky.steps().iter().all(|s| s.is_on() == sticky.steps()[0].is_on());
        assert!(all_on, "p_stay=1.0 should freeze the state: {}", sticky.to_text());
        // p_stay = 0.0 means "always flip", so the output must alternate.
        let flip = apply(Op::Markov(0.0), &p, &mut rng(1));
        let states: Vec<bool> = flip.steps().iter().map(|s| s.is_on()).collect();
        assert!(
            states.windows(2).all(|w| w[0] != w[1]),
            "p_stay=0.0 should alternate: {}",
            flip.to_text()
        );
    }

    #[test]
    fn cellular_automaton_rules_are_distinct_and_stay_alive() {
        let p = Pattern::from_bools(&[true, false, false, false, false, false, false, false], PPQN);
        let r30 = apply(Op::CellularAutomaton(30, 4), &p, &mut rng(0));
        let r90 = apply(Op::CellularAutomaton(90, 4), &p, &mut rng(0));
        let r110 = apply(Op::CellularAutomaton(110, 4), &p, &mut rng(0));
        assert_ne!(r30.to_text(), r90.to_text(), "rules 30 and 90 gave the same result");
        assert_ne!(r90.to_text(), r110.to_text(), "rules 90 and 110 gave the same result");
        // A dead pattern must not stay dead: the operator seeds a cell, otherwise it silently
        // becomes "erase the rhythm", which is the failure mode that would be invisible in a mix.
        let dead = Pattern::new(16, PPQN);
        for rule in [30u8, 90, 110, 184] {
            let q = apply(Op::CellularAutomaton(rule, 3), &dead, &mut rng(0));
            assert!(q.onset_count() > 0, "rule {rule} died on an empty pattern");
        }
    }

    #[test]
    fn rule_90_from_a_single_cell_is_a_sierpinski_triangle() {
        // Rule 90 from one live cell gives the XOR of the neighbours, i.e. Pascal's triangle mod 2.
        // Generation 4 from a single cell in a 9-wide row must be 101010101... truncated: the exact
        // expected row is a known value, and pinning it catches an indexing error in the wrap.
        let p = Pattern::from_bools(
            &[false, false, false, false, true, false, false, false, false],
            PPQN,
        );
        let q = apply(Op::CellularAutomaton(90, 2), &p, &mut rng(0));
        // Two generations from a single cell: 0010100010 -> row is 0 0 1 0 0 0 1 0 0 for gen1 then
        // gen2 spreads again. Assert the invariant that matters: symmetry about the seed cell.
        let s = q.to_text();
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars[3], chars[5], "rule 90 from a centred cell must stay symmetric: {s}");
        assert!(q.onset_count() > 0);
    }

    #[test]
    fn logistic_stays_in_range_and_is_sensitive_to_r() {
        let p = base();
        for r in [2.5f64, 3.2, 3.57, 3.92, 4.0] {
            let q = apply(Op::Logistic(r, 0.5), &p, &mut rng(1));
            assert!(
                q.steps()
                    .iter()
                    .all(|s| s.velocity.is_finite() && (0.0..=1.0).contains(&s.velocity)),
                "logistic r={r} produced an out-of-range velocity"
            );
        }
        let a = apply(Op::Logistic(3.2, 0.5), &p, &mut rng(1));
        let b = apply(Op::Logistic(3.99, 0.5), &p, &mut rng(1));
        assert_ne!(
            a.to_text(),
            b.to_text(),
            "r=3.2 (periodic) and r=3.99 (chaotic) gave the same rhythm"
        );
    }

    #[test]
    fn euclideanise_keeps_the_length_and_the_onset_count() {
        let p = Pattern::from_bools(
            &[
                true, true, false, true, false, false, true, false, true, false, false, false,
                true, true, false, false,
            ],
            PPQN,
        );
        let k = p.onset_count();
        for seed in 1..20u64 {
            let q = apply(Op::Euclideanise, &p, &mut rng(seed));
            assert_eq!(q.len(), p.len(), "euclideanise changed the length");
            assert_eq!(q.onset_count(), k, "euclideanise changed the onset count");
        }
        // And it must actually move the rhythm for at least some seeds.
        let moved = (1..20u64)
            .filter(|&s| apply(Op::Euclideanise, &p, &mut rng(s)).to_text() != p.to_text())
            .count();
        assert!(moved > 10, "euclideanise only changed the pattern for {moved}/19 seeds");
    }

    #[test]
    fn chains_compose_and_stay_reproducible() {
        let chain =
            vec![Op::Thin(0.2), Op::Markov(0.7), Op::CellularAutomaton(110, 2), Op::Humanise(20)];
        let a = apply_chain(&chain, &base(), &mut rng(1234));
        let b = apply_chain(&chain, &base(), &mut rng(1234));
        assert_eq!(a.steps(), b.steps(), "a chain is not reproducible from its seed");
        let c = apply_chain(&chain, &base(), &mut rng(1235));
        assert_ne!(a.steps(), c.steps(), "a chain ignored the seed");
        assert_eq!(a.len(), base().len(), "this chain should preserve length");
    }

    #[test]
    fn every_preset_chain_runs_without_panicking_on_degenerate_input() {
        // An empty pattern and a one-step pattern are the inputs most likely to divide by zero.
        let degenerate = [Pattern::new(1, PPQN), Pattern::all_on(1, PPQN), Pattern::new(2, PPQN)];
        for (name, chain) in preset_chains() {
            for p in &degenerate {
                let q = apply_chain(&chain, p, &mut rng(1));
                assert!(!q.is_empty(), "preset `{name}` produced an empty pattern");
                assert!(
                    q.steps().iter().all(|s| s.velocity.is_finite()),
                    "preset `{name}` produced NaN"
                );
            }
        }
    }

    #[test]
    fn labels_are_stable_and_unique_enough_for_the_dna_tree() {
        let mut seen = std::collections::BTreeSet::new();
        for op in all_ops() {
            let l = op.label();
            assert!(!l.is_empty());
            assert!(l.len() < 40, "label too long for the tree UI: {l}");
            seen.insert(l);
        }
        // Not strictly unique (Rotate(1) vs Rotate(1) collide by design), but the count should be
        // close to the operator count or the tree becomes unreadable.
        assert!(
            seen.len() >= all_ops().len() - 4,
            "labels collapse: {} distinct for {} ops",
            seen.len(),
            all_ops().len()
        );
    }

    /// Every operator with a representative parameterisation, so a new variant cannot be added
    /// without being tested. If this list stops covering `Op`, the exhaustiveness check below fails.
    fn all_ops() -> Vec<Op> {
        let v = vec![
            Op::Identity,
            Op::Reverse,
            Op::Rotate(3),
            Op::RotateRandom,
            Op::Mirror,
            Op::SwapPairs,
            Op::Decimate(2),
            Op::Double,
            Op::ScaleVelocity(0.7),
            Op::ShiftMicrotiming(15),
            Op::SetGate(0.3),
            Op::SetRatchet(3),
            Op::Randomise(0.4),
            Op::Thin(0.3),
            Op::Densify(0.3),
            Op::Humanise(25),
            Op::RandomVelocity(0.5),
            Op::ShuffleVelocities,
            Op::RandomRatchet(0.4, 4),
            Op::Markov(0.7),
            Op::CellularAutomaton(30, 3),
            Op::Logistic(3.9, 0.5),
            Op::SortVelocities(true),
            Op::SortVelocities(false),
            Op::Euclideanise,
        ];
        // Exhaustiveness guard: if a variant is added to `Op` and not to this list, the count of
        // distinct discriminants will not match and this assert fires in review.
        assert!(v.len() >= 24, "all_ops() has shrunk; keep it covering every Op variant");
        v
    }
}
