//! The sparq module contract (WO-007): what a module is, what it may declare, and what may
//! connect to what.
//!
//! # Where the truth lives
//!
//! The contract's *explanations* are prose: `docs/api/module-api-v1.md`, `docs/api/manifest-schema.md`
//! and ADR-005. Its *tables* are data: `docs/api/compat-matrix.toml` (what may connect to what) and
//! `docs/api/manifest-fields.toml` (every field, its domain, and the error code it produces). This
//! crate is the third representation — the one the engine compiles — and where the three disagree,
//! the data wins and this crate is wrong. `tools/contract_check.py` exists to make disagreement a
//! failed gate rather than a surprise.
//!
//! # Contract v1 (WO-008 increment 4), and the limits that remain, stated rather than discovered
//!
//! * [`module::AudioCtx`] is multi-port: audio, `cv` and `event` payloads travel, up to
//!   [`module::MAX_PORTS_PER_CLASS`] ports per type per direction — a fixed capacity, because the context is
//!   assembled on the audio thread and may not allocate.
//! * [`params::MAX_PARAMS`] parameters per module, because a snapshot must be `Copy` to travel
//!   through the kernel's lock-free ring without allocating on the audio thread. Same family:
//!   [`event::EVENTS_PER_BLOCK`] events per output port per block.
//! * `data` and `gpu` port *payloads* are declared and validated but still not carried (`data`
//!   waits for the schema/staleness machinery; `gpu` is never an audio-thread payload — Phase 0's
//!   `ana/tap` and `dsp/scope` will exercise it through the ring publication). `atom` never
//!   travels through `AudioCtx` at all: it is the control-thread [`module::Module::message`] door.
//! * The compatibility matrix lives twice — prose-checked data in `docs/api/compat-matrix.toml`
//!   and compiled functions in [`port`] — and `tests/compat_matrix.rs` is the drift gate that
//!   pins the two together. Deleting the mirror outright needs the ratified TOML's prose `when`
//!   cells restructured into machine predicates first; that decision is escalated, not taken
//!   silently here.
//!
//! # Real-time discipline
//!
//! [`module::Module::process`] receives a [`module::AudioCtx`] that exposes no allocator, no clock,
//! no filesystem and no lock — the rule is the absence of an API, not a comment (module-api §9).
//! Because Rust cannot forbid `Vec::new()` by types alone, the claim is *measured*: `tests/contract.rs`
//! installs the kernel's counting allocator as its global allocator and fails if a reference module
//! allocates once inside `process`.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

pub mod decode;
pub mod discovery;
pub mod error;
pub mod event;
pub mod manifest;
pub mod module;
pub mod params;
pub mod port;
pub mod registry;
pub mod toml;

pub use decode::decode;
pub use discovery::{discover, DiscoveryReport, Origin, Source};
pub use error::{CodeKind, ValidationReport};
pub use event::{Event, EventBuf, EventKind, EventSink, EVENTS_PER_BLOCK};
pub use manifest::{Manifest, ValidatedManifest};
pub use module::{
    AudioCtx, BlockStatus, CvIn, CvOut, Module, ModuleError, Resources, MAX_PORTS_PER_CLASS,
};
pub use params::{ParamBus, ParamSet, ParamSlot, MAX_PARAMS};
pub use port::{Adapter, ChannelSet, Phase, PortType, Verdict};
pub use registry::{Factory, RegisterError, Registration, Registry};
