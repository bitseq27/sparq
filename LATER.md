# LATER.md — the parking lot

Everything here is a **no** for now. The rule: if an idea isn't in a work order, it lives here. Promoting an item requires naming the phase and the thing it displaces.

**Promotion rule.** An item may be promoted only when (a) the current phase's musical deliverable is done, and (b) you can name what gets dropped to make room. "I'll just add it quickly" is how this project dies.

---

## Synthesis / DSP
- [ ] **A real bandlimited per-sample oscillator** (PolyBLEP, BLIT, or variable-length wavetable) to replace the additive `syn/polyblep`. Additive is provably alias-free but costs one `sin()` per partial per sample, so it cannot support 128-voice polyphony or fast FM. Acceptance gate: the existing `aliasing_floor_is_at_machine_precision` test plus a CPU benchmark at 55 Hz. *Attempted in Phase B and failed the aliasing measurement at −29 dB; the failure and the three variants tried are documented in the module header of `crates/sparq-audio/src/dsp/osc.rs`.*
- [ ] **Triangle wave.** Removed in Phase B after the integrate-the-square approach measured −8.9 dB instead of −19.1 dB for the 3rd harmonic. Needs a proper leaky integrator with a measured harmonic test.
- [ ] Wave-terrain surface editor with 3D touch manipulation *(Phase 2 candidate — the function-plotter version ships first)*
- [ ] Modal synthesis from measured impulse responses of real objects
- [ ] Neural synthesis tier (RAVE/BRAVE-class) as T3 modules
- [ ] Cross-synthesis between live input and library samples
- [ ] Feedback-path "circuit bend" module with deliberate instability + safety limiter
- [ ] Spring/plate physical model with nonlinear drive
- [ ] Additive resynthesis of an entire sample into editable partial lists
- [ ] Per-partial spatialisation (each harmonic its own object in the room)

## Rhythm / generative
- [ ] Irrational-ratio lattices as a first-class UI (1:φ grids) *(the math ships in Phase 4; the dedicated UI can wait)*
- [ ] Genetic evolution with a listener-in-the-loop fitness function (tap keep/kill while auditioning)
- [ ] Constraint solver for progressions ("give me 8 bars that satisfy these rules")
- [ ] Corpus-trained Markov models over your own back catalogue
- [ ] DTW-aligned interpolation between two drum patterns
- [ ] Rhythm "diff and patch" — apply the difference between two patterns to a third

## Data / streams
- [ ] Seismic, tide, transit, air-quality and astronomy feeds as standard adapters
- [ ] UWB indoor positioning for object trajectories in the room
- [ ] Hand-tracking (pose) as a continuous controller surface
- [ ] EEG / heart-rate variability as a slow "form" stream
- [ ] Network latency and packet loss as a compositional material
- [ ] sparq Field Kit v2: wireless, body-worn, 6-axis per limb

## Spatial
- [ ] Room simulator with measured venue IRs (a library of real rooms)
- [ ] Per-venue auto-calibration from a phone mic
- [ ] Object trajectories driven by choreography data (BVH/OSC)
- [ ] Binaural rendering for a recorded "virtual venue" release format

## Visuals / performance
- [ ] Audience-facing web thin client (visuals + macro pads on their phones)
- [ ] DMX / Art-Net / sACN output for lighting integration
- [ ] NDI output for VJ-style compositing
- [ ] Automatic performance video: UI capture + audio + journal, assembled into a film
- [ ] Projection-mapped output onto a physical sculptural surface

## Library / network
- [ ] Public registry with signed packs and download counts
- [ ] Query-by-description search using a local model
- [ ] Collaborative patch sessions (two machines, one graph)
- [ ] Licence/clearance report generation per release

## Platform / product
- [ ] `sparq stage` appliance (ARM64 Linux, headless + thin client)
- [ ] iPad/Android companion as a pure control surface
- [ ] Ableton Link and MTC/LTC sync
- [ ] CLAP/VST3 host wrapper module
- [ ] Faust → sparq module codegen path
- [ ] Open-sourcing the engine + module API (D-8)
- [ ] Module author SDK with templates, docs site and example packs

## Explicitly rejected (do not re-propose without new information)
- Timeline/clip arrangement view as a primary surface *(contradicts pillar: the patch is the score)*
- Full web app version of sparq *(D-10: thin client only)*
- Lua/JS scripting on the audio thread *(ADR-002)*
- VST3/CLAP as the native module format *(ADR-002)*
- Automatic global latency compensation *(ADR-006: latency is a timbral resource)*
- Any feature requiring a modifier key, right-click or hover *(pillar 6)*

## WO-012 increment 5+ (parked from the shell, updated 2026-09-30 — increments 2 (live audio), 3 (convergence chrome) and 4 (the MTA audio-control thread, mouse support, master meters, palette tiles) shipped)

