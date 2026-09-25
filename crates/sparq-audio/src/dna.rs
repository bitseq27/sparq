//! `gen/dna-tree` — mutation lineage.
//!
//! Plan §10.5 calls this the strongest defence against "generative mush", and the reasoning is worth
//! restating because it is the reason this module exists at all:
//!
//! > Random exploration produces a fog. You mutate, something gets better, you mutate again, and
//! > twenty minutes later you have no idea how you got there or how to get back. A **lineage** turns
//! > that into a tree you can navigate: every node records the operator, the seed and a fingerprint
//! > of the result, so any branch can be replayed, compared, pruned or exported.
//!
//! The tree is therefore not a history log. It is a *navigable score*: `export_score()` emits the
//! operator chain plus seeds that reproduces a leaf from the root, which is exactly the artefact
//! plan §10.5 means by "export a lineage as a generative score".
//!
//! Keeping nodes append-only and immutable is what makes it a tree rather than a smear — the same
//! property `mutation::apply` has (it never modifies its input).

use crate::hash::{fnv1a64, hex64};

/// One mutation in the lineage.
#[derive(Clone, Debug, PartialEq)]
pub struct DnaNode {
    /// Unique id within the tree.
    pub id: u64,
    /// The node this one was derived from. `None` for the root.
    pub parent: Option<u64>,
    /// The operator applied, as its label (see `mutation::Op::label`).
    pub op: String,
    /// The seed the operator used. Meaningless for deterministic operators, but recorded anyway so
    /// the score is replayable without needing to know which operators were stochastic.
    pub seed: u64,
    /// Fingerprint of the resulting pattern, so two nodes can be compared without holding the data.
    pub pattern_hash: String,
    /// Whether the author marked this node as worth keeping.
    pub kept: bool,
    /// Monotonic creation counter — not a wall clock, so the tree stays deterministic (ADR-007).
    pub order: u64,
}

/// A mutation lineage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DnaTree {
    nodes: Vec<DnaNode>,
}

impl DnaTree {
    /// An empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add the root: the pattern before any mutation.
    pub fn root(&mut self, pattern_text: &str, seed: u64) -> u64 {
        self.push(None, "<root>".into(), seed, pattern_text)
    }

    /// Record a mutation. Returns the new node's id.
    ///
    /// `parent` must already exist; recording against an unknown parent is a programming error and
    /// is refused rather than silently re-rooted, because a broken lineage is worse than no lineage.
    pub fn record(
        &mut self,
        parent: u64,
        op: String,
        seed: u64,
        pattern_text: &str,
    ) -> Option<u64> {
        self.node(parent)?;
        Some(self.push(Some(parent), op, seed, pattern_text))
    }

    fn push(&mut self, parent: Option<u64>, op: String, seed: u64, pattern_text: &str) -> u64 {
        let id = self.nodes.len() as u64;
        self.nodes.push(DnaNode {
            id,
            parent,
            op,
            seed,
            pattern_hash: hex64(fnv1a64(pattern_text.as_bytes())),
            kept: false,
            order: id,
        });
        id
    }

    /// A node by id.
    #[must_use]
    pub fn node(&self, id: u64) -> Option<&DnaNode> {
        self.nodes.get(id as usize)
    }

    /// All nodes, in creation order.
    #[must_use]
    pub fn nodes(&self) -> &[DnaNode] {
        &self.nodes
    }

    /// How many mutations have been recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree has no nodes at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Mark a node as kept or discarded. This is the human-in-the-loop fitness signal from
    /// plan §10.5: you tap "keep" while auditioning, and the tree becomes searchable by taste.
    pub fn set_kept(&mut self, id: u64, kept: bool) -> bool {
        match self.nodes.get_mut(id as usize) {
            Some(n) => {
                n.kept = kept;
                true
            },
            None => false,
        }
    }

    /// The chain of ancestors from the root down to `id`, inclusive.
    #[must_use]
    pub fn lineage(&self, id: u64) -> Vec<&DnaNode> {
        let mut out = Vec::new();
        let mut cur = Some(id);
        // The parent chain is acyclic by construction (a node can only point at a lower id), but the
        // bound makes a corrupted tree a truncated result rather than an infinite loop.
        let mut guard = self.nodes.len() + 1;
        while let Some(c) = cur {
            if guard == 0 {
                break;
            }
            guard -= 1;
            match self.node(c) {
                Some(n) => {
                    out.push(n);
                    cur = n.parent;
                },
                None => break,
            }
        }
        out.reverse();
        out
    }

    /// Direct children of `id`.
    #[must_use]
    pub fn children(&self, id: u64) -> Vec<&DnaNode> {
        self.nodes.iter().filter(|n| n.parent == Some(id)).collect()
    }

