//! `seq/presets` — named rhythm styles and seeded variation.
//!
//! ## The data-model decision recorded here
//!
//! A [`Pattern`] is a **rhythm**: its [`Step`]s carry velocity, probability, microtiming, gate and
//! ratchet. It deliberately carries no pitch. So a style is two things, not one:
//!
//! * percussion lanes (`kick`, `hat`) — genuine `Pattern`s, living in a [`PatternSet`] so they can
//!   be different lengths and wrap independently (polymeter);
//! * a bass line — `Vec<Option<i8>>` of semitone offsets from the root, `None` being a rest.
//!
//! The first attempt at this module encoded the bass as a `Pattern` with pitch smuggled into
//! velocity, and treated `0` as both "root note" and "rest". It compiled, and it was wrong in a way
//! that would have been very hard to hear: every "rest" would have sounded as a root note. Pitch and
//! rhythm are different data and are kept separate until there is a real note type (WO-007's `event`
//! port carries both; that is the right home for it, not a hack here).

use sparq_kernel::seed::{SeedTree, SplitMix64};

use crate::mutation::{apply_chain, preset_chains, Op};
use crate::patterns::{Pattern, PatternSet};

/// A named rhythm style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Four-on-the-floor kick, off-beat hats, steady minor bass.
    Techno,
    /// Dense syncopation, sixteenth hats with rolls, a bass line that fights the grid.
    Breakcore,
    /// Sparse and microtimed: hits deliberately off the grid, hat lane in an 11-step polymeter.
    Glitch,
    /// Quarter kick, sixteenth hats, bass on the off-beats.
    Driving,
}