* **The library's port-type filter** (PN's "All Ports" select): search + category ship in increment 6; the port filter waits until the catalogue is big enough to need it (17 modules rank fine on name + category).
* **A third wire style** (stepped/orthogonal): the toolbar select ships SMOOTH and STRAIGHT; orthogonal routing needs crossing policy work the wire vocabulary does not have yet.
* **Inspector at 1440/1280**: the reflow rule yields the inspector to the canvas floor while the library keeps its column (params live on the cards now). Collapsing the library brings the inspector back at any width; a smarter split (overlay inspector, narrower library) is a design decision, not a fix.
* **Hover affordances** (cursor shapes, hover highlights) stay parked — §8 forbids hover-*only* affordances, and the adapter now drops unpressed mouse motion entirely (WO-012 inc 4), so hover is unobserved, not ignored.
* **Dock wheel-scroll** waits for dock content that scrolls (the palette fits at 1920; a 40-module registry would not) — until then the wheel over the dock pans the camera, declared. The LOG tab (increment 5b) ships a TAIL view instead of a scroll: the newest lines the dock height carries, older ones age out of sight but stay in the bounded ring for digests — a scroll there is the moment the log outruns a dock.
* **Meter bars beyond `out/main`** — withdrawn by operator ruling 2026-09-30 after living in increment 5 for a day: the ring still carries per-port stereo peaks for every audio output (increment 2's contract), so re-lighting them later is a painter change, not a data change. The ruling stands until the operator revisits it.
* **Per-channel meters on every node** — SHIPPED (WO-012 increment 5, D1): the bars draw on every audio-output node from the per-port ring the increment-2 drain already filled; `out/main` was the first paint, not the rule.

* **Node inset displays** — SHIPPED (WO-012 increment 5, slice B): the well registry in `sparq-ui::canvas::inset` dispatches the mockup's per-module wells (meters, envelope triangle, lfo period, svf curve thumbnail, scope); param-derived shapes at rest, live where the rings carry it.
* **The inspector response plot** — SHIPPED (WO-012 increment 5): the curve contract svf-first (`SvfFilter::magnitude_at`, pinned to the sine sweep) and the plot well with its read-only probe marker.
* **Live envelope/sparkline overlays** (D1′ of the increment-5 plan): the rolling cv-history `Sparklines` session set (the `ScopeTraces` discipline — session-owned, sized at the negotiated rate, fed from the ring, pruned on node-leave, empty-map at rest), waiting on a ring that carries per-block cv history. The param shapes ship at rest; this is the half that makes them glow with a level worth encoding.
* **Coefficient ramps beyond the gain-like class** (defect #85 shipped the five gain-like modules): SVF cutoff/resonance ramps need state continuity design (a coefficient glide moves the poles, not just a multiply); frequency steps are slope changes, not clicks; cv-side gains ramp at the cv consumer, not the cv source. Each waits on a measured click report, not on speculation.
* **Curve modules beyond svf** — the contract's second arm (an EQ, a delay's comb response) is one dispatch branch in `bridge::response_curve` plus its own DSP method; neither the display model nor the contract shape changes.
* **Drag-to-cutoff on the marker** — the marker is a read-only probe; writing cutoff from it would be a second param door. Revisit only if the operator asks.
* **Perform mode itself** — removed from the RUNTIME by operator ruling 2026-09-30 ("load directly into design mode, remove the perform mode for now"): the `ShellMode` enum, the mode toggles (top-bar word button, rail glyph), the XL macro pads, the nothing-below-class-L audit rule and the below-breakpoint Design refusal all went with it; small viewports now reflow Design (panels collapse to keep the canvas floor) and say so once in words. When the operator asks for it back, rebuild from `perform-mode.svg` + the inc-5b tree in the sync chain (pads, hero glyphs, the Perform audit rule) — the convergence work below still waits on Design reading like the mockup first.
* **Perform-mode convergence** against `perform-mode.svg` — its own increment, after Design reads like the mockup.

## WO-012 increment 2+ (parked from the shell prototype, 2026-09-21)

* **WM_POINTER contact area** → real palm rejection on the stage digitiser (winit reports no area; the recogniser's palm path is tested with synthetic areas only).
* Damage-driven repaint (stop continuous `request_redraw`; consume egui's repaint requests) — battery/CPU, not correctness.
* Haptics (§6 of the input model): detent ticks, latch confirmation, panic double-pulse — needs the device API and a haptic-capable surface.
* Rotate-gesture consumers (rig map, geometry displays) — recogniser emits `Rotate`, nothing binds it yet.
* Keyboard/controller parity layer (§5): intents are the binding surface; the `.sparqmap` asset is WO-009+.
* On-screen first-use hint for the ×10 fine drag (the intent exists — `DragFineChanged` — the coaching UI does not).
* Command palette / two-finger-tap search surface (Phase 1; the intent plumbing is already there via `DoubleTap`).
* egui default typeface → the chosen WO-002 faces once `chosen` is filled (sizes already token-correct).
* `preview.html` / `tokens.css` HC rendering: the COLOR_HC_* constants exist and drive the in-app theme switch, but the static preview emitters still render only the base palette — add an HC swatch section when the preview page is next touched.

## WO-013 increment 6+ (parked from the graph canvas, updated 2026-09-29 — increment 5 shipped the "Else column": rename, inspector scroll, per-cv levels, the LOD pass; WO-012 inc 2 shipped the live level source; increment 6 shipped the scope screen)

**Shipped in increment 6 (2026-09-29)**: **the scope screen** — `dsp/scope` draws. The
toolkit-independent model (`sparq-ui::canvas::scope`: rolling `TraceBuf` sized by
`timebase × the negotiated rate` and clamped to `MAX_TRACE`, a resize that zooms rather than
resets, rising-edge trigger with a declared free-run fallback, stride-decimated polyline
geometry with edge clamping, X/Y pairing over the shorter axis, NaN sanitised at the geometry
boundary, param clamping that does not trust its input) + the session's binding resolution (the
WIRE is the binding, re-resolved every frame so it cannot go stale; a rebind clears the traces
so no dead source's tail survives) + the painter (the `scope.trace` map's own row: amber trace
on `ground.inset`, `hairline.faint` crosshair grid, the WIRES' glow vocabulary — lerp toward
the class glow by the window peak plus the under-glow pass; the well draws at Full AND
Simplified, the TRACE at Full only, the Dot contract untouched). The traces live ONLY in the
session (D3′: the plan's canvas-field swap was built, the LOD smoke caught it alternating two
sets and blanking frames, and the single-source read replaced it — no canvas copy exists). The
manifest's param ORDER is pinned by a test (the painter reads by index). 32 audit smokes; the
LOD contract is measured in SHAPE counts, because this egui tessellates a frame into one
clipped primitive.

**Shipped in increment 5 (2026-09-27)**: the four items increment 4 declared for its next pass.
**Rename text entry** — `sparq-ui::canvas::entry` (`TextEntry`: end-caret, printables-only,
32-char cap = the node header's budget; `RenameState`: the sheet geometry on the browser's
clamping rule), the RENAME menu row, commit through `Op::Rename` (undoable since increment 1 —
EMPTY commits `None` = the module default, an unchanged buffer is a stated no-op), and the
shell's **unified modal key feed** (`read_keys` parses egui events ONCE; the rename sheet and
the browser query both consume the batch — the shared surface the provisional feed waited for).
**Inspector scrolling** — `compute_at` with a clamped offset, a fixed header the rows slide
under, `row_at`/`row_visible` one rule both directions (a bottom sliver is touchable, a row
under the header is not), a hairline thumb drawn only when the panel scrolls, and the gesture:
the two-finger `Pan` — which now carries its `center`, like `Zoom` always has — scrolls the
panel when the centre lands in it; a pinch over the panel is DECLINED so the canvas behind it
never moves (which also keeps the recogniser's sequential-contact span wobble off the camera).
Scroll is transient view state; it resets when the inspected node changes. **Per-cv wire
levels** — `NodeLevels` grew per-port entries, `wire_level` prefers the source PORT's level and
falls back to the node fold (audio semantics untouched, pinned), and `bridge::node_levels` reads
each cv output's real published value (`node_cv_block`/`node_cv_audio`, magnitude rule) — the
increment-4 declared limit, retired without touching the executor. **The LOD pass** — Dot wires
are hairlines (the contract this file always stated), Simplified is truly text-free, and states
ride the look-board's pattern language at every LOD: hatch = bypassed (the mockup's `hatch8`),
dashed border = muted, double border = locked, header chip / accent ring = master; at Dot,
hollow = bypassed, dimmed = muted, concentric ring = locked. The SUM word is suppressed at Dot
(declared). Five new audit smokes (25 total); the canvas-render.wav baseline semantics were
protected on purpose (the cv smoke reads the bridge directly, smoke 13 owns the menu path).

**Shipped in increment 4 (2026-09-27)**: **live wire levels** — the "signature sparq image". Wires
now animate from the executor's REAL meters: `sparq-ui::canvas::levels` (`NodeLevels` + `wire_level`,
toolkit-independent and unit-tested), `bridge::node_levels` (reads each node's peak meter after a
preview render, maps kernel→canvas nodes — the only source of a level, never faked), the painter's
class-colour→glow blend + under-glow (both endpoint colours are tokens), the transient
`CanvasState.levels` field the shell refreshes after RENDER WAV, and audit smoke 20. Plus the
`out/main` **master-handover** rule (`resolve_master` prefers a wired out/main; `OUT_MAIN_ID` a named
constant). **Not faked** — two bridge tests prove the levels are metered AND follow the signal.

