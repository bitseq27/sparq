//! The instrument host (WO-018) — the door third-party packages come through.
//!
//! ADR-010 made instruments a hand-off: a five-file package dropped in `instruments/` loads at
//! launch without intervention, and `sparq mod validate` is the gate that stands in for a review
//! queue. This crate is the host side of that deal, in two halves:
//!
//! * **The static half (this build, zero dependencies).** The package model and its cap
//!   ([`package`]), drop-in discovery in module-api §11's precedence slot ([`package::discover`]),
//!   the `gpu_class` ceiling table that closes contract v1.1's freeze debt #1 ([`ceilings`]), and
//!   the hand-in gate's static stages ([`validate`]) — schema, package, declared budgets,
//!   signature/badge. Everything here runs in the default `cargo test --workspace`: the gate is
//!   provable on a runner that will never host a wasm sandbox.
//! * **The runtime half (feature `instrument-host`, wasmtime).** The loader, fuel/epoch wired to
//!   the block watchdog, hot reload at block boundaries through the boundary-swap engine, and the
//!   gate stages that need a live guest (smoke, golden render, budget measurement, visual
//!   conformance). The design, the two-instance `configure` ordering that closes freeze debt #2,
//!   and the increment's test plan are `docs/instrument-host.md`. Until it lands, the static gate
//!   REFUSES those stages in words ([`validate::Verdict::Refused`]) — a partial gate never
//!   pretends to be the whole one.
//!
//! # House rules this crate inherits
//!
//! The failure vocabulary is the contract crate's ([`sparq_module_api::error`]): every refusal
//! names the field, the value found, the values allowed and the fix, and reports are shown
//! verbatim in the module browser. One broken package never hides the others. Nothing here runs
//! on the audio thread — discovery and validation are control-thread doors, so filesystem access
//! is legal here (it is banned from `sparq-module-api` on purpose) but the `clippy.toml` bans on
//! `std::fs::read`/`write` and wall clocks still apply: reads go through `read_to_string` and
//! `metadata`, and no stage times anything (the measurement stages are the runtime half's).

#![allow(clippy::missing_docs_in_private_items)]

pub mod ceilings;
pub mod package;
pub mod sources;
pub mod validate;

pub use ceilings::{ceilings_for, GpuCeilings};
pub use package::{discover, Inventory, Package, Profile, Role};
pub use validate::{schema_check, GateReport, Stage, StageOutcome, Verdict, HOST_MODULE_API};
