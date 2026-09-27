//! `sparq-music` — clocks and transport v0 (WO-009): the stream-facing half of ADR-006's
//! three-clock model.
//!
//! # The layering, stated so it stays true
//!
//! * The **kernel** owns the pure map: [`sparq_kernel::clock::Clock`] is the piecewise
//!   sample↔musical function with smoothed derivative (ADR-006 rule 2). It knows nothing about
//!   playing, stopping or walls.
//! * **This crate** owns the broker and the transport: [`broker::ClockBroker`] adds the stream
//!   position and the `t_wall` side (drift estimation, rule 4's quantisation input), and
//!   [`transport::Transport`] is the state machine — play/stop, tempo (now, and by construction
//!   a stream: every change is a map segment), loop region, tap tempo, the tick-scheduled event
//!   queue (rule 7: ±0 samples) and the bar/beat trigger emission that makes the transport an
//!   event SOURCE for the graph.
//! * The **executor** stays below: `Transport` does not know about graphs. The driver — the
//!   offline render loop today, the HAL pump later — calls [`transport::Transport::advance_block`]
//!   per block and forwards the bounded [`transport::BlockEvents`] it fills to
//!   `sparq_audio::executor::Executor::push_host_event` (the contract-v1 control-side door).
//!   Five lines of glue, in the app, which is where wiring belongs.
//!
//! # Real-time discipline
//!
//! [`transport::Transport::advance_block`] and [`transport::BlockEvents`] allocate NOTHING: the
//! collector is a fixed 64-slot array (matching the contract's `EVENTS_PER_BLOCK`), overflow is
//! counted, and the schedule cursor only walks. Mutating calls (`schedule`, `set_tempo`,
//! `set_loop`, `tap`) are control-thread work and may allocate — they are the tempo-automation
//! surface, and ADR-006's rule for them is continuity, which the kernel map enforces.
//!
//! # What v0 does NOT contain (declared, per the work order)
//!
//! External sync / the slave PLL (Phase 4 — the broker's `observe_wall` is the door it will
//! knock on), metric-modulation UI, per-track time signatures, count-in, metronome, recording.
//! Loop folding happens at BLOCK granularity (the fold is applied at the end of the block that
//! crosses the loop end): sub-block transport positions are the same forbidden territory as
//! sub-block processing (module-api §16 Q2) — the remedy for tighter loops is a smaller host
//! block, and the limit is declared here rather than discovered in a performance.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

pub mod broker;
pub mod transport;

pub use broker::ClockBroker;
pub use transport::{BeatConfig, BlockEvents, Transport, BLOCK_EVENTS_CAP};
