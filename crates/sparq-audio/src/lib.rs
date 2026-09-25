//! sparq DSP primitives.
//!
//! Phase 0 (WO-005/WO-014): the block graph, two DSP nodes, WAV output and the deterministic hash
//! used by the golden-reference harness (ADR-007).
//!
//! Everything here is allocation-free in `process` and deterministic: the same configuration and
//! seed produce bit-identical output on any machine.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

pub mod dna;
pub mod dsp;
pub mod executor;
pub mod graph;
pub mod hash;
pub mod modules;
pub mod mutation;
pub mod patch;
pub mod patterns;
pub mod presets;
pub mod wav;

pub use dna::{DnaNode, DnaTree};
pub use dsp::{Gain, SineOsc};
pub use graph::{GraphConfig, Wo005Graph};
pub use hash::fnv1a64_f32;
pub use mutation::{apply as apply_op, apply_chain, preset_chains, Op};
pub use patch::{demo_stream, render_demo, DemoConfig};
pub use patterns::{gcd, lcm, Pattern, PatternSet, Step};
pub use presets::{
    bass_len, bass_step_for, chain_label, mutation_names, resolve_chain, variation, Style,
};
