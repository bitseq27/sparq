# WO-012 increment 5 — convergence slice B: node inset displays + the inspector response plot (plan of record, 2026-09-30)

Increment 4 declared this as its own out-of-scope line — *"the inspector response plot and node
inset displays (increment 5, the convergence's slice B)"* — and LATER.md §WO-012 increment 5+
names the two items exactly:

> * **Node inset displays**: the mockup's per-module wells — envelope triangles, meter bars,
>   sparklines — drawn from LIVE values where the rings carry them (meters, analysis) and at
>   rest otherwise; the scope well shipped in WO-013 inc 6 and is the pattern.
> * **The inspector response plot**: the mockup's filter-curve display with its draggable
>   marker — needs a per-module curve contract (svf first), declared before drawn.

This is the second slice of the convergence against `design/mockups/design-mode.svg` (slice A was
increment 3's chrome). The CHECKLIST's standing order: *"the scope well from WO-013 inc 6 is the
pattern; the response plot needs a per-module curve contract, svf first, declared before drawn.
Plan of record first, the WO008-INC7 discipline."* Decisions before code, below.

The pattern this increment copies is already in the tree twice and is not negotiable:

1. **Computed, not drawn** (`sparq-ui::canvas`): a toolkit-independent model owns the pure
   mapping (params / live ring values → geometry); the egui painter in `sparq-app` reads a
   number and never invents one. Swapping the renderer replaces the drawing, not the model
   (`scope.rs`, `levels.rs` are the precedents).
2. **At rest, never faked** (WO-012 inc 4 D3, WO-013 inc 6): a well whose live source has not
   published shows its honest rest state — an empty meter well, a flat centre line, a param-shaped
   outline — *never* a frozen value from a dead stream. The shell hands the painter an empty map
   and the wells sit empty.
3. **Tokens only in the painter** (`token_audit.py` R6): every colour a `Palette` field or
   `COLOR_*` token, every dimension a `LAYOUT_*` token. No literal reaches `canvas_ui.rs`.
4. **One source of truth** (defects #68/#82/#84): a contract has ONE copy. The filter's
   response math lives in the filter; the display maps it; nothing is duplicated-and-pinned when
   it can be single-sourced.

---

## D1 — Node inset displays: generalise the per-module well dispatch.

**What exists.** `draw_node_box` already keys two wells on `node.spec.module_id`:
`SCOPE_ID → draw_scope_display` (WO-013 inc 6) and `OUT_MAIN_ID → draw_master_meters` (WO-012
inc 4). The dispatch is a hard-coded `if`; the wells are bespoke functions. Slice B turns the
`if`-chain into a **well registry** and adds the mockup's remaining per-module wells, each
following the scope/meter discipline exactly.

**Decision: a `Well` enum + one `inset_well(module_id) -> Option<Well>` lookup in
`sparq-ui::canvas`**, consumed by `draw_node_box`. `Well ∈ { Scope, Meters, Envelope, Curve,
Sparkline, None }`. The painter matches on the `Well` and calls the matching draw fn; the lookup
is the single place that says which module wears which well (the `OUT_MAIN_ID`/`SCOPE_ID`
named-constant discipline, extended — no module-id literal scattered through the painter). New
module-id constants join `canvas/mod.rs` beside the existing two (`ENV_AD_ID`, `LFO_ID`,
`RMS_ID`, `TAP_ID`, `SVF_ID`) so an id and its well cannot drift.

**Which well each module wears, and its live source** (the CHECKLIST's rule, made concrete —
*LIVE where the ring carries it, at rest otherwise*):

| Module | Well | Live source (ring) | At-rest shape |
|---|---|---|---|
| every node with an audio **output** | **Meters** (stereo bars) | `LiveMeters` — the ring already carries per-port stereo peaks for ALL audio outputs (LATER.md: *"the ring already carries per-port stereo peaks for all of them"*); inc 4 painted only `out/main` | two empty wells |
| `dsp/scope` | **Scope** (shipped inc 6) | `ScopeTraces` ← analysis ring | flat rest line |
| `env/ad` | **Envelope** (triangle) | live cv **value** overlay where the session carries the block cv (see D1′); the SHAPE is param-derived, not live | the attack/decay/curve triangle from params — the module's declared shape, honest at rest |
| `mod/lfo` | **Sparkline** | rolling cv history (D1′) where carried; else one param-derived period of `shape`/`rate`/`depth` | flat centre line, or one static period outline |
| `ana/rms`, `ana/tap` | **Meters** (single bar from the cv `level`/`peak`/`rms` port) | the port's published cv value (`NodeLevels::port`) — already read for wire levels | empty well |
| `flt/svf` | **Curve** (a thumbnail of the D2 response, no marker) | param-derived (the curve IS the params) | the response curve, always drawn from params |
| everything else | **None** | — | no well (the body is the box; honest) |

**D1′ — live cv history is a declared later half; the shapes ship at rest first.** The meter
ring carries AUDIO outputs; a cv-only source's *history* is not on a ring the session drains
today (its per-block value crosses for wire levels via the bridge preview, not as a rolling
buffer). So, honestly: the **Meters** well goes live for every audio node this increment (the
ring already carries it — only the painter's `OUT_MAIN_ID` gate and, if needed, the session's
drain filter widen); the **Envelope**/**Sparkline**/**Curve** wells ship drawing their
**param-derived rest shape** in the well vocabulary, and their LIVE overlay (a `Sparklines`
session set following the `ScopeTraces` discipline — session-owned, sized at the negotiated
rate, fed from the ring, pruned on node-leave, empty-map at rest) is the declared next half,
named in Out-of-scope. This is the CHECKLIST's own wording — *"LIVE where the rings carry them
and at rest otherwise"* — not a shortcut: a param-shaped envelope triangle is the module's
declared response, exactly as the svf curve is, and it is honest before the first block.

**Geometry.** Wells reuse the scope's inset rect rule (`ground.inset` fill, `LAYOUT_SPACE_2`
inset, clearing the port-circle + label budget on the left) and the meter's bar rule (6 px wells
at `LAYOUT_SPACE_2`, data-class fill, audio-class peak-hold block decaying on AUDIO time — block
counts, no wall clock). The envelope triangle and sparkline draw inside the same inset rect with
the wires' glow vocabulary (class colour lerped toward class glow by the live level, under-glow
pass at the wires' alpha rule) where a live value exists; at rest they draw the hairline-faint
outline only. **LOD**: wells draw at Full and Simplified (a meter/shape is a reading, not text —
inc 4 D3's rule); at Dot the node is a dot and wears no well. **Gestures**: a well steals none —
it is pixels under the node body's existing class-L touch target (the scope's rule); the response
PLOT's marker (D3) is the one new control and lives in the inspector, not on a node.

**Model split.** The param→shape mappings (envelope triangle from `attack`/`decay`/`curve`;
one lfo period from `shape`/`rate`/`depth`; the svf thumbnail from D2) are pure functions in a
new `sparq-ui::canvas::inset` module — toolkit-independent, unit-tested like `scope.rs`, reading
only the node's `effective_params()` (manifest order, clamped at the boundary, NaN→default, the
`ScopeView::from_params` discipline). The painter reads the returned polyline/rect and draws it.