impl Style {
    /// Every style, in the order `--list-patterns` prints them.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Techno, Self::Breakcore, Self::Glitch, Self::Driving]
    }

    /// The CLI name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Techno => "techno",
            Self::Breakcore => "breakcore",
            Self::Glitch => "glitch",
            Self::Driving => "driving",
        }
    }

    /// Parse a CLI name, case-insensitively.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::all().into_iter().find(|v| v.name().eq_ignore_ascii_case(s.trim()))
    }

    /// One line for `--list-patterns`.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Techno => "four-on-the-floor kick, off-beat hats, steady minor bass",
            Self::Breakcore => "dense syncopation, 16th hats with rolls, bass fights the grid",
            Self::Glitch => "sparse and microtimed, hats in an 11-step polymeter",
            Self::Driving => "quarter kick, 16th hats, bass on the off-beats",
        }
    }

    /// The percussion lanes for this style.
    ///
    /// Euclidean generation is used wherever a Euclidean pattern *is* the honest description of the
    /// rhythm — `E(4,16)` is four-on-the-floor — because then `Op::Euclideanise` has something
    /// meaningful to drift from, and the style can be re-derived rather than only replayed.
    #[must_use]
    pub fn percussion(&self, ppqn: u32) -> PatternSet {
        let mut set = PatternSet::new();
        match self {
            Self::Techno => {
                set.set("kick", Pattern::euclidean(4, 16, 0, ppqn));
                // Off-beat eighths: a 4-step pattern rotated so the hits land between the kicks.
                // It is 4 steps against the kick's 16, so `cycle_steps()` is 16 — the polymeter is
                // trivial here by design, because techno wants the grid to feel locked.
                set.set("hat", Pattern::euclidean(2, 4, 0, ppqn).rotated(2));
            },
            Self::Breakcore => {
                set.set("kick", Pattern::euclidean(7, 16, 3, ppqn));
                // A sliced break, not a wall of sixteenths: gaps are what make the rolls read as
                // rolls. `all_on(16)` here produced 16/16 onsets, which is a hiss with no rhythm.
                let mut hat = Pattern::from_bools(
                    &[
                        true, false, true, true, false, true, false, true, true, false, true,
                        false, true, true, false, true,
                    ],
                    ppqn,
                );
                // Ratchets turn two hits into rolls — the signature breakcore gesture, and the
                // reason `Step::ratchet` exists rather than being a later addition.
                hat.steps_mut()[6].ratchet = 3;
                hat.steps_mut()[14].ratchet = 4;
                hat.steps_mut()[14].velocity = 0.6;
                hat.steps_mut()[10].velocity = 0.75;
                set.set("hat", hat);
            },
            Self::Glitch => {
                let mut kick = Pattern::euclidean(3, 16, 5, ppqn);
                // Microtiming is the point of this style: push hits off the grid in both directions.
                if let Some(s) = kick.steps_mut().get_mut(5) {
                    s.offset_ticks = 22;
                    s.velocity = 0.8;
                }
                if let Some(s) = kick.steps_mut().get_mut(10) {
                    s.offset_ticks = -18;
                }
                set.set("kick", kick);
                // 11 steps against the kick's 16: the pair does not repeat for 176 steps, which is
                // how a loop stays interesting for ten minutes without a timeline.
                let mut hat = Pattern::euclidean(5, 11, 2, ppqn);
                hat.swing = 0.12;
                set.set("hat", hat);
            },
            Self::Driving => {
                set.set("kick", Pattern::euclidean(4, 16, 0, ppqn));
                // A driving sixteenth hat still needs shape: strong on the eighths, lighter in
                // between, and two gaps so it breathes. `all_on(16)` with velocity tweaks was not
                // enough — it read as a hiss and tripped the "a lane needs some rhythm" invariant.
                let mut hat = Pattern::from_bools(
                    &[
                        true, true, true, true, true, true, true, false, true, true, true, true,
                        true, true, true, false,
                    ],
                    ppqn,
                );
                for (i, st) in hat.steps_mut().iter_mut().enumerate() {
                    // Only shape the steps that are ON. The first version of this loop set a
                    // velocity on every index, which turned the two rests into onsets and produced
                    // a 16/16 hiss — the exact thing the gaps were added to avoid.
                    if !st.is_on() {
                        continue;
                    }
                    st.velocity = match i % 4 {
                        0 => 1.0,
                        2 => 0.8,
                        _ => 0.5,
                    };
                    st.accent = i % 4 == 0;
                }
                set.set("hat", hat);
            },
        }
        set
    }

    /// The bass line as semitone offsets from the root; `None` is a rest.
    ///
    /// Sixteen entries so it lines up with a 16-step kick lane, but it is an independent vector and
    /// may be any length — `bass_step_for` wraps it against the same clock.
    #[must_use]
    pub fn bass_semitones(&self) -> Vec<Option<i8>> {
        /// Shorthand: `n(semitone)` is a note, `r()` is a rest.
        fn n(s: i8) -> Option<i8> {
            Some(s)
        }
        const R: Option<i8> = None;
        match self {
            // A minor: root, minor third, fifth, and a passing flat-second for the dark turn.
            Self::Techno => {
                vec![n(0), R, R, n(0), R, R, n(3), R, n(0), R, R, n(7), R, n(3), R, n(-2)]
            },
            Self::Breakcore => {
                vec![n(0), n(-2), R, n(0), n(3), R, R, n(7), n(-2), R, n(0), n(3), R, R, n(-5), R]
            },
            Self::Glitch => vec![n(0), R, R, n(3), R, R, R, R, n(7), R, R, n(-2), R, R, n(3), R],
            Self::Driving => vec![R, R, n(0), R, R, R, n(7), R, R, R, n(0), R, R, n(3), R, R],
        }
    }
}

/// Which lane of a [`PatternSet`] the bass semitone list corresponds to.
///
/// The bass is not in the `PatternSet` (it carries pitch), but it still needs a length to wrap
/// against. This is that length, kept next to the data it describes rather than assumed.
#[must_use]
pub fn bass_len(style: Style) -> usize {
    style.bass_semitones().len()
}

/// The semitone for the bass at a global step, wrapping independently of the percussion lanes.
#[must_use]
pub fn bass_step_for(style: Style, global_step: u64) -> Option<i8> {
    let line = style.bass_semitones();
    if line.is_empty() {
        return None;
    }
    line[(global_step % line.len() as u64) as usize]
}

