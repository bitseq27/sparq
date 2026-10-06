//! `observatory-core` — the pure core of `dat/observatory` (WO-020 plan D13).
//!
//! This crate is the instrument's logic with **no wit-bindgen, no serde, no std I/O and no
//! third-party dependency** — deliberately. It is the piece both the wasm component
//! (`observatory-wasm`, the SDK glue) and the native harness (`observatory-harness`) link, so
//! the same cell layout, the same seven renderers and the same display-list IR are what ships
//! and what the sandbox proves, with no wasmtime anywhere (plan D2/D13). The split is the
//! WO-019 "identical through both painters from the same display list" property, arranged by
//! construction: there is one display-list producer, and it lives here.
//!
//! # The three mirrors of the frozen vocabulary (why `record` is here at all)
//!
//! The frozen `data-record`/`data-value` shape lives in `docs/api/instrument-wit/wit/types.wit`.
//! It is mirrored twice already: natively host-side in `crates/sparq-streams/src/record.rs` (the
//! broker's output), and here a third time. The third mirror is not redundancy, it is the
//! boundary D13 draws — this crate cannot link `sparq-streams` (that crate carries `std::fs`,
//! serde and the live transport; the guest must stay pure and wasm-safe), and it cannot link the
//! SDK (no wit-bindgen in the core, by construction). So each boundary owns its mapping:
//!
//! * the harness maps `sparq_streams::record::DataRecord` → [`record::Record`];
//! * the wasm glue maps the WIT `data-record` (via `sources.snapshot`) → [`record::Record`].
//!
//! Both mappings are total and field-for-field, and both are drift-tested against the schema's
//! channel order ([`schema`]-shaped accessors here, `sparq_streams::schema` there). A guest that
//! misread a channel order would draw the wrong feed with no symptom, so the order is pinned by
//! test, never by memory.
//!
//! # What lives where
//!
//! * [`ir`] — the display-list IR: a field-for-field mirror of the frozen `display.wit` v1
//!   vocabulary (the eight [`ir::Item`] primitives, [`ir::Style`], the closed stroke/corner/dash
//!   enums). Token ids are strings; there is no colour/size literal anywhere, because the
//!   contract makes literal appearance *unrepresentable* (display.wit header, ADR-010 d3).
//! * [`record`] — the pure mirror of the frozen data-record vocabulary + the five observatory
//!   schema shapes as typed accessors.
//! * [`view`] — window→view transforms: a record window + a stream's registry metadata →
//!   the typed [`view::CellView`] a renderer draws. This is D13's "window→view transforms".
//! * [`layout`] — the card/cell geometry (§5.1's numbers as functions, tested).
//! * [`params`] — the 26-param struct decoded from the contract's `param-set` (normalised f32s).
//! * [`state`] — deterministic save/restore (the journal hashes it; two calls agree byte-for-byte).
//! * [`render`] — the seven renderers (§5.5) + the cell chrome (§5.3) + the ticker band (§5.5).
//! * [`wall`] — the top-level `draw`: params + per-cell views + frame-context → one display list.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod coastline;
pub mod ir;
pub mod layout;
pub mod params;
pub mod record;
pub mod render;
pub mod state;
pub mod streams_table;
pub mod view;
pub mod wall;

pub use ir::{Item, Lod, Style, Surface};
pub use layout::{CellRect, Layout, WallLayout};
pub use params::Params;
pub use record::{DataFlags, DataRecord, DataValue};
pub use view::CellView;
pub use wall::{FrameContext, Wall};

/// The instrument's id — MUST equal `identity.id` in the generated manifest (the host
/// cross-checks at registration; a module that lies about who it is breaks patch provenance).
pub const INSTRUMENT_ID: &str = "dat/observatory";

/// The one declared display's id (the manifest's `ui.displays[0].id`; `sources.snapshot` and
/// `draw`'s `frame-context.display-id` both name it).
pub const DISPLAY_ID: &str = "wall";

/// The cell grid is 4×4 at the default layout, so 16 cells — the operator's headline case
/// (§5.4). Every cell has a `cell_NN` enum param and a `cell-NN` source binding.
pub const CELL_COUNT: usize = 16;
