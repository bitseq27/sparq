//! sparq real-time kernel.
//!
//! Owns time, threads, memory discipline, block processing and device abstraction (plan §5).
//! This crate is the **only** place in the workspace permitted to contain `unsafe`, and only
//! inside the modules listed in `docs/unsafe-allowlist.md`.
//!
//! Phase 0 scope (WO-000 + WO-005 + WO-006 in progress): block context, three-clock model,
//! real-time discipline (allocation counting, MMCSS/priority/working-set setup), lock-free SPSC
//! rings, the null/virtual device that makes every layer above the HAL testable without audio
//! hardware (ADR-009), and the HAL itself: trait, diagnostics, null backend and — behind the
//! `hal-wasapi` feature on Windows — the WASAPI exclusive/shared backends.
//!
//! WO-008 increment 1 adds [`graph`]: the structural patch graph — nodes, edges, the
//! `unit_delay`/`block_delay` cycle vocabulary of plan §5.4, the topology version, and the
//! cached deterministic topological order of ADR-009 decision 1. Deliberately absent until
//! their work orders (or their increments): the executor loop, buffer pool and RCU swap
//! (WO-008 tasks 2–7), the journal (WO-011), the ASIO backend (WO-006 increment 2), arenas and
//! the watchdog.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::undocumented_unsafe_blocks
)]

pub mod alloc;
pub mod block;
pub mod clock;
pub mod device;
pub mod graph;
pub mod hal;
pub mod rt;
pub mod seed;
pub mod sync;

/// Workspace version, for diagnostics readouts.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
