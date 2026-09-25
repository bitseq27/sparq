//! Cross-thread communication primitives.
//!
//! Plan §4.3: all cross-thread communication uses bounded lock-free ring buffers with an explicit
//! overflow policy, and every ring exposes a drop counter. Invisible failures are forbidden.

pub mod rings;

pub use rings::SpscRing;