**Shipped in increment 3** (no longer parked): the module browser + fuzzy search (long-press empty
canvas → ADD MODULE; the catalogue IS the registry, #58 structurally), the inspector with touch
sliders (per-node param state in the model, `Op::SetParam`, one drag = one undo step, the bridge
renders node state), and wire endpoint re-patch (grab handles the layout computes; the verdict
runs on the post-removal graph; refusals restore byte-exact).

* **Numeric entry for params**: sliders only in v0 — the text-entry surface now EXISTS
  (`canvas::entry`, increment 5), so this is one parse step away: a numeric row opens the same
  sheet, the buffer parses as a number, the model clamps/snaps as it always does. Until then the
  log line reports every committed value.
* ~~**Inspector scrolling**~~ **SHIPPED (increment 5)**: two-finger pan inside the panel, fixed
  header, clamped offset, thumb, reset on selection change. Still parked HERE: a scroll gesture
  over the BROWSER sheet (rows past one page still ride arrows + selection-scrolling), mouse-wheel
  piping for desktop sessions, and keyboard paging for the inspector.
* **`enum`/`text`/`blob` param editing**: shown greyed with the kind named; the manifest schema
  grows `options[]` first (v1), then the inspector grows an options row.
* **Multi-select inspection**: exactly one selected node inspects; multi-select param editing
  (common ranges, "edit 3 gains at once") is a later increment.
* **Browser scroll gestures + two-finger-tap search**: rows past one page are reachable by arrows
  (keyboard) and selection-scrolling only; a drag-to-scroll sheet and the `DoubleTap` command
  surface ride the same plumbing when it lands.
* ~~**Browser keyboard feed is provisional**~~ **UNIFIED (increment 5)**: one `read_keys` parse
  feeds both modal sheets (rename first, then the browser) — no widget, no focus policy, the
  wrap-egui rule intact. What stays parked: caret MOVEMENT (the caret is the buffer end in v0),
  selection/copy-paste, IME composition, and a focus policy for multiple simultaneous fields
  (there is never more than one modal sheet).
* **Render length + transport binding** (WO-009): `bridge::render_wav` renders a fixed, stated 5.0 s until transport exists; then RENDER WAV renders the arrangement (or the loop range), and the evidence line says which.
* **Master handover to `out/main`** (WO-014): SET MASTER + the resolve rule are the bridge-era answer to "which node feeds the listener"; when `out/main` ships, an out node in the patch supersedes the rule and the MASTER badge moves to it.
* **Live wire levels** ~~(the "signature sparq image")~~ **SHIPPED (increment 4)** for AUDIO
  signal flow, and **cv-wire levels SHIPPED (increment 5)**: `NodeLevels` carries per-port
  entries, `wire_level` prefers the source PORT's level, and the bridge reads each cv output's
  real published value from the executor (`node_cv_block` / `node_cv_audio`, magnitude for a
  bipolar swing, clamped), and **per-port AUDIO levels + the continuous play-time refresh
  SHIPPED (WO-012 increment 2, the live audio session)**: `MeterUpdate` carries port ids (one
  entry per audio OUTPUT port, `FOLDED` for a cv-only node), the analysis ring carries block-rate
  cv as one-sample waveforms, and while a session runs the shell drains both PER FRAME into the
  same `levels` field — a wire out of `util/mixer` now lights from its own channel, live, from
  the engine's own meters, and the **scope screen SHIPPED (WO-013 increment 6)** — the analysis
  ring's waveforms now have something DRAWING them (`sparq-ui::canvas::scope` + the painter; see
  the increment-6 note at the top of this section). What stays parked: **per-port meter
  DISPLAYS** beyond the wire glow (a mixer channel strip's own meters), and the offline path's
  parity — RENDER WAV still refreshes from a preview render, which the live drain simply
  overwrites while playing.
* **LOD visual iteration against `design-mode.svg`**: **the sandbox pass shipped (increment 5)** —
  the renderings now match the declared contracts (Dot = hairline wires; Simplified = no text) and
  the look-board's pattern-first state language at all three levels. What remains is the WO
  acceptance box itself: the **side-by-side human review** on the stage screen (test006 step H
  asks the operator to write what differs) — a mockup comparison is a judgement, not a gate, and
  it stays device-track until a real eye has signed it.
* **WM_POINTER contact area → real palm rejection while wiring**: the recogniser's palm path is proven with synthetic areas only (winit reports none); shared with WO-012 increment 2, and the wiring case ("palm resting on the screen while drawing a wire causes zero spurious input") is the acceptance criterion that needs it.
* **Grid-dot draw cost**: at Full LOD the world-anchored dot grid emits ~1000 circles/frame (the headless frame-logic median is grid-dominated); cache to a tile/texture or cull to the major grid when the Phase 6 rasteriser lands. Correctness is fine; this is a device-fps concern.
* ~~**Rename**~~ **SHIPPED (increment 5)**. **Randomise** stays parked on the seed tree
  (ADR-007): the menu row lands the moment a node can own a derivable seed.
* **Sub-graph / patch nesting** (Phase 1) and **mapping canvas** (Phase 5) remain out of scope per the WO.

## WO-014 increment 5+ (parked from the module set, updated 2026-09-27 — increments 3, 4 AND 5 shipped; the set is complete at SEVENTEEN)

**Shipped in increment 2** (no longer parked): `syn/noise`, `syn/polyblep` (aliasing acceptance
measured at −166.8 dB through the registry build), `flt/svf`, `util/delay` (parametric latency),
`fx/bitcrush`, `util/panner` — nine first-party modules total, docs generated from manifests
(`tools/module_docs.py --check` gates staleness in CI).

**Shipped in increment 3 (2026-09-26, contract v1's first consumers)**: `syn/membrane` (the
Phase-B kick topology promoted — trigger at sample 37 starts the hit AT sample 37, measured),
`env/ad` (event in → audio-rate cv out, no audio ports at all; loop semantics measured against
the wrapped AdEnv's epsilon-arrival decay, not assumed), `util/mixer` (4×4 stereo matrix, the
explicit merge; identity default is a bit-exact wire) — the drum-demo golden (`f2303f13aa0cf299`)
exercising host-trigger → membrane → mixer end to end.

**Shipped in increment 4 (2026-09-26, WO-009's first consumers)**: `mod/lfo` (four shapes,
audio-rate unipolar cv, event phase-reset on the exact sample; the lfo→svf sweep renders
BIT-IDENTICAL to a hand-driven reference) and `mod/clk-div` (divide/multiply/probability, the
first event-in-event-out module; the 16ths→÷4→membrane chain golden `914d9063ce9d8a0f` lands
kicks on exact frames) — **fourteen first-party modules**.

**Shipped in increment 5 (2026-09-27, the set completes at seventeen)**: `ana/tap` (the generic
signal tap — audio-rate bipolar `wave` plus block-rate `peak`/`rms`, the analysis source a display
rides; its wave golden `3f325d4f99ca2a01` CROSS-VALIDATES against the `syn/sine` 1 s golden — the
mono mix of a mono-fanned sine is the sine itself, bit for bit, so the tap colours nothing),
`dsp/scope` (the first real visual module — a display whose `process` is a deliberate NO-OP, so
its zero-audio-thread-cost acceptance is architecture; the proof is that adding a tap+scope to a
patch leaves the master render BIT-IDENTICAL, the scope-rig golden equalling the out/main golden
`75bc7f2f18cac9d5`), and `out/main` (the master output with the metering hook — a unity-bit-exact
pass-through with trim + hard mute, shipping with the WO-013-side master-handover rule so the
MASTER badge names the output module rather than guessing the last gain). The analysis-payload
CONTRACT they waited on also shipped: `sparq-audio::engine::AnalysisUpdate` extends ADR-009 d8's
publication from meters (peak/rms) to WAVEFORMS — `AudioEngine::publish_analysis` pushes every
audio-rate `cv` output onto a bounded `SpscRing` once per block, and `SharedEngine::read_analysis`
drains it control-side, so a scope reads signal off the ring and never touches the audio thread
(`tests/analysis_pub.rs`: the waveform crosses, an absent reader is counted-not-queued, and the
audio thread allocates nothing while publishing).

* **Analysis publication is GENERIC, not tap-only (a declared choice, not an oversight):**
  `publish_analysis` publishes every audio-rate `cv` output in the patch — a tap's wave, an
  `env/ad` envelope, a `mod/lfo` shape. The rule needs no per-node kind storage (the executor does
  not retain manifests post-build), it is bounded by the ring depth and refuses-and-counts when
  full, and it means any audio-rate cv signal is scopable. A consumer filters by the `(node, port)`
  its display is bound to. If a large rig's analysis traffic ever wants narrowing to declared
  analysis sources, that is a per-node kind flag at build — a small, well-specified change, parked
  here rather than pre-optimised.
* **Per-port meters stay folded** (a multi-output node's meter folds its audio outputs): the rings
  CAN carry port ids when a display needs per-port levels, but nothing in increment 5 does, so the
  fold stands. Declared, not forgotten.
* **LFO rate in BEATS — DECIDED *and* SHIPPED (2026-09-27): module-side bpm derivation, NOT a
  per-frame tick view.** The decision the WO asked for before building, the reasoning, and now the
  implementation: a per-frame tick view in `BlockContext` is a CONTRACT CHANGE that ripples through
  all 41 implementors and the `api_snapshot` pins, to buy sample-accurate tempo that almost no
  module needs; the module-side derivation reads the block-start `tick`/`ppqn` the executor ALREADY
  carries (WO-009's `set_musical_position` door, and the cross-thread `SetMusical` command that
  feeds it) and derives `bpm = Δtick / ppqn / Δseconds` from consecutive blocks (`TempoFollower`,
  a private helper in `sparq-audio::modules`) — no contract change, and the one block of lag on a
  tempo EDIT is 1.3 ms at 96 kHz/64, below any musical threshold. **SHIPPED:** `mod/lfo` 0.2.0
  grew `sync-mode` (0 Hz / 1 beat) + `division` (cycles per beat) and `util/delay` 0.2.0 grew
  `tempo-sync` + `division` (beats) riding the once-unreachable `DelayLine::set_tempo_sync`; both
  fall back to their free-running parameter when no transport feeds a tick (a beat-locked module
  never silently stops), and both are additive — at the defaults they are byte-for-byte the prior
  versions, which the unchanged goldens prove. `tests/tempo_sync.rs` (7 gates): the derived rate is
  the transport's (not the fallback's), it tracks a tempo change, the fallbacks hold, the
  tempo-synced delay renders BIT-IDENTICAL to a hand-timed 250 ms, a beat-locked golden
  (`db4013f41d1fa678`), and zero allocations while deriving. The delay's `set_tempo_sync` path is
  now reached, closing the last of this item.

## WO-009 increment 1+ (parked from clocks/transport, 2026-09-26 — the musical half shipped)

* **External sync — the slave PLL (ADR-006 rule 3)**: Phase 4, per the WO's out-of-scope list.
  `ClockBroker::observe_wall` is the door a rate+phase estimator will knock on; the stiffness
  parameter is a musical decision that wants real Link/MIDI hardware in front of it.
* **The 30-minute drift measurement** (the WO's third acceptance box, hardware half): the
  estimator is shipped and synthetic-verified (+1000 ppm → ±2); the HAL pump feeding
  `observe_wall` between blocks on SATURN, against an independent measurement, closes it.
* **Loop-relative beat phase for unaligned regions**: beats ride the absolute tick grid (exact);
  a loop whose span is not beat-aligned keeps the global grid rather than restarting the phase.
  The proportional-placement alternative was measured putting boundary beats one sample early —
  the transport header records it. A correct folded-domain map (or sub-block transport) is the
  fix, not a clamp.
* **Sub-block transport positions**: loops fold at block granularity — the same §16 Q2 boundary
  as sub-block processing, with the same remedy (a smaller host block).
* **Tempo as a full automation stream**: every change is already a map segment (the interface
  allows it, as the WO demanded); what's missing is an automation-lane UI and journal
  integration (WO-011/WO-013 territory), not clock math.
* **Metric modulation UI + per-track time signatures + count-in/metronome/recording**: out of
  the WO's scope by name; `Clock::reanchored` is the primitive metric modulation will use.
* ~~**`util/mixer`'s cv merge side**~~ **SHIPPED as v0.2.0 (WO-014 increment 6, 2026-09-27):**
  four block-rate unipolar cv inputs (`cv_reduce = "mean"`, declared per G3), per-input gains
  (params 20..23, appended LAST so every v0.1.0 snapshot keeps its indices and bits), one summed
  `cv-out` clamped to the declared range — the identity default makes an untouched cv side a
  bit-exact wire from `cv-0`. The executor's fan-in refusal STAYS (no implicit summing, ever) and
  now carries the working remedy in words: wire each source to its own `cv-N` and feed the
  destination from `cv-out`. `tests/mixer_cv.rs` (9 gates) proves it. What stays parked: the
  BIPOLAR cv side (waits for `util/range`, Phase 1 — a bipolar port today would refuse every
  unipolar source in the shipped set), a 4×4 cv matrix (16 more cells = 36 > MAX_PARAMS), and the
  canvas's one-tap INSERT offer for merges (`Adapter::Merge.first_phase` stays Phase 1 on
  purpose: the merge is a three-wire re-patch, not the inline pair shape the insert mechanism
  fits — recorded in `WO014-INC6-PLAN.md` §8).
* **`util/mixer` is 4×4, not 8×8**: 16 cells + 4 trims + 4 cv gains = 24 of the 32 snapshot
  parameters (v0.2.0). The matrix grows when `MAX_PARAMS` does (its manifest header says so).
* **Per-module example patches**: WO-015's study is the example patch that matters, and a `.sparq`
  project file needs WO-011's format writer — parked until then rather than inventing a format.
* ~~The rms→filter modulation demo~~ **SHIPPED with contract v1 (2026-09-26):**
  `sparq exec --patch mod-demo` renders it, and `tests/contract_v1.rs` proves it with arithmetic —
  the cv-wired filter renders BIT-IDENTICAL to a hand-driven reference set per block to the
  declared value, golden `1621e1f65b1b64e1` checked in.
* **The <15 %-of-a-core benchmark at 96 kHz/64**: stage-machine acceptance — sandbox CPU ratios
  do not transfer (2-core virtualised), so no sandbox number is recorded as evidence.
* **Allocation-free parameter TRANSITIONS**: polyblep's partial-table rebuild on a freq change
  allocates when the table grows; steady-state `process` is measured zero. Contract v1 shipped the
  cv wire (block-rate cv IS a control-rate path, and `flt/svf`'s modulation input proves it
  allocation-free per block), but param *smoothing* and table-rebuild-without-growth remain open —
  they need the arena/growth-policy design, not another port type.
* **`syn/polyblep` the per-sample oscillator**: the id keeps its promise when the real PolyBLEP
  (or BLIT/wavetable) lands from a reference derivation with the same aliasing gate — the osc.rs
  module docs and LATER's Phase-B entry both carry the derivation debt; the additive interim is
  measured, not hoped.

## WO-008 increment 3+ (parked from the executor, updated 2026-09-29 — tasks 4–7 shipped; the live-HAL reader side shipped as WO-012 inc 2)

**Shipped in increment 3** (no longer parked): the boundary-swap `Engine` + the 10 000-mutation
stress (allocation-gated, replay-deterministic), per-path latency accounting (kernel map, three
hand-computed reference graphs) + the raw/compensated switch (per-edge fan-in alignment), the
watchdog (N consecutive overruns ⇒ auto-bypass + bounded journal), and the determinism harness
(`sparq_audio::determinism`, scripted replay with hash equality).

* ~~The cross-thread hot-swap primitive~~ **SHIPPED as WO-008 increment 5 (2026-09-27):**
  `sparq-kernel::sync::hotswap` (allowlist entry 6 — d3's literal pointer swap, epoch-tagged
  retirement slots, deferral-not-forcing under slot exhaustion, Miri in CI) and the
  cross-thread `SharedEngine`/`AudioEngine` over it, proven by `tests/cross_thread.rs`'s
  10 000-mutation two-thread stress (zero failed blocks, zero audio-thread allocations, every
  retirement reclaimed and dropped exactly once). What stays parked is the shape around it:
  * **Multi-reader epochs** — the primitive's invariants are written for one control + one
    audio thread. A second reader (a visual analysis thread tapping the same live patch) needs
    per-reader quiescent states; the epoch machinery is ready for it, the second reader is not.
  * **The paced zero-xrun proof** — the allowlist's stress sentence ends "zero xruns", which is
    a real-time claim: it rides the loaded soak on SATURN, like every other device-track box.
  * ~~**HAL integration** — routing a live device stream through `SharedEngine`~~ **SHIPPED as
    WO-012 increment 2 (2026-09-29, `WO012-INC2-PLAN.md`):** the canvas shell's PLAY opens a
    HAL stream whose callback holds the `AudioEngine`; the operator's session-6 pivot
    re-sequenced it ahead of WO-006's exclusive acceptance (shared mode is the device-proven
    path; the ADR-008 exit stays gated). `sparq play` keeps pumping the static WO-005 graph —
    it is the HAL's own acceptance vehicle, not the canvas's.