## D2 — The per-module curve contract, svf first (declared before drawn).

**The contract.** A module *may* declare a frequency-response curve: a pure function from its
param snapshot + the negotiated sample rate to a magnitude response over a frequency grid. The
inspector plot (D3) and the svf node thumbnail (D1) both read it. Modules that declare none get
the honest **no-curve** state (D3), never a faked flat line. **svf is first and is the
reference implementation**; the contract is shaped so a second module (a future EQ, a delay's
comb response) slots in without touching the display model.

**Single source of the filter math (defect #68/#82/#84 discipline).** The response is computed
in **`sparq-audio`**, on `SvfFilter` itself, from the filter's OWN cached coefficients
(`g`, `k`, `a1`, `a2`, `a3` that `recompute()` already produces) — NOT a second copy of the math
in the UI crate. New **pure display method** (never mutates state, never called on the audio
thread, so no golden can move):

```
impl SvfFilter {
    /// Linear magnitude |H(e^{jω})| at `freq_hz` for the CURRENT mode, from the live
    /// coefficients. Display-only: pure, allocation-free, audio-thread-forbidden by contract.
    pub fn magnitude_at(&self, freq_hz: f64) -> f64;
}
```

The exact discrete-time response, solved in the z-domain (`z = e^{jωT}`, `ω = 2πf/fs`,
`I = 2/(z+1)` the trapezoidal integrator's frequency-domain gain), from the same `a1/a2/a3/k`
the sample loop uses. With `Den = (1 − I(1−a3))(1 − a1 I) + a2² I²`:

* `H_lp = a3 / Den`  (the identity `a2² = a1·a3` collapses the general numerator — `a1·a3 =
  a1·g·a2 = a2·a2` since `a2 = g·a1`, `a3 = g·a2`)
* `H_bp = a2 (1 − I) / Den`
* `H_hp = (1 − I)[(1 − a1 I) − k a2] / Den`
* `H_notch = H_lp + H_hp`, `H_peak = H_lp − H_hp`  (the mode's own sum/difference, matching
  `tick`'s `Notch`/`Peak` arms exactly)

`magnitude_at` returns `|H|` for `self.mode`. Verified at the boundaries during derivation: DC
(`I=1`) gives `H_lp = 1`, `H_hp = H_bp = 0`; Nyquist (`I→∞`) gives `H_lp → 0`, `H_hp → 1` — the
analytic curve and the filter agree at the two points every response must get right.

**Acceptance for the math (the drift gate):** a `sparq-audio` unit test asserts `magnitude_at`
matches the **time-domain sine-sweep** already in `filter.rs`'s test harness (`response()`)
within a declared tolerance (≤ 0.5 dB in the passband, ≤ 1 dB elsewhere) at several
cutoff/resonance points across all five modes. The analytic curve is pinned to the measured
filter, so the display cannot drift from the DSP — the single-source guarantee, tested.

**Frequency grid + axes (the display model, `sparq-ui::canvas::response`).** Toolkit-independent,
no `sparq-audio` dep (it consumes magnitudes the way `scope.rs` consumes traces). Owns: the
**log frequency axis** (declared range `10 Hz … min(20 kHz, 0.4999·fs)` — the floor is the svf
manifest's own cutoff minimum, the ceiling its `recompute` clamp `0.4999·fs`, so the curve always
covers the full cutoff travel and never plots a frequency the filter refuses), the **dB magnitude
axis** (declared range `−60 … +18 dB`, the resonance peak's honest headroom), the mapping of a
`&[(f64 freq, f64 mag)]` grid to a polyline in a `Rect`, and the marker geometry (D3). A fixed
grid (e.g. 128 log-spaced points) is computed once per param change, not per pixel; the model
clamps/sanitises non-finite magnitudes to the axis floor (a misbehaving coefficient set gets a
flat line at −60 dB, never NaN pixels — the scope's rule).

**The contract dispatch** (module id → curve) lives in `sparq-app` (the bridge/inspector), the
only layer that sees both crates: `svf` → build an `SvfFilter` from the node's params at the
negotiated rate, call `magnitude_at` over the grid; any other module → `None` → D3's no-curve
state. Adding a second curve module later is one arm here plus its own DSP method — the display
model and the contract shape do not change.

## D3 — The inspector response plot, with its draggable marker.

**Where.** The inspector today is a port-dot summary strip + control-cyan param sliders
(inc 3). The selected node gains a **response-plot well** ABOVE the sliders when its module
declares a curve (svf first): a `ground.inset` well (the token language for a display, the
scope/rename precedent), the log-f × dB grid hairline-faint, the curve in the module's
dominant-class colour with the wires' glow vocabulary, and the axis words (a few Hz/dB ticks,
xs, `text_tertiary`) at Full LOD only.

**The draggable marker — a read-only probe, not a second param door.** The mockup shows a
marker on the curve. Decision: the marker is a **vertical hairline cursor the user drags
horizontally to probe the curve**, with a readout of the frequency (Hz, log-mapped from x) and
the magnitude (dB, from the curve at that frequency) at the marker. It does **NOT** edit cutoff:
cutoff stays the slider's job (one source of param truth — a marker that also wrote cutoff would
be a second door to the same param, the class of defect this project refuses). Drag-to-cutoff is
declared in Out-of-scope as a possible later refinement if the operator asks.

**Marker state = display state, not project state** (the camera/scroll discipline): it lives on
`CanvasState` beside the inspector scroll offset (`response_marker: Option<(NodeId, f64_hz)>`),
clamped to the plot's frequency range, **not undoable**, reset when the selection changes or the
node leaves the graph. Dragging it produces a `GestureIntent` → a marker-move (the recogniser's
existing drag vocabulary routed to the plot rect, the panel-over-panel routing inc 4 shipped);
it re-stages nothing and crosses no command ring (it moves no audio param).

**The marker is a control, so it is audited.** Its capture rect (≥ 44 px, zoom-invariant — the
port-capture rule) registers as a class-S `InteractiveElement` in the layout audit; the plot well
itself registers nothing (it is a reading). The breakpoint matrix and the 44 px floor still hold
(the well shrinks with the inspector; the marker's capture stays ≥ 44 px or the audit fails, which
is the point).

**No-curve state (honest).** A selected node whose module declares no curve shows, in the well,
the module's `summary` in xs `text_disabled` and the words *"NO RESPONSE CURVE"* — an answer, not
a blank well and not a faked flat line (the browser's *"NO MATCH — CLEAR THE SEARCH"* precedent,
the scope's *"a flat line, not a crash"* honesty).

## D4 — Model split, determinism, and the shape-of-the-diff proof.

**New toolkit-independent models in `sparq-ui::canvas`:** `response` (D2 axes/polyline/marker
geometry) and `inset` (D1 param→shape mappings + the `Well` registry). Both pure, both
unit-tested to the `scope.rs`/`levels.rs` bar (clamping, NaN sanitising, rest states, log/dB
mapping, marker clamp, registry lookup, determinism of iteration). The painter (`canvas_ui.rs`,
the inspector painter) reads them and invents nothing.

**New pure DSP display method in `sparq-audio`:** `SvfFilter::magnitude_at` (D2) + its
sine-sweep drift test. It reads cached coefficients; it never mutates, never allocates on a hot
path, and is never called from `process`/`tick` — the audio path is untouched.

**Session/bridge (sparq-app):** the meter drain widens from `out/main` to every audio-output
port (the ring already publishes them — D1); the curve-contract dispatch (D2) builds an
`SvfFilter` from params at the negotiated rate for the selected svf node and hands the
magnitudes to the response model; the marker intent routes through the recogniser's drag
vocabulary (D3). No new audio-thread work: meters are already published, the curve is computed
on the UI thread from params.

## What must NOT move (the shape-of-the-diff proof)

* **Every golden stays bit-identical.** The audio path, the executor, module DSP `process`/`tick`,
  the render path, and `publish_meters` are untouched. `magnitude_at` is additive and pure; the
  meter drain widening changes only which already-published entries the UI reads, not what the
  audio thread publishes. Stress hash `7bb06379bd6845e5`, the exec renders, `canvas-render.wav`,
  and the determinism harness reproduce exactly.
* **No param writes from a display.** The response plot and the marker read params; they never
  write one (cutoff stays the slider's). No new command-ring traffic, no re-stage.
* **No literal colour/size in the painter** (R6): every well/plot/marker dimension is a
  `LAYOUT_*` token, every colour a `Palette` field or `COLOR_*` token. `token_gen --check` and
  `token_audit` stay clean; the token set may GROW (a new well/plot token) through the generator,
  never through a literal.
* **The 44 px floor and the breakpoint matrix hold** — the marker's capture is audited; wells
  shrink with LOD and never invent a touch target below the floor.

## Acceptance (sandbox)

* **New unit tests** (target: the count grows from 777; every new model carries its own):
  `magnitude_at` matches the sine-sweep across 5 modes (the drift gate); the response model's
  log-f/dB axis mapping, polyline, marker clamp, NaN→floor sanitising; the `inset` registry
  (`inset_well` returns the right `Well` per module id, `None` for the uncategorised); the
  envelope triangle from params (attack/decay/curve), the lfo period outline, the rms/tap bar;
  rest states (empty meter well, flat sparkline, no-curve words); the marker is display state
  (not undoable, reset on selection change).
* **Smokes** (the count grows from 37): a meter well on a NON-`out/main` audio node is non-empty
  after PLAY+pump and empty at rest; the inspector shows the svf curve and the marker reads a
  frequency + dB; dragging the marker moves the readout and writes no param (the patch is
  byte-identical after); a no-curve module shows *"NO RESPONSE CURVE"*; the svf node thumbnail
  draws its curve.
* **Standing gates:** `cargo fmt --all --check`; clippy `-D warnings` in every runnable cell
  (default, `ui`, `bootstrap-audio`, MSVC×N cross-lint, and the native `ui-window` cell — the
  painter and inspector change, so `ui-window` re-lints); `cargo test --workspace` at the new
  count; the 5 python gates (`check_text_io`, `token_gen --check`, `token_audit`, `unsafe_audit`,
  `module_docs --check`); release goldens bit-identical; `selftest --golden` 9/9 · 17/17;
  `ui --audit` (breakpoint matrix 0 violations, 44 px floor). Release builds with
  `CARGO_PROFILE_RELEASE_LTO=false CODEGEN_UNITS=16 DEBUG=0` and `ui-window` with `-j 1
  CARGO_PROFILE_DEV_DEBUG=0` (the 1 GB sandbox's OOM rules — environment notes).
* **Visual review (slice B):** `sparq ui --svg-out PATH --width 2560 --height 1600`, rendered
  with `/opt/arena-python/bin/resvg` AFTER substituting the monospace family for `DejaVu Sans
  Mono`, compared against `design/mockups/design-mode.svg`; the standing sheet becomes
  `design/mockups/convergence-wo012-inc5.png` (mockup over shell), the inc-3 protocol repeated.
* **Seal (only when the increment is COMPLETE and green):** `sync_check.py --write --sync
  <name> --sent <date>` → `--quiet` (verify) → `--self-test` (prove failable); rebuild the zip;
  keep heading = list = contents counts equal in `SYNC.md`; **move `tools/log_check.py`'s
  BASELINE with the seal** (defect #83). A partial increment is NOT sealed — the stamp moves once,
  at completion.

## Proof burden

* **The curve cannot drift from the DSP:** proven by the sine-sweep test (D2) — the analytic
  `magnitude_at` and the measured filter agree within tolerance across all five modes. This is
  the one new mathematical claim and it carries its own instrument.
* **Wells are honest at rest:** a smoke asserts an empty meter map → empty wells (never a frozen
  bar), a no-curve module → the words, an unbound sparkline → the flat line — the inc-4/inc-6
  rest discipline, re-asserted for every new well.
* **The marker moves no audio:** a smoke drags it and asserts the patch bytes are unchanged and
  no command-ring traffic crossed — display state, proven where it lives.
* **Determinism untouched:** the goldens and the stress hash reproduce bit-identically (the
  shape-of-the-diff proof, measured not hoped).

## Out of scope (declared)

* **Curve modules beyond svf** — svf is first and the reference; a second module (EQ, delay comb)
  is a later increment that adds one dispatch arm + its own DSP method, changing neither the
  display model nor the contract shape.
* **Live envelope/sparkline overlays** (D1′) — the param-derived rest shapes ship; the rolling
  cv-history `Sparklines` session set (the `ScopeTraces` discipline) is the declared next half,
  waiting on a ring that carries per-block cv history.
* **Per-port AUDIO wire levels** — still parked (levels.rs header); audio wires keep the inc-4
  folded-node semantics.
* **Drag-to-cutoff on the marker** — the marker is a read-only probe (D3); writing cutoff from it
  would be a second param door.
* **Perform-mode convergence** against `perform-mode.svg` — its own increment, after Design reads
  like the mockup.
* **Hover affordances / cursor shapes** — parked (inc 4 D2; §8 forbids hover-only affordances).

## Postscript — the acceptance, measured (to be filled after the sandbox run)

_(This plan is the pre-build artefact the CHECKLIST mandates — "plan of record first". The
postscript records the measured gates, the smoke lessons and the seal stamp once the increment
is built and green, per the WO008-INC7 / WO012-INC4 house discipline.)_
