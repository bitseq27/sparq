# sparq — architecture decision records

One page each. Format: **Status · Context · Decision · Consequences · Alternatives considered · Review trigger.**

An ADR is written *when the decision is made*, not afterwards. If you find yourself re-litigating a choice in month 8, read the ADR first; if the ADR is wrong, supersede it with a new one (never edit history — mark the old one `superseded by NNN`).

| ADR | Title | Status | Decided |
|---|---|---|---|
| [000](000-conventions.md) | Engineering conventions | accepted | 2026-09-20 |
| [001](001-language-rust.md) | Implementation language: Rust | accepted (D-3) | 2026-09-20 |
| [002](002-module-tiers.md) | Module execution tiers | accepted (D-2) — amended by 010 | 2026-09-20 |
| [003](003-ui-renderer-path.md) | UI renderer path: egui scaffold → custom WebGPU shell | accepted (D-1) | 2026-09-20 |
| [004](004-platform-windows-primary.md) | Primary platform: Windows | accepted (D-4) | 2026-09-20 |
| [005](005-port-type-system.md) | Closed port type system | accepted | 2026-09-20 |
| [006](006-clock-model.md) | Three-clock model and latency policy | accepted | 2026-09-20 |
| [007](007-determinism-journal.md) | Determinism, seed tree and journaling | accepted | 2026-09-20 |
| [008](008-bootstrap-device.md) | Disposable bootstrap audio path | accepted | 2026-09-20 |
| [009](009-executor-and-hal.md) | Graph executor and audio HAL shape | draft — ratify in WO-006/WO-008 | 2026-09-20 |
| [010](010-instrument-layer.md) | The instrument layer and the third-party hand-off system | accepted (D-14) — amends 002 | 2026-10-04 |
| [011](011-stream-plane.md) | The stream plane: host-side broker and stream-source bindings | accepted (WO-020) — extends 010 | 2026-10-06 |

## Decisions still open

| ID | Decision | Resolve by |
|---|---|---|
| D-5 | Sample-rate policy (48 k default, 96 k for design/mastering?) | Phase 0 gate |
| D-6 | Block-size policy (adaptive, floor 32?) | Phase 0 gate |
| D-7 | Library backend (local-only → self-hosted → managed) | Phase 3 |
| D-8 | Distribution intent (private / open core / commercial) | before Phase 3 |
| D-9 | Dedicated appliance (`sparq stage`) | post-v1.0 |
| D-10 | Web thin client scope | Phase 6 |
| D-11 | Stage touch device | **WO-001, week 1** |
| D-12 | Interface channel count | **WO-001, week 1** |
| D-13 | Bootstrap audio crate (kill date) | WO-005 → WO-006 |

See `../../SPARQ-PLAN.md` §22 for the full table.
