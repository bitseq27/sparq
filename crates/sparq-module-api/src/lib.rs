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
//! # v0 limits, stated rather than discovered
//!
//! * One interleaved audio input and one interleaved audio output per [`module::AudioCtx`].
//!   Multi-port buffers arrive with the WO-008 executor, which owns buffer pooling.
//! * [`params::MAX_PARAMS`] parameters per module, because a snapshot must be `Copy` to travel
//!   through the kernel's lock-free ring without allocating on the audio thread.
//! * Manifests are validated from Rust values. The TOML reader is a separate increment: it needs a
//!   dependency-free subset parser (the core build has zero third-party dependencies), and once it
//!   exists the compatibility matrix can be read from the TOML at discovery instead of being
//!   duplicated here at all.
//! * `data` and `gpu` port *payloads* are declared and validated but not yet carried; `event`
//!   payloads are declared and not yet carried. Phase 0's `ana/tap` and `dsp/scope` are what will
//!   exercise `gpu` (ADR-005 addendum: the port type is not deferred, only the tier question).
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
pub mod manifest;
pub mod module;
pub mod params;
pub mod port;
pub mod registry;
pub mod toml;

pub use decode::decode;
pub use discovery::{discover, DiscoveryReport, Origin, Source};
pub use error::{CodeKind, ValidationReport};
pub use manifest::{Manifest, ValidatedManifest};
pub use module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
pub use params::{ParamBus, ParamSet, ParamSlot, MAX_PARAMS};
pub use port::{Adapter, ChannelSet, Phase, PortType, Verdict};
pub use registry::{Factory, RegisterError, Registration, Registry};
