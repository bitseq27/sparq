//! The graph canvas (WO-013): editing a patch by touch, in the token language.
//!
//! Like the rest of `sparq-ui`, this module is **computed, not drawn**. It owns five things and
//! nothing else:
//!
//! * [`model`] — the graph as data (nodes, wires, flags), every mutation an [`model::Op`] with an
//!   inverse, and an undo/redo stack. This is the surface the WO-008 executor will eventually
//!   read; until then it is the canvas's own in-memory truth.
//! * [`camera`] — pan / zoom / LOD and the zoom-to-fit computation, clamped to the canvas tokens.
//! * [`layout`] — one pure pass from (graph, camera, canvas rect) to screen-space rects, port
//!   positions and wire béziers, plus hit-testing. The egui painter in `sparq-app` draws exactly
//!   this and nothing it invents.
//! * [`connect`] — the connection verdict, delegated to `sparq-module-api`'s own `connect_*`
//!   functions so the compatibility matrix keeps its single copy (the discipline WO-007 shipped).
//! * [`interact`] — [`interact::CanvasState`], the intent→operation table: gestures in, ops and
//!   *explained* refusals out.
//!
//! The rule from the input model holds here unchanged: **a gesture produces an intent, not an
//! effect**. The recogniser (WO-012) emits intents; `interact` turns the ones aimed at the canvas
//! into ops on the model; the drawing half is a consumer of the computed layout. Swapping egui for
//! the Phase 6 renderer replaces only the painter.

pub mod camera;
pub mod connect;
pub mod interact;
pub mod layout;
pub mod model;

pub use camera::{Camera, Lod};
pub use connect::{ConnectContext, ConnectOutcome, Rejection};
pub use interact::{CanvasEvent, CanvasState, Interaction, MenuRow, MenuTarget};
pub use layout::{CanvasLayout, Hit, NodeLayout, PortLayout, WireLayout};
pub use model::{Graph, Node, NodeFlags, NodeId, NodeSpec, Op, PortRef, UndoStack, Wire, WireId};
