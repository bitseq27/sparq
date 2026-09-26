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

## WO-013 increment 3+ (parked from the graph canvas, updated 2026-09-25 — increment 3 shipped the browser, the inspector and wire re-patch)

**Shipped in increment 3** (no longer parked): the module browser + fuzzy search (long-press empty
canvas → ADD MODULE; the catalogue IS the registry, #58 structurally), the inspector with touch
sliders (per-node param state in the model, `Op::SetParam`, one drag = one undo step, the bridge
renders node state), and wire endpoint re-patch (grab handles the layout computes; the verdict
runs on the post-removal graph; refusals restore byte-exact).

* **Numeric entry for params**: sliders only in v0 — exact-value entry needs the text-entry
  surface (shared with rename below); until then the log line reports every committed value.
* **Inspector scrolling**: rows below the panel bottom are clipped and NOT touchable (honest, but
  a 32-param module shows only its first ~10); a scroll gesture for the panel is parked here.
* **`enum`/`text`/`blob` param editing**: shown greyed with the kind named; the manifest schema
  grows `options[]` first (v1), then the inspector grows an options row.
* **Multi-select inspection**: exactly one selected node inspects; multi-select param editing
  (common ranges, "edit 3 gains at once") is a later increment.
* **Browser scroll gestures + two-finger-tap search**: rows past one page are reachable by arrows
  (keyboard) and selection-scrolling only; a drag-to-scroll sheet and the `DoubleTap` command
  surface ride the same plumbing when it lands.
* **Browser keyboard feed is provisional**: `feed_browser_keys` reads egui input events while the
  sheet is modal (no widget, no focus policy); a real text-entry surface replaces it (rename too).
* **Render length + transport binding** (WO-009): `bridge::render_wav` renders a fixed, stated 5.0 s until transport exists; then RENDER WAV renders the arrangement (or the loop range), and the evidence line says which.
* **Master handover to `out/main`** (WO-014): SET MASTER + the resolve rule are the bridge-era answer to "which node feeds the listener"; when `out/main` ships, an out node in the patch supersedes the rule and the MASTER badge moves to it.
* **Live wire levels** (the "signature sparq image"): needs the WO-008 executor's analysis taps + meter ring; until then wires draw in class colour at rest, no level animation.
* **LOD visual iteration against `design-mode.svg`**: the three levels are computed from tokens and switch correctly, but the Simplified/Dot *renderings* are first-pass — the WO risk column ("LOD making the graph unreadable when zoomed out — iterate on the mockup first") is still open.
* **WM_POINTER contact area → real palm rejection while wiring**: the recogniser's palm path is proven with synthetic areas only (winit reports none); shared with WO-012 increment 2, and the wiring case ("palm resting on the screen while drawing a wire causes zero spurious input") is the acceptance criterion that needs it.
* **Grid-dot draw cost**: at Full LOD the world-anchored dot grid emits ~1000 circles/frame (the headless frame-logic median is grid-dominated); cache to a tile/texture or cull to the major grid when the Phase 6 rasteriser lands. Correctness is fine; this is a device-fps concern.
* **Rename / randomise context rows**: need text entry (rename) and the seed tree (randomise, ADR-007) — the `Op::Rename` value and the menu plumbing exist, the rows are deferred until their surfaces do.
* **Sub-graph / patch nesting** (Phase 1) and **mapping canvas** (Phase 5) remain out of scope per the WO.

## WO-014 increment 2+ (parked from the module set, updated 2026-09-26 — the audio-domain six shipped)

**Shipped in increment 2** (no longer parked): `syn/noise`, `syn/polyblep` (aliasing acceptance
measured at −166.8 dB through the registry build), `flt/svf`, `util/delay` (parametric latency),
`fx/bitcrush`, `util/panner` — nine first-party modules total, docs generated from manifests
(`tools/module_docs.py --check` gates staleness in CI).

* **The eight remaining modules wait on contracts, not on effort:** `syn/membrane`, `env/ad`,
  `mod/lfo`, `mod/clk-div` (event/trigger ports — need the multi-port `AudioCtx`, contract v1, and
  `mod/*` additionally need WO-009's clocks); `util/mixer` (N×M needs multi-port); `ana/tap`,
  `dsp/scope` (data/gpu plumbing needs the ring publication of ADR-009 d8); `out/main` (device
  channel mapping + the canvas master handover — ship it together with the WO-013-side rule
  change so the MASTER badge never lies). A module whose trigger port cannot receive a trigger is
  a lie with a manifest; these land when their payloads can travel.
* **Per-module example patches**: WO-015's study is the example patch that matters, and a `.sparq`
  project file needs WO-011's format writer — parked until then rather than inventing a format.
* **The rms→filter modulation demo** (analysis-as-control-source, end to end): needs cv edges to
  carry samples = contract v1. The principle is proven today at the executor level (the rms tap
  value agrees with the master's metered rms to the bit); the *audible* demo waits for the payload.
* **The <15 %-of-a-core benchmark at 96 kHz/64**: stage-machine acceptance — sandbox CPU ratios
  do not transfer (2-core virtualised), so no sandbox number is recorded as evidence.
* **Allocation-free parameter TRANSITIONS**: polyblep's partial-table rebuild on a freq change
  allocates when the table grows; steady-state `process` is measured zero. Control-rate parameter
  design (contract v1) is where smoothed, allocation-free transitions land.
* **`syn/polyblep` the per-sample oscillator**: the id keeps its promise when the real PolyBLEP
  (or BLIT/wavetable) lands from a reference derivation with the same aliasing gate — the osc.rs
  module docs and LATER's Phase-B entry both carry the derivation debt; the additive interim is
  measured, not hoped.

## WO-008 increment 3+ (parked from the executor, updated 2026-09-25 — tasks 4–7 shipped)

**Shipped in increment 3** (no longer parked): the boundary-swap `Engine` + the 10 000-mutation
stress (allocation-gated, replay-deterministic), per-path latency accounting (kernel map, three
hand-computed reference graphs) + the raw/compensated switch (per-edge fan-in alignment), the
watchdog (N consecutive overruns ⇒ auto-bypass + bounded journal), and the determinism harness
(`sparq_audio::determinism`, scripted replay with hash equality).

* **The cross-thread hot-swap primitive** — ADR-009 d3's literal pointer swap with an epoch-based
  grace period, as allowlisted-`unsafe` kernel work (`sparq-kernel::sync`) with its own proofs.
  The `Engine` is single-owner until it lands; the boundary semantics are proven by interleaved
  stress, the concurrency by nothing yet — declared, not hidden.
* **Multi-port `AudioCtx` (contract v1)** so cv/event/data payloads travel — until then a
  non-audio edge is REFUSED at build in words, never silently ignored (the v0 shape stands).
* **Cross-thread meter/analysis publication** (decision 8's lock-free rings): meters are relaxed
  atomics readable from any thread holding a reference; the ring publication the UI consumes
  arrives with (or after) the hot-swap increment. Live wire levels (WO-013) wait on this.
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
* **`test005`'s stray-artefact check is one file deep.** Step [00] looks for `decode (2).rs` by name (defect #71). Step [00b] now catches any stray under `crates/*/src` generically — as an `EXTRA` warning plus a fingerprint failure — so the named check can go the next time that script is touched.
