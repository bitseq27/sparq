# WO-013 increment 6 — the scope screen: `dsp/scope` draws from the analysis ring (plan of record, 2026-09-29)

The named follow-on of WO-012 increment 2 (its D1, checklist item 6): "the scope screen — the
analysis ring already publishes waveforms with nothing drawing them". The operator chose it as
the next sandbox build. Everything the display needs already exists and is documented: the
`dsp/scope` manifest (WO-014 inc 5) PRE-SPECIFIES the UI contract — *"the wire is the binding,
the ring is the payload"*, params configure the DISPLAY and are "read by the UI, not by
`process`", the UI "accumulates blocks up to the timebase" and "owns any persistence/phosphor
buffer" — and `LiveSession::drain` already reads every `AnalysisUpdate` off the ring. This
increment draws it. Decisions before code, the `WO008-INC7-PLAN.md` discipline.

## The decisions

**D1 — Name and home.** WO-013 increment 6: it is a canvas painter increment (WO-013 owns the
canvas rendering), riding WO-012 inc 2's session. The toolkit-independent model lands as
`sparq-ui::canvas::scope` (accumulation, trigger, geometry — unit-tested without a painter, the
`levels`/`entry` precedent); the pixels land in `canvas_ui.rs`; the binding resolution and trace
feeding land in `LiveSession`. No new crate, no manifest move, no executor change.

**D2 — The binding is the wire, resolved every frame, in canvas terms.** A scope's `x` (port 0)
and `y` (port 1) inputs are each resolved from the graph: the wire whose `to` is
`(scope, 0|1)` names the SOURCE `(node, port)` whose published waveform the trace reads — the
manifest's own rule, and the analysis ring's own key (the drain already maps kernel→canvas).
Resolution is per-frame over the scope nodes' wires (a handful of nodes, a linear wire scan —
cheap, and structurally impossible to stale: it reads the graph the painter is drawing). A
scope input with no wire is UNBOUND: its trace stays empty and the display shows the flat rest
line — the manifest's promised "a flat line, not a crash". An unbound scope never invents a
source.

**D3 — Traces are session-owned display state, swapped onto the canvas per frame.** The rolling
buffers live in `LiveSession` (like `levels`, they are facts about the live engine, not edits);
`CanvasState` grows the transient `scope_traces` field the painter reads, filled by
`std::mem::swap` — O(1), no per-frame copy of audio-sized buffers. Session stop drops the
traces; the canvas shows the rest state, honestly. NOT undoable, NOT project state (the
manifest: "display state, not project state") — the field docs say so in the `levels` words.

**D4 — The model: rolling accumulation, bounded, sized by the negotiated truth.** `TraceBuf`
is a rolling `Vec<f32>`: `push` appends the drained block payloads in ring order (oldest falls
off the front). Capacity = `timebase_ms/1000 × sample_rate`, rounded up to whole blocks,
clamped to `MAX_TRACE = 65 536` samples (at 96 kHz that is ~683 ms — the 500 ms manifest
maximum fits at every rate the HAL negotiates; the clamp is declared, not silent). The session
sizes buffers from its OWN negotiated `exec_cfg.sample_rate` — the same "build for the truth"
discipline as the executor. A timebase param edit resizes at the next push (retain the newest
samples that fit — a zoom, not a reset; declared).

**D5 — Trigger v0: rising-edge or free-run, exactly the manifest's words.** `trigger == 0` is
free-run: the window is the newest `timebase` samples. `trigger != 0`: the window starts at the
LAST rising crossing of the trigger level within reach of the buffer (search back from the
newest sample); no crossing found → free-run fallback for that frame (a trace that waits for a
crossing that never comes would be a blank screen pretending to be a bug). X/Y mode ignores the
trigger (a Lissajous figure has no time axis to align — declared in the model docs).

**D6 — Geometry: decimated polyline inside the display rect, one point per pixel column.**
`trace_polyline(buf, window, rect, gain)` maps the window onto the rect: x = time, y =
`sample × gain` at the rect's vertical centre, clamped to the rect (the gain is a display zoom;
a signal driven past the edges FLATTENS at the boundary — the honest oscilloscope behaviour,
declared). Decimation is stride-based to ≤ rect-width points; min/max envelope rendering is a
declared later refinement (the stride can hide a spike narrower than a column — said in the
model docs, not buried). X/Y mode: `xy_polyline` pairs `x[i]` against `y[i]` over the shorter
of the two windows, both axes gain-scaled and centred. An empty/short buffer draws the centre
flat line — the rest state and the zero signal look the same because they ARE the same claim.

