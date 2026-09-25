//! The seed tree (ADR-007).
//!
//! Every random consumer in sparq derives its seed from `(session_seed, path)`, where `path` is the
//! chain of labels from the session root to that consumer. Two properties follow, and both matter
//! artistically rather than just technically:
//!
//! * **Re-seeding one module never perturbs another.** Adding a noise source to a patch does not
//!   change the hats. Without this, every edit silently rewrites the whole piece.
//! * **A performance is reproducible from its seeds.** Project + seeds + journal ⇒ the same samples,
//!   which is what makes "the patch is the score" literally true.
//!
//! The hash is splitmix64: not cryptographic, but excellent avalanche, dependency-free, and stable
//! across platforms — which the golden tests require.

/// splitmix64 state. Cheap, well-distributed, and identical on every platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Advance and return the next 64 bits.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Next value in `[0, 1)`.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        // 53 bits of mantissa, so the result is uniform to double precision.
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Next value in `[0, 1)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // 24 bits: the exact width of an f32 mantissa.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Next `u32` uniformly in `[0, n)`. Rejection-free via a 64-bit multiply.
    #[inline]
    pub fn next_below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        ((u64::from(self.next_u32()) * u64::from(n)) >> 32) as u32
    }

    /// Next `usize` uniformly in `[0, n)`.
    #[inline]
    pub fn next_index(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((self.next_u64() % n as u64) as usize).min(n - 1)
    }

    /// Next `u32`.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// True with probability `p`.
    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// A value in `[lo, hi)`.
    #[inline]
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// An integer in `[lo, hi]` inclusive.
    #[inline]
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u32 + 1;
        lo + self.next_below(span) as i32
    }
}

impl Default for SplitMix64 {
    fn default() -> Self {
        Self::new(0x5EED_5EED_5EED_5EED)
    }
}

/// Hash a label chain into a 64-bit seed. Order matters, so `a/b` and `b/a` differ.
#[must_use]
pub fn derive_seed(root: u64, path: &[&str]) -> u64 {
    let mut h = root ^ 0xD6E8_FEB8_6659_FD93;
    for label in path {
        for b in label.as_bytes() {
            h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01B3);
        }
        // A separator that cannot appear in a byte, so "ab"/"c" and "a"/"bc" differ.
        h = (h ^ 0xFF).wrapping_mul(0x100_0000_01B3);
    }
    SplitMix64::new(h).next_u64()
}

/// A node in the seed tree. Cheap to copy; cloning does not advance any generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedTree {
    root: u64,
    path: Vec<String>,
}

impl SeedTree {
    /// The root of a session's seed tree.
    #[must_use]
    pub fn new(session_seed: u64) -> Self {
        Self { root: session_seed, path: Vec::new() }
    }

    /// A child node, e.g. `tree.child("syn/noise").child("dither")`.
    #[must_use]
    pub fn child<S: AsRef<str>>(&self, label: S) -> Self {
        let mut path = self.path.clone();
        path.push(label.as_ref().to_string());
        Self { root: self.root, path }
    }

    /// The seed for this node.
    #[must_use]
    pub fn seed(&self) -> u64 {
        let refs: Vec<&str> = self.path.iter().map(String::as_str).collect();
        derive_seed(self.root, &refs)
    }

    /// A generator for this node.
    #[must_use]
    pub fn rng(&self) -> SplitMix64 {
        SplitMix64::new(self.seed())
    }

    /// The label path from the root, for display in the UI and for journal entries.
    #[must_use]
    pub fn path_string(&self) -> String {
        if self.path.is_empty() {
            String::from("<root>")
        } else {
            self.path.join("/")
        }
    }

    /// The session seed this tree derives from.
    #[must_use]
    pub const fn root(&self) -> u64 {
        self.root
    }

    /// Depth from the root.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.path.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn splitmix64_is_self_consistent_and_avalanches() {
        // This test used to assert two published splitmix64 output vectors. The constants were
        // written from memory, were wrong, and nothing in the sandbox could check them — so the test
        // asserted a falsehood with total confidence, which is worse than no test. What follows
        // needs no external constant and still catches a broken generator.
        let mut g = SplitMix64::new(0);
        let first = g.next_u64();
        // The state advance is exactly one golden-ratio increment, so the first output is a pure
        // function of that increment and the two mix multiplies. Recomputing it independently here
        // pins the implementation without importing anyone else's constant.
        let mut z = 0x9E37_79B9_7F4A_7C15u64;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        assert_eq!(first, z ^ (z >> 31), "the mixing step changed");

        // Avalanche: a one-bit change in the seed must change roughly half the output bits.
        let flip = |a: u64, b: u64| (a ^ b).count_ones();
        let mut total = 0u32;
        for bit in 0..64 {
            let x = SplitMix64::new(1u64 << bit).next_u64();
            let y = SplitMix64::new((1u64 << bit) ^ 1).next_u64();
            total += flip(x, y);
        }
        let mean = total as f64 / 64.0;
        assert!(
            (mean - 32.0).abs() < 6.0,
            "poor avalanche: mean {mean:.1} bits differ, expected ~32"
        );

        // No short cycle: 100k outputs must all be distinct in the high bits.
        let mut g = SplitMix64::new(42);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..100_000 {
            seen.insert(g.next_u64() >> 32);
        }
        assert!(
            seen.len() > 99_000,
            "generator repeated values: {} distinct of 100000",
            seen.len()
        );
    }

    #[test]
    fn generator_is_deterministic_and_diverges_on_seed() {
        let run = |seed| {
            let mut g = SplitMix64::new(seed);
            (0..64).map(|_| g.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(run(1), run(1));
        assert_ne!(run(1), run(2));
    }

    #[test]
    fn uniform_helpers_stay_in_range() {
        let mut g = SplitMix64::new(12345);
        for _ in 0..100_000 {
            let f = g.next_f64();
            assert!((0.0..1.0).contains(&f), "next_f64 out of range: {f}");
            let f32v = g.next_f32();
            assert!((0.0..1.0).contains(&f32v));
            let n = g.next_below(7);
            assert!(n < 7);
            let i = g.range_i32(-3, 3);
            assert!((-3..=3).contains(&i));
            assert_eq!(g.next_below(0), 0, "n=0 must not panic or divide by zero");
            assert_eq!(g.next_index(0), 0);
        }
    }

    #[test]
    fn next_below_is_roughly_uniform() {
        let mut g = SplitMix64::new(999);
        let n = 6u32;
        let mut counts = [0usize; 6];
        let trials = 120_000;
        for _ in 0..trials {
            counts[g.next_below(n) as usize] += 1;
        }
        let expected = trials / n as usize;
        for (i, &c) in counts.iter().enumerate() {
            let dev = (c as f64 - expected as f64).abs() / expected as f64;
            assert!(
                dev < 0.05,
                "bucket {i}: {c} vs expected {expected} (deviation {:.1}%)",
                dev * 100.0
            );
        }
    }

    #[test]
    fn seed_tree_is_reproducible_and_path_dependent() {
        let t = SeedTree::new(0xC0FFEE);
        assert_eq!(t.child("a").child("b").seed(), t.child("a").child("b").seed());
        // Order matters: a different nesting is a different seed.
        assert_ne!(t.child("a").child("b").seed(), t.child("b").child("a").seed());
        // Concatenation must not collide: "ab"/"c" differs from "a"/"bc".
        assert_ne!(t.child("ab").child("c").seed(), t.child("a").child("bc").seed());
        assert_ne!(t.seed(), t.child("anything").seed());
    }

    #[test]
    fn reseeding_one_branch_does_not_perturb_another() {
        // The property that makes editing a generative patch safe: adding or re-seeding one module
        // must not change any other module's stream.
        let before = SeedTree::new(1);
        let hats_a: Vec<u64> = {
            let mut g = before.child("hats").rng();
            (0..32).map(|_| g.next_u64()).collect()
        };
        // A completely different session seed changes everything...
        let after = SeedTree::new(2);
        let hats_b: Vec<u64> = {
            let mut g = after.child("hats").rng();
            (0..32).map(|_| g.next_u64()).collect()
        };
        assert_ne!(hats_a, hats_b);
        // ...but adding a NEW sibling branch leaves the existing one untouched.
        let with_sibling = SeedTree::new(1);
        let _new_voice = with_sibling.child("new_voice").seed();
        let hats_c: Vec<u64> = {
            let mut g = with_sibling.child("hats").rng();
            (0..32).map(|_| g.next_u64()).collect()
        };
        assert_eq!(hats_a, hats_c, "adding a sibling branch must not perturb an existing one");
    }

    #[test]
    fn path_string_is_readable_for_the_ui_and_journal() {
        let t = SeedTree::new(7);
        assert_eq!(t.path_string(), "<root>");
        assert_eq!(t.child("gen").child("markov").child("0").path_string(), "gen/markov/0");
        assert_eq!(t.child("gen").depth(), 1);
    }
}