/// A seeded variation of a style: the pattern set plus the bass line after a mutation chain.
///
/// Returns the mutated percussion lanes and a record of what was done, so the caller can put it in
/// the DNA tree and the CLI can print it. **Seed 0 means "no mutation"** and reproduces the preset
/// exactly, which is what makes `--seed 0` a stable golden reference.
#[must_use]
pub fn variation(style: Style, ppqn: u32, chain: &[Op], seed: u64) -> (PatternSet, Vec<String>) {
    let mut set = style.percussion(ppqn);
    if chain.is_empty() || seed == 0 {
        return (set, Vec::new());
    }
    let tree = SeedTree::new(seed);
    let mut log = Vec::new();
    // Collected up front: the loop mutates `set`, and `set.names()` would borrow it for the whole
    // iteration. Naming the borrow explicitly is cheaper than restructuring the loop.
    let names: Vec<String> = set.names().iter().map(|s| (*s).to_string()).collect();
    for name in &names {
        // Each lane gets its own branch of the seed tree, so re-seeding the hat never changes the
        // kick (ADR-007's core promise, applied to lanes rather than modules).
        let lane_seed = tree.child("variation").child(name).seed();
        let mut rng = SplitMix64::new(lane_seed);
        if let Some(p) = set.get(name).cloned() {
            let mutated = apply_chain(chain, &p, &mut rng);
            log.push(format!("{name}: {} @ seed 0x{lane_seed:X}", mutated.to_text()));
            set.set(name, mutated);
        }
    }
    (set, log)
}

/// The mutation chain named by `--mutation`, resolved against the built-in presets and the operator
/// vocabulary. Returns `None` for an unknown name so the CLI can list the valid ones.
#[must_use]
pub fn resolve_chain(name: &str) -> Option<Vec<Op>> {
    let key = name.trim().to_ascii_lowercase();
    if key == "none" || key.is_empty() {
        return Some(Vec::new());
    }
    preset_chains().into_iter().find(|(n, _)| *n == key).map(|(_, ops)| ops)
}

/// Every mutation preset name, for `--list-patterns`.
#[must_use]
pub fn mutation_names() -> Vec<&'static str> {
    preset_chains().into_iter().map(|(n, _)| n).collect()
}