* ~~Multi-port `AudioCtx` (contract v1)~~ **SHIPPED as WO-008 increment 4 (2026-09-26):**
  audio/cv/event payloads travel (data/gpu/atom still refuse in words, now per type); the executor's
  refusals, the merge order and the rate conversions are all measured in `tests/contract_v1.rs`.
* ~~**`cv_interp = "spline"`** is declared in the vocabulary and REFUSED at build until the host
  implements it~~ **SHIPPED as WO-008 increment 6 (2026-09-27):** the host performs all three
  spellings of G4. `spline` is the parabola through the last three block values —
  `v(t) = prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2`, `t = i/frames` — equivalently a cubic
  Hermite with the causal central-difference start tangent and the second-order backward end
  tangent. Exact for quadratic-in-block-index sweeps, the exact line for collinear knots,
  `linear`'s arrival contract (frame 0 IS the previous knot, `cur` is reached at the next block
  boundary — the shape of the ride changes, the timing never does), f64 with one rounding per
  frame, clamped to the wire's declared range (`CvRange::clamp_f64`): the overshoot is host-made
  momentum, so the host bounds it, while `hold`/`linear` still never clamp — an out-of-range knot
  there is a SOURCE bug and stays visible. `CvPlan` grew `prev2` + `range`; both history slots
  start at 0.0 (the declared zero-history ramp `linear` always had), so the first two blocks are
  a documented deterministic transient. The build refusal retired; the matrix cell, §17, the
  schema row and the author guide now carry the same sentence, pinned by the widened G4 drift
  gate. `tests/cv_spline.rs` (8 gates), two `port.rs` unit pins, contract_v1's refusal gate
  replaced by a behaviour gate. **752 tests**, every pre-existing golden unchanged, stress hash
  `b42068ec7b206789` unmoved (debug AND release).
  * **Still parked, declared:** the *non-causal* splines — true Catmull-Rom (needs `v[N+1]`) and
    the natural cubic spline (a global solve over future knots) both buy C² smoothness with a
    block of LATENCY, which would make `spline` time its wire differently from `linear`; taking
    that trade is a contract decision, not an implementation detail, so it stays out. A monotone
    (PCHIP/Fritsch–Carlson) variant was also rejected: its limiter bends parabolas near extrema,
    losing the exactness property, and the safety it buys is already provided by the clamp where
    it matters. If a listening test ever says the momentum overshoot is wrong for a given wire,
    the remedy is a new vocabulary word with its own gate, never a quiet change to this one.
