//! The graph canvas (WO-013): editing a patch by touch, in the token language.
//!
//! Like the rest of `sparq-ui`, this module is **computed, not drawn**. It owns five things and
//! nothing else:
//!
//! * [`model`] — the graph as data (nodes, wires, flags, **per-node param values**), every
//!   mutation an [`model::Op`] with an inverse, and an undo/redo stack. This is the surface the
//!   WO-008 executor will eventually read; until then it is the canvas's own in-memory truth.
//! * [`camera`] — pan / zoom / LOD and the zoom-to-fit computation, clamped to the canvas tokens.
//! * [`layout`] — one pure pass from (graph, camera, canvas rect) to screen-space rects, port
//!   positions and wire béziers, plus hit-testing. The egui painter in `sparq-app` draws exactly
//!   this and nothing it invents.
//! * [`connect`] — the connection verdict, delegated to `sparq-module-api`'s own `connect_*`
//!   functions so the compatibility matrix keeps its single copy (the discipline WO-007 shipped).
//! * [`browser`] — the module browser: fuzzy ranking over a shell-supplied catalogue and the
//!   sheet geometry, so the row you see is the row you touch.
//! * [`inspector`] — the parameter panel: computed row/slider geometry for the selected node,
//!   under a clamped scroll offset (a fixed header, rows that slide under it).
//! * [`entry`] — text entry: the single-line buffer and sheet geometry the rename modal runs on,
//!   the surface the browser's provisional keyboard feed was declared to be waiting for.
//! * [`scope`] — the `dsp/scope` display model: rolling trace accumulation, trigger alignment
//!   and polyline geometry, toolkit-independent — the manifest's "the UI accumulates blocks up
//!   to the timebase" made code (WO-013 increment 6).
//! * [`inset`] — the node inset displays (WO-012 increment 5): the well registry that says which
//!   module wears which well, and the param → shape mappings (the envelope triangle, the LFO
//!   period) the wells draw at rest.
//! * [`response`] — the response-plot model (WO-012 increment 5): log-f × dB axes, the
//!   magnitude-grid → polyline mapping and the probe marker's geometry. It CONSUMES magnitudes;
//!   the curve contract's DSP lives in the module that owns it (`SvfFilter::magnitude_at`).
//! * [`interact`] — [`interact::CanvasState`], the intent→operation table: gestures in, ops and
//!   *explained* refusals out.
//!
//! The rule from the input model holds here unchanged: **a gesture produces an intent, not an
//! effect**. The recogniser (WO-012) emits intents; `interact` turns the ones aimed at the canvas
//! into ops on the model; the drawing half is a consumer of the computed layout. Swapping egui for
//! the Phase 6 renderer replaces only the painter.

pub mod browser;
pub mod camera;
pub mod connect;
pub mod entry;
pub mod inset;
pub mod inspector;
pub mod interact;
pub mod layout;
pub mod levels;
pub mod model;
pub mod response;
pub mod scope;

/// The stable id of the master-output module (WO-014 increment 5). The canvas's master-handover
/// rule keys on it: when a patch contains an `out/main`, THAT node is the master by name, so the
/// MASTER badge never lies about which node feeds the listener. A named constant, not a literal
/// scattered through the resolve rule and its tests, so the id and the rule cannot drift apart.
pub const OUT_MAIN_ID: &str = "sparq/out/main";

/// The stable id of the scope display module (WO-013 increment 6). The painter keys its display
/// rendering on it and the live session keys its trace bindings on it — one named constant, so
/// the id and its two consumers cannot drift apart (the `OUT_MAIN_ID` discipline).
pub const SCOPE_ID: &str = "sparq/dsp/scope";

pub use browser::{fuzzy_score, rank, BrowserHit, BrowserItem, BrowserState};
pub use camera::{Camera, Lod};
pub use connect::{ConnectContext, ConnectOutcome, Rejection};
pub use entry::{RenameState, TextEntry, ENTRY_MAX_CHARS, RENAME_HINT};
pub use inset::{inset_well, well_for, Well};
pub use inspector::{InspectorLayout, ParamRow};
pub use interact::{CanvasEvent, CanvasState, Interaction, MenuRow, MenuTarget};
pub use layout::{CanvasLayout, Hit, NodeLayout, PortLayout, WireEndSide, WireLayout};
pub use levels::{wire_level, NodeLevels};
pub use model::{Graph, Node, NodeFlags, NodeId, NodeSpec, Op, PortRef, UndoStack, Wire, WireId};
pub use response::{Axes as ResponseAxes, Curves as ResponseCurves, ResponseFrame};