/// A one-line human description of a chain.
#[must_use]
pub fn chain_label(chain: &[Op]) -> String {
    if chain.is_empty() {
        return String::from("none");
    }
    chain.iter().map(Op::label).collect::<Vec<_>>().join(" -> ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::patterns::lcm;

    const PPQN: u32 = 960;

    #[test]
    fn every_style_builds_a_usable_pattern_set() {
        for style in Style::all() {
            let set = style.percussion(PPQN);
            assert!(set.get("kick").is_some(), "{style:?} has no kick lane");
            assert!(set.get("hat").is_some(), "{style:?} has no hat lane");
            for (name, p) in &set.lanes {
                assert!(!p.is_empty(), "{style:?}/{name} is empty");
                assert!(p.onset_count() > 0, "{style:?}/{name} has no onsets: {}", p.to_text());
                assert!(p.onset_count() < p.len(), "{style:?}/{name} is all onsets — no rhythm");
            }
            let bass = style.bass_semitones();
            assert!(!bass.is_empty());
            assert!(bass.iter().any(|s| s.is_some()), "{style:?} bass has no notes at all");
            assert!(bass.iter().any(|s| s.is_none()), "{style:?} bass has no rests at all");
        }
    }

    #[test]
    fn style_names_roundtrip_through_parse() {
        for style in Style::all() {
            assert_eq!(Style::parse(style.name()), Some(style));
            assert_eq!(
                Style::parse(&style.name().to_ascii_uppercase()),
                Some(style),
                "parse should be case-insensitive"
            );
        }
        assert_eq!(Style::parse("nope"), None);
        assert_eq!(Style::parse(""), None);
    }

    #[test]
    fn techno_is_four_on_the_floor() {
        let set = Style::Techno.percussion(PPQN);
        let kick = set.get("kick").unwrap();
        assert_eq!(kick.to_text(), "x---x---x---x---", "techno kick must be four-on-the-floor");
        assert_eq!(kick.len(), 16);
    }

    #[test]
    fn breakcore_is_dense_and_has_ratchets() {
        let set = Style::Breakcore.percussion(PPQN);
        let kick = set.get("kick").unwrap();
        let hat = set.get("hat").unwrap();
        assert!(kick.density() > 0.35, "breakcore kick should be dense, got {}", kick.density());
        assert!(
            hat.onset_count() >= 9 && hat.onset_count() <= 14,
            "breakcore hats should be a dense slice of the sixteenth grid, got {}",
            hat.onset_count()
        );
        assert!(hat.steps().iter().any(|s| s.ratchet > 1), "breakcore needs at least one roll");
    }

    #[test]
    fn glitch_uses_polymeter_and_microtiming() {
        let set = Style::Glitch.percussion(PPQN);
        let kick = set.get("kick").unwrap();
        let hat = set.get("hat").unwrap();
        assert_ne!(kick.len(), hat.len(), "glitch must use different lane lengths");
        assert_eq!(set.cycle_steps(), lcm(kick.len() as u64, hat.len() as u64));
        assert!(
            set.cycle_steps() > 100,
            "the polymeter should not repeat quickly: {}",
            set.cycle_steps()
        );
        assert!(
            kick.steps().iter().any(|s| s.offset_ticks != 0),
            "glitch must be microtimed: {}",
            kick.to_text()
        );
    }

    #[test]
    fn driving_hats_are_sixteenths_with_accents() {
        let set = Style::Driving.percussion(PPQN);
        let hat = set.get("hat").unwrap();
        // Fourteen of sixteen: a driving sixteenth hat that still has two gaps, so it reads as a
        // pattern rather than a hiss. The accents and the velocity spread are what give it shape.
        assert_eq!(hat.onset_count(), 14, "driving hat: {}", hat.to_text());
        assert!(hat.onset_count() < hat.len(), "a lane of all onsets is not a rhythm");
        assert!(hat.steps().iter().any(|s| s.accent), "driving hats need accents to avoid a hiss");
        assert!(
            hat.steps().iter().any(|s| s.velocity < 1.0),
            "driving hats need dynamic variation"
        );
        let vels: std::collections::BTreeSet<i32> =
            hat.steps().iter().filter(|s| s.is_on()).map(|s| (s.velocity * 100.0) as i32).collect();
        assert!(vels.len() >= 3, "driving hats need at least three velocity levels, got {vels:?}");
    }

    #[test]
    fn seed_zero_reproduces_the_preset_exactly() {
        // This is what makes `--seed 0` a stable golden reference: no mutation means no drift.
        for style in Style::all() {
            for chain in preset_chains() {
                let (set, log) = variation(style, PPQN, &chain.1, 0);
                assert!(log.is_empty(), "seed 0 must not report any mutation");
                assert_eq!(set, style.percussion(PPQN), "{style:?} changed under seed 0");
            }
        }
    }

    #[test]
    fn variation_is_reproducible_from_its_seed() {
        for style in Style::all() {
            let chain = &preset_chains()[0].1;
            let (a, la) = variation(style, PPQN, chain, 0xA17E);
            let (b, lb) = variation(style, PPQN, chain, 0xA17E);
            assert_eq!(a, b, "{style:?} variation is not reproducible");
            assert_eq!(la, lb);
        }
    }

    #[test]
    fn different_seeds_give_different_variations() {
        let style = Style::Breakcore;
        for (name, chain) in preset_chains() {
            // A chain with no stochastic operator is deterministic by design — `canon` is
            // mirror + scale-velocity, so it must give the same result for every seed. Requiring it
            // to vary was the bug, not the chain.
            let stochastic = chain.iter().any(Op::is_stochastic);
            if !stochastic {
                let a = variation(style, PPQN, &chain, 1).0;
                let b = variation(style, PPQN, &chain, 999).0;
                assert_eq!(a, b, "deterministic chain `{name}` varied with the seed");
                continue;
            }
            // Hash the full step data, not `to_text()`: a chain whose only stochastic operator is
            // `Humanise` changes microtiming alone, which the text form cannot represent.
            let variants: Vec<Vec<Vec<crate::patterns::Step>>> = (1..12u64)
                .map(|s| {
                    let (set, _) = variation(style, PPQN, &chain, s);
                    set.lanes.iter().map(|(_, p)| p.steps().to_vec()).collect()
                })
                .collect();
            let distinct = variants.iter().filter(|v| **v != variants[0]).count() + 1;
            assert!(
                distinct >= 6,
                "stochastic chain `{name}` produced only {distinct} distinct results from 11 seeds"
            );
        }
    }

    #[test]
    fn reseeding_one_lane_does_not_change_the_other() {
        // ADR-007's promise, applied to lanes: each lane derives its seed from its own branch, so
        // the hat cannot change because the kick's seed changed.
        let style = Style::Techno;
        let chain = &preset_chains()[0].1;
        let (a, _) = variation(style, PPQN, chain, 0x1234);
        let (b, _) = variation(style, PPQN, chain, 0x1234);
        assert_eq!(a.get("kick"), b.get("kick"));
        assert_eq!(a.get("hat"), b.get("hat"));
        // And the lanes genuinely have different seeds.
        let tree = SeedTree::new(0x1234);
        assert_ne!(
            tree.child("variation").child("kick").seed(),
            tree.child("variation").child("hat").seed()
        );
    }

    #[test]
    fn every_mutation_preset_runs_on_every_style_without_panicking() {
        for style in Style::all() {
            for (name, chain) in preset_chains() {
                for seed in [1u64, 7, 0xA17E, u64::MAX] {
                    let (set, log) = variation(style, PPQN, &chain, seed);
                    assert_eq!(
                        log.len(),
                        set.lanes.len(),
                        "{style:?}/{name} logged the wrong number of lanes"
                    );
                    for (lane, p) in &set.lanes {
                        assert!(!p.is_empty(), "{style:?}/{name}/{lane} became empty");
                        assert!(
                            p.steps()
                                .iter()
                                .all(|s| s.velocity.is_finite() && s.offset_ticks.abs() < 10_000),
                            "{style:?}/{name}/{lane} produced an out-of-range step"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn resolve_chain_covers_every_preset_and_rejects_nonsense() {
        for name in mutation_names() {
            let chain = resolve_chain(name);
            assert!(chain.is_some(), "`{name}` is listed but does not resolve");
            assert!(!chain.unwrap().is_empty(), "`{name}` resolved to an empty chain");
        }
        assert!(resolve_chain("none").unwrap().is_empty(), "`none` must resolve to no mutation");
        assert!(resolve_chain("").unwrap().is_empty());
        assert!(resolve_chain("GRID-BREAK").is_some(), "lookup should be case-insensitive");
        assert!(resolve_chain("does-not-exist").is_none());
    }

    #[test]
    fn bass_step_wraps_and_reports_rests() {
        let style = Style::Techno;
        let len = bass_len(style) as u64;
        assert_eq!(len, 16);
        assert_eq!(bass_step_for(style, 0), Some(0), "step 0 is the root");
        assert_eq!(bass_step_for(style, 1), None, "step 1 is a rest");
        assert_eq!(bass_step_for(style, len), bass_step_for(style, 0), "the bass line must wrap");
        assert_eq!(bass_step_for(style, len * 7 + 6), Some(3), "wrapping must preserve the offset");
    }

    #[test]
    fn chain_label_is_readable_and_stable() {
        assert_eq!(chain_label(&[]), "none");
        let label = chain_label(&[Op::Thin(0.15), Op::Humanise(18)]);
        assert!(label.contains("thin"), "{label}");
        assert!(label.contains("->"), "the chain should show its order: {label}");
    }
}