**D7 — The display rect rides the EXISTING node box; no layout move.** v0 draws inside the
scope node's standard body (2 inputs → 128 px tall): the display is the body below the header,
inset by the existing `LAYOUT_SPACE_*` tokens, leaving the ports' own edge zones (port circles
and labels draw over the inset ground, as over any body fill). No `node_size` change, no new
layout token, no audit-matrix change — the node's touch target, registration and hit-testing
are untouched, and the display steals no gestures (it is pixels under the existing body
target). The display-sheet's larger tiles are a Phase 6 rasteriser concern; declared, parked.

**D8 — LOD contract: the trace draws at FULL only.** Simplified is text-free AND trace-free
(the box, coloured ports — the camera.rs contract; a 40-px box cannot carry a readable trace,
and an unreadable one is decoration). Dot is a dot. The display GROUND (`ground.inset` rect)
draws at Full and Simplified — the scope still reads as a scope at a glance, pattern over
colour; at Dot nothing. Rest state at Full: the flat centre line at `hairline.faint`.

**D9 — Colours and glow are the wire vocabulary, token-only.** Display ground =
`pal.ground_inset`; grid = centre line + quarters at `hairline.faint` (the `scope.trace` map's
own row); trace stroke = `lerp(colour, glow, level)` with the SAME expressions the wires use
(`level` = the window's peak |sample|), plus the wires' under-glow pass at
`0.15 + 0.45 × level` — no new numeric vocabulary, `token_audit` R6 keeps its jurisdiction.
The `colormap` param resolves index 0 (`scope.trace`, the identity — the only map the tokens
carry today); a non-zero index resolves to the same identity UNTIL `colormaps.toml` grows the
scope-valid table — the param is future-facing by manifest design ("the index→map table lives
with the tokens and the painter resolves it"), and the resolution is one match arm when the
table grows. `motion.toml [trace]`'s amplitude^1.5 glow law and phosphor persistence are
declared parked (the wires do not implement them either — one glow refinement increment will
move both, data-first: motion tokens are not yet generated constants).

**D10 — Params are read from the node, per frame, clamped by the model.** The painter reads
`timebase`/`mode`/`trigger`/`gain` from the scope node's `effective_params()` (the inspector
edits them like any param — the ledger already carries `SetParam` to the live engine, and the
display side reads the same graph). The model clamps every value to the manifest's declared
range before use; a NaN/odd snapshot cannot produce odd geometry (the params arrived through
`op_set_param`'s clamp, but the model does not trust that — belt, braces, and a unit test).

**D11 — Feeding: the drain grows one honest step.** `LiveSession::drain` already visits every
`AnalysisUpdate`; it additionally pushes `a.wave()` into the trace of every scope bound to
`(a.node, a.port)` on the bound axis. Ring order is block order (SpscRing is FIFO), so the
trace is time-ordered by construction. Audio-rate AND block-rate payloads both arrive (inc 2's
one-sample waveforms) — a scope bound to a block-rate source accumulates one sample per block:
legal, and its trace reads as a step-wise trend (declared; the manifest's own payload rule).
The peak for the wire levels (inc 2's cv rule) and the trace feed come from the SAME update —
one pass, no second read.

**D12 — Zero audio-thread cost stays structural.** `Scope::process` remains the no-op it
shipped as (its acceptance box); every pixel-side byte in this increment lives on the
control/UI thread (drain, model, painter). The existing "audio thread allocates nothing while
publishing analysis" gate is the proof that the publish side did not move; the new code adds
nothing to that thread by construction — there is no new code on it.

## Proof burden

* **Model unit tests** (`sparq-ui::canvas::scope`): rolling push keeps the newest cap; capacity
  from timebase×rate with the MAX_TRACE clamp; timebase resize retains the newest; trigger
  finds the LAST rising crossing (hand-computed buffer) and falls back to free-run when there
  is none; free-run takes the newest window; polyline maps window→rect with gain and clamps at
  the edges; decimation bounds the point count by the rect width; X/Y pairs over the shorter
  window; empty buffer → the centre flat line; param clamping (out-of-range and NaN inputs).
* **Session tests** (`live.rs`): sine→tap→scope rig on the manual null — pumped blocks land in
  the trace as the SIGNED sine (both signs present, peak ≈ 0.5, in ring order); an unbound
  scope's trace stays empty; a rebind (structural sync) resets the affected trace; session stop
  drops the traces.
* **Audit smokes (+2 → 32):** (31) the live scope: rig built through the driver doors, PLAY,
  pump, frame — the canvas `scope_traces` holds the real signal for the bound scope and is
  empty for an unbound one, asserted on STATE; (32) the LOD contract: with a live trace, the
  frame's primitive count at Full exceeds Simplified (the trace draws at Full only — the
  harness already returns shape/primitive counts; the assertion is the contract, not a number).
* **Every pre-existing golden UNCHANGED** (no render path moves — `Scope::process` is and stays
  a no-op; the demo graph has no scope): stress `7bb06379bd6845e5`, the three pinned exec
  renders, `canvas-render.wav` (1 920 046 B · `d7ad294e…`), determinism `0f5c3e86c7f117a9`,
  selftest 9/9, `modules --strict` 17/17 (no manifest moved), the 30 existing smokes, docs
  `--check` (generated from manifests — untouched). Test-count baseline 766 moves with the new
  gates; `log_check.py` BASELINE moves WITH the seal (#83).

## Postscript — two corrections by the code, and the acceptance measured (2026-09-29)

**D3′ — the swap was WRONG, and smoke 32 caught it before any device ever saw it.** D3
prescribed a `CanvasState.scope_traces` field filled by `std::mem::swap` per frame. Built that
way, the LOD smoke failed with all four measurements identical, and the debug line told the
truth: the swap ALTERNATES two trace sets — the session feeds whichever set it currently holds,
so under manual pumping each set only receives every other frame's updates (a gappy trace), and
the canvas blanks on the frames where the session's set had not been fed since the last swap.
A paced device would have hidden the gap most of the time — which is exactly the kind of defect
a hermetic smoke exists to catch. The replacement is simpler AND more honest: the session is the
SINGLE SOURCE (`LiveSession::traces()`), the painter reads it through a `canvas_ui::draw`
parameter, and at rest the shell passes an empty set — no canvas field, no copy, no swap, and a
dead stream's signal cannot linger because there is nowhere for it to linger. The D3 field on
`CanvasState` does not exist.

**The LOD smoke measures SHAPE counts, not primitives:** this egui tessellates a whole frame
into ONE clipped primitive (`ui --headless` has always printed "1 primitives" — the number was
in the tool's own output, unread). Shapes are the countable truth; the smoke's comment says so.

**Measured, on the final tree:** fmt clean · clippy clean in the default, `bootstrap-audio`,
`ui`, combined audio+hal, native `ui-window` (single-job) and all six runnable MSVC cells ·
**777 tests, 0 failed, 1 ignored** (+11: the scope-model gates) plus 3 new session gates and 2
new smokes behind `ui` (18 app-ui unit tests now) · the 5 python gates clean (`token_audit` R6
accepted the painter: every dimension a token, the one count `SCOPE_LABEL_CHARS` documented as
a count) · release goldens bit-identical: the three pinned exec renders, stress
`7bb06379bd6845e5` · 2 383 · 7 617 debug AND release, `selftest --golden` PASS (9 gates),
`canvas-render.wav` 1 920 046 B · `d7ad294e…` · `modules --strict` 17/17 · `probe_alloc`
0/5 000 · `ui --audit` **PASS, 32 smokes** · stamp `src 94f/2122410B` · sealed
`sync-wo013-inc6.zip` (16 entries). The one clippy pass the code owed: `div_ceil` (MSRV 1.80
legal) replaced three hand-rolled ceilings the model shipped with.

## Out of scope (declared, parked in LATER.md)

Phosphor persistence and the motion-token glow law (one refinement increment, data-first);
min/max envelope decimation; larger display tiles (the display-sheet sizes — a layout-token
increment); the other eleven display modules (spectrum, sonogram, phase portrait… — each its
own increment over the same `scope` machinery where it fits); X/Y mode's per-axis gain params
(the manifest has one `gain`); saving traces (project state — WO-011); offline (RENDER WAV)
scope previews — the analysis ring is a LIVE publication, so a scope at rest after an offline
render shows rest, never a stale or faked trace.

## Implementation order

1. `sparq-ui::canvas::scope` — the model + its unit tests (no dependencies beyond `geom`).
2. `CanvasState.scope_traces` transient field (+ Clone/docs), the swap point.
3. `LiveSession` — binding resolution, trace feeding in `drain`, sizing from `exec_cfg`,
   reset-on-rebind, drop-on-stop; session tests.
4. `canvas_ui.rs` — the display rect, ground/grid/trace/glow at Full (ground at Simplified),
   param reads; the two audit smokes.
5. Gates, then the seal: LATER/README/CHECKLIST/SYNC/PHASE0 moves, test006 expectation counts,
   stamp, log_check BASELINE, the zip.

## Acceptance (sandbox)

`cargo fmt --all --check` · every runnable clippy cell · workspace tests green at the new count
· the 5 python gates (incl. `token_audit` over the new painter code — no literals) · release
goldens bit-identical · selftest 9/9 · `ui --audit` PASS at 32 smokes · stress evidence line
`7bb06379bd6845e5` · 2 383 · 7 617 debug AND release.
