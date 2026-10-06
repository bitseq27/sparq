//! sparq shell: design tokens and the toolkit-independent UI core (WO-012).
//!
//! The tokens are **generated** from `design/tokens/*.toml` by `tools/token_gen.py` and must
//! never be hand-edited; `token_gen.py --check` fails CI if they are stale. This is what makes
//! the Phase 6 renderer swap (ADR-003) safe: the identity lives in data, and the drawing backend
//! is a consumer of it.
//!
//! WO-020 INC4 adds [`displaylist`]: the host-side painter core for the frozen display-list
//! vocabulary (plan §8.1) — the D13 interchange in, resolved geometry + style out, for the SVG and
//! egui backends — and [`json`], the subset reader that keeps it dependency-free.
//!
//! WO-012 adds the parts of the shell that must *survive* that swap (input-model.md §1): the
//! pointer model, the gesture recogniser, the shell layout computation and the touch-target
//! audit. WO-013 adds [`canvas`]: the graph surface — model, camera, computed layout, connection
//! verdicts and the intent→operation table — built on the same rule (gestures in, ops out; the
//! drawing half lives in `sparq-app`).
//!
//! This crate carries **zero third-party dependencies** — egui/winit live in `sparq-app` behind
//! the `ui` features and consume these types at their boundary, exactly like the HAL callback
//! contract keeps the audio path toolkit-free. The single first-party exception is `canvas`,
//! which reads the module contract (`sparq-module-api`) so the compatibility matrix is never
//! mirrored; the gesture/shell/audit core reaches nothing outside `std`.

#![allow(
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

pub mod audit;
pub mod canvas;
pub mod displaylist;
pub mod geom;
pub mod gesture;
pub mod json;
pub mod pointer;
pub mod shell;
pub mod tokens;

pub use tokens::Color;
