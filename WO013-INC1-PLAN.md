# WO-013 increment 1 — the graph canvas, gestures-first

**Scratch plan doc** (like the run sheets: not part of a sync zip until it ships; fold the outcome
into the PHASE0-WORKORDERS build log when done). Mirrors the WO-012 increment-1 discipline:
**toolkit-independent core in `sparq-ui`, egui as a painter in `sparq-app`, everything headless-
testable with synthetic intent traces, device acceptance listed honestly at the end.**

## Scope of increment 1

**In:**
- `sparq-ui::canvas` — the zero-toolkit canvas core:
  - `model.rs`: graph as data (nodes from validated `sparq-module-api` manifests, wires, flags,
    selection), **ops as data with inverses**, undo/redo stack, cycle rejection at connect time
    (§5.4: only delay edges may close loops — no delay module exists yet, so cycles are refused
    with words).
  - `camera.rs`: pan/zoom (token clamps 0.25…4.0), screen↔world, zoom-to-fit, **LOD** from tokens
    (Full ≥0.6, Simplified ≥0.35, Dot below).
  - `layout.rs`: computed node/port/wire geometry (node 240 w, header 32, port row 48 — new token
    `node_port_row` = 2 × port capture radius so capture circles never overlap), bezier-horizontal
    wires (control offset = 0.5 × dx, per token), polyline hit-testing (wire hit width 20 px,
    port capture 24 px — both tokens).
  - `connect.rs`: drag-to-connect validation → `Verdict` affordances via **`sparq-module-api`'s
    own `connect_*` functions** (no mirror — the compat matrix stays one copy). Direction rule
    (out→in), multiplicity (Single input occupied → atomic replace op; cv fan-in needs
    `util/mixer` per the matrix), adapter offers **gated on module availability** (defect #58:
    the canvas may not offer a converter that does not exist).
  - `interact.rs`: `CanvasState` = camera + selection + interaction + context menu; the
    intent→op table (below); every suppression/refusal returned as an event **in words**.
- `sparq-app`: `ui/canvas_ui.rs` draws the computed layout with the Painter (grid anchored to
  world space, signal-class wire colours **with the token redundant encodings** — solid/thin/
  dashed-6-3/dotted-2-4/double-stroke + A/C/E/D/S letters), nodes at three LODs, pending wire
  with glow/dim affordances, marquee, context menu (rows 44 px, registered for the audit).
  Shell routes canvas-rect intents to the canvas; binds the two WO-012 stubs: DoubleTap →
  zoom-to-fit, Context → menu, Undo → graph undo.
- Demo graph = the three conforming reference modules (sine → gain → rms, mono→stereo fan-out
  visible), built from `sparq-module-api` types directly. Real discovery/browser is increment 2.
- Headless smokes in `sparq ui --audit`: tap-select, drag-move + 8 px snap, port-drag connect,
  incompatible refusal (in words), replace-on-single-input, 3-finger undo restores, double-tap
  zoom-to-fit, long-press menu + duplicate, cycle refusal.
- `docs/ui/gestures.md` (the WO artefact): the canvas gesture→intent→effect map.

**Out (increment 2+, parked in LATER.md):** module browser + fuzzy search, inspector with touch
sliders/numeric entry, rename/randomise (need text input + seed tree), wire endpoint re-patch by
drag, live wire levels (needs WO-008 executor taps), LOD visual iteration against the mockup,
keyboard/controller parity for canvas ops, param-state undo (rides with the inspector).

## Decision taken: `sparq-ui` gains one first-party dependency

`sparq-ui` → `sparq-module-api` (which → `sparq-kernel`). The canvas needs the port vocabulary
(`Port`, `PortType`, `ChannelSet`, `CvRange`, `Multiplicity`, `Phase`, `Adapter`, `Verdict`,
`connect_*`) and the docs already say the canvas reads it: *"The canvas renders this directly"*
(port.rs on `Verdict`), *"the single table host validation and the canvas affordances both read"*
(README on compat-matrix.toml). Mirroring the vocabulary inside sparq-ui is exactly the drift
class WO-007 refused to ship. Still **zero third-party dependencies, zero unsafe** (module-api
is `forbid(unsafe)` like every crate but the kernel). The gesture/shell/audit core does not
touch the dependency; only `canvas` does. lib.rs docs amended to say this precisely.

## Intent → canvas effect (the table gestures.md will carry)

| Intent (from the recogniser) | Target | Canvas effect |
|---|---|---|
| DragStart | port | begin pending wire |
| DragStart | node body | move selection (locked → refused, in words) |
| DragStart | wire | select wire (endpoint re-patch: increment 2) |
| DragStart | empty | marquee multi-select |
| DragUpdate | — | live move / wire cursor / marquee; second finger = ×10 fine |
| DragEnd | port | connect attempt → verdict op or refusal in words; cancelled → dropped, logged |
| DragEnd | empty (wire) | connection cancelled, logged |
| DragEnd | — | commit move (8 px snap) / marquee selection |
| Activate | node / wire / empty | select / select / deselect+close menu |
| Activate | menu row | run the row's op |
| DoubleTap | canvas | zoom-to-fit |
| Long press (Context) | node / wire / empty | context menu (duplicate, bypass, mute, lock, delete / delete / select-all, zoom-fit, redo) |
| Pan | canvas | camera pan |
| Zoom | canvas | camera zoom at centre |
| Undo (3-finger tap) | global | graph undo |

## Device-side acceptance (honest list — SATURN, after this increment)

Over RDP-from-iPad: canvas touch-test the table above (gestures, connect, undo, menu). NOT
accepted over RDP: 60 fps at 200 nodes (WARP), palm rejection (iPadOS filters before Windows;
sparq's own path needs WM_POINTER — increment 2), full DPI matrix (RDP virtual display).
Those stay on the WO-012/WO-013 device-evidence list in RESUME.md §4.
