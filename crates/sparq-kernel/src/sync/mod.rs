//! Cross-thread communication primitives.
//!
//! Plan §4.3: all cross-thread communication uses bounded lock-free ring buffers with an explicit
//! overflow policy, and every ring exposes a drop counter. Invisible failures are forbidden.
//!
//! Two primitives, both allowlisted-`unsafe` with their invariants stated in-file:
//!
//! * [`SpscRing`] — the bounded message transport (control messages in, meters/analysis out).
//! * [`hotswap`] — ADR-009 decision 3's boundary swap: the control thread stages a complete
//!   payload, the audio thread takes it at a block boundary, and the outgoing payload is
//!   retired through an epoch-gated grace period and dropped control-side (decision 3's RCU
//!   shape, WO-008 increment 5).

pub mod hotswap;
pub mod rings;

pub use hotswap::{HotSwap, HotSwapAudio, HotSwapControl, SwapStats, RETIRE_SLOTS};
pub use rings::SpscRing;
