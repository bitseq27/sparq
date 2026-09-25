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

## WO-013 increment 3+ (parked from the graph canvas, updated 2026-09-24 — increment 2 shipped the bridge)

* **Module browser + fuzzy search** (command surface v0): the dock's MODULES tab is drawn disabled; the canvas can add nodes only via the demo graph + DUPLICATE until the browser lands. Two-finger-tap search surface rides the same `DoubleTap`/command plumbing.
* **Inspector with touch sliders + numeric entry** for the selected module's params (WO-013 in-scope, needs the validated-param accessor from `sparq-module-api`); param-state then joins the undo vocabulary (the `Op` enum already anticipates graph-level undo). **The bridge now depends on this:** `bridge::build` renders every node at its manifest DEFAULT params until nodes carry param state — stated in the bridge's module docs.
* **Render length + transport binding** (WO-009): `bridge::render_wav` renders a fixed, stated 5.0 s until transport exists; then RENDER WAV renders the arrangement (or the loop range), and the evidence line says which.
* **Master handover to `out/main`** (WO-014): SET MASTER + the resolve rule are the bridge-era answer to "which node feeds the listener"; when `out/main` ships, an out node in the patch supersedes the rule and the MASTER badge moves to it.
* **Wire endpoint re-patch by drag** (grab a connected port or a wire end and move it): increment 1 selects a wire on drag and says so in words rather than silently doing nothing.
* **Live wire levels** (the "signature sparq image"): needs the WO-008 executor's analysis taps + meter ring; until then wires draw in class colour at rest, no level animation.
* **LOD visual iteration against `design-mode.svg`**: the three levels are computed from tokens and switch correctly, but the Simplified/Dot *renderings* are first-pass — the WO risk column ("LOD making the graph unreadable when zoomed out — iterate on the mockup first") is still open.
* **WM_POINTER contact area → real palm rejection while wiring**: the recogniser's palm path is proven with synthetic areas only (winit reports none); shared with WO-012 increment 2, and the wiring case ("palm resting on the screen while drawing a wire causes zero spurious input") is the acceptance criterion that needs it.
* **Grid-dot draw cost**: at Full LOD the world-anchored dot grid emits ~1000 circles/frame (the headless frame-logic median is grid-dominated); cache to a tile/texture or cull to the major grid when the Phase 6 rasteriser lands. Correctness is fine; this is a device-fps concern.
* **Rename / randomise context rows**: need text entry (rename) and the seed tree (randomise, ADR-007) — the `Op::Rename` value and the menu plumbing exist, the rows are deferred until their surfaces do.
* **Sub-graph / patch nesting** (Phase 1) and **mapping canvas** (Phase 5) remain out of scope per the WO.

## Delivery / build plumbing (parked from WO-013 increment 2b, 2026-09-25 — defect #78)

* **A pre-pack gate.** `tools/sync_check.py --write` then `--quiet` should run *inside* the packing step, so a zip is verified against its own manifest before it is sent. Today the detector runs on the receiving machine, which is one round trip later than it could be — and #78 cost exactly one round trip.
* **Fold `pack source.bat` and the stamp generator.** Two tools must agree about what a sync contains; they do not know about each other. One command should list the payload, hash it, write `SYNC-STAMP.txt` and zip the lot.
* **Widen the purge if a dependency ever goes stale.** `build.bat` deletes `target\release\.fingerprint\sparq-*` — the five first-party crates. If a sync ever re-vendors a third-party crate (`Cargo.lock` plus a patched source), the same mtime trap reopens one level down and the errors will point at a crate nobody edited. The run sheet's branch 2 is the tripwire; the fix is to widen the glob, not to delete `target\` every build.
* **Cargo's own content-hash freshness.** `-Z checksum-freshness` replaces mtimes with checksums in cargo's fingerprints (unstable; the tracking issue is rust-lang/cargo#14136, which describes stabilising a `build.fingerprint` config field). That is precisely #78's mechanism (b) fixed upstream. The toolchain is pinned to stable in `rust-toolchain.toml`, so the purge stays — but on the day that flag stabilises, delete the purge from `build.bat` and `gates.bat` and keep the stamp check, which answers a different question (did the sync land) that cargo will never answer.
* **`test005`'s stray-artefact check is one file deep.** Step [00] looks for `decode (2).rs` by name (defect #71). Step [00b] now catches any stray under `crates/*/src` generically — as an `EXTRA` warning plus a fingerprint failure — so the named check can go the next time that script is touched.