* ~~**`required`-unconnected inputs are not refused at build** (v0 behaviour kept deliberately: the
  determinism world's unwired spare depends on it, and moving refusals mid-stress would move the
  harness counters). Host-side enforcement gets its own increment, with the stress baseline moved
  on purpose and recorded.~~ **SHIPPED as WO-008 increment 7 (2026-09-29, `WO008-INC7-PLAN.md`):**
  `Executor::build` refuses a bare required input in words; the world's spare is wired (from the
  chain gain, so retunes reach the master), the canvas demo's spare became a bare `out/main`, the
  unconnected-signal semantics live at optional ports (their declared home), and the baseline
  moved on purpose and is recorded: **`7bb06379bd6845e5` · 2 383 swaps · 7 617 refused** (debug
  == release), was `b42068ec7b206789` · 7 203 · 2 797.
* **A canvas badge for the required-unconnected node.** The refusal surfaces verbatim in the
  shell log when a render is attempted (the bridge passes `ExecError` through), but the node
  itself draws no warning — the operator learns at RENDER WAV, not at drop time. A red hairline
  or port-badge for "this node's required input is bare" is a WO-013-side painter increment,
  parked beside the watchdog→UI hairline it would share machinery with.
* ~~Cross-thread meter/analysis publication~~ **SHIPPED as WO-008 increment 5 (2026-09-27),
  analysis payloads extended in WO-014 increment 5:**
  `AudioEngine` publishes per-node `MeterUpdate`s (peak/rms/status + the stream's block count)
  into an `SpscRing` every block, and an `EngineCmd` ring carries `Copy` commands the other way
  (`SetParams`/`SetMusical`/`ClearMusical`/`ClearAutoBypass`) — both refuse-and-count when full.
  **WO-014 inc 5 added the analysis half:** `AnalysisUpdate` (node, port, block, a bounded waveform)
  rides a sibling `SpscRing`; `AudioEngine::publish_analysis` pushes every audio-rate `cv` output
  per block via `Executor::with_audio_rate_cv_out`, and `SharedEngine::read_analysis` drains it —
  the payload `ana/tap` writes and `dsp/scope`'s UI reads (`tests/analysis_pub.rs`) — **read
  since WO-013 increment 6: the scope screen draws it**. And the
  **UI consumer wiring shipped as WO-013 inc 4** (live wire levels read the meters), and
  **WO-012 inc 2 shipped the rest of the reader side**: **per-port meters** (port ids on
  `MeterUpdate`, one entry per audio OUTPUT port via `Executor::with_audio_out`, a `FOLDED`
  sentinel entry for cv-only nodes so their status still crosses) and the **continuous play-time
  refresh** (the shell drains `read_meters` + `read_analysis` per frame into `CanvasState.levels`;
  block-rate cv outputs now publish as one-sample waveforms so a cv wire's LIVE level reads the
  same magnitude rule as the offline one). Still parked: **narrowing analysis publication** to
  declared analysis sources (today it publishes every cv output — generic, bounded, counted; a
  per-node kind flag at build would narrow it if a large rig's ring traffic ever wants it) and
  the **multi-reader epoch** (a second reader of the live patch).
* **Watchdog → UI flag**: `auto_bypassed(node)` is readable and the journal exists; the canvas's
  red hairline + BYPASS-WATCHDOG badge (ADR-009 d6) is a WO-013-side painter increment.
* **The degradation ladder beyond auto-bypass** (plan §5.2: FFT size → display refresh → voice
  count → oversampling → block size): each rung logged and reversible — Phase 1, per the WO's
  out-of-scope column.
* **Compensation for delay-edge arms**: v0 aligns PLAIN fan-in arms only; unit/block-delay arms
  neither set nor receive compensation (a feedback arm's offset IS the sound; sub-block
  placement of a 1-sample arm is out of scope). Stated in the executor's docs.
* **The loaded soak** — 200 modules, 30 min, zero xruns on the stage machine — is the WO's
  hardware acceptance and stays device-track (needs SATURN + the HAL).

## Delivery / build plumbing (parked from WO-013 increment 2b, 2026-09-25 — defect #78)

* **A pre-pack gate.** `tools/sync_check.py --write` then `--quiet` should run *inside* the packing step, so a zip is verified against its own manifest before it is sent. Today the detector runs on the receiving machine, which is one round trip later than it could be — and #78 cost exactly one round trip.
* **Fold `pack source.bat` and the stamp generator.** Two tools must agree about what a sync contains; they do not know about each other. One command should list the payload, hash it, write `SYNC-STAMP.txt` and zip the lot.
* **Widen the purge if a dependency ever goes stale.** `build.bat` deletes `target\release\.fingerprint\sparq-*` — the five first-party crates. If a sync ever re-vendors a third-party crate (`Cargo.lock` plus a patched source), the same mtime trap reopens one level down and the errors will point at a crate nobody edited. The run sheet's branch 2 is the tripwire; the fix is to widen the glob, not to delete `target\` every build.
* **Cargo's own content-hash freshness.** `-Z checksum-freshness` replaces mtimes with checksums in cargo's fingerprints (unstable; the tracking issue is rust-lang/cargo#14136, which describes stabilising a `build.fingerprint` config field). That is precisely #78's mechanism (b) fixed upstream. The toolchain is pinned to stable in `rust-toolchain.toml`, so the purge stays — but on the day that flag stabilises, delete the purge from `build.bat` and `gates.bat` and keep the stamp check, which answers a different question (did the sync land) that cargo will never answer.
* **Digest hooks for the remaining test scripts.** `test004`, `test006` and `gates` now make their `-digest.log` copies at the end of a run (2026-09-28, WO-006 inc 1.4 — the operator's context-window ask). `test001`/`test002`/`test003`/`test005` still rely on `scripts\digest.bat` or a manual `python tools\log_digest.py <log>`; add the same small hook the next time each script is touched for its own reasons — not worth a bundle by itself.
* **`test005`'s stray-artefact check is one file deep.** Step [00] looks for `decode (2).rs` by name (defect #71). Step [00b] now catches any stray under `crates/*/src` generically — as an `EXTRA` warning plus a fingerprint failure — so the named check can go the next time that script is touched.

## WO-006 increment 1.5+ (parked from the exclusive acceptance, 2026-09-29 — #91 fixed, #76 shipped, #92 open on the device side)

* **Runtime stall REACTION (the reopen half stays parked; the detection half shipped).** Inc 1.5 shipped the instrument: drift is delivered-frames-vs-wall (#76), suspect past ±1 % after a 2 s trust window, and the play/soak verdicts ACT on it — the attempt-2/3 signature (sustained wake intervals ≫ the period, half throughput, healthy FIFO, no `WAIT_TIMEOUT`) now reads as a measurement with a verdict instead of an "implausible clock" excuse. What stays parked is a REACTION beyond the verdict — e.g. one automatic reopen at a coarser cadence or a lower rate from a startup verification window. Attempt 3's evidence narrowed the design space first: the stall is period-independent (identical ~16/s × ~33–40 ms wall at 3 ms and 10 ms periods), so reopening *within the period ladder* buys nothing unless the cadence jumps to ≥ 5× the stall duration — which in event mode means ≥ 50 ms of latency, worse than the shared engine's 22.67 ms. If attempt 4 shows the stall is also rate-independent, the reopen target is not a ladder rung at all: it is push mode (below). The control-side shape (verify ~1 s after start, stop, reopen, print both truths) avoids the pump-thread COM/event/state-machine hazards this entry was originally parked for — but it inherits their design burden the moment the reaction is automatic rather than operator-decided.
* **Push-mode exclusive — #92's named lever if attempt 4 comes back rate-independent.** Event-driven exclusive cannot legally bank more than one period (`hnsPeriodicity` ≡ `hnsBufferDuration` under `EVENTCALLBACK`; the pump's per-wake write is capped at one buffer — attempts 2–3 measured exactly that identity, windows-notes §4d/§4e). Push mode lifts the law: `Initialize` without `EVENTCALLBACK`, periodicity 0, buffer 2–4 periods, pump self-paces (~period/4) and writes against padding — the shape the audio engine itself runs on this endpoint (2112 fr over ~960 fr ticks, 2 h clean) and the default of mature WASAPI outputs on this driver class. Design before code, its own increment: the pump's self-pacing discipline (timer granularity; the callback-side no-clock rule is untouched — the pump is harness side), ADR-004's "event-driven" wording (an addendum, not a quiet change), late-wake semantics when the expected cadence is a policy number instead of a driver number, and the `BackendKind` display names.
* **A period probe in caps.** `devices --caps` probes exclusive RATES but not exclusive PERIODS: a ~200 ms `Initialize` probe per ladder candidate would have surfaced #89 at step [03] instead of step [04] — at the cost of touching the device during enumeration and a slower caps command. Parked until a device needs it; the [04] evidence path is sufficient (and the digest keeps it cheap).
* **Increment 2 backlog** (`docs/hal/windows-notes.md` §5): ASIO, full duplex, round-trip measurement, the STA retry for #75 — and no longer the clock-drift re-derivation (#76 shipped in inc 1.5: delivered-vs-wall, the `IAudioClock` binding deleted, both §4b fixtures pinned as tests).