    /// Every descendant of `id`, in creation order.
    #[must_use]
    pub fn descendants(&self, id: u64) -> Vec<&DnaNode> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            for c in self.children(cur) {
                out.push(c);
                stack.push(c.id);
            }
        }
        out.sort_by_key(|n| n.order);
        out
    }

    /// Depth of `id` from the root (the root is depth 0).
    #[must_use]
    pub fn depth(&self, id: u64) -> usize {
        self.lineage(id).len().saturating_sub(1)
    }

    /// Nodes marked kept, in creation order.
    #[must_use]
    pub fn kept(&self) -> Vec<&DnaNode> {
        self.nodes.iter().filter(|n| n.kept).collect()
    }

    /// The deepest node, which is where a "keep going" gesture continues from.
    #[must_use]
    pub fn deepest(&self) -> Option<&DnaNode> {
        self.nodes.iter().max_by_key(|n| self.depth(n.id))
    }

    /// Export the lineage to `id` as a replayable score: the ordered operator chain with seeds.
    ///
    /// This is the artefact plan §10.5 calls a *generative score* — apply these operators, with
    /// these seeds, to the root pattern, and you get this leaf back, forever, on any machine.
    #[must_use]
    pub fn export_score(&self, id: u64) -> String {
        let lineage = self.lineage(id);
        let mut out = String::from("# sparq generative score\n");
        out.push_str(
            "# apply these operators in order to the root pattern, with a fresh RNG per line\n",
        );
        for n in &lineage {
            if n.parent.is_none() {
                out.push_str(&format!("root  seed=0x{:X}  hash={}\n", n.seed, n.pattern_hash));
            } else {
                out.push_str(&format!(
                    "op    {:<24} seed=0x{:X}  hash={}{}\n",
                    n.op,
                    n.seed,
                    n.pattern_hash,
                    if n.kept { "  # kept" } else { "" }
                ));
            }
        }
        out
    }

    /// An indented text rendering of the whole tree, for the CLI and the `dsp/dna-view` display.
    ///
    /// Depth is encoded by indentation and by a greyscale ramp in the UI (`colormaps.toml
    /// [dna_tree.depth]`), never by hue — the tree must read in monochrome (look-board §10).
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.nodes.first() {
            self.render_into(root.id, 0, &mut out);
        }
        out
    }

    fn render_into(&self, id: u64, depth: usize, out: &mut String) {
        let Some(n) = self.node(id) else { return };
        let indent = "  ".repeat(depth);
        let mark = if n.kept { "*" } else { " " };
        if n.parent.is_none() {
            out.push_str(&format!(
                "{indent}{mark}root  {}\n",
                &n.pattern_hash[..8.min(n.pattern_hash.len())]
            ));
        } else {
            out.push_str(&format!(
                "{indent}{mark}{}  {}  0x{:X}\n",
                n.op,
                &n.pattern_hash[..8.min(n.pattern_hash.len())],
                n.seed
            ));
        }
        for c in self.children(id) {
            self.render_into(c.id, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn tree() -> DnaTree {
        let mut t = DnaTree::new();
        let root = t.root("x---x---x---x---", 0xA17E);
        let a = t.record(root, "thin 0.30".into(), 1, "x---x-------x---").unwrap();
        let b = t.record(root, "markov 0.70".into(), 2, "x--x-x--x-x---x-").unwrap();
        let c = t.record(a, "ca r110 g2".into(), 3, "x-x---x-x---x-x-").unwrap();
        t.record(c, "humanise ±20".into(), 4, "x-x---x-x---x-x-").unwrap();
        t.set_kept(b, true);
        t.set_kept(c, true);
        t
    }

    #[test]
    fn ids_are_sequential_and_parents_are_valid() {
        let t = tree();
        assert_eq!(t.len(), 5);
        for n in t.nodes() {
            assert_eq!(n.id as usize, n.order as usize);
            if let Some(p) = n.parent {
                assert!(p < n.id, "a node pointed at a later node: {} -> {p}", n.id);
                assert!(t.node(p).is_some(), "node {} has a missing parent {p}", n.id);
            }
        }
    }

    #[test]
    fn recording_against_an_unknown_parent_is_refused() {
        let mut t = DnaTree::new();
        assert!(t.record(999, "thin".into(), 1, "x-").is_none(), "must not silently re-root");
        assert!(t.is_empty());
    }

    #[test]
    fn lineage_runs_from_the_root_down() {
        let t = tree();
        let leaf = 3; // the child of node 1
        let l = t.lineage(leaf);
        assert_eq!(l.len(), 3, "root -> thin -> ca, got {}", l.len());
        assert!(l[0].parent.is_none(), "lineage must start at the root");
        assert_eq!(l[l.len() - 1].id, leaf);
        assert_eq!(t.depth(leaf), 2);
        assert_eq!(t.depth(0), 0);
    }

    #[test]
    fn children_and_descendants_are_correct() {
        let t = tree();
        assert_eq!(t.children(0).len(), 2, "the root has two direct children");
        assert_eq!(t.descendants(0).len(), 4, "the root has four descendants in total");
        assert_eq!(t.descendants(1).len(), 2, "node 1 has the ca node and its child");
        assert!(t.children(4).is_empty(), "a leaf has no children");
    }

    #[test]
    fn kept_marks_survive_and_are_queryable() {
        let t = tree();
        let kept = t.kept();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|n| n.kept));
        assert!(kept.iter().any(|n| n.op == "markov 0.70"));
    }

    #[test]
    fn set_kept_reports_failure_for_an_unknown_id() {
        let mut t = tree();
        assert!(t.set_kept(1, true));
        assert!(!t.set_kept(9999, true), "marking a nonexistent node must report failure");
    }

    #[test]
    fn the_same_pattern_hashes_the_same_and_different_patterns_differ() {
        let mut t = DnaTree::new();
        let r = t.root("x---x---", 1);
        let a = t.record(r, "identity".into(), 2, "x---x---").unwrap();
        let b = t.record(r, "thin".into(), 3, "x-------").unwrap();
        assert_eq!(t.node(r).unwrap().pattern_hash, t.node(a).unwrap().pattern_hash);
        assert_ne!(t.node(r).unwrap().pattern_hash, t.node(b).unwrap().pattern_hash);
    }

    #[test]
    fn the_exported_score_is_a_replayable_recipe() {
        let t = tree();
        let score = t.export_score(4); // the deepest leaf
        assert!(score.starts_with("# sparq generative score"), "score needs a header");
        assert!(score.contains("root"), "score must name the root");
        // Every operator in the lineage must appear, in order.
        let order: Vec<&str> = vec!["thin 0.30", "ca r110 g2", "humanise ±20"];
        let mut last = 0usize;
        for op in order {
            let at = score.find(op).unwrap_or_else(|| panic!("score is missing `{op}`:\n{score}"));
            assert!(at > last, "operators are out of order in the score:\n{score}");
            last = at;
        }
        // Seeds must be present so the score is actually reproducible.
        assert!(score.contains("seed=0x"), "score must record seeds:\n{score}");
        assert!(score.contains("# kept"), "kept marks should be visible in the score");
    }

    #[test]
    fn render_text_is_indented_by_depth() {
        let t = tree();
        let text = t.render_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5, "one line per node:\n{text}");
        // Every line carries a kept/dropped mark before its text, so the root line begins with a
        // single space; what must be true is that it has no depth indentation.
        assert!(!lines[0].starts_with("  "), "the root must not be indented: {:?}", lines[0]);
        assert!(lines[1].starts_with("  "), "depth 1 is indented once");
        assert!(lines[3].starts_with("    "), "depth 2 is indented twice:\n{text}");
        assert!(text.contains('*'), "kept nodes are starred");
    }

    #[test]
    fn deepest_finds_the_longest_branch() {
        let t = tree();
        let d = t.deepest().unwrap();
        assert_eq!(t.depth(d.id), 3, "the root->thin->ca->humanise branch is three deep");
    }

    #[test]
    fn an_empty_tree_is_safe_to_query() {
        let t = DnaTree::new();
        assert!(t.is_empty());
        assert!(t.node(0).is_none());
        assert!(t.lineage(0).is_empty());
        assert!(t.deepest().is_none());
        assert!(t.render_text().is_empty());
        // The header line contains the word "root", so `contains("root")` is trivially true; the
        // real assertion is that no *root entry* was emitted for a tree that has no root.
        assert!(!t.export_score(0).lines().any(|l| l.starts_with("root")));
        assert!(t.export_score(0).lines().count() <= 2, "an empty tree exports only its header");
    }

    #[test]
    fn a_deep_chain_does_not_recurse_into_a_stack_overflow() {
        // render_into is recursive, so a long lineage is the thing most likely to break it.
        let mut t = DnaTree::new();
        let mut parent = t.root("x", 0);
        for i in 0..2000 {
            parent = t.record(parent, format!("op {i}"), i as u64, "x").unwrap();
        }
        assert_eq!(t.len(), 2001);
        assert_eq!(t.depth(parent), 2000);
        let text = t.render_text();
        assert_eq!(text.lines().count(), 2001);
        assert!(t.export_score(parent).lines().count() > 2000);
    }
}
