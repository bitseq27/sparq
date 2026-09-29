# SPARQ — Phase 0 Work Orders

**Phase:** 0 — Proof of concept (weeks 1–6; a 4-week cut list is in §7)
**Stack (locked):** Rust core · egui scaffold → custom `wgpu` shell later · T1 native modules (+ T3 process bridge design only) · **Windows primary** (WASAPI exclusive → ASIO), Linux/macOS in CI from Phase 1
**Companion doc:** `SPARQ-PLAN.md` (§ numbers below refer to it)
**Rule for this phase:** *nothing gets built that isn't in a work order, and no work order is done until its acceptance criteria are demonstrably met.* Still no production code — this is the buildable ticket set.

---

## 1. Phase 0 objective

Prove, on the actual stage hardware, that the four riskiest assumptions of the whole project hold:

| # | Assumption | Proven by |
|---|---|---|
| A1 | Rust + a custom HAL can deliver **zero-xrun** audio at 96 kHz / 64 samples on Windows under a real-time-safe graph | WO-006, WO-008, WO-016 |
| A2 | A **typed module contract** lets you add a module without touching engine code | WO-007, WO-014 |
| A3 | The **visual identity** works on a touchscreen and reads as scientific instrument, not music software | WO-004, WO-013, WO-016 |
| A4 | The system is **deterministic and reproducible** (same seeds ⇒ same samples; journal ⇒ re-render) | WO-010, WO-011 |

Plus one non-negotiable cultural assumption: **A5 — you can make music with it.** (WO-015). If A5 fails, nothing else matters and Phase 1 gets re-scoped.

---

## 2. Work-order index and dependency graph

| ID | Title | Week | Est. (days) | Depends on |
|---|---|---|---|---|
| WO-000 | Repository, workspace, CI, ADR scaffold | 1 | 1.5 | — |
| WO-001 | Stage hardware decision & Windows audio baseline | 1 | 1 | — |
| WO-002 | Design tokens v0 + typography + colour maps | 1 | 2 | — |
| WO-003 | `docs/VISION.md` + ADR-001…007 | 1 | 1 | WO-002 |
| WO-004 | Static canvas mockups (identity before code) | 2 | 1.5 | WO-002 |
| WO-005 | First sound out (bootstrap device layer) | 2 | 1 | WO-000, WO-001 |
| WO-006 | Real-time HAL: WASAPI exclusive + ASIO, MMCSS, diagnostics | 2–3 | 4 | WO-005 |
| WO-007 | Module contract v0 (manifest, ports, params, lifecycle) | 3 | 3 | WO-000 |
| WO-008 | Graph + executor (typed edges, atomic mutation, RT-safe) | 4 | 4 | WO-006, WO-007 |
| WO-009 | Clocks + transport v0 (three clocks, event queue) | 5 | 2.5 | WO-008 |
| WO-010 | Offline render + golden-reference test harness | 5 | 2 | WO-008 |
| WO-011 | Journal v0 + project file + deterministic replay | 6 | 2.5 | WO-009, WO-010 |
| WO-012 | egui shell: window, DPI, touch input, panels | 4–5 | 2 | WO-002, WO-004 |
| WO-013 | Graph canvas: nodes, wires, touch gestures, undo | 5–6 | 3.5 | WO-012, WO-008 |
| WO-014 | Module set v0 (17 modules incl. scope display) | 6 | 3 | WO-007, WO-008 |
| WO-015 | The Phase 0 study (60 s of music + capture) | 6 | 2 | WO-013, WO-014 |
| WO-016 | Phase gate: exit criteria, soak, devlog, Phase 1 scope | 6 | 1.5 | all |

**Total: ~39 working days**, which is why Phase 0 is scheduled as **6 weeks** at ~25–30 h/week rather than the 4 weeks a naive estimate suggests. A realistic Phase 0 is worth more than a fast one — everything downstream inherits its foundations. If you need it shorter, use the cut list in §7: cut in that order, and never cut WO-001, WO-006, WO-007, WO-008, WO-012/013 or WO-015.

```
WO-000 ─┬─> WO-005 ─> WO-006 ─┐
WO-001 ─┘                     ├─> WO-008 ─┬─> WO-009 ─> WO-011 ─┐
WO-007 ───────────────────────┘           └─> WO-010 ───────────┤
WO-002 ─┬─> WO-003                        WO-008 ─> WO-013 ─────┤
        └─> WO-004 ─> WO-012 ───────────────────────────────────┤
                                          WO-014 ────────────────┤
                                                                 └─> WO-015 ─> WO-016
```

---

## 2.1 Build log — status and deviations

| WO | Status | Notes |
|---|---|---|
| WO-000 | **done** | Workspace (4 crates), toolchain pin, `rustfmt`/`clippy` config, `clippy.toml` real-time denials, `unsafe` allowlist + `tools/unsafe_audit.py`, CI workflow, `justfile`, golden-reference conventions, `LATER.md`. Deviation: no `cargo xtask` crate yet — `just` is the command surface and CI calls the commands directly. Add xtask only if `just` becomes a barrier. |
| WO-001 | **not started** | Needs your hardware. Sheets ready in `docs/hardware/stage-baseline.md`. **Everything measured so far is Linux/x86-64 and does not transfer to Windows** — the WASAPI/ASIO latency matrix and the DPC budget are the whole point of this WO. |
| WO-002 | **done** | 5 token files, generator (6 artefacts + 10 conformance checks), preview page, look board. **Open:** the two typefaces are still `chosen = ""`. |
| WO-003 | **done** | `VISION.md` + ADR-000…009 (009 still `draft` — ratify with the WO-006/008 measurements). |
| WO-004 | **done (3 of 3 mockups)** | Design mode, Perform mode, display sheet — all generated from tokens, all passing `token_audit`. **Open:** the human protocols (blind identity, distance, dark-room, glove, monochrome) and the display-sheet tablet/wall breakpoints. |
| WO-005 | **done except live playback (Phase A)** | Offline render, DSP chain, control ring, diagnostics, golden reference, selftest, soak. Deviation: **no window** — the egui shell is WO-012, so Phase 0 uses a terminal control surface. Everything the window would have driven (control ring, counters, render path) is real and tested. `play` compiles and lints clean in CI on all three OSes but **has never produced sound**: no audio device exists in this environment. |
| WO-006 | **in progress — increment 1.5 built (2026-09-29) after test004 attempt 3 GRANTED the default 10 ms period and stalled anyway (defect #92: the stall is period-independent — delivered = wakes × buffer, exactly, on both attempts) and exposed defect #91 (the exclusive rate envelope was sieved through the shared probe, so play's 48 kHz ask was adjusted UP to 96 kHz); acceptance = the `test004` re-run at the driver's own 48 kHz, with #76's delivered-vs-wall drift as the instrument** | HAL trait + diagnostics + null backend + conformance + WASAPI exclusive/shared, 242 tests, four clippy matrix cells clean. **Physical SATURN session (operator present, no RDP, High-performance scheme):** the endpoints are RESOLVED by name — the default is **OUT 1-2 (BEHRINGER UMC 204HD 192k)**; shared mode is proven on the real interface: **first sound ever through the HAL** (tone audible), 10 s plays + a 5-min soak @96k/64 all **0 xruns / 0 allocations** (450 307 blocks, p50 1.0 µs / p99 2.0 µs / max 108 µs, 3 outliers), 4 ch f32 @96k negotiated on OUT 1-4 via the two-shapes ladder (#38/#46 fix working on hardware), **the unplug acceptance criterion PASSED** (mid-run removal → `Removed` state, dev-err 1, 0 xruns, clean stop, recovery play + tone after re-plug), reopen-leak 0 on hardware. **Multichannel criterion resolved honestly** (interface maxes at 4 out; caps say so; null proves ≥8). Remaining for acceptance: **the 2 h zero-xrun soak at 96 kHz/64 EXCLUSIVE — blocked by defect #77**: the exclusive ladder probes f32 only and the Behringer driver refuses f32 exclusive (`AUDCLNT_E_UNSUPPORTED_FORMAT`, not policy — both checkboxes ticked); increment 1.2 adds integer rungs (i32/24-in-32/i16 + pump conversion), then test004 runs the acceptance. Also found: **#75** (friendly-name `E_ACCESSDENIED` on REAL endpoints with no remote session — #40's "RDP quirk" attribution falsified; registry reads names fine → fallback path proven) and **#76** (the `SUSPECT` drift decoded: `(buffer_frames ÷ event_period) ÷ rate − 1` = +1.2 M ppm on BOTH the RDP and Behringer sessions to four digits — `IAudioClock` advances in buffer steps per event tick; the #45 guard did its job, throughput stayed truthful). `docs/hal/windows-notes.md` §4/§4b carry the rows and the arithmetic. |
| WO-012 | **in progress — increments 1–3 built and sandbox-green; increment 2 is the LIVE audio session (PLAY streams the drawn patch through the HAL over `SharedEngine`; continuous per-frame wire levels from the engine's rings), increment 3 the mockup-conformance chrome (the shell draws in `design-mode.svg`'s language; `sparq ui --svg-out` is the visual gate); awaiting device: test006 steps J–K and the chrome's first real-screen look** | Toolkit-independent UI core in `sparq-ui` (pointer model, gesture recogniser with the full §14.3 table, shell layout computation, touch-target audit — 37 tests, zero dependencies) + the egui shell in `sparq-app` behind `ui`/`ui-window` features (token-generated style adapter incl. the derived high-contrast theme, window host with per-monitor DPI via winit, headless driver). `sparq ui --audit` is a gate: 5 viewports × 4 DPI scales × 2 modes + DPI-invariance + 9 synthetic-gesture smokes — **PASS (0 failures)** in sandbox; frame logic med 94 µs headless. Defects #50–#53 found and fixed (table below). **Awaiting device:** the DPI matrix on real monitors, touch with a real finger, palm rejection (needs WM_POINTER contact area — winit reports none), 60 fps on the stage device. |
| WO-007 | **task 1 done — the contract is data, and ten decisions are taken** | `docs/api/compat-matrix.toml` (6 port types, 21 same-type cases, 5 verdicts, 4 adapters, 1 cell still open) and `docs/api/manifest-fields.toml` (82 field rows, 22 required, an error code per violation) — the tables-first artefact task 1 asks for, both parsing, neither consumed by code yet. `WO007-TASK1-REVIEW.md` carries the ten ratified decisions and the two acceptance blockers found. §16's five open questions are all answered, Q1 **by measurement** (`tools/dispatch-bench`: enum dispatch is within noise of monomorphised; per-sample trait objects cost 27–36 µs for 100 null modules against a 20 µs budget, so `process(block)` may be a trait object and nothing per-sample may be). Amended: `manifest-schema.md`, `module-api-v1.md` (§2 cascade rule, §3, §14, §16), ADR-005 addendum, **ADR-009 executor decision 7 + Consequences**, Appendix B and §17 Phase 1 (12 → 15 modules). Defects #54–#65 below. **Tasks 2–3 built** (increment below): `sparq-module-api` — the closed port vocabulary as types, the connection rules as functions, the derived error catalogue (19 legacy kinds + 5), the manifest with every required field an `Option` so absence is reportable, `validate()` returning either a `ValidatedManifest` the executor may use or *every* failure at once, `Copy` param snapshots through the kernel's lock-free ring, and the dyn-compatible `Module` trait whose `AudioCtx` exposes no allocator, clock, filesystem or lock. Three conforming modules as contract tests (`util/gain`, `syn/sine`, `ana/rms`). **368 tests** (was 280 before WO-007), clippy clean in seven of eight cells, **0 allocations across 15 000 `process` calls through `Box<dyn Module>`**. Increment 2 added the **TOML reader** — `toml.rs`, a dependency-free subset parser, and `decode.rs`, text → `ValidatedManifest` reporting syntax and type failures before semantic ones — so a real `sparqmod.toml` can now be read; sections v0 does not decode are still *key-checked*, because accepting `[ui]` while ignoring a typo inside it would be an invisible failure. New stamp **`src 65f/1034459B`**. Defects #60–#63 below. Increment 3 added the **registry and discovery** and task 5's two gates — the API-surface snapshot and the throwaway-module test that walks every engine source file to prove criterion 1 mechanically — so **all six acceptance criteria are now addressed** (3 only as far as Rust allows, and the log says so). **388 tests**, stamp `src 67f/1063788B`, defects #64–#65. Increment 4 closed the last two: **task 4** (`docs/module-author-guide-v0.md`, written from the three reference modules) and the **app-side scan** (`sparq modules [--root DIR] [--strict]` in `crates/sparq-app/src/modules.rs` — the only place in the workspace that reads a directory; the contract crate still holds no I/O). **WO-007 is complete.** The one piece deliberately left open is reading `compat-matrix.toml` at discovery instead of mirroring it in `port.rs` — carried to WO-008, because until then the mirror is the only copy of the matrix that can drift. **P1 device run (2026-09-23, SATURN physical): the first real MSVC build of the increment is green where it counts** — clippy audio+hal clean, release build 58 s, golden bit-identical (`ba577186c988db21`), selftest 8/8, `ui --audit` PASS, 1059× realtime, exe stamp `src 68f/1070593B` — and found six defects, #66–#71 below, fixed in `sync-p1-fixes.zip` (tests + scripts only; stamp unchanged) |
| WO-013 | **in progress — increments 2/2b device-VERIFIED (test005 PASS 2026-09-25); increments 3 (browser + inspector + wire re-patch), 4 (live wire levels + out/main master handover), 5 (rename entry + inspector scroll + per-cv levels + the LOD pass) and 6 (the SCOPE SCREEN — `dsp/scope` draws from the analysis ring) built and sandbox-green, device run pending (test006, now with steps F–K)** | The graph canvas: `sparq-ui::canvas` (model with invertible ops + undo/redo, camera + LOD, computed layout + hit-testing, connect verdicts delegated to `sparq-module-api`'s own `connect_*`, the intent→op interaction table) + the egui painter in `sparq-app/src/ui/canvas_ui.rs` (nodes/wires/ports in the token signal-class language, glow/dim affordances, marquee, long-press menu). Shell routes canvas intents and binds the WO-012 `DoubleTap`/`Context`/`Undo` stubs. Demo patch = the reference modules that actually exist (#58 honoured). **474 tests**, `sparq ui --audit` PASS (16 smokes incl. 7 canvas; Design cells audit 24 touch targets), goldens unchanged, `ui-window` compiles. **Increment 2 shipped the bridge:** `sparq-app/src/bridge.rs` (ungated — the default CI test path exercises canvas graph → registry → executor → WAV), master resolution (SET MASTER + MASTER badge + the documented default rule), the RENDER WAV menu row, the registry-driven demo graph, `scripts/test005.bat` for SATURN. Artefacts `sparq-ui::canvas`, `docs/ui/gestures.md`. Defect #73 (recogniser release position) found + fixed. **Increment 2b (no product code):** test005's first run on
SATURN compiled the new `sparq-app` against the *previous* increment's `sparq-ui` — cargo answered
`Fresh` for an rlib whose source had been replaced by a zip with archive-restored mtimes (#41's
mechanism, on the side of the failure the stamp guard cannot see). Now `SYNC-STAMP.txt` +
`tools/sync_check.py` verify the tree by sha256 *before* cargo runs, and `build.bat` purges the
first-party fingerprints every build; `test005` reports both as step [00b]. Defect #78. **Awaiting device:** 60 fps at 200 nodes, real palm rejection (WM_POINTER, increment 2), full DPI matrix — none claimable over RDP. **Increment 3 (2026-09-25): shipped the browser, the inspector and wire re-patch — 519 tests, audit PASS with 19 smokes, goldens unchanged** (per-node param state in the model with `Op::SetParam`; the bridge renders node state, untouched patches bit-identical; fuzzy ranking deterministic; re-patch verdicts run on the post-removal graph, refusals restore byte-exact; one drag = one undo step for both moves and sliders). **Live wire levels SHIPPED in increment 4 (2026-09-27)** — the executor taps existed (WO-008's `meter` + the cross-thread `read_meters`), so the painter half landed and does NOT fake them: `canvas::levels` (toolkit-independent), `bridge::node_levels` reading REAL peak meters, the painter's token-colour→glow blend, `CanvasState.levels` refreshed after RENDER WAV, audit smoke 20, and the `out/main` master-handover rule (the MASTER badge names the output module, never a guess). 713 tests, audit PASS 20 smokes. Full entry below; device evidence is `test006` |
| WO-008 | **in progress — increments 1–7 built and sandbox-green: graph core + executor + tasks 4–7 (swap, latency, watchdog, determinism) + CONTRACT V1 (multi-port `AudioCtx`, cv/event payloads travel) + the CROSS-THREAD HOT-SWAP PRIMITIVE (d3's pointer swap with epoch retirement, d8's meter/cmd rings, `Module: Send`); remaining: the device-side loaded soak** | `sparq-kernel::graph` (task 1): stable never-reused `NodeId`/`EdgeId`; `EdgeKind {Plain, UnitDelay, BlockDelay}` — §5.4's cycle vocabulary as a type; a topology **version** bumped on committed mutations only (latency edits are data, not topology); the **cached deterministic topological sort** (Kahn, smallest-id frontier — the same graph state always yields the identical order, which is what ADR-007 replay demands; `order_computes()` makes "recomputed only when the version changes" observable, ADR-009 decision 1); plain-edge **cycle refusal carrying the loop's path** in the error, rendered as a sentence that names the delay-edge remedy; delay edges close loops legally and stay out of the ordering; `remove_node` returns the detached edges for the journal/undo layer; per-node `latency_samples` stored for task 5. **Layering decision, recorded:** the kernel graph is *structural* — module-api sits above the kernel in the dependency order, so typed verdicts stay with `connect_*`'s single copy of the matrix and only validated edges are offered to the kernel. 12 tests incl. a 200-node/399-edge order at the acceptance scale; **430 tests total**, clippy/fmt/4 python gates clean, no new unsafe. **Increment 2 (same day):** `sparq-audio::executor` — builds a runnable patch from a kernel graph + contract modules and renders it block-by-block: channel negotiation (mono↔multi, buffer pool pre-allocated and pre-touched at build, budget reported per ADR-009 d4), one `Box<dyn Module>` dispatch per node per block (d7's hybrid), block-delay/unit-delay feedback memories refreshed at block end, `Failed` → silenced + flagged (never unwound), relaxed-atomic meters, deterministic fan-in sums, bit-identical renders across builds (hashed). **Measured: 0 allocations across 1000 blocks and across a 201-node block under the counting allocator**; 13 integration tests + 7 unit; the kernel gained the increment-2 cycle refinement (a `unit_delay` edge is in-block, so it ORDERS like a plain edge and cannot close a loop — loops must contain a `block_delay`; §5.4's own "keeps the executor a simple topological sort" clause, made structural, 2 new tests). **452 tests total**, clippy clean in three cells (workspace + both MSVC cross), goldens unchanged. **Increment 3 (2026-09-25) shipped tasks 4–7 — 549 tests, goldens unchanged:** the boundary-swap `Engine` (clock inheritance, retire-at-boundary, refusals in words) + **the 10 000-mutation stress** (10 001 blocks · 7 203 swaps · 2 797 refusals · **0 audio-path allocations** · hash bit-identical across debug/release); the kernel **per-path latency map** (version+edit-count cached; three hand-computed reference graphs) + the **raw/compensated switch** (per-edge fan-in alignment, no global offset, chains byte-identical); the **watchdog** (N consecutive overruns ⇒ auto-bypass + bounded journal + passthrough-or-silence, the rest of the graph keeps playing); the **determinism harness** (`sparq_audio::determinism` — scripted seeded replay, hash equality; it caught a real HashMap-order nondeterminism in the stress schedule on day one, fixed before shipping). **Increment 4 (2026-09-26) shipped contract v1** — the multi-port `AudioCtx` (audio ≤ 8/class, cv block+audio-rate views at the RECEIVER's declared rate, bounded sorted event ports), cv/event edges execute with every matrix rule enforced at build (data/gpu/atom still refuse, per type, in words), the WO-014 rms→filter acceptance PROVEN BIT-EXACT against a hand-driven reference (`sparq exec --patch mod-demo` is the audible artefact), the compat-matrix drift gate (defects #80–#83 below), and `ana/rms` + `flt/svf` 0.2.0 made honest by it. **614 tests**, every pre-existing golden unchanged, the stress hash `b42068ec7b206789` unchanged. Full entry below. **Increment 5 (2026-09-27) shipped the cross-thread hot-swap primitive** — `sparq-kernel::sync::hotswap` (allowlist entry 6: one atomic staging slot, eight epoch-tagged retirement slots, deferral-not-forcing, drop hygiene, 9 kernel tests + Miri cell widened to `sync::`), the `Module: Send` contract bound (pinned), the cross-thread `SharedEngine`/`AudioEngine` with d8's meter publication and the `EngineCmd` ring, and the two-thread 10 000-mutation acceptance (`tests/cross_thread.rs`: 7 400 swaps · ~43 000 blocks · 0 failed · 0 audio-thread allocations · every retirement reclaimed exactly once). **682 tests**, every pre-existing golden unchanged (stress hash `b42068ec7b206789` in debug AND release), selftest 9/9, defect #85 (exec evidence durations). Full entry below. **Increment 6 (2026-09-27, fourth session) ships `cv_interp = "spline"` — the G4 vocabulary is now PERFORMED in full**: the parabola through the last three block values (equivalently a cubic Hermite with causal second-order tangents — one compiled copy in `CvInterp`, the same sentence in the matrix cell, §17, the schema row and the author guide, pinned by the widened G4 drift gate), exact for quadratic-in-block-index sweeps, bit-identical to `linear` for collinear knots, `linear`'s arrival timing (frame 0 IS the previous knot; the curve reaches the current one at the next boundary), f64 evaluation with one rounding per frame, clamped to the wire's declared range — the host bounds its own momentum overshoot there and nowhere else, so an out-of-range knot under hold/linear stays a visible SOURCE bug. The build refusal retired the day the promise was kept; `CvPlan` grew the two-slot history (`prev2`) and the wire's range, still zero audio-path allocations. `tests/cv_spline.rs` (8 gates) + two port.rs unit pins + contract_v1's refusal gate replaced by a behaviour gate. **752 tests**, every pre-existing golden unchanged (stress `b42068ec7b206789` debug AND release — the hold/linear branches did not move, by the shape of the diff), selftest 9/9, ui-audit PASS, `modules --strict` 17/17, defect #88 (the author guide's stale mixer clause, docs-only, fixed in passing). Full entry below. **Increment 7 (2026-09-29, sixth/seventh session) ships the host-side `required`-unconnected enforcement** — the checklist's named next sandbox candidate, its own increment on purpose because it moves the declared baselines: `Executor::build` refuses a bare required input in words (`ExecError::RequiredUnconnected` names node, module, port and both remedies), the determinism world WIRES its spare (from the chain gain, so retunes reach the master), the canvas demo's fourth node becomes a bare `out/main` (canvas-render.wav bit-identical, 1 920 046 B), and **the stress baseline moved on purpose and is recorded: `7bb06379bd6845e5` · 2 383 swaps · 7 617 refused, debug == release** (was `b42068ec7b206789` · 7 203 · 2 797). 762 tests, every audio golden bit-identical. Full entry below. **Remaining:** the loaded soak (200 modules, 30 min, zero xruns) stays device-track — and now doubles as the paced-real-time half of entry 6's stress sentence; HAL integration of the shared engine waits on WO-006's exclusive acceptance |
| WO-014 | **in progress — increments 1–5 built and sandbox-green: the first-party set is COMPLETE at SEVENTEEN (batch 2's six audio-domain measured at −166.8 dB aliasing; batch 3's membrane/env-ad/mixer are contract v1's first consumers; batch 4's lfo/clk-div are WO-009's; batch 5's ana/tap + dsp/scope + out/main ship on the analysis-payload contract — the tap's wave golden cross-validates == the sine golden, the no-op scope's rig golden == the out/main golden, and out/main is the master with the metering hook); **increment 6 (2026-09-27, third session) ships `util/mixer` 0.2.0 — the cv merge side the compat-matrix's fan-in cell always named** (4→1, block-rate unipolar, `cv_reduce = "mean"` declared per G3, identity default, gains appended LAST so every v0.1.0 snapshot keeps its bits), retiring the executor refusal's "not built yet" half with a working remedy in words — the no-implicit-summing RULE stands; `tests/mixer_cv.rs` 9 gates; **742 tests**, every golden unchanged)** | The three reference modules promoted from contract tests to library code (`sparq-audio::modules`: `syn/sine`, `util/gain`, `ana/rms`), with their manifests as real files under `modules/` **and** compiled in via `include_str!` — one copy, two consumers (disk discovery + built-in registry), and a test that fails if the file and the binary ever drift. `sparq modules` discovers all three from disk; §11 precedence demonstrated live (the disk copies shadow against the built-ins, reported not errored). New **`sparq exec`** command: registry → factories → kernel graph → executor → WAV with no device — prints the ADR-009 d4 budget line, the analysis-tap value, the master meters and the render's golden hash. **Goldens checked in** (demo patch 2.8 s = `53de3b1f3f40e3c9`, sine 1 s = `3f325d4f99ca2a01`); zero allocations measured per module (5 000 `process` calls each) and through the executor path (1 000 blocks); the rms tap value and the master's metered rms agree to the bit (0.16621882 — analysis-as-control-source, cross-validated). **468 tests**, clippy clean (workspace + ui + MSVC audio cross), existing goldens unchanged. **Increment 2 (2026-09-26): the six audio-domain modules** (`syn/noise` seeded+resettable, `syn/polyblep` bandlimited-additive with the aliasing acceptance MEASURED through the registry build at **−166.8 dB** vs the −60 dB bar, `flt/svf` five modes, `util/delay` with the set's first parametric latency declaration, `fx/bitcrush` seeded dither, `util/panner` two laws) + six module goldens + a full-chain golden + per-module zero-allocation gates + **`tools/module_docs.py`** (the generated-docs acceptance, `--check` wired into CI/justfile/gates.bat, proven failable). 559 tests, goldens unchanged, audit PASS 19 smokes, `modules --strict` 9/9. **Increment 3 (2026-09-26) shipped the three modules contract v1 unblocked** — `syn/membrane`
(trigger at sample 37 ⇒ hit at sample 37, measured), `env/ad` (event in → audio-rate cv out, NO
audio ports, loop measured against the primitive's epsilon-arrival semantics), `util/mixer` (4×4
stereo matrix, identity default bit-exact, the explicit merge) — plus `sparq exec --patch
drum-demo` (host triggers at 120 BPM through the event door → membrane → mixer, golden
`f2303f13aa0cf299`, kicks on exactly frames 0/24000/48000/72000) and defect #84 (TOPS gained
`env`/`mod` table-first, with the field table's first drift pin). **627 tests**, all pre-existing
goldens unchanged, `modules --strict` 12/12. Full entry below. **Contract v1 landed 2026-09-26 (WO-008 inc 4) and took the rms→filter modulation demo with it** — the acceptance is proven with arithmetic (the cv-wired filter renders BIT-IDENTICAL to a hand-driven reference set per block to the declared value; golden `1621e1f65b1b64e1`, `sparq exec --patch mod-demo` audible) — and `flt/svf` grew the `cutoff-mod` cv input for it (0.2.0, additive; at defaults bit-identical to inc 2, which its golden proves). **Increment 4 (2026-09-26) shipped the two the clocks unblocked** — `mod/lfo` (four shapes in 0..1, audio-rate unipolar cv, event phase-reset at exact samples; the range decision — unipolar until `util/range` ships — is recorded in the manifest header, not buried) and `mod/clk-div` (divide-from-the-first-input, multiply across the MEASURED interval, seeded probability gate; the first event-in-event-out module), plus the chain golden 16ths→÷4→membrane (`914d9063ce9d8a0f`, kicks on exactly frames 0/24576/49152/73728) and the lfo→svf bit-identical sweep. **665 tests**, `modules --strict` 14/14, all pre-existing goldens unchanged. Full entry below. **Increment 5 (2026-09-27) shipped the last three — the set is COMPLETE at seventeen:** `ana/tap` (audio-rate bipolar wave + block peak/rms; wave golden `3f325d4f99ca2a01` CROSS-VALIDATES == the sine golden), `dsp/scope` (the no-op display; scope-rig golden == the out/main golden `75bc7f2f18cac9d5`, proving zero-cost bit-transparency), `out/main` (unity-bit-exact master with the metering hook), on the `AnalysisUpdate` ring that extends d8's publication to waveforms; plus the LFO beat-rate
decision DECIDED *and* SHIPPED (`mod/lfo` + `util/delay` 0.2.0 derive bpm module-side from the
block-start tick via a shared `TempoFollower`, reaching the delay's once-unreachable
`set_tempo_sync`; `tests/tempo_sync.rs`, golden `db4013f41d1fa678`). **713 tests** (with WO-013
inc 4 + the beat-rate follow-on), `modules --strict` **17/17**, all pre-existing goldens unchanged.
Full entry below. **Remaining:** mixer's cv side (LATER.md); per-module example patches (WO-015's
study is the example patch; `.sparq` needs WO-011); the <15 %-of-a-core benchmark on the stage
machine |
| WO-009 | **increment 1 built and sandbox-green (2026-09-26): the three-clock model's musical half, the transport, and the sample-accurate queue — all four acceptance criteria met in sandbox (the drift criterion's 30-min hardware box stays device-track, declared)** | Kernel `Clock` v2: the piecewise tempo map ADR-006 rule 2 specifies — segments anchored at exact `(sample, tick)` pairs with LINEAR BPM RAMPS (position continuous by anchoring, derivative continuous by inheritance — a mid-ramp change glides from the rate reached, never steps), quadratic integral inside ramps, `sample_at_tick` by EXACT INTEGER BISECTION consistent with the forward rounding, the constant-segment fast path bit-identical to v0 (why the phase-b goldens did not move), surface kept except `Copy` → `Clone` (declared). **`sparq-music`** (the planned crate's first contents): `ClockBroker` (position + the wall side — windowed-ratio drift estimator, one-pole smoothed, re-anchored, backwards readings refused-and-counted) and `Transport` (play/stop with FREEZE semantics, tempo-by-segments, loop region with block-granular fold, tap tempo with a 2 s memory, the tick-scheduled queue, bar/beat trigger emission, `advance_block` measured zero-allocation into a bounded 64-slot collector). Executor: `set_musical_position` — the transport computes, the executor carries; never-set renders the static `tick = 0` every golden knows (stress hash `b42068ec7b206789` unmoved). `sparq exec --patch drum-demo` is now TRANSPORT-driven and hashes to the batch-3 golden `f2303f13aa0cf299` — two independent trigger mechanisms, one bit pattern. **655 tests**, ±0-sample acceptance run against an INDEPENDENT integral implementation in the test. Remaining: external sync (Phase 4), the 30-min drift measurement (device), loop-relative beat phase for unaligned regions, sub-block transport (forbidden territory, declared). Full entry below |
| WO-010, WO-011, WO-015…WO-016 | not started | WO-010/011 follow WO-009 on the plan's own gate order. WO-013 builds directly on the shell: the gesture layer it needs (drag/pan/pinch/context/undo) is implemented and tested — but see #58: the canvas may not offer a converter module that does not yet exist |

**Phase A additions (Windows bring-up, so a first run on untested hardware is diagnosable rather than mysterious):**

* `scripts/setup.bat · verify.bat · build.bat · devices.bat · run.bat · diag.bat · gates.bat · soak.bat` and `WINDOWS.md`. `verify.bat` links a trivial program to prove the MSVC toolchain works — the step that fails on fresh Windows installs and otherwise produces a confusing wall of linker errors.
* The bootstrap now: enumerates devices on every backend with their rate/channel/format ranges; walks an **attempt ladder** (requested format → device default → 48k stereo → 44.1k stereo) and reports each rung's failure verbatim; handles **F32, I16 and U16** callbacks (refusing all but F32 would have failed on most Windows shared-mode devices, whose default is usually I16); prints callback count, frames delivered, estimated underruns, min/max callback gap and a **frame deficit** every 2 s; and can write a WAV in parallel with playback.
* `diag.bat` produces `sparq-diag.log` + `sparq-diag.wav` and a printed interpretation table, which separates "sparq produced silence" from "the device path is broken" in a single run — the distinction that cannot be made from a silent speaker.
* Exit codes are meaningful (no device ⇒ 2) so the scripts can branch.

**Phase A findings from the first real-hardware run (Windows 10 19045, AMD Zen 4, 32 cores, over RDP):**

| Finding | Evidence | Action |
|---|---|---|
| **Phase A passes at the engine level** | Device produced peak 0.125594 (−18.02 dBFS); independent calculation of `amp 0.5 × gain −12 dB` gives 0.125594 — agreement to 3.2e-7, one f32 ULP. 441 970 frames in 10.02 s, 0 underruns, golden hash reproduced on Windows | none — record it |
| **The UMC204HD is invisible to an RDP session** | Only device is `Remote Audio`, locked to 44100 Hz / 2 ch. RDP does not forward USB audio | documented in `WINDOWS.md`; startup warning added when this pattern is detected |
| **My telemetry was wrong** | Log printed "expected 1.45 ms" gap, but the device delivered ~442 frames/callback (10 ms period), not the 64 we asked for. The underrun budget was being compared against a period the device never gave us | `Stats.cb_frames` / `max_cb_frames` added; the budget, the "expected gap" line and the final report now use the device's real period. Found by a real log, not by a test |
| **The attempt ladder was ordered by preference, not by likelihood** | Rung 1 (requested 48 kHz) always failed on a 44.1 kHz-only endpoint, costing a device open every run | Ladder reordered: device default first, requested format second. Failures are still reported verbatim |
| **The RDP path cannot verify real-time behaviour** | Callback gaps 8.95–69.28 ms against a 10 ms period — bursty network delivery | `WINDOWS.md`: DSP is verified by offline render + golden hash; latency/underrun/exclusive/ASIO/multichannel are explicitly *not* claimed until measured at the physical machine |

**Phase B (DSP toolkit) — built ahead of schedule, because it needs no hardware:**

`flt/svf` (ZDF trapezoidal SVF, 5 modes + morph) · `syn/additive` (bandlimited saw/square/pulse, sine, hard sync) · `env/ad` + `env/adsr` (linear and exponential) · `syn/noise` (white/pink/brown/blue/violet, seeded) · `syn/dust` · `fx/bitcrush` (depth + rate reduction, dither, optional anti-alias) · `util/delay` (fractional, damped feedback, tempo-sync) · a test-only FFT · and a fixed-topology **demo patch** (kick/bass/hat/delay, sample-accurate step sequencer off the three-clock model) rendered by `sparq demo`.

**134 tests**, clippy clean at `-D warnings` in both feature configs, two golden references. The demo renders 4 bars in 334 ms (20 853× realtime) and was verified independently of sparq: a Python analysis of the output WAV found **16/16 beats with a clear low-band on-beat peak** (ratio ~10:1) on the correct 138 bpm grid.

**Defects Phase B caught:**

| # | Defect | Caught by |
|---|---|---|
| 11 | `SvfFilter::new()` computed coefficients before the sample rate was known, landing on the 1 Hz clamp — **every filter in the crate was silently a 1 Hz lowpass** that passed nothing | `cutoff_readback_matches_what_was_set`; now pinned by `new_before_prepare_is_not_silently_a_1hz_lowpass` |
| 12 | **PolyBLEP did not work.** Measured −29 dB aliasing against a claimed −60 dB floor, barely better than the naive saw at −28 dB. Three sign/timing variants failed against an analytic reference. Rather than ship a wrong claim, the shape was replaced with truncated additive synthesis, which is bandlimited by construction and now measures **below the noise floor** | `aliasing_floor_is_at_machine_precision` + a reference-vs-measurement comparison in Python |
| 13 | The aliasing *test* was also wrong twice: it measured "energy above 21 kHz", but at 1 kHz/48 kHz the 21st harmonic is legitimately in band; and with a frame-exact frequency, **aliasing folds back onto harmonic bins** and is invisible to any non-harmonic measurement | reasoning from the fold equation `|k·f0 − m·sr|`; the test now uses a non-dividing frequency |
| 14 | `set_freq()` rebuilt the 2048-entry partial table on **every sample** — the patch test suite took 207 s. Guarded: 2.1 s | suite wall-clock |
| 15 | **The kick was exactly silent.** `AdEnv::level()` was read without `tick()`, so the envelope sat frozen in Attack at zero forever. Every mix-level test still passed, because bass and hat covered for it | `every_voice_is_audible_when_soloed` (a per-voice test — the only kind that catches a dead voice) |
| 16 | Bass notes were held for up to a whole bar (gated only when a step had no note *and* `s % 4 == 0`), turning the bass into a drone that masked the kick | `the_kick_lands_on_the_quarter_notes` failing |
| 17 | Triangle wave implemented as "integrate the bandlimited square" measured −8.9 dB for the 3rd harmonic instead of −19.1 dB (integrator gain applied twice). **Removed** rather than shipped wrong | `triangle_is_bandlimited...` |
| 18 | Three test bugs of the same family: a golden test whose frame wasn't a whole number of cycles; a "sine has no DC" test at a frequency that wasn't frame-exact; a gate-length test that forgot to solo the bass and to zero the filter envelope (so it measured two envelopes at once) | the tests themselves |

**Phase B accepted on real hardware (2026-09-20, SATURN, Windows 10 19045, over RDP):** the demo patch rendered offline, was copied to the client machine and **listened to: "works and sounds like techno"**. That is the first time this project has produced music that a listener recognised as the target genre. It was produced with no audio device in the loop — the offline render path is the verification path, exactly as planned for RDP working.

Two usability defects found by the same session and fixed: the `.bat` scripts closed their window before the results could be read (they were written for terminal use), and output was not persisted. All nine scripts now tee to `logs\<name>.log`, print the log path, and pause only when launched from Explorer (`CMD_CLICK_STARTED`), so they remain usable from a terminal and in CI.

**Defects found by running the gates on Windows for the first time (the primary platform, ADR-004):**

| # | Defect | Caught by |
|---|---|---|
| 19 | All four Python tools used `Path.read_text()` / `write_text()` with no explicit encoding or newline. On Windows: cp1252 decoding crashed `token_audit` and `unsafe_audit` on the first `·` in a design document, and LF→CRLF translation made `token_gen --check` report every generated file as stale | the user running `scripts\gates.bat` on SATURN |
| 20 | Worse variant of the same bug: the auditors passed `errors="replace"` **without** `encoding=`, so they would not have crashed — they would have silently decoded UTF-8 as cp1252 and audited corrupted text | review while fixing #19 |
| 21 | The `.bat` scripts closed their window before results could be read and persisted nothing | the same session |

Fixes: every tool now has `read_text()` (`encoding="utf-8"`), `write_text()` (`encoding="utf-8", newline="\n"`) and, for the auditors, `read_text_lossy()` (UTF-8 **and** `errors="replace"`). A new gate, `tools/check_text_io.py`, fails CI if a bare text read/write reappears; it runs first among the Python gates and is proven against injected violations. All nine scripts now tee to `logs\<name>.log`, print the path, and pause only when launched from Explorer (`CMD_CLICK_STARTED`), so they stay usable from a terminal.

**Verified on Windows as a result:** 133 tests pass, and **both golden hashes match bit-exactly on Windows in debug and release** — so the platform `libm`'s `sin` agrees with Linux and the bit-exact regression net is portable. That was the open question from the Phase A log.

**Phase C1 (music systems) — built and verified:**

`patterns.rs` (Step with velocity/probability/microtiming/gate/ratchet, Pattern, **Euclidean `E(k,n)` with rotation**, polymetric `PatternSet` with independent lane wrapping and LCM cycle reporting) · `mutation.rs` (**25 operators** in 4 families — deterministic transforms, stochastic, structural/generative including Markov, cellular automata, logistic map and Euclideanise — plus chain composition and 11 named presets) · `dna.rs` (lineage tree with kept-marks, `export_score()` producing a reproducible generative score, and a depth-indented text rendering for the future `dsp/dna-view`) · `presets.rs` (four styles: techno, breakcore, glitch, driving; each lane seeded from its own branch of the seed tree) · `sparq-kernel::seed` (splitmix64 + the ADR-007 seed tree).

The demo patch no longer contains a single hardcoded rhythm. `sparq demo --pattern breakcore --mutation grid-break --seed 7 --show-dna` renders a different loop per seed and prints the score that reproduces it. **206 tests**, clippy clean, golden reference regenerated with the reason recorded. Verified by analysis of the rendered WAVs: four style/mutation combinations give four distinct hashes, techno shows 16 low-band onsets over 4 bars (four-on-the-floor, correct), breakcore 13, and glitch the densest (rms 0.123 vs 0.079).

**Defects Phase C1 caught:**

| # | Defect | Caught by |
|---|---|---|
| 22 | **Bjorklund was wrong twice.** The first implementation lost trailing rests (`E(2,5)` came out four steps long); the rewrite double-counted carry-over groups and emitted twelve steps for `n=8`. Both produced *plausible-looking* rhythms. Replaced with the direct evenness formula `onset i = floor(i·n/k)`, whose evenness follows from the arithmetic | `euclidean_has_exactly_k_onsets_in_n_steps` and `euclidean_gaps_differ_by_at_most_one`, sweeping every `(k,n)` to 32 |
| 23 | `Op::Markov` transitioned from the **input** step instead of its own previous output, making it an exact copy of the pattern — it looked like it worked because the output was always a plausible rhythm | `markov_respects_the_stay_probability_at_both_extremes` |
| 24 | The `Driving` preset set a velocity on every step, turning its two intended rests into onsets — the exact hiss the rests were added to prevent | `every_style_builds_a_usable_pattern_set` |
| 25 | Three tests were statistically unreasonable: asserting two seeds differ for `Thin(0.3)` (5.7% chance of legitimately agreeing), asserting a deterministic chain varies with seed, and comparing `to_text()` for `Humanise` — which changes only microtiming, invisible in the text form | the tests failing; all three now compare full step data or use seed sets |
| 26 | The `base()` fixture gave every onset velocity 1.0, so `ShuffleVelocities` was the identity and a third of the mutation tests silently exercised nothing | `stochastic_operators_actually_vary_with_the_seed` |
| 27 | `splitmix64_matches_the_reference_vectors` asserted two constants **written from memory**, which were wrong. Nothing in the sandbox could check them, so the test asserted a falsehood with total confidence — worse than no test | the test failing; replaced with a self-consistency + avalanche + no-short-cycle test that needs no external constant |
| 28 | `print_dna` recorded the root's fingerprint at every intermediate node, so the tree showed nothing changing until the last step | `--show-dna` output inspection |

The pattern in that table is worth naming: **five of the seven were cases where the code produced plausible output and only a property test caught it.** A "does it look like a rhythm" eyeball check would have passed all of them.

**Defect found by the user running Phase C1 on Windows:**

| # | Defect | Caught by |
|---|---|---|
| 29 | **All nine `.bat` scripts silently discarded every command-line argument.** They were wrapped in `call :sparq_main` to capture a log, and inside a called subroutine `%*` refers to the subroutine's arguments, not the script's. `demo.bat --pattern breakcore --seed 7` rendered the default techno patch, printed no error, and produced a correct-looking report — the only tell was an empty `[3/3] rendering ` line | the user reporting "same wav no matter what arguments" |
| 30 | Console mojibake: the binary prints UTF-8 (`·`, `—`) to a cp850 console | the same report (`┬À`, `ÔÇö`) |

This is the second defect in the same class as #19–21: **tooling written and exercised on Linux, never on the primary platform.** The pattern is now explicit enough to name as a rule — anything that runs on Windows gets run on Windows before it is called done. Fixed by capturing `%*` before any `call`, forwarding it, and adding `chcp 65001` to each wrapper.

Note what did *not* catch it: the Rust argument parser has tests, the render path is golden-hashed, and every gate was green. The failure was entirely in the shell wrapper, which no gate covered. `scripts\demo.bat` now prints the arguments it is about to use, so the next failure of this kind is visible in the output rather than inferred from a WAV file.

**Known limitations, stated rather than hidden:** additive synthesis costs one `sin()` per partial per sample, so a 55 Hz voice wants ~436 partials — the patch caps the bass at 48 (reaching 2.6 kHz) and this is the right Phase B trade but the wrong Phase 3 one. `LATER.md` carries the item: a real PolyBLEP/BLIT/wavetable oscillator, from a reference derivation, with the same aliasing test as its acceptance gate.

**Extra work done beyond the work orders** (because the gates needed it):

* `sparq soak` — the WO-016 long-run gate, built now because a 2 h soak needs bounded memory (chunked render) and because it fails *fast* on the first allocation or underrun rather than reporting them at the end.
* `sparq-app --example probe_alloc` — measures the audio path outside the test harness, where harness lazy-init pollutes a cold measurement.
* `clippy.toml` denials for `Instant::now`, `thread::sleep`, file I/O and `Mutex` — turns plan §5.2's rules into build failures.

**Defects the gates caught during this build** (each one is an argument for having written the gate first):

| # | Defect | Caught by |
|---|---|---|
| 1 | SPSC ring used wrapped slot indices, so `tail - head` was ambiguous between "empty" and "full" — it reported full when empty and silently dropped messages (132 849 refusals in a test where zero were possible) | `spsc_stress_no_tearing` |
| 2 | `drops()` semantics were ambiguous: it counts *refusals*, so a retrying producer inflates it. The test asserted "delivered ⇒ zero drops" and was wrong, not the ring | the same test, on the second run |
| 3 | A clock-reading overload hook had been placed **inside** the DSP module — breaking the exact rule the harness exists to check | `clippy::disallowed_methods` |
| 4 | `sparq-app` printed `alloc 0` without ever installing the counting allocator: an unmeasured number presented as a measurement | `audit_is_live()` probe + the selftest's positive control |
| 5 | The allocation audit reported "clean" in any binary that hadn't installed the counter — a gate that silently passes | `NullDevice::render` now warns, and the kernel test asserts the *warning* path |
| 6 | A golden test computed its expectation from `48000/256` blocks, which is not a whole number of cycles | `phase_stays_wrapped_over_long_renders` |
| 7 | WAV tests hard-coded byte offsets that are wrong for float (18-byte `fmt` chunk vs 16) | the tests themselves; now they walk the chunk list |
| 8 | The DNA-tree mockup invented its own greyscale ramp instead of sampling the declared colour map | `token_audit` R1 |
| 9 | `token_audit`'s first version accepted all 1 785 interpolated LUT colours, making R1 meaningless | self-review; split into R1 + R1b |
| 10 | My own "is this a data mark or chrome?" heuristic was wrong twice (a spectrogram bin is a `<rect>`; a scatter node is a `<circle>`) | the audit failing on correct mockups; resolved by narrowing the auto-gate and registering LUT regions for human review |


**WO-006 increment 1 (real HAL) — built in-sandbox, hardware acceptance pending:**

`sparq-kernel::hal` — the trait from ADR-009 (`enumerate → capabilities → open → start/stop → latency_report → error_report`), extended with two things the first week of use demanded: `actual_config()` (negotiated ≠ requested must be *visible*, the Phase A telemetry lesson turned into an API) and `pump()`/`inject_fault()` (manual stepping and simulated faults, so determinism and failure paths are testable without hardware). `diag.rs` records xruns, budget overruns and late wakes **as separate counters** (different diseases: our DSP vs the system vs the driver), a 32-bucket log2 block-time histogram (p50/p99/max without storing 10.8 M samples), exact wake jitter, and device-clock drift vs wall in ppm — all relaxed atomics, snapshot from any thread. `null.rs` implements the backend twice over: paced (hybrid sleep+spin pump thread simulating the device interrupt) and manual (test-driven, fully deterministic), with `Fault::{Unplug, XrunNext, PumpStallNext}` making the WO-006 failure acceptance criteria ("clean error state and a recoverable stop, not a crash") executable in CI. `conformance.rs` is task 1's "review the trait before implementing" made runnable: every backend — null today, WASAPI on Windows, ASIO in increment 2 — runs the same promises (`sparq devices --conformance`).

`hal/wasapi.rs` — WASAPI exclusive (event-driven) and shared, written on a Linux sandbox and verified by `cargo clippy --target x86_64-pc-windows-msvc -D warnings` (new CI job `windows-crosscheck`; the gate that makes blind development honest). Persistent per-thread MTA COM on the control side; interfaces *move* to the pump thread and return on join — the `Mutex<Wo005Graph>` bootstrap violation does not exist on this path (the graph is owned by the callback; the terminal pushes through an `Arc` ring handle, which is what the clippy.toml note demanded for WO-006). Device period and sparq block are decoupled by a pre-allocated FIFO; shared-mode i16 mixes are converted in the pump; exclusive alignment uses the documented `BUFFER_SIZE_NOT_ALIGNED` two-step with a fresh client. MMCSS "Pro Audio" + TIME_CRITICAL + ideal processor + working-set raise, **each reported separately** (`rt setup` line) because a silent success is indistinguishable from a silent failure in a log. `AUDCLNT_E_DEVICE_INVALIDATED` → `Removed` state, clean stop, honest restart refusal.

App surface: `sparq devices [--caps] [--conformance]` · `sparq play --backend null|wasapi-exclusive|wasapi-shared` · `sparq soak --backend … [--heavy N]` (the deliberately heavy dummy callback of task 6) · selftest gate 8 runs the null conformance + the reopen-without-leak cycle in-process. `--backend wasapi` (bare) still routes to the cpal bootstrap: the proven path is never rerouted by an unproven one; the bootstrap dies at WO-006 *acceptance*, not at WO-006 *compilation*. **239 tests** (from 206), fmt/clippy clean on both targets, all four Python gates clean, selftest 8/8, both golden hashes unchanged (`0f5c3e86c7f117a9`, `dd975a24f03b19c1`).

**Defects WO-006 increment 1 caught:**

| # | Defect | Caught by |
|---|---|---|
| 30 | The high-level `windows` crate (0.62) **cannot be compiled in a 1 GB memory budget** — rustc was OOM-killed even with debuginfo off. The whole WASAPI plan nearly rested on a dependency the sandbox and CI cannot build | the first cross-check attempt (SIGKILL); switched to `windows-sys` + hand-written vtables against the SDK slot order, each slot commented with its header method |
| 31 | windows-sys 0.61's binding of `AvSetMmMaxThreadCharacteristicsW` is `(PCWSTR, PCWSTR, *mut u32) -> HANDLE` — the documented Win32 signature is `(HANDLE, LPCWSTR) -> BOOL`. Calling it as bound would pass garbage to the kernel | reading the generated signature instead of trusting it; call removed, `TIME_CRITICAL` provides the boost (the miniaudio/cpal pattern), tracked in windows-notes.md §1 |
| 32 | `WAVEFORMATEX`/`WAVEFORMATEXTENSIBLE` are `packed(1)` in windows-sys: **every `&field` is UB** (E0793), including inside `assert_eq!` and `format!`. The first draft did this in ~10 places | the MSVC-target compiler; every read now copies to a plain local first, and the module says so where it happens |
| 33 | The conformance signal was a ±0.5 ramp (−6 dB) — `devices --conformance` on the stage machine would have been a *loud* surprise, while its own printout promised "−60 dB" | review of the script text against the code; ramp scaled to ±0.001 |
| 34 | The paced null pump used plain `sleep` pacing: measured **−1694 ppm drift and 4.6 ms max jitter** on the sandbox — the *simulated* device glitched worse than real hardware would, poisoning every rehearsal run | the drift/jitter diagnostics added in the same increment (their first catch); hybrid sleep+spin pacing → +0.2 ppm drift |
| 35 | The `unsafe` allowlist named entries 1–2 as *directories* (`hal/wasapi/`), but the auditor's row regex only recognises `*.rs` paths — the gate would have silently never covered the exact files it was written for | checking the auditor before writing the code; rows now name the shipped files |

**Not done, declared:** ASIO backend (increment 2 — the WO's own task order is exclusive → shared → ASIO, and ASIO deserves a proven WASAPI reference to diff against); full-duplex WASAPI (the trait and the null backend already exercise the code path, per the acceptance criterion's own escape clause); the round-trip *measurement* utility (`measured_roundtrip_frames` stays `None` rather than dressing an estimate as a measurement); every hardware acceptance item — 2 h zero-xrun soak, latency table, unplug test, ≥8-out multichannel on a real interface — which is what `scripts\hal.bat` + `scripts\soak.bat 120 wasapi-exclusive` exist to produce, on SATURN, at the physical machine.

**WO-006 increment 1.1 — the first hardware run found four defects (2026-09-21, SATURN over RDP):**

`scripts\hal.bat` + `scripts\gates.bat` ran on SATURN. Enumeration worked first try (6 render endpoints, stable ids, default marked, capabilities probed, event-driven); the diagnostics, conformance report and honest refusals all printed as designed. And the run found exactly what a first hardware run is for:

| # | Defect | Caught by |
|---|---|---|
| 36 | `rt/thread.rs` gated its `windows_sys` uses on `cfg(windows)` alone — but the dependency only exists under `hal-wasapi`, so **default-features builds failed to compile on the primary platform**. The cross-check matrix had Linux-default and Windows-features cells but not Windows-default | the user's `gates.bat` ("clippy default"); fixed by gating on `all(windows, feature = "hal-wasapi")`, and the missing matrix cell is now a permanent CI step and `just check-windows` line |
| 37 | `unsafe_audit.py` **crashed with `UnicodeEncodeError` on a cp1252 stream** while printing its own allowlist summary (`→` in an entry note). Defect #19's fix had covered *file* encodings but never *stdout*; latent since WO-000, exposed by the allowlist row fix (#35) which made the auditor print those notes for the first time | the user's `gates.bat`; all five tools now reconfigure stdout/stderr to UTF-8 (`errors="replace"` — reports degrade visibly, never die), and `check_text_io.py` gained rule 2 so the fix cannot silently rot |
| 38 | **Shared-mode open trusted `IsFormatSupported` as the arbiter.** The RDP endpoint refused *every* probe — apparently including its own 44.1 kHz/4ch f32 mix format — so the ladder hit bottom and `open` failed, while `Initialize` with that exact format would very likely have succeeded. Worse, the probes' HRESULTs were swallowed (`bool`), so the log couldn't say *why* | the user's `hal.bat` steps 4–7; the ladder now records every probe HRESULT, offers the mix format **verbatim** (extensible bytes included — a rebuilt bare copy would make the driver read past the struct), and ends in an `Initialize`-with-mix last resort that logs the probe table either way. Caps print a named quirk note when every probe is refused instead of presenting fallback values as measurements |
| 39 | **Conformance, `play` and `soak` hardcoded 48 kHz/stereo** — capability-blind requests that a 44.1 kHz/4ch endpoint could only refuse. The suite that exists to verify backends was measuring its own assumptions | the same run (`[FAIL] open` on both WASAPI backends while caps *said* 44100/4); all three now derive their config from probed caps (with `adjusted …` lines), and `play`/`soak` additionally do a **two-phase open**: a silent probe open reads `actual_config`, the graph is built for *that*, then the real stream opens — a mid-stream format surprise is now impossible by construction |
| 40 | Endpoint **friendly names all failed to read** (`<endpoint N>` shown for all 6): the property-store path returns `None` for an unknown reason — property key, PROPVARIANT layout, or RDP behaviour | the same run; instrumented rather than guessed: the next run prints `GetValue HRESULT …, vt …` once (`note_name_failure`), and the quirk log in `docs/hal/windows-notes.md` carries the row until it is explained |

The conformance suite also learned the SKIP/PASS/FAIL discipline the run demanded: an exclusive-less endpoint now *verifies the honesty of the refusal* (must be `Format`, must not silently reroute) and skips the lifecycle checks **with a printed reason** — while a missing counting allocator remains a FAIL, because that is infrastructure, not the device. Paced reopen-leak cycles run on device backends too (short bursts; the thread-local-audit caveat is documented in the fn docs, with the pump-side per-block counter as the second net).

**Increment 1.1b — the sync mechanism itself was the next defect (2026-09-21, same session):**

The re-run of `hal.bat` after syncing increment 1.1 produced *byte-identical increment-1 output*: same binary size, same `src 25f/251464B` stamp, same old error strings — while `hal.bat` itself (a script, not compiled) was visibly the new version. Diagnosis: **zip extraction restores the archive's timestamps**, so freshly-synced sources looked *older* than the cached build artifacts, and cargo's mtime-based freshness check skipped the rebuild ("Finished in 0.03s"). The stamp system correctly *showed* the staleness (`src 25f/…` in the log — exactly what it was built for), but nothing acted on it.

| # | Defect | Caught by |
|---|---|---|
| 41 | **Zip-synced trees defeat cargo's freshness check**: extraction preserves archive mtimes; synced sources can appear older than `target\`, and the previous binary keeps being served — silently, with green-looking "Finished in 0.03s". Compounding it, the build-stamp fingerprint counted only *top-level* `src/*.rs` files, so changes in `hal/`, `rt/`, `play/` subdirectories could not even in principle alter the stamp | the user's second `hal.bat` run: new script text printing beside an old binary's byte-identical stamp and error strings |

**Increment 1.1c — the fix for #41 shipped a new defect the same day (2026-09-21):**

| # | Defect | Caught by |
|---|---|---|
| 42 | **The new `build.bat` exited silently, without a log.** The stamp guard's PowerShell line contained a UTF-8 middle dot (`·`) — the only non-ASCII bytes in any of the ten scripts, violating the pure-ASCII invariant the scripts have held since the cp850 mojibake fixes. With `chcp 65001` active, cmd reads batch files through the console code page; a multibyte character makes the parser lose byte-sync and the script dies *mid-`call`*, before the wrapper's `type` ever runs: window closes, console empty, `logs\build.log` partial-or-missing. The invariant existed; nothing enforced it | the user's one-line report ("the build.bat exits without a log") + a byte-level scan that found exactly 2 non-ASCII bytes on line 115 of build.bat and zero in the other nine scripts |

Fix: the stamp guard is now **pure batch** — no PowerShell at all, which also removes execution-policy, PS-availability and quoting as failure modes on stage machines; the fingerprint (`COUNTf/BYTESB`, recursive over the four `src` roots) is computed with `for /r` + `set /a` and compared against the binary's own `src …` token by substring substitution. `build.bat` is written with an ASCII-enforcing encoder and CRLF endings. And the invariant is now a **gate**: `check_text_io.py` rule 3 byte-scans every `scripts/*.bat` for non-ASCII and fails with the file, byte count and line — proven failable by injecting a violator (`zz_scratch_test.bat`) and watching the gate name it. The `chcp 65001` in the wrappers stays: it is for the *binary's* UTF-8 output, which is exactly where non-ASCII belongs in this pipeline.

Fixes, in the order they defend: **(a)** the fingerprint in `crates/sparq-app/build.rs` is now *recursive* (47 files, not 25 — every `*.rs` under the four `src` roots); **(b)** `scripts\build.bat` gained a **stamp guard**: after building, it recomputes the same fingerprint from the current tree (PowerShell, timestamp-free) and compares it to the binary's own `src Nf/NB` — on mismatch it does `cargo clean -p` of the four first-party crates, rebuilds, re-verifies, and *fails loudly* if still stale (uncertainty resolves to rebuild, never to trust); **(c)** `run.bat`/`soak.bat`/`devices.bat`/`diag.bat` now **always** call `build.bat` instead of building only when the exe is missing (the demo.bat Phase-C1 lesson, applied to the four scripts that never got it); **(d)** `gates.bat` touches every first-party source once per run, because the same mtime trap applies to cached *clippy and test* artifacts — a gates run that re-serves the previous sync's results is worse than no gates. The fingerprint definition now lives in two places (build.rs and build.bat's guard) and both say so loudly in comments; if one changes, both change.

**Increment 1.1d — the WASAPI pump ran on real hardware for the first time (2026-09-21, SATURN over RDP), and the diagnostics earned their keep:**

The manual run (`devices --caps/--conformance`, `play --backend wasapi-shared --seconds 10`, `soak --backend wasapi-shared --minutes 2`) produced the first real numbers for ADR-009's HAL claims: **`rt setup mmcss priority workset` — MMCSS Pro Audio, TIME_CRITICAL and the working-set raise all applied on a real Windows 10 machine** (ideal-proc not requested); **0 allocations across 6945 callback invocations**; callback p50 2.0 µs · p99 8.2 µs · max 16.6 µs against a 1.45 ms budget; the two-phase open negotiated 44.1 kHz/4 ch honestly (`NEGOTIATED … asked 48000 Hz · 2 ch`) after every 48 kHz probe was refused; the shared ladder's mix-verbatim rung opened the endpoint that refused every plain-format probe; and the exclusive backend's refusal passed conformance as *verified honesty* ("never silently rerouted"). The run also found four defects — three in the HAL, one in the gate that watches it:

| # | Defect | Caught by |
|---|---|---|
| 43 | **Guaranteed startup xrun**: the pump primed the FIFO to one period + one block, then the preroll wrote a full period to the device — leaving exactly one block when the first event demanded a period. Every SATURN run showed exactly `xruns 1 (late wakes 0, overruns 0)`, and the soak's fail-fast died on it inside the first second of a 2-minute run | the xrun-split diagnostics (late/overrun/starvation counted separately, so "1 xrun with 0 late wakes and 0 overruns" could only be startup starvation); prime depth is now 2 periods + a block |
| 44 | **The reopen-leak gate reported 176 phantom allocations** (22 per cycle, perfectly deterministic — the signature of a structural error, not a leak): thread-local audit arming counts an allocation on the control thread but not its free on the pump thread, so every spawn closure that crossed threads looked "outstanding". The gate was failing a backend that leaks nothing | the same run; `alloc.rs` gained `start/stop_counting_all_threads()` (process-wide arming; the quiescent-process requirement is documented) and the conformance gate uses it. The caveat docs were rewritten to name the asymmetry in the correct direction |
| 45 | **Drift telemetry believed a lying clock**: `+1 200 367 ppm` (+120 %) on the RDP endpoint — its `IAudioClock` is fiction, provable from our own numbers (6945 blocks × 64 frames in 10 s ≈ 44.4 k fps ≈ the correct rate). Third appearance of the Phase A lesson "the telemetry was wrong" | the same run; readings beyond ±1 % (`DRIFT_SUSPECT_PPM`) now carry a `SUSPECT` label in every readout, with the reason and what to trust instead. The number stays visible — hiding it would be the older sin |
| 46 | **Probes trusted one format shape**: the endpoint refused every plain `WAVEFORMATEX` f32 probe with `AUDCLNT_E_UNSUPPORTED_FORMAT` — and accepted its own `WAVEFORMATEXTENSIBLE` mix verbatim. Capability reports built on plain-format probes alone read a driver *preference* as a device *limitation* | the same run's HRESULT evidence; caps and the shared ladder now probe BOTH shapes at every preference level (rungs renumbered 1–7, comment-documented) |
| 47 | **The stamp guard could not see the tree it was guarding.** Its first pure-batch fingerprint walk (`for /r` nested in `if exist "path\"`) counted **0 files / 0 bytes** on SATURN while the same tree had just compiled 47 files in 14.75 s — so the guard declared a *correct* binary stale, forced a "rebuild" via `cargo clean -p` (which removed **0 files**, because `-p` targets the default-feature artifacts, not the `--features bootstrap-audio,hal-wasapi` build being guarded), and then reported `STILL STALE … Do NOT trust this binary` about a binary whose own stamp was exactly right. A verification gate that cries stale at a good build trains the operator to ignore it — the failure mode #41's guard existed to prevent | the user's build.bat log: `sources 0f/0B - binary 47f/685147B`, with the compile line above it proving the sources exist. Fixed three ways: the walk is now `for /f` over `dir /s /b /a-d` (verified to compute `47f/685147B` against this tree); **zero files counted is now a GUARD failure that trusts the build and says so loudly**, never a staleness verdict; and the forced rebuild touches sources (timestamps are the trap, so timestamps are the cure) instead of `cargo clean -p`. The mismatch message now asks the reader to say *which side* was implausible |
| 49 | **Both staleness remedies were no-ops, and a gates run "passed" over a stale tree.** The 1.1f build.bat correctly *detected* staleness (`sources 47f/686889B - binary 47f/685147B` — the guard's own numbers were right) but its cure, touching sources with `type nul >>`, changed nothing: both builds finished in 0.03 s and the binary kept the old stamp. A touch sets mtime to NOW, and cargo's fingerprint files already recorded a comparable NOW from the build minutes earlier, so cargo still concluded "fresh" — the same reason `cargo clean -p` removed 0 files (clean consults the fingerprints it is supposed to override). Worse: gates.bat's touch-based remedy has the same hole, so "all gates passed" could be served from cached artifacts of the *previous* sync — a gate table that can quietly measure the wrong tree is the exact failure the gates exist to prevent | the user's build.bat log (two 0.03 s builds flanking an unchanged stamp). Fixed where it cannot be argued with: the forced path now **deletes** `target\*\.fingerprint` (cargo's evidence of what it built) plus the exe — dependencies stay cached, so it costs seconds — and gates.bat clears fingerprints at the start of every run instead of touching sources. Deleting only the exe was explicitly rejected: cargo would relink from the cached build-script output and re-embed the *stale* stamp, looping the guard |
| 48 | **The leak gate blamed the backend for its own arithmetic.** After #44's cross-thread fix the WASAPI reopen gate still reported 152 phantom allocations (19/cycle, deterministic). The cause was not a thread boundary at all: `realloc` bumped `ALLOC_COUNT` without touching `DEALLOC_COUNT`, so every `String`/`Vec` growth — the `format!` calls building diagnostics text, six endpoint names per discovery, the refused-rung table — left the balance permanently one higher. `outstanding_allocations()` had *documented* this behaviour as a caveat instead of questioning it, and the audio-path gates never caught it because they measure deltas inside one thread, never an outstanding total | the user's hal.bat run (152 where 0 was expected); fixed by counting a realloc as what it is — one live block replaced by another, so **neither** counter moves — and proven by `outstanding_balance_survives_realloc_growth` in `rt_discipline.rs`, which grows a Vec and a String hard, drops both, and asserts the balance returns to its starting value |

Also resolved: **#40's evidence arrived** — friendly-name reads fail with `E_ACCESSDENIED (0x80070005)` from the RDP endpoint's property store (ids, activation and formats all work; only names are denied). Device-specific and cosmetic; quirk-logged with the HRESULT, STA-apartment retry parked in increment 2. A new quirk joined the log beside it: the endpoint's event cadence is ~10 ms while `GetBufferSize` reports 970 frames (22 ms) — the wake-jitter stats show the *delivered* cadence, the Phase A "use the real period" lesson now measured from both sides.

**Increment 1.1g — the remedies that were no-ops (2026-09-21):** the 1.1f sync ran and the
stamp guard performed exactly as designed on the detection side — `sources 47f/686889B` matched the
tree, `binary 47f/685147B` named the stale exe — and then cured nothing: two consecutive 0.03 s
"builds" and an unchanged stamp (defect #49, table below). Touching sources cannot beat cargo's
fingerprint arithmetic, and neither can `cargo clean -p`; the forced path in `build.bat` and the
freshness step in `gates.bat` now delete `target\*\.fingerprint` — cargo's own record of what it
built — which cannot be stale by construction. Dependencies stay cached, so the cost is seconds.
The Rust side is unchanged from 1.1f (243 tests, four clippy cells, selftest 8/8, goldens and
stamp `47f/686889B` verified); 1.1g is a scripts-and-docs increment.

**Hardware confirmation (2026-09-21, SATURN over RDP, 1.1g binary):** the reopen-leak gate ran
green on real hardware — `[PASS] reopen cycles do not leak — 8 open/start/run/stop/drop cycles,
0 outstanding allocations` on the WASAPI shared backend. That closes defect #48 on the machine
that found it, and with it the whole instrument chain from this increment series: the stamp guard
verifies the binary against the tree (#41/#47/#49), the diagnostics split xrun causes and label
lying clocks (#43/#45), the probes report their HRESULTs in both format shapes (#38/#46), the
conformance suite scores honest refusals instead of demanding capabilities the device never
claimed (#39), the leak gate counts blocks correctly across threads (#44/#48), and the Python
gates survive the primary platform's console (#37). **Every measurement WO-006's acceptance will
rest on is now one that has been wrong once and fixed in public.** What remains for the work
order is not instrumentation but evidence at the physical machine: the 2 h zero-xrun soak at
96 kHz/64 exclusive, the unplug test, the latency table, and the multichannel verdict on the
UMC204HD.

**Increment 1.1f — the leak gate's own arithmetic (2026-09-21):** the re-run with 1.1e came back
almost clean and is worth reading as a scorecard: **`all gates passed` on Windows for the first
time since WO-006 started** (rustfmt, clippy default *and* clippy audio+hal, golden reference,
release build, all four Python gates including the one that used to crash on `→`);
`stamp guard: sources 47f/685147B - binary 47f/685147B`; **`play --backend wasapi-shared`:
`PASS (0 xruns, 0 allocations, clean stop)`** — 6943 blocks over 10 s with the startup starvation
gone (#43), callback p50 1.0 µs / p99 4.1 µs / max 11.8 µs against a 1.45 ms budget; drift labelled
`+1200473 ppm SUSPECT` with the reason printed (#45); `adjusted rate 48000 -> 44100` and
`adjusted channels 2 -> 4` showing caps-derived requests instead of assumptions (#39); the shared
ladder landing on **rung 2**, i.e. the extensible probe at the requested channel count — the
endpoint that had refused every plain-format probe now negotiates two rungs earlier than the
verbatim-mix fallback (#46); exclusive mode refusing over RDP with the HRESULT and the OS rule
named in the message, which conformance scores as a PASS for honesty; and negotiation logged once
per stream instead of twice. One failure remained, and it was the instrument's, not the subject's
— defect #48, in the table above. With it fixed, the whole pre-flight is green in-sandbox
(243 tests, three clippy cells, selftest 8/8, goldens unchanged, stamp `47f/686889B`) and awaits
one confirming run.

**Increment 1.1e — the guard that cried stale (2026-09-21):** `scripts/build.bat`'s fingerprint
walk counted `0f/0B` on SATURN while the same tree compiled 47 files in 14.75 s, so it declared a
*correct* binary (`47f/685147B` — exactly right) untrustworthy, then "forced a rebuild" with
`cargo clean -p` that removed 0 files, because `-p` targets default-feature artifacts and the build
being guarded is `--features bootstrap-audio,hal-wasapi`. Recorded as defect #47. Three changes:
the walk is `for /f` over `dir /s /b /a-d` (verified against this tree: computes `47f/685147B`,
matching the binary); **a zero count is now a guard failure that trusts the build and says so**,
never a staleness verdict — a gate that cannot distinguish "the exe is old" from "I am broken"
trains the operator to ignore it, which is worse than no gate; and the forced rebuild touches
sources (`type nul >>`) instead of `cargo clean -p`, because the trap is timestamps and so is the
cure. The mismatch message now asks which side was implausible.

**Increments 1.1–1.1d verified in-sandbox:** fmt/clippy clean in all four matrix cells (native, native+features, MSVC+features, MSVC-default); 242 tests; four Python gates clean (cp1252 simulation of #37, ASCII-script rule of #42); selftest 8/8 `--golden`; both golden hashes unchanged; recursive stamp verified binary-vs-tree. **Awaiting:** the re-run on SATURN with 1.1d — expected: `play --backend wasapi-shared` PASS with 0 xruns (the #43 prime fix), the reopen-leak gate green (#44), `drift … SUSPECT` labelled (#45), caps reporting real rate/channel measurements via extensible probes (#46), and the negotiation log printed once per stream instead of twice. Exclusive stays honestly refused over RDP; the acceptance soak belongs to the physical machine.

**WO-012 increment 1 — the shell, gestures-first (2026-09-21):** built in the order the Phase 6
swap demands: the *logic* first, in the zero-dependency crate, and the toolkit last, behind
features. `sparq-ui` gained `geom`/`pointer`/`gesture`/`shell`/`audit` — the whole §14.3 gesture
table as a deterministic recogniser (tap, double-tap, long-press, drag, ×10 second-finger fine,
pan, pinch, rotate, 3-finger undo, 3-finger panic swipe, 5-finger recovery hold, flick, edge
swipe, palm rejection, sibling lockout, suppression log), the §14.2 layout as one pure function
(rail/canvas/inspector/dock, token clamps, 8 px snap on user resizes, the 60 % canvas floor with
reported forced collapses, breakpoint refusal) and the 44 px audit with touch classes S/M/L/XL,
the Design-mode dense exception badged and the Perform-mode "nothing below L" rule enforced as a
violation of *presence*, not size. `sparq-app` gained the egui half behind `ui` (hermetic:
headless driver + audit gate) and `ui-window` (winit + egui-winit + egui-wgpu, dx12 backend on
Windows, gles elsewhere — per-target so the 1 GB sandbox can still compile-check the window
host). egui is used **as a painter only**: no egui buttons exist; hit-testing runs through sparq's
recogniser against sparq's rects, which is the WO-012 risk mitigation made structural. The style
adapter is generated from tokens end-to-end, including the high-contrast theme: `token_gen.py`
now derives `[theme.contrast-high]` (+0.08 oklab L on every accent, overrides for ground/hairline/
text) into `COLOR_HC_*` constants, so the theme switch contains no colour arithmetic.
**Measured in sandbox:** 280 tests (was 243; +37 UI-core); clippy clean in six cells (default,
bootstrap, ui, ui-window/gles native, MSVC+hal-wasapi, MSVC+ui-window); `sparq ui --audit` PASS —
40 matrix cells + DPI-invariance + 9 gesture smokes, 0 failures; headless frame logic med 94 µs,
p99 113 µs per frame (layout+draw+tessellate, no GPU); selftest 8/8, goldens unchanged
(`ba577186c988db21`, `dd975a24f03b19c1`); token gates clean with `crates/sparq-app/src/ui` added
to the R6 colour-literal scan. **Not claimed (device-only):** real DPI matrix on real monitors,
real-finger gestures, palm rejection on a real digitiser (winit reports no contact area —
increment 2 goes to WM_POINTER for it), 60 fps with a rasteriser, mixed-DPI multi-monitor.
**Environment notes (not defects, recorded so the choices are legible):** wgpu's default backend
set does not fit the sandbox — `ash` (Vulkan) and the release-LTO compile of `read_fonts` both
OOM at 1 GB — hence the per-target backend choice and relaxed sandbox codegen settings for
release verification (SATURN builds with the repo profile).

**Defects WO-012 increment 1 caught:**

| # | Defect | Caught by |
|---|---|---|
| 50 | **The shell violated its own Perform-mode rule on day one**: `draw_top_bar` registered five class-S buttons in every mode, but "Design chrome is not merely hidden — it is not hit-testable" (input-model §4) means Perform must contain nothing below class L. The audit scored 5 violations in every Perform cell of the matrix — the gate's first run condemned its author's widget code, which is the only credible evidence a new gate works | `sparq ui --audit`, first ever run; fixed by drawing the Perform top bar as a label (mode return lives in the XL canvas pad) |
| 51 | **Two clocks in one driver**: the smoke suite stamped pointer events with its own ms counter while `step()` fed `advance()` from egui's frame time — time-based recognisers (long press, 5-finger hold) measured against a different epoch than the events and silently never fired. Event-relative gestures (tap/drag/pinch) all passed, so the suite looked ⅞ healthy while its clock contract was broken | the `long-press fires context` smoke, failing while the recogniser's own unit tests passed; fixed by making `step()` take `now_ms` from the driver — one driver, one clock, per the input-model §2 adapter contract |
| 52 | **The headless driver dropped `FullOutput.textures_delta`**, and epaint refuses to let an unapplied delta die silently: first audit run panicked with `Dropped TexturesDelta with 1 unapplied deltas` (the font atlas arrives as one on frame 1). A renderer-less host still has to *handle* texture lifecycle — clear() is the honest handling when nothing can upload | the first `sparq ui --audit` run, before it printed a single row |
| 53 | **Motion-history ring indexed by `head + len − 1` instead of `head − 1`**: while the ring was not yet full, "newest" pointed at an untouched zero slot, so a 10 px drag reported a 130 px delta and flick velocity was computed against the origin. Latent in every DragUpdate and Flick until the ring filled (8 samples) | the `second_finger_during_drag_engages_ten_x_fine` test — it asserted delta 10.0 and got 130, the exact arithmetic signature of reading slot 0 |

**WO-007 task 1 — the contract as data, ten decisions, and five defects found in the existing documents (2026-09-22):**
task 1 is *"write the manifest schema and the compatibility matrix as data/tables first, review, then
code"*, so this increment is **documents and tables only — no Rust changed, the audio path is
untouched and the goldens were not re-rendered**. Two new data files (`docs/api/compat-matrix.toml`,
`docs/api/manifest-fields.toml`), the review packet (`WO007-TASK1-REVIEW.md`), and a measurement harness at
`tools/dispatch-bench/` — a standalone cargo project (its own empty `[workspace]`, and listed in the
root `exclude`) so it never joins `cargo test --workspace` or the clippy matrix. It reads
`Instant::now`, which `clippy.toml` bans workspace-wide, so it carries the same targeted
`#![allow(clippy::disallowed_methods)]` and the same justification as `device/null.rs`: the harness
measures the code, it is not the code. It ships in the sync zip precisely because ADR-009 asks for
the measurement to be repeated on the stage device.

**The ten decisions** (full reasoning and consequences in the packet): G1 object counts must match or
convert, never renumber · **G2 `cv` range mismatch refused, `util/range` offered** · **G3 the
receiving module declares `cv_reduce`** · G4 the host interpolates per the receiver's `cv_interp` ·
G5 `event` fan-in free with a stable tie-break · **G6 a version-mismatched data stream loads with
every record flagged `stale`** (the consumer's own `stale_policy` decides; badge + journal entry
mandatory) · **G7 `gpu` tiers deferred to Phase 5** with an interim rule · Q1 dispatch answered by
measurement · Q4 host-provided oversampling · **Q2 sub-block processing forbidden in v0**. G3 and G6
were decided against the recommendation; both are recorded with the extra work they create and, for
G6, the fallback if it ever produces wrong sound. Q2's consequence: `resources.internal_rate` is
deleted from the schema, and the escape hatch is a smaller host block (~0.4 % CPU for 4× resolution,
measured) rather than a contract change.

**Defects found in the existing documents** (#59 is this increment's own, logged because the discipline applies to its author too):

| # | Defect | Caught by |
|---|---|---|
| 54 | **The schema had pre-committed to an answer the contract called open.** `manifest-schema.md` §8 declared `resources.internal_rate (optional sub-block rate)` while `module-api-v1.md` §16 Q2 listed sub-block processing as undecided — and §14's granular ✅ rested on Q2. A validator would have accepted a field whose semantics nobody had agreed on | writing the field table row-by-row against the prose |
| 55 | **Two acceptance blockers in the validation catalogue.** 19 codes existed; none rejected a *missing required field* (20 rows are `Req: yes`, `E-UNKNOWN-KEY` catches only extras) and none rejected an invalid value for the ~20 closed vocabularies without a specific code (`tier = "t9"` had no error to produce). WO-007's acceptance criterion names "missing required ports" explicitly, so it was unmeetable as written. Fixed by deriving codes from the schema (`E-KEY-MISSING:<path>`, `E-ENUM-UNKNOWN:<path>`, `E-CROSS-FIELD:<id>`, `E-VALUE-TOO-LONG`, `E-VALUE-MALFORMED`) so a new field brings its own | deriving the catalogue from the field table instead of extending it by hand |
| 56 | **One module has two names in two authoritative lists.** §17 Phase 1 says `ana/scope-tap`; Appendix B and WO-014 both say `ana/tap`. Module ids are stable ids that can never be renamed without an alias (§12), so this gets more expensive every day it is not fixed. **RESOLVED 2026-09-22 — the author chose `ana/tap`**, and §17 was corrected, so all three lists agree. It cost one line today; after WO-014 builds the module it would have cost an alias table and a migration | cross-checking the two Phase-1 lists while amending both |
| 57 | **WO-014's in-scope line said "plus four" and listed five** (17 = 12 + 5). Wording corrected; the 17-module count and every acceptance criterion are unchanged | counting the table it introduces |
| 58 | **ADR-005 promised four one-tap adapters; three did not exist at any phase that could honour the promise** (`data→cv` "a Mapper" is Phase 5–7, no `util/offset`, no gate/trigger converter at all). Shipping the offer before the modules produces a button that does nothing — the exact failure `sparq-ui`'s suppression log exists to make impossible. Fixed by adding three trivial modules to Phase 1 and by the rule that `data→cv` degrades to *refused* until `dat/mapper` ships | writing the adapter cells of the matrix and looking each one up in Appendix B |

| 59 | **This increment's own omission, found by following a cross-reference.** `module-api-v1.md` §16 Q1 cited "§ADR-009.7" — ADR-009's executor decision 7, which *asked for exactly this measurement* ("measure trait-object vs enum-dispatch cost with a 100-null-module graph; record the number and choose"). The measurement was run and recorded in the ADR-005 addendum and in §16, but ADR-009 item 7 was left still saying "measure … and choose", so the ADR that *owns* the decision did not contain it. Fixed, and its Consequences bullet upgraded: enum generation from module registration is now a **requirement**, not a mitigation, because a hand-written enum would falsify WO-007's zero-engine-edits criterion | following the §16 Q1 cross-reference back to its source, instead of amending only the document that cited it |
**Verification of this increment** (measured, including what was *not* run): `cargo fmt --all --check`
clean · `cargo test --workspace` **280 passed, 0 failed, 1 ignored** · clippy clean in three cells
(default, `bootstrap-audio`, `ui`), with the gate **proven failable** by injecting a violation and
reverting byte-identical · 4 Python gates clean (`token_gen --check` 0 files, `token_audit`,
`unsafe_audit` 10 allowlisted / 62 `.rs`, `check_text_io`). No gate scans `.md`, so these edits
cannot break one — re-run anyway to confirm. **Not run:** the three MSVC cross-check cells and the
release-dependent gates (`golden`, `golden-demo`, `selftest --golden`, `ui --audit`), so the golden
hashes `ba577186c988db21` / `dd975a24f03b19c1` are quoted from the 1.1g ledger, not re-measured. **[Superseded later the same day: increment 4's close-out ran the release gates for real — `selftest --golden` PASS 8/8 with `ba577186c988db21` matching, both release golden tests passing, `ui --audit` PASS. This entry is left as written because a build log is a ledger, not a document that revises itself.]**
Also reconciled: the tree holds 288 `#[test]`, 9 of them `cfg(all(windows, feature = "hal-wasapi"))`
⇒ 279 on Linux, plus 2 `sparq-kernel` doctests (1 ignored) = **281 discovered, 280 passed** — the
exact match to the ledger is the evidence the restored tree is the tree that produced it. Per binary:
audio 172 unit + 12 integration, kernel 58, ui 37, **app 0** (covered by the golden and selftest
gates instead; noted, not a defect).

**WO-007 tasks 2–3 — `sparq-module-api` and the three conforming modules (2026-09-22):** the crate the
contract lives in, six source files plus one integration-test binary, added to the workspace and to
`build.rs`'s fingerprint roots. What it contains, and why each piece is shaped the way it is:

* **The closed vocabulary as types** (`port.rs`): six port types, nine channel sets including
  `ambisonics:N` / `objects:K` / `raw:M` / `variable`, `cv` rate and range, direction, multiplicity.
  Channel-set parse failures keep their two distinct codes (`E-CHANNELSET-UNKNOWN` vs
  `E-AMBI-ORDER-RANGE`) rather than collapsing into one.
* **The connection rules as functions** (`connect_audio`, `connect_cv`, `connect_cross`), mirroring
  `docs/api/compat-matrix.toml` cell for cell. The "never offer a converter that does not exist" rule
  is a *function*, not a policy: every `Adapter` carries the phase it first ships in, and an offer
  made before that phase returns `Refused`. So `data→cv` is refused until `dat/mapper` exists in
  Phase 5, and `ambisonics→stereo` is refused in Phase 0 and offers `spa/hoa-decode` in Phase 5.
* **The derived error catalogue** (`error.rs`): the 19 legacy kinds plus the five derived ones, each
  error carrying path, found, allowed and fix, because a message without a fix is half a message.
  `E-KEY-MISSING:<path>` is what makes the "rejects missing required ports" acceptance criterion
  measurable for the first time (defect #55).
* **The manifest with every required field an `Option`** (`manifest.rs`) — not defensive styling but
  the only way absence can be reported. `validate()` returns either a `ValidatedManifest`, whose
  ports are already parsed into types, or a report holding *every* failure at once, so an author
  fixes a module in one pass. That the executor only ever receives the validated type is what makes
  "a bad manifest never loads" a property of the type system rather than of caller discipline.
* **Param delivery** (`params.rs`): a `Copy` 32-slot snapshot through the kernel's `SpscRing`, so the
  audio thread never allocates and a burst of UI writes coalesces to the newest. `begin_block` is the
  only place the swap happens, which is what makes "a change during a block is deferred to the next
  one" a testable property rather than an intention. `MAX_PARAMS` is a hard cap for that reason, and
  a manifest over it is refused rather than truncated.
* **The `Module` trait** (`module.rs`): dyn-compatible on purpose (ADR-009 decision 7 measured that
  block-level trait objects cost 1.65–1.95 µs for 100 modules, and per-sample ones cost 27–36 µs
  against a 20 µs budget). `AudioCtx` carries a block context, an immutable snapshot and two borrowed
  buffers, and exposes no allocator, clock, filesystem or lock. `BlockStatus::Silenced` exists so an
  unconnected input is a statement rather than silence-by-accident.
* **Three conforming modules as contract tests** (`tests/contract.rs`): `util/gain` (transparent at
  unity, bit-exact), `syn/sine` (`f64` phase accumulation that wraps, so 200 whole periods cancel to
  a mean under 1e-6), `ana/rms` (the analysis-as-control-source principle, with a floor that turns
  denormal dust into an exact zero). Each has a manifest that validates, and each module's `id()` is
  asserted equal to its manifest's — the implementation and the manifest cannot disagree about who
  they are.

**Measured, not asserted:** **338 tests** (was 280: +46 unit, +12 contract) · clippy clean in **seven**
cells (default workspace, `bootstrap-audio`, `ui`, native `ui-window`/gles at 2 m 14 s, MSVC
`sparq-module-api`, MSVC kernel+`hal-wasapi`, MSVC app+`hal-wasapi`); the eighth, MSVC `ui-window`,
was **not** re-run this session · fmt clean · 4 Python gates clean · and **0 allocations across
15 000 `process` calls through `Box<dyn Module>`**, measured with the kernel's counting allocator
installed as the test binary's global allocator, after an un-measured warm-up so the harness's own
one-shot initialisation is not counted as the audio path's.

**Acceptance criteria status:** (2) all five named rejections are implemented and tested · (4) the
param swap is atomic at the boundary and a mid-block change is provably deferred, tested end to end
through a real module · (6) dispatch measured, 10–16× inside budget · (3) met **as far as the
language allows**: `AudioCtx` exposes no capability, `clippy.toml` denies the types and methods, and
the allocation claim is measured — Rust cannot forbid `Vec::new()` by types alone, so the honest
statement is "no capability offered, no allocation measured", not "impossible" · (1) and (5) — the
throwaway-module CI test and the API-surface snapshot — are **task 5, not started**.

**Still outstanding for WO-007:** task 4 (`docs/module-author-guide-v0.md`), task 5 (API snapshot +
the throwaway-module CI test), and the **TOML reader**: validation currently runs on Rust values, so
discovery cannot yet read a real `sparqmod.toml`. It needs a dependency-free subset parser (the core
build has zero third-party dependencies), and once it exists the compatibility matrix can be read
from `docs/api/compat-matrix.toml` at discovery instead of being mirrored in `port.rs` at all — which
is the better answer and the reason the mirror is documented as temporary.

**Defects found in this increment:**

| # | Defect | Caught by |
|---|---|---|
| 60 | **`build.rs`'s fingerprint was blind to a fifth crate.** The stamp walked a hardcoded list of four `src` roots, so adding `sparq-module-api` would have shipped a binary whose stamp was computed without it — the same class as #41, where the fingerprint counted top-level files only and `hal/`, `rt/` and `play/` changes were invisible "in principle". The stamp would have kept reading `57f/853914B` while the tree had grown, and the guard whose entire job is to detect that would have agreed with the lie | recomputing the stamp by hand after adding the crate to `members`, and getting a different answer than the file said |
| 61 | **Two of the new contract tests were wrong, and their own assertions caught them on the first run.** `sine_stays_in_range_is_deterministic_and_wraps_its_phase` integrated 440 Hz (109.09 frames per period) over a whole number of 64-frame blocks — never a whole number of periods, so the mean could not cancel and the test measured its own arithmetic instead of phase drift; fixed by testing at 750 Hz, where one block *is* one period, and tightening the threshold from 1e-4 to 1e-6. `the_block_level_trait_object_path_is_the_one_the_executor_uses` ran three modules through one shared output buffer and then asserted the sine had written to it — but `ana/rms` legitimately zeroes its output before returning `Silenced`, so the assertion was checking whichever module ran last; fixed by giving each module its own buffer and asserting all three | the tests themselves, failing on their first run |

Both are recorded because a test that fails for the wrong reason and is then loosened until it
passes is how a gate quietly stops testing anything. Neither was loosened: the first was given a
frequency that makes the property exactly true, the second was given a buffer per module so it
asserts what it claims to.

**The new build stamp is `src 63f/963862B`** (was `57f/853914B`): +6 `.rs` files, +109 948 bytes,
all of them in the new crate. On SATURN the guard will report a mismatch against the 1.1g binary and
force a rebuild — that is #49's remedy working, not a failure.

**WO-007 increment 2 — the TOML reader, so a manifest can actually be read (2026-09-22):** two new
modules in `sparq-module-api`. `src/toml.rs` is a **dependency-free TOML subset parser** (20 tests):
comments, `[table]` and `[[array.of.tables]]`, dotted keys, bare and quoted keys, basic / literal /
multi-line strings, integers with `_` separators, floats, booleans, multi-line arrays and inline
tables. Deliberately outside the subset — and *refused with a line number rather than misread* —
datetimes, hex/octal/binary integers, `inf`/`nan`, and dotted keys inside inline tables. A manifest
that loads as something other than what its author wrote is a wrong-sound bug with no audible
symptom, so the subset is narrow on purpose. `src/decode.rs` turns parsed text into a
[`ValidatedManifest`] in three stages (parse → decode → validate), never stopping at the first
problem, and reports syntax/type failures *before* semantic ones because a report is read top-down.

Two design points worth recording. **The crate performs no I/O at all**: `decode` takes `&str` and the
caller reads the file. `clippy.toml` bans `std::fs::read` workspace-wide but not `read_to_string`, so
relying on the letter of that list would have been easy; holding no filesystem access is stronger and
puts discovery where it belongs, on `sparq-app`'s control thread. **Sections v0 accepts but does not
decode** (`ui`, `lifecycle`, `capabilities`, `distribution`, `signature`, `data_schemas`) are still
*key-checked*, so `colour_klass` inside `[ui]` is `E-UNKNOWN-KEY` rather than a silent no-op —
accepting the section while ignoring its contents would have been exactly the invisible failure
plan §4.3 forbids. `MODELLED` is public so the gap is visible to a caller, and a test asserts every
modelled table is a legal top-level key. `ResourceDecl` also grew `latency_param` for the
`latency = "param:<id>"` form, with validation requiring exactly one of the two forms and
`check_declared_latency` declining to judge a parametric latency statically.

**Measured:** **368 tests** (was 338: +20 parser, +10 decoder) · clippy clean `--workspace
--all-targets` · fmt clean and **stable across two consecutive `--check` runs** · 4 Python gates
clean · stamp **`src 65f/1034459B`** (was `63f/963862B`).

**Defects found in this increment:**

| # | Defect | Caught by |
|---|---|---|
| 62 | **The parser advanced one byte at a time, so any multi-byte UTF-8 character in a string value panicked the slice** (`end byte index 6 is not a char boundary; it is inside 'µ'`). With `panic = "abort"` in the release profile that is a process abort: a manifest whose `summary` contained an en dash would have killed the application at discovery. Not an exotic case — this repo's own documents are full of `·`, `—` and `×`. Fixed at the root rather than at the call site: `bump()` now advances one **character**. Every structural byte the parser compares against is ASCII, so the structure is unaffected and every slicing site is fixed at once | the `non_ascii_content_survives_intact` test, on its first run |
| 63 | **The same byte-orientation leaked into an author-facing message.** A table header with a non-ASCII bare key reported the offending character as `Ã`: `peek()` returns the first *byte* of `ä`, and rendering a byte as a char produces mojibake in precisely the message meant to help the author fix their manifest. Fixed by decoding a character, and the test now asserts the real character appears | the same test, *after* #62 was fixed — the panic had been hiding the message that followed it |

Both are one mistake seen twice: the parser was written byte-oriented in a UTF-8 world. They are
logged as two defects rather than one because the second was invisible until the first was fixed.

**Test-authoring defects, also fixed rather than loosened:** a table-header error blamed the missing
`]` when the real cause was an illegal character earlier in the line (the message now names the
character and offers quoting); the dotted-key test asserted two parse trees were *equal* when they
legitimately differ in recorded line numbers — the header form's key is on line 2, the dotted form's
on line 1, and the line is what an error points at; and the ports test replaced only the first
`[[ports]]`, leaving a table and an array with the same name, which the parser rightly refuses as a
conflict, so the test was measuring a different error than the one it claimed to.

**Note for `gates.bat`:** rustfmt needed **two passes** to converge on the nested `match` in
`insert`/`declare_table`. The tree is now stable across consecutive `--check` runs, but a
non-idempotent format would fail a gate on a tree `cargo fmt` had just produced, so if `gates.bat`
ever reports fmt diffs on an untouched tree, run `cargo fmt --all` twice before believing it.

**Still outstanding for WO-007:** task 4 (`docs/module-author-guide-v0.md`), task 5 (API-surface
snapshot + the throwaway-module CI test that proves acceptance criterion 1), **discovery** (the crate
can now read a manifest, but nothing scans `modules/` or holds the built-in registry), and the
payoff item deferred from increment 1: with a parser in hand, `connect_audio`/`connect_cv`/
`connect_cross` in `port.rs` can be *read from* `docs/api/compat-matrix.toml` at discovery instead of
mirroring it — which deletes the only copy of the matrix that can drift. The mirror stays documented
as temporary until that lands.

**WO-007 increment 3 — the registry, discovery, and task 5's two gates (2026-09-22):** this closes
**all six** of WO-007's acceptance criteria, the last two of which were claims rather than facts.

* **`registry.rs`.** A module enters the registry as a manifest plus a [`Factory`] — a plain `fn`
  pointer, so storing one cannot allocate and a `Registration` needs no shared ownership. Registration
  **builds the module once and refuses it if `Module::id()` disagrees with `identity.id`**, because a
  module that lies about who it is breaks patch provenance silently: the project records the id and
  the wrong code runs. A second registration of the same id is an error carrying both versions, per
  module-api §11 — *"conflicts resolve by explicit version pin, never by latest wins"* — and
  `get_pinned(id, version)` is how a project asks for what it needs.
* **`discovery.rs`.** Takes [`Source`]s (origin, label, **text**) and never touches the filesystem, so
  discovery is testable without creating a file and the crate still holds no I/O. Precedence follows
  §11 (built-in → user `modules/` → project-local → registry cache) and is *stable within an origin*,
  because a reproducible module list is what makes a journal replay see the same modules (ADR-007).
  One broken manifest cannot hide the others, every failure keeps its label, and `render()` produces
  the text the module browser shows verbatim. **Shadowing is reported rather than resolved quietly**:
  a module that silently stops being the one that loads is a patch that changed sound because of a
  directory somebody else edited.
* **`tests/api_snapshot.rs`** — task 5, criterion 5. ~90 `const _: fn(..) = ..` pins: coercing a
  function to a fn-pointer type pins argument types, order and return type exactly, so an
  incompatible change to the contract is a *compile* error in this file. Plus layout pins for the
  facts other code depends on (`ParamSet` size and `Copy`-ness, because it crosses to the audio
  thread in a ring slot; `Factory` is one word). Where a receiver is generic over a lifetime
  (`AudioCtx`) rustc refuses the coercion, so those three are pinned by a typed call site — weaker in
  one respect, and the file says so at the pin rather than implying uniformity.
* **`tests/throwaway_module.rs`** — task 5, criterion 1, and the more interesting of the two. It adds
  a module that exists nowhere else in the repository, registers it from manifest text, runs a block
  through it, and then **walks every `.rs` under every `crates/*/src` asserting that not one file
  mentions it**. That walk is what turns "zero engine edits" into something a gate can fail on: if a
  future change makes adding a module require a line in `port.rs`, `manifest.rs` or a default
  registry, this test names the file.

**Proven failable, three ways, each reverted byte-identical (sha256 checked before and after):**
`ParamSet::version` → `u32` broke the build; writing the throwaway's id into a comment in
`registry.rs` failed the criterion-1 test *and named the offending file*; and — the isolating case —
`Origin::precedence` → `u16` compiles everywhere else in the workspace and fails only the snapshot
pin, which is the proof that the pin itself is load-bearing rather than incidental.

**Measured:** **388 tests** (was 368: +12 registry/discovery unit, +3 snapshot, +5 throwaway) · fmt
clean · clippy clean (`--workspace --all-targets`, MSVC `sparq-module-api`, `ui` cell) · 4 Python
gates clean · stamp **`src 67f/1063788B`** (was `65f/1034459B`). **Re-measured at the end of the session:** `selftest --golden` **PASS (8 gates)** with golden **`ba577186c988db21` matching**, both release golden tests passing, `ui --audit` **PASS (0 failures)**, determinism bit-identical (`0f5c3e86c7f117a9`), 0 allocations with the allocation gate proven live, **315× realtime** for 1 min at 96 kHz/64, HAL null conformance 9 checks with reopen-leak 0 outstanding. `ui --audit` needed relaxed sandbox codegen (`lto=false`, `codegen-units=16`) because `read-fonts` is OOM-killed at 1 GB under the repo profile; SATURN builds with the repo profile. **Still never run anywhere:** the MSVC `ui-window` clippy cell.

| # | Defect | Caught by |
|---|---|---|
| 64 | **`shadowed()` reported the same hidden module more than once.** It walked every loaded entry and collected everything below it, so in a three-way collision the second entry also reported the third and the lowest-precedence module appeared twice — the module browser would have shown a lie about which file was being hidden. Fixed by skipping any id that already has a winner | its own test, on the first run: expected `["modules:g", "project:g"]`, got `["modules:g", "project:g", "project:g"]` |
| 65 | **The engine-source walk descended into `examples/`** and then tripped the guard asserting it only covers `src/` — so criterion 1's own proof was failing for a reason that was not a violation. Fixed by walking `crates/*/src` directly; `tests/`, `benches/` and `examples/` are allowed to know about a module, and counting them would make the test fail for the wrong reason | its own guard assertion |

**Outstanding for WO-007:** task 4 (`docs/module-author-guide-v0.md`, to be written *from* the three
reference modules rather than from the spec); the **app-side scan** of `modules/` — the contract crate
deliberately holds no I/O, so walking the directory belongs to `sparq-app`'s control thread; and the
deferred payoff, reading `docs/api/compat-matrix.toml` at discovery instead of mirroring it in
`port.rs`, which now that a parser exists is the last thing standing between the matrix and having a
single source of truth.

**WO-007 increment 4 — task 4 and discovery; the work order closes (2026-09-22):**
`docs/module-author-guide-v0.md`, written *from* `util/gain`, `syn/sine` and `ana/rms` rather than from
the spec, which is the only way it could find the places where the spec is not enough — it documents
the v0 limits as limits (one audio in/out, no sub-block processing, `data`/`gpu`/`event` payloads
declared but not carried), states criterion 3's honest form ("no capability offered, no allocation
measured", not "impossible"), and records the §15 two-hour test as the standard the guide is measured
against. `crates/sparq-app/src/modules.rs` adds `sparq modules [--root DIR] [--strict]`: it walks a
directory depth-limited to 6, skips `target`/`.git`/`node_modules`, sorts what it finds **so the module
list is reproducible and a journal replay sees the same modules** (ADR-007), and prints the report
verbatim — loaded, refused-with-fix, and shadowed. `--strict` exits non-zero on any refusal so CI can
gate on a clean module set. This is the only place in the workspace that reads a directory; the
contract crate still takes text and holds no I/O.

**Measured:** 388 tests · fmt clean · clippy clean · 4 Python gates clean · stamp
**`src 68f/1070593B`** · and the new command exercised end to end against a scratch tree: 3 manifests
found, 2 loaded, 1 refused with an actionable message naming the line, `--strict` exit 1, and a
missing root directory handled as a message rather than an error. **`modules.rs` has no unit tests** —
the CLI smoke run is weaker than a gate and is recorded as such rather than described as tested.

Shipped as **`sync-wo007-complete.zip`** (42 entries; heading, manifest list and zip contents verified
equal, and every entry byte-identical to the tree). Two defects in the packaging were caught by that
verification rather than by reading: `SYNC.md` was listed twice in the archive, and one manifest row
lost its column padding because its path was longer than the field width — the same off-by-one class as
the 35-vs-36 heading in the WO-012 increment, which is why the count is computed from the archive and
compared, never typed.

**WO-007 P1 device run — the first real MSVC build: green where it counts, six defects found (2026-09-23, SATURN physical):**

P1 ran as the run sheet ordered: `scripts\build.bat`, `scripts\gates.bat`, then — because the test
stage failed and cargo fail-fasts, hiding everything after the failing binary — a full
`cargo test --workspace --no-fail-fast`. **What is now proven on a real MSVC toolchain:** the whole
increment compiles and links (clippy `audio+hal` clean, release build with `bootstrap-audio,
hal-wasapi,ui-window` in 58.12 s); **the golden render is bit-identical on MSVC** (debug 2/2,
release 2/2, and `selftest --golden` reports hash `ba577186c988db21` — the DSP does not depend on
the compiler's `sin`); selftest **8/8 PASS** on the device, determinism `0f5c3e86c7f117a9`
matching, allocation gate live, reopen-leak 0 outstanding, throughput **1059.1× realtime** (sandbox:
315×); `ui --audit` PASS from the gates run itself; all four Python gates clean; and the exe
self-reports stamp **`src 68f/1070593B`** — the zip-sync chain is byte-faithful. The no-fail-fast
run measured **386 passed / 2 failed / 1 ignored of 389** — the same population as the sandbox;
both failures are test-side defects, reproduced in the sandbox, fixed below. `sparq-module-api`
itself went 107/108 on MSVC, 108/108 after the fix: **WO-007's contract crate is verified on
Windows.** `build.bat`, however, left no stamp in its log — two defects of its own:

| # | Defect | Caught by |
|---|---|---|
| 66 | **The allocation-audit windows contaminated each other across test threads.** Arming is per-thread but the counters the deltas read are process-wide (`ALLOC_COUNT`), so a second test armed at the same moment adds its allocations to the first one's window. The 2-core sandbox never exposed it: libtest ran at most two tests at once, and the short tests all finished inside `audio_path_makes_zero_allocations`'s *unarmed* warm-up. SATURN ran all eight `rt_discipline` tests concurrently, and `outstanding_balance_survives_realloc_growth`'s deliberate 129-allocation burst landed inside `multichannel_render_stays_allocation_free`'s armed render windows — "32 channels allocated, left: 129". **Not a Windows bug, a core-count bug**: reproduced in the sandbox with `--test-threads=8` (9 of 10 runs; victims also `full_render` and `control_drain`, plus a `left: 1` variant that is the one-shot `audit_is_live` probe landing in an open window). Fixed by an `AUDIT_LOCK` serialising whole test bodies and pre-firing the probe under the lock — the engine is untouched, and its zero-allocation claim stands on the device's own single-threaded selftest. Post-fix: 10/10 green at 8 threads | SATURN P1 gates ("tests" stage, rt_discipline.rs:148) + sandbox reproduction at `--test-threads=8` |
| 67 | **The throwaway-module walk guard hardcoded the Unix separator.** `p.to_string_lossy().contains("/src/")` is never true on a `Q:\...` path, so criterion 1's own mechanical proof failed on its first Windows run — the guard added by #65 assumed the platform it was written on. Fixed with `p.components().any(|c| c.as_os_str() == "src")`, which is separator-free; the walk itself had worked (it found >40 files and the offender scan is content-based) | SATURN P1 no-fail-fast run (`nothing_in_the_engine_names_the_throwaway`, throwaway_module.rs:210) |
| 68 | **`build.bat`'s stamp guard walked four `src` roots while `build.rs` walks five** — the cmd mirror of the fingerprint never learned about `sparq-module-api`. The same class as #60, which fixed the `build.rs` side only. From WO-007 on, the guard would have declared every freshly built binary stale (CUR = four-root count vs BIN = five-root stamp), deleted it, rebuilt, and declared it stale again — "STILL STALE: Do NOT trust this binary" next to a perfect build. Fixed by adding the fifth root to the `dir` walk; the comment now says five and names both files that must change together | P1 triage: build.log ended with no stamp line; recomputing the guard's walk against build.rs's roots |
| 69 | **`build.bat` died with the cmd parser error `. was unexpected at this time.` immediately after a successful 58 s build** — before any "stamp guard:" line could print, which is why P1's `stamp` was MISSING despite a healthy binary. Every statement between cargo's "Finished" and the guard's first echo is clean on paper; the prime suspect is the per-line `IF` with delayed-expansion substring substitution in the version-capture loop. The capture is now hardened (`findstr /c:" src "` anchors; the loop body is a single `set`), and **`scripts\probe-stamp.bat` reproduces the old and new forms section by section in child cmds with echo ON**, so `logs\probe.log` names the culprit — the last echoed statement before the error is the one that kills cmd. **RESOLVED 2026-09-23 — the probe named it on its first device run: section S0, the BUILD-FAILED hint block.** The killer was `echo ... Cargo.toml (the per-target wgpu features live there).`: inside a parenthesised block the first unescaped `)` in echo text ends the block — a same-line `(` that looks balanced does not nest against it — and the leftover `.` is a token the IF parser rejects: `. was unexpected at this time.`, exit 255. The block is parsed on every run whether or not the condition fires, so build.bat had died after EVERY successful build since WO-012 added that hint — a healthy exe and no stamp line, exactly as P1a observed. Fixed by caret-escaping both parens, and `check_text_io.py` gained **rule 4** (unescaped parens in echo text at block depth > 0), proven failable against a verbatim copy of the original — it flags line 75 — before shipping. The probe keeps the killer as S0a (must die; a reproduction that was fixed proves nothing) and verifies the escaped form as S0b. The probe's other sections closed the rest of the hunt and corrected the record: **S2 exonerated the per-line IF-substitution loop** that was the prime suspect at ship time (it extracted `BIN=68f/1070593B` cleanly); the hardened findstr capture stays regardless and is verified as S5. S1 quantified #68 on the device (four roots saw `58f/860719B`); S4 verified the five-root fix byte-exact (`68f/1070593B` == baseline), which simultaneously proved SATURN's tree clean of the #71 stray | SATURN P1 build.log (its last two lines); root cause via `scripts\probe-stamp.bat` S0a, exit 255 |
| 70 | **`gates.bat` had no `version`/`selftest` stage**, so `selftest`, `golden_hash`, `allocations`, `realtime_x` and the `reopen_leak` line could never appear in gates.log on Windows — permanently MISSING in log_check even after a fully green P1, and the run sheet's own "Expected: selftest 8/8, both goldens" was unreachable through the script it named. The Linux `just gates` recipe always had the stage; the Windows port dropped it. Fixed: `version stamp` + `selftest + golden` stages run after the release build, which also puts the stamp in gates.log as a second source | the P1 log_check report (11 MISSING — four of them this gap) |
| 71 | **The published git repo carries a stray `crates/sparq-module-api/src/decode (2).rs`** — a byte-identical Windows copy artefact of `decode.rs` (the "(2)" suffix an Explorer/zip extraction creates instead of overwriting). Harmless to compilation (no `mod` declares it) but it poisons the stamp: a tree containing it fingerprints `69f/1097801B`, and log_check would call every binary built from it stale against the 68f baseline forever. SATURN's tree is provably clean (its exe stamp is 68f/1070593B); the file is deleted in the workspace clone. **Do not clone the GitHub repo onto a stage machine — keep syncing by zip — and delete the stray on any machine that did** | recomputing the stamp against the baseline during P1 triage — and **confirmed on SATURN hours later**: the post-fix build stamped `src 69f/1097801B`, the stray-inclusive fingerprint to the byte, so the device tree acquired the duplicate between the two P1 runs (route pending the operator's answer; the arithmetic matches a repo clone exactly). Note the checker's verdict text said "stale exe; rebuild" — rebuilding cannot fix a tree-content drift, which is why #72 also rewrote that message |
| 72 | **A green gates.log counts the golden tests twice.** The `tests` stage runs them (debug) inside `cargo test --workspace`; the later `golden reference` stage re-runs the same two in release; log_check summed every "test result:" line in the whole log, so the first fully green P1 run read **390 against the 388 baseline** — a hard FAIL manufactured by the checker, not by the tree. The broken pre-fix run had masked it: fail-fast left only the release rerun's +2 in the log (which is why 185 fitted the arithmetic so neatly). Fixed by scoping the sum to the `---- tests ----` stage when the log carries gates.bat's stage headers, with header-less logs (the Linux `just gates` format, the fixtures) falling back to the whole text; a new self-test check pins the scoping (25/25 pass) | the post-fix P1 run on SATURN: 390/0 with every other field green |

Shipped as **`sync-p1-fixes.zip`** (7 entries: `scripts/build.bat`, `scripts/gates.bat`,
`scripts/probe-stamp.bat`, `crates/sparq-audio/tests/rt_discipline.rs`,
`crates/sparq-module-api/tests/throwaway_module.rs`, `PHASE0-WORKORDERS.md`,
`P1-FIXES-RUN-SHEET.md`), extracted at the repo root over `sync-wo007-complete.zip`.
**Sandbox re-verification after the fixes:** 388 passed / 0 failed / 1 ignored · fmt clean ·
clippy clean in the default, `bootstrap-audio`+`hal-wasapi` and MSVC cross-lint cells · 4 Python
gates clean · log_check self-test 24/24 · `rt_discipline` 10/10 green under `--test-threads=8`
(before the fix: 9/10 failing with the same victim and `left: 129` as SATURN). Tests and scripts
only — the stamp stays **`src 68f/1070593B`**, so the log_check baseline and every recorded
measurement remain valid. Device-side steps: `P1-FIXES-RUN-SHEET.md`. Follow-up **`sync-p1b.zip`** (3 entries: `tools/log_check.py`, `PHASE0-WORKORDERS.md`, `P1B-RUN-SHEET.md`) after the green device re-run exposed #72 in the checker and #71 in the device tree; sandbox re-verification: log_check self-test **25/25**, a simulated green gates.log reads **388**, Python gates clean. **`sync-p1c.zip`** (5 entries: `scripts/build.bat`, `scripts/probe-stamp.bat`, `tools/check_text_io.py`, `PHASE0-WORKORDERS.md`, `P1C-RUN-SHEET.md`) closed #69 the same day: the probe named the killer on its first device run, the parens are escaped, and check_text_io rule 4 now gates the class — proven failable against the verbatim original before shipping.

**WO-013 increment 1 — the graph canvas, gestures-first (2026-09-24):** the signature sparq
surface, built in the same order WO-012 established — *logic* first in the toolkit-independent
crate, *drawing* last behind the feature. `sparq-ui::canvas` gained five modules: `model` (the
graph as data — nodes from validated `sparq-module-api` manifests, wires, flags, selection — with
**every mutation an `Op` carrying its inverse** and a bounded undo/redo stack, so "undo restores the
graph" is a property of the representation), `camera` (pan/zoom on the token clamps, zoom-to-fit,
LOD from the zoom tokens), `layout` (one pure pass graph+camera+view → screen rects, port
positions, bézier-horizontal wires, and hit-testing at the token capture/hit widths), `connect`
(the drop verdict, **delegated to `sparq-module-api`'s own `connect_*`** so the compatibility matrix
keeps its single copy — direction, duplicate, cycle, fan-in replacement, and defect #58's
availability gate all layered on top), and `interact` (`CanvasState`: the intent→op table, the
in-flight move/wire/marquee, the long-press menu, and every refusal returned **in words**).
`sparq-app` gained `ui/canvas_ui.rs` — egui **as a painter only** (no egui widgets; hit-testing
runs on sparq's rects), drawing the world-anchored dot grid, signal-class wires with their token
stroke *encodings* (solid/thin/dashed-6-3/dotted-2-4/double, parsed from the token strings — no
pattern hard-coded), nodes at three LODs with A/C/E/D/S port letters, the in-flight wire with
glow/dim affordances, the marquee and the context menu, registering node/port/menu touch targets
into the same audit list the chrome uses. `ShellUi` routes canvas-rect intents to the canvas and
binds the two WO-012 stubs it left (`DoubleTap`→zoom-to-fit, `Context`→menu, `Undo`→graph undo).
The demo patch is the three conforming reference modules plus a second Gain instance with a free
input (sine→gain→rms, mono→stereo fan-out visible) — **only modules that actually exist** (#58).
**Decision taken:** `sparq-ui` gains one *first-party* dependency, `sparq-module-api`, because
`port.rs`'s own `Verdict` doc says "the canvas renders this directly" and mirroring the port
vocabulary is the drift class WO-007 refused to ship; the gesture/shell/audit core still reaches
nothing outside `std`, and the crate stays zero-*third-party*, zero-`unsafe`. One token added:
`layout.toml [canvas] node_port_row = 48` (≥ 2 × port capture radius, so capture circles on one
edge never overlap), regenerated through `token_gen.py`.
**Measured in sandbox (no GPU, no digitiser):** **418 tests** (was 388; +30 canvas unit tests);
clippy clean in the default and `ui` cells (the `ui-window` cell **compiles** — `cargo build
--features ui-window` green single-threaded with debuginfo off; the sandbox OOMs on `naga` at
`debuginfo=2`, an environment limit, not a code fault — SATURN/CI build it normally); `sparq ui
--audit` **PASS, 0 failures** — 40 matrix cells + DPI-invariance + **14 smokes** (6 chrome + 5 new
canvas: drag-node+8px-snap+×10-fine, three-finger undo restores, cv→audio refused in words with no
wire, stereo→free-stereo connects, two-finger spread zooms), Design cells now auditing **24 touch
targets** (was 4; nodes class-L, port captures class-S, all ≥ minimum); goldens **unchanged**
(`ba577186c988db21`) — no audio path touched; 3 Python gates clean (`canvas_ui.rs` + `canvas/*.rs`
added to the R6 scan, no colour literals); headless frame logic med ~3.4 ms (grid-dot dominated;
GPU raster is the device question). **Not claimed (device-only, and not claimable over
RDP-from-iPad):** 60 fps at 200 nodes (WARP ≠ stage GPU), real palm rejection while wiring
(iPadOS filters the palm before Windows; sparq's `contact_area` path needs WM_POINTER — increment
2), the full DPI matrix and mixed-DPI dual monitor (RDP is one virtual display). Artefacts:
`sparq-ui::canvas`, `docs/ui/gestures.md`.

**Defects WO-013 increment 1 caught:**

| # | Defect | Caught by |
|---|---|---|
| 73 | **The recogniser reported a stale release position.** `on_up` copied the tracked contact and read `t.last` — the last *sampled `Moved`* point — without recording the release event's own position, so `DragEnd`/`Activate` carried where the finger last *moved*, not where it *lifted*. Invisible in WO-012 (no surface needed a pixel-accurate drop), it broke WO-013's wire drop: a synthetic drag that released over a port with no trailing `Moved` dropped the wire at the previous move point and missed the 24 px capture ("wire dropped on empty canvas" on a drop that was, to the finger, on the port). It is a real device risk, not a test artefact: **Windows coalesces pointer frames**, so the release can sit a delivered-frame away from the last move at drag speed. Fixed by recording the Up position before reading `t.last`, which only ever sharpens the reported point (a tap releases within its slop anyway) | the `drag stereo-out → free stereo-in connects` audit smoke, failing while every canvas unit test passed (they drive `GestureIntent`s directly, so they never exercised the pointer→intent release path); all 37 WO-012 gesture tests stayed green — they released at the last move point, where the fix is a no-op |

**WO-006 increment 1.2 — integer exclusive rungs, defect #77 (2026-09-24, sandbox-built):** the
first physical session (test001/test002, `docs/hal/windows-notes.md` §4/§4b) resolved the endpoints
— SATURN's default IS the **BEHRINGER UMC 204HD 192k** — and split the findings three ways.
**Passed on hardware:** shared-mode play (the first sound ever through the HAL, WO-005's "hear it"),
10 s runs + a 5-min soak @96k/64 with **0 xruns / 0 allocations** (450 307 blocks, p50 1.0 µs /
p99 2.0 µs / max 108 µs), 4 ch f32 @96k negotiated on OUT 1-4 (the #38/#46 two-shapes ladder doing
its job on real silicon), **the unplug acceptance criterion** (mid-run removal → `Removed`, dev-err 1,
0 xruns, clean stop, recovery tone after re-plug), reopen-leak 0 on hardware, and the multichannel
criterion resolved honestly (interface maxes at 4 out; caps say so; null proves ≥8). **Found:**
defect **#75** — friendly-name `E_ACCESSDENIED` on REAL endpoints with no remote session, so #40's
"RDP quirk" attribution is falsified; the MMDevices registry reads every name fine (test002 [02e]),
which is also the proven fallback path. Defect **#76** — the `SUSPECT` drift decoded:
`(buffer_frames ÷ event_period) ÷ rate − 1` = +1.2 M ppm fits BOTH the RDP session (970 fr/10 ms
@44.1k) and the Behringer (2112 fr/10 ms @96k) to four digits — `IAudioClock` advances in
buffer-sized steps per event tick on these streams; the #45 guard did exactly its job (throughput
stayed truthful everywhere). Defect **#77** — the acceptance blocker: the exclusive ladder probed
**f32 only** ("the format sparq thinks in"), and USB DAC drivers refuse float in exclusive, so a
fully exclusive-capable interface reported `exclusive none` (`AUDCLNT_E_UNSUPPORTED_FORMAT`, not
policy — both exclusive checkboxes ticked, no session holding the device). **The 2 h shared soak then PASSED on hardware (test003, same day): 10 798 597 blocks, 0 xruns / 0 late / 0 overruns / 0 allocs, p50/p99 2.0 µs vs the 667 µs budget, max tail 248 µs, jitter avg 10.00 ms over 719 903 wakes, reopen-leak 0 post-soak — the transport is proven at duration; the loaded soak is WO-008's acceptance (200 modules, 30 min).** **The fix (this
increment):** a four-rung exclusive ladder — f32 → **i24-in-32** → i32 → i16, all extensible (the
documented exclusive shape), first S_OK wins, S_FALSE is a no, and a total refusal now carries the
full probe table with HRESULTs (the #38/#46 lesson kept). `Negotiated.f32: bool` became
`fmt: DevFmt {F32, I16, I32}` + a log-facing `fmt_name`; the pump's i16 path generalised with
`pop_into_i32` (saturating, clamp-then-scale, one arithmetic serving 32-bit and left-justified
24-in-32 — the DAC ignores the low 8 bits); caps probe all four rungs per rate; the open log line
names the rung that actually opened. Two new unit tests (i32 conversion saturation/zero-fill;
`wfxe_int` packed-struct math). **Measured in sandbox:** MSVC cross-lint clean in both
`hal-wasapi` cells (kernel + app) — the only compiler this code has ever met; 418 Linux tests,
fmt clean, unsafe/text-io/token gates clean; the wasapi changes are compile-verified only, as
increment 1 was — **hardware acceptance is `test004`**: caps must now list exclusive rates for the
Behringer, the exclusive tone must be audible (naming its rung in the log), the unplug test re-runs
in exclusive, and the **2 h zero-xrun soak at 96 kHz/64 exclusive** finally has a device that can
run it. When it passes: WO-006 acceptance closes and ADR-008's exit condition fires — the cpal
bootstrap is deleted and the HAL becomes `play`'s default.

**WO-014 increment 1 — the first-party module batch: the chain is now audible end to end
(2026-09-24, sandbox-built):** the three reference modules (`syn/sine`, `util/gain`, `ana/rms`)
promoted from `contract.rs` test code to `sparq-audio::modules` library code — character-for-
character where the contract defined behaviour (the f64 phase accumulator and 8-byte state, the
4-byte gain state and `reset` message, the f64-summing rms with floor), because those tests are
the executable spec. The manifests became **files** (`modules/{syn/sine,util/gain,ana/rms}/
sparqmod.toml`) and are `include_str!`'d by the library: `sparq modules` discovers them from
disk, `register_builtins` compiles them in, and `manifests_on_disk_equal_the_compiled_in_bytes`
makes drift a failed test. Discovery's §11 precedence got its first live demonstration: the
on-disk copies of the built-ins are reported as *shadowed*, not duplicated, not errored; a
manifest with no compiled-in factory is reported as "no T1 factory in this build" rather than
half-loaded. **`sparq exec`** (new command) walks the whole WO-007→WO-008 chain without a sound
card: registry → factory → kernel graph → executor → blocks → WAV, printing the budget line
ADR-009 d4 asks for at patch load, the analysis tap, the master meters, and the golden hash of
the render. The demo patch (`sparq_audio::modules::demo_patch`, a LIBRARY function so the CLI and
the goldens render one copy): sine 440 Hz @0.5 → gain 0.5 → stereo master, with `ana/rms`
tapping the master — the smallest patch that exercises fan-out, master conversion, and analysis-
as-control-source. **Two honest findings recorded while testing it:** (1) the per-block RMS of a
440 Hz sine at 48 kHz/64 is **0.59 cycles of window**, so the tap value legitimately ripples a
few percent around amp/√2 (measured 0.1662 vs the full-cycle 0.1768) — the golden's tolerance is
the window ripple, documented, not a fudge; (2) a cv-convention node's *meter* averages its
value-then-zeros buffer, so the tap VALUE is read from `node_output[0]` and the meter semantics
for cv nodes are a noted question for the meters increment. The cross-check that matters: the tap
value and the master's metered rms **agree to the bit** (0.16621882 both) — the analysis module
and the audio path independently compute the same level. **Measured in sandbox:** 16 new tests
(7 library unit + 9 golden/discipline): zero allocations per module across 5 000 `process` calls
and across 1 000 executor blocks under the counting allocator; demo-patch golden
`53de3b1f3f40e3c9` (2.8 s) and sine golden `3f325d4f99ca2a01` (1 s) checked in; determinism
across builds; **468 passed / 0 failed**; clippy clean in the workspace, `ui` and MSVC-audio-cross
cells; `sparq modules --strict` exits 0; the pre-existing goldens are unchanged
(`ba577186c988db21`, `0f5c3e86c7f117a9`) — no existing DSP touched. **Declared absent (the WO's
rest):** 14 more modules in Appendix-B batches (next: the adapters `util/range`/`util/offset`/
`util/gate-to-cv`, which unlock the canvas's one-tap-insert path gated by #58, then `syn/noise`,
`flt/svf`, `env/ad`, `out/main`…), example patches per module (patch FILES wait on WO-011's
format — `exec --patch` refuses unknown names in words until then), manifest-generated docs, the
polyblep aliasing measurement, and the stage-machine CPU benchmark.

**WO-008 increment 2 — the executor: modules now RUN in graph order (2026-09-24, sandbox-built):**
tasks 2–3 shipped as `sparq-audio::executor`. **Placement, recorded for the ADR-009 ratification:**
the executor calls `Module::process`, and `Module` lives in `sparq-module-api`, which sits above
the kernel — so the machinery lives in `sparq-audio` (which gains the first-party module-api
dependency) while the kernel keeps the structure (`graph`). The build gauntlet, in refusal order:
node sets must match exactly → `module.id()` must equal the manifest id (the contract's first
invariant, enforced where it loads) → v0 shape (≤1 audio in / ≤1 audio out — the contract's own
v0 limit, refused in words, not discovered at runtime) → **every edge must carry audio and run
out→in**: a cv/event/data edge is refused at build with the reason ("the v0 AudioCtx carries
interleaved audio only — remove the edge or wait for contract v1"), because a wire the user drew
must never do nothing silently → channel negotiation per §5.4 (equal / mono→multi replicate /
multi→mono raw sum; anything else refused citing the matrix). Buffers (node in/out, delay
histories) are allocated and black-box-touched at build — the audio path only writes into
existing memory — and `memory_budget_bytes()` is the ADR-009 d4 "printed at patch load" number.
`prepare`+`activate` run in execution order (module-api §2 cascade rule). Per block: wire (fan-in
sums in edge-id order — the matrix's stable tie-break; block-delay edges read the PREVIOUS
block's history, unit-delay edges shift one sample with a held final frame, both refreshed at
block end) → one `Box<dyn Module>` dispatch per node (d7's measured hybrid) → status handling
(`Failed` silences the output and flags the node — the audio thread never unwinds; `Overrun`
counts toward the task-6 watchdog) → relaxed-atomic peak/RMS meters (d8's "published, not
polled", same-thread until the RCU increment ships the rings) → master render with device-shape
conversion → the sample clock advances (the musical clock is WO-009). **The kernel refinement
this increment forced** (recorded on the graph itself): a `unit_delay` edge shifts *within* the
block, so its source must run first — it is an ORDERING edge and cannot close a loop; the legal
closer is `block_delay`. Ordering now sorts the plain+unit-delay subgraph, and a connect that
would close an in-block loop is refused with the path and the remedy ("close it with a
block_delay"). This is §5.4's own parenthetical — "keeps the executor a simple topological
sort" — made structural; it is a Phase 0 finding for the ADR-009 ratification. **Measured in
sandbox:** **0 allocations across 1000 rendered blocks** and **0 across a 201-node block** under
the kernel counting allocator (tests/executor.rs installs it as the global allocator, the
contract.rs pattern); block-delay feedback verified by exact arithmetic (0.5 → 0.75 → 0.875
converging on 1.0); unit-delay shift exact (held-sample frame 0); two identical builds render
**bit-identical** output over 100 feedback blocks (FNV hash equality); the 13 integration + 7
unit tests join 2 new kernel graph tests — **452 passed / 0 failed**; clippy clean in the
workspace cell and both MSVC cross cells; fmt + 4 python gates clean; selftest determinism and
both goldens unchanged (`0f5c3e86c7f117a9`, `ba577186c988db21`) — no DSP touched. **Declared
absent (the WO's own remaining tasks):** RCU swap + the 10 000-mutation stress test (4 — until
then mutation is rebuild-and-swap, which is already the shape task 4 will use), latency
accounting (5 — `latency_samples` rides in the kernel graph for it), watchdog action (6 —
overruns counted, bypass not yet automated), the determinism harness proper (7), cross-thread
meter rings, and multi-port `AudioCtx` (contract v1) for cv/event/data payloads.

**WO-008 increment 1 — the structural graph core (2026-09-24, sandbox-built):** task 1 of the WO
("define graph + version types; implement topo sort and validation") shipped as
`sparq-kernel::graph`. The design constraint that shaped it: the kernel sits *below* the module
contract (`sparq-module-api` → `sparq-kernel`), so the graph is **structural** — nodes, opaque
port references, `EdgeKind {Plain, UnitDelay, BlockDelay}`, version, order — and knows nothing of
port types or the compatibility matrix. The typed verdict stays where WO-007 put it (the single
copy in `connect_*`); the layer above validates, the kernel guarantees. What the kernel
guarantees: **ids stable and never reused** (undo and journal replay depend on it); a **topology
version** bumped on every committed mutation and nothing else (a refused cycle leaves no trace; a
latency edit does not invalidate a valid sort); the **cached topological sort** of ADR-009
decision 1, Kahn with a smallest-id frontier so the order is a *pure function of graph state* —
byte-identical across replays (ADR-007) — with `order_computes()` as the observable proof the
cache works; **the §5.4 cycle rule as structure**: plain edges that would close a loop are
refused with the loop's path inside the error (`Cycle { from, to, path }`), and its `Display`
renders the full cycle and names the remedy ("only unit_delay/block_delay edges may close
loops"), because the acceptance criterion says *actionable* and plan §7.8 says *in words*;
delay edges skip the cycle check by definition and are invisible to the ordering (the executor
will service them from previous-sample/previous-block storage — increment 2+); `remove_node`
returns the detached edges so journal/undo layers above never have to reconstruct them;
per-node `latency_samples` is stored now so task 5's path accounting is a kernel-level
computation later. Declared absent in the module docs, per the WO's own task list: buffer pool +
channel negotiation (2), executor loop (3), RCU swap + the 10 000-mutation stress test (4),
latency accounting (5), watchdog (6), determinism harness (7). **Measured in sandbox:** 12 new
tests — order correctness, determinism, cache-recompute discipline, cycle refusal with path +
version untouched, delay self-loop legal / plain self-loop refused, duplicate refusal, 200-node
/399-edge order at the acceptance scale, id stability, error-message remedies — **430 passed /
0 failed** workspace-wide, clippy clean, fmt clean, 4 python gates clean, **no new unsafe** (the
kernel's allowlisted surface is unchanged). **Device-dependent:** nothing — this increment is
hermetic by design, which is the point of doing it while the studio waits.

**Environment/design notes (not defects, recorded so the choices are legible):** the layout audit
registers a node body as class L **only when its on-screen shorter side actually clears 72 px** —
below that a node is navigated or marquee-selected, not offered as a discrete touch target, so
registering it would either fail the gate honestly at low zoom or lie about touchability; port
captures are zoom-invariant (24 px screen) so they always clear class S (`docs/ui/gestures.md` §5).
The `ui-window` clippy/build cell needs `debuginfo=0` + single job in the 1 GB sandbox (`naga` OOMs
otherwise) — SATURN and CI use the repo profile unchanged.

**WO-013 increment 2 — the bridge: drawn patches render (2026-09-24, sandbox-built):** the canvas
stopped being a picture of a patch and became the patch. `sparq-app/src/bridge.rs` — deliberately
**not** `ui`-gated, because it touches no egui type (canvas model + registry + executor + WAV), so
`cargo test --workspace` on the default CI path exercises manifest → registry → factory → kernel
graph → executor → samples → file, the whole chain. What it does: resolves the **master** — the
node the listener hears — by explicit designation (long-press a node → SET MASTER; the painter
stamps a MASTER badge, in words) or the documented default rule (*highest-id node with an audio
output and at least one wire, preferring a terminus*; unwired spare modules are excluded so a
render can never silently pick a silent spare — three graph shapes unit-tested). Every canvas node
is mapped through the registry — #58's invariant re-checked at the last gate before sound: a
module the canvas shows but the registry lacks refuses in words — wires become `Plain` kernel
edges, the WO-008 executor builds (its refusals, including the v0 `AudioCtx`'s audio-only payload
rule, pass through verbatim into the shell log), and a stated **5.0 s @48 kHz/64/2ch f32** renders
to `canvas-render.wav` (gitignored) with an evidence line: master name, blocks, bytes, memory
budget, FNV-1a hash. The empty-canvas menu gains **RENDER WAV**; the demo graph is now
**registry-driven** (`bridge::demo_graph` reads the live manifests — the last hand-copied port
list in the app is gone, the drift class WO-007 exists to prevent has one less hiding place).
Parameters are the manifest **defaults** until the inspector (increment 3) gives nodes param
state — declared in the module docs and parked in `LATER.md`, alongside render length (WO-009)
and the master handover to `out/main` (WO-014). **Measured in sandbox:** **474 passed / 0 failed /
1 ignored** (+6: 3 bridge, 3 canvas master-rule/menu tests); the bridge tests prove the demo patch
renders to a 0.5-peak sine, an uninstalled module is refused in words, and `render_wav` writes a
real 1 920 046-byte WAV with evidence; `sparq ui --audit` **PASS, 0 failures** — **16 smokes**
(6 chrome + 7 canvas + 3 breakpoint), the two new ones full-chain touch: double-tap zoom-to-fit →
long-press empty → RENDER WAV row → file on disk + evidence in the log, and long-press node → SET
MASTER → master state. Clippy clean (default, `ui`, MSVC audio-cross cells); python gates clean;
goldens **unchanged** (`ba577186c988db21`, `0f5c3e86c7f117a9`, `dd975a24f03b19c1`, and the two
WO-014 module goldens in their tests) — no audio path touched. **One test-side bug caught by the
harness, recorded as a lesson, not a product defect:** the first draft of the RENDER WAV smoke
tapped the menu row without lifting the long-press finger, and the recogniser correctly read the
row tap as a *second finger* (pinch state, no Activate) — the product was right and the test was
wrong; the smoke now releases the contact before tapping, exactly as a real finger must.
**RDP-relevant:** this whole chain needs **no audio device** — `scripts/test005.bat` (shipped in
`sync-wo013-inc2.zip`) is the SATURN acceptance: build, `modules --strict`, `exec` render, the
audit's touch-driven render, SHA-256 hashes of both WAVs, and an optional listen step if RDP audio
redirects. test004 (exclusive) still waits for the physical session.

**WO-013 increment 2b — the sync stamp, defect #78 (2026-09-25, sandbox-measured):** no product
code changed; the delivery path did. `test005`'s first run on SATURN failed at step [01] with six
rustc errors of one shape — `no method named resolve_master found for &CanvasState`, `no field
master on type CanvasState`, `no variant … named RenderWav / SetMaster` — and the compiler's own
note gave the game away: *available fields are: `camera`, `selection`, `interaction`, `menu`,
`history`*. That is **inc1's** `CanvasState`, five fields; inc2's has six, `master` among them. All
six error line numbers match the inc2 `sparq-app` files byte-for-byte, so the app crate on the stage
machine *was* the new one — it was compiling against the **previous increment's `sparq-ui`**. The log
confirms it from the other side: only `Compiling sparq-app` appears. cargo never rebuilt `sparq-ui`;
it answered *Fresh* and served the cached rlib.

Two mechanisms produce that, and a build log cannot tell them apart: **(a)** `interact.rs` never
landed — and #71 proves this tree has form: an Explorer extraction there produced a stray
`decode (2).rs` *instead of overwriting*; **(b)** it landed and cargo did not look at it, because zip
extraction restores the *archive's* timestamps, so a freshly-synced file can be **older** than the
cached build. (b) is defect #41's exact mechanism in a place #41's remedy does not cover: #41/#49
were about a stale **binary** passing a run, so `build.bat` grew a stamp guard that inspects the exe
*after a successful build*. A stale **dependency rlib** fails the compile instead — the build never
reaches the guard. Detector and remedy were both on the wrong side of the failure.

**(b) reproduced and measured in the sandbox** (release, 2 cores): build `sparq-ui` from inc1's
`interact.rs` → **1.28 s**, rlib written; put inc2's file in place carrying the archive mtime →
`cargo build --release -p sparq-ui -v` answers **`Fresh sparq-ui` in 0.06 s**, and that rlib contains
`resolve_master` **0 times**; delete `target\release\.fingerprint\sparq-*` and rebuild → **4.20 s**,
and the rlib contains it **6 times**. The content changed and cargo did not look. (0.06 s beside a
verified-stale artefact is the same signature #49 recorded.)

**The fix is two-sided, and the detecting side is content, not timestamps.**

* **`SYNC-STAMP.txt` + `tools/sync_check.py`** (new): sha256 + byte size for every file that decides
  what the binary is — `crates/*/src/**/*.rs`, each `build.rs`, every `Cargo.toml`, `modules/**` (the
  manifests `modules --strict` and the registry-driven demo graph read at *runtime*), `scripts/*.bat`,
  `tools/*.py`, the three toolchain configs — **120 files**. Documents are excluded on purpose: a stale
  `.md` cannot break a build, and 400 KB of prose would bury the one line that matters. `build.bat`
  runs it **before** cargo and refuses to build on a mismatch, so mechanism (a) arrives as a sentence
  naming the file instead of six errors from a crate that is not the one that is wrong. `Cargo.lock`
  is **SOFT** (cargo rewrites it for its own reasons; worth reading, not worth blocking on) and a
  CRLF-only difference is a **warning** (rustc does not care, and the binary's own stamp stays
  self-consistent because `build.rs` and `:stamp_check` measure the same on-disk bytes). A file on
  disk that the stamp does not know is **EXTRA** — which is how the next `decode (2).rs` gets named
  by a gate instead of by a fingerprint arithmetic puzzle. `--self-test` proves the gate failable —
  **11 cases**: the seven verdicts, and four for the freshness marker below, one of which is this
  defect's own shape (content changed, mtime set to 1970). Against the real tree it is proven the
  same way: with inc1's `interact.rs` put back in place, the check prints
  `SIZE crates/sparq-ui/src/canvas/interact.rs — disk 43962B, stamp 51583B` plus the fingerprint
  drift `tree 80f/1341350B, stamp 80f/1348971B`. CI runs that self-test; it does **not** run the
  verify mode, which would demand a regenerated stamp on every source edit — that check belongs in
  the packing step, and is parked in `LATER.md` until the packer and the stamper are one tool.
* **`build.bat` deletes the five first-party fingerprints whenever the tree's *content* changed
  since the last good build**, so (b) cannot happen at all. The decision is `sync_check.py
  --freshness`: digest the covered tree, compare it with a marker written after a successful build,
  purge on any difference — or on a missing marker, or when python is missing, because "cannot
  check" must never mean "assume fresh". Content, not timestamps: a file whose bytes changed while
  its mtime went *backwards* — which is exactly what a zip extraction produces — reads as PURGE, and
  the self-test pins that case with the mtime set to 1970. Only the five `sparq-*` crates are purged:
  `gates.bat` deletes the whole `.fingerprint` tree, which is right for a gate table and wrong here,
  because `run.bat`/`demo.bat`/`devices.bat` all come through `build.bat` and an unchanged tree must
  still cost seconds. Measured cost of a purge: `sparq-ui` 4.20 s in the sandbox; on SATURN the five
  crates in release are a few minutes, dependencies untouched.
* **`scripts\synccheck.bat`** (new) runs the check alone into `logs\synccheck.log` — seconds, no
  cargo — and **`test005.bat` gains step [00b]**, so the per-file verdict travels in the one log that
  comes back. `build.bat --skip-sync-check` is the documented escape hatch for a deliberate local
  edit; a missing `python` is a loud **cannot-run**, never a pass.
* The `fp` line mirrors `build.rs` and `build.bat`'s `:stamp_check` — **three** copies of that
  definition now. That is #68's lesson written down where it will be seen: change one, change all
  three.

Not a product change: no `.rs` under the five `src` roots moved, the stamp stays
**`src 80f/1348971B`**, the goldens are untouched, the test count is unchanged at **474**.

**WO-013 increment 3 — the browser, the inspector, wire re-patch (2026-09-25, sandbox-built):** the
three surfaces the increment-2 status line named, built in the established order — *logic* first in
the toolkit-independent crate with tests, *drawing* last behind the feature. **Live wire levels
stay parked** (they need the WO-008 executor's analysis taps; faking them from the canvas's own
data would be a lie in motion).

* **`sparq-ui::canvas::model` gains per-node param state.** `NodeSpec.params: Vec<ParamDesc>` — a
  *validated view* of the manifest's `params[]` in the same discipline `ports` holds (the `kind` is
  the contract's own `ParamKind`, re-exported, never copied), built by `NodeSpec::from_manifest`,
  which is now THE registry→spec constructor (`demo_graph` and the browser catalogue both go
  through it). `Node.param_values` starts materialised from the spec defaults; nodes restored from
  pre-increment-3 history (empty vector) read as defaults and materialise on first edit.
  **`Op::SetParam`** is invertible like every op, and the clamp lives in `Graph::op_set_param` —
  one range/stepping rule for every edit path (float clamps, int snaps to whole steps, bool snaps
  to poles, `enum`/`text`/`blob` refuse), so history never holds an out-of-range value.
* **`sparq-ui::canvas::browser` (new): fuzzy search + browser state + sheet geometry.** The scorer
  is a deterministic case-insensitive in-order subsequence: **contiguity (+6/char) outranks word
  starts (+4) outranks earliness (−first index)**; ties break by field (display name > module id >
  category) then catalogue position — same catalogue + query ⇒ same ranking, which is what makes
  the rows auditable. The catalogue is *input*: the shell supplies `BrowserItem`s built from the
  registry (`bridge::browser_catalog`, sorted by id), so the browser **cannot offer a module that
  is not installed** — #58's rule, structural, at the third consumer. Rows are 56 px
  (`touch.row_height_browser`, the token that existed for exactly this), one page with
  selection-scrolling, sheet clamped into the view by the menu's own `clamp_origin`. `caps()` is
  the ONE size contract shared by hit-test, painter and smokes.
* **`sparq-ui::canvas::inspector` (new): computed param-panel geometry.** Title + one 44 px row
  per param (label | track | value); the drawn track is thin but the **touch target is the full
  row** — the port-capture trick applied to sliders. `value_from_x`/`knob_x` round-trip; rows
  clipped below the panel are **not touchable** (an invisible control cannot be hit by accident;
  scrolling is parked in `LATER.md`); `value_text` renders every kind in words ("440.00 Hz",
  "ON"/"OFF", "Enum · v1" for the not-editable-in-v0 kinds).
* **Wire-end re-patch.** `WireLayout` gains `grab_from`/`grab_to`: screen points 32 px
  (port-capture 24 + one space step) along the bézier from each port — clear of the port's own
  capture, so *"draw a new wire from this port"* and *"move this wire's end"* are two
  distinguishable touches; on a wire shorter than 2× the offset both converge to the midpoint and
  the source end wins (documented at the constant). `Hit::WireEnd` sits between ports and bodies
  in the hit order and is LOD-gated like ports. The drag detaches one end (`Interaction::Repatch`,
  fixed end stays home, magnet + glow/dim via `connect::preview` against the MOVING end); on drop
  the old wire is removed **first**, then `resolve` judges — so cycle, duplicate and single-input
  replacement all see the post-re-patch graph. Success stores `Batch[RemoveWire, <connect ops>]`
  (one undo restores the original wire, **id included**); refusal / cancel / empty-drop re-applies
  the inverse — the drag was a question, and "no" changes nothing. Wrong-direction drops are
  refused in words naming which end lives where.
* **`interact`: the intent→op table grows three rows, refusals stay verbal.** Empty-canvas menu
  gains **ADD MODULE** (first row; disabled *with the reason* when the catalogue is empty). The
  browser is modal over the canvas like the menu: row tap spawns at the press (grid-snapped,
  cascading off an occupied spot so two spawns never stack), header tap points at the text entry
  in words, outside tap closes; a no-match query **keeps the sheet open** and its single NO-MATCH
  row slot is tappable so the refusal arrives in words instead of silence. Slider gestures:
  `Activate` on a track tap-to-sets; `DragStart`+updates edit continuously with **one drag = one
  history entry** (mid-drag updates coalesce into the open entry, keeping its ORIGINAL `from` so
  one undo restores the value the finger *found*). The shell routes inspector-rect drags to the
  canvas (`route_to_canvas` grows the row-hit case) and pipes keystrokes while the sheet is modal
  (`feed_browser_keys` reads egui **input events** — no widget, no focus policy; the wrap-egui
  rule holds. Headless drivers call `browser_set_query` directly.).
* **`sparq-app`: paint + bridge.** `bridge::build` renders **node param state**; a never-touched
  node renders at its manifest defaults, so untouched patches are bit-identical to increment 2 —
  and the goldens prove it rather than the doc claiming it. The painter draws the browser sheet
  (query header with caret + match count in words, ranked rows name/id + summary·category, the
  selection worn as a filled band + accent bar), the inspector (track, filled run, knob, value
  text; non-editable kinds greyed), the re-patch drag (detached end wears an open warning ring at
  the cursor) and the wire-end rings **exactly where the layout says they are targetable** — what
  you can touch is what you see, at every LOD that has them. Inspector rows and browser rows
  register into the same audit list as everything else (class S / M).
* **Caught in-development by the new tests, fixed before shipping** (no defect numbers — they
  never left the sandbox): (1) the slider coalescing replaced the whole top op *including its
  `from`*, so undo would have restored the drag's first waypoint instead of the pre-gesture value
  — the merge now keeps the original `from`; (2) the browser's `height()` reserved the NO-MATCH
  row but `hit()` did not cover it, so tapping the only visible row closed the sheet silently —
  hit and height now agree by construction.

**Measured in sandbox:** **519 tests** (was 474; +45: model params 7, browser 16, inspector 7, layout 3,
interact 10, bridge 2); clippy clean in the default, `ui`,
`bootstrap-audio` and **MSVC × 3** cells; the native `ui-window`/gles cell is clean at `-j 1` with
dev debuginfo off — the "naga OOM" RESUME recorded is **parallel-job pressure at 1 GB**, not naga
itself (single-job rustc fits); MSVC × `ui-window` remains unrunnable anywhere (the `windows`
crate's rustc is OOM-killed at 1 GB; environment, not code — CI has no such cell either).
`sparq ui --audit` **PASS, 0 failures** — 40 matrix cells + DPI invariance unchanged (Design still
audits 24 targets with nothing selected; inspector rows register only when the panel shows a
selection) + **19 smokes** (16 + 3 new: *ADD MODULE → fuzzy query → row tap spawns the module*,
*inslider drag edits the param and ONE undo restores it — graph AND param state*, *drag a wire end
→ it re-patches; one undo restores the original wire, same id*). Goldens **unchanged**: release
golden tests pass (`ba577186c988db21` matching), `selftest --golden` **8/8** with determinism
`0f5c3e86c7f117a9` identical to the ledger, 525.9× realtime (relaxed sandbox codegen — SATURN
builds with the repo profile), 0 allocations, reopen-leak 0; `sparq modules --strict` clean,
`sparq exec` PASS with the cross-validated rms tap (0.16621882). 4 Python gates clean.
**Not claimed (device-only):** 60 fps at 200 nodes, real-finger browser/inspector/re-patch, palm
rejection while re-patching, the DPI matrix — none claimable over RDP; `test006` runs them.
Artefacts: `sparq-ui::canvas::{browser,inspector}`, `docs/ui/gestures.md` (§2 rows, §3b, §4b, §4c).

**WO-006 increment 1.3 — the exclusive device-period ladder, defect #79 (2026-09-25, sandbox-built):**
`test004`'s first acceptance attempt (2026-09-24 21:36, SATURN physical, inc-1.2 build
`src 75f/1226586B`) split cleanly in two. **The caps checkpoint PASSED** — with the four-rung
format ladder the Behringer endpoints list `exclusive 96000` instead of `exclusive none`, so #77's
fix is verified on the device that raised it. **Exclusive playback then failed on every rung** with
one code: `Initialize (exclusive): HRESULT 0x88890020` = `AUDCLNT_E_INVALID_DEVICE_PERIOD`. The
format ladder was innocent; the **period** was the lie. The open asked `IAudioClient::Initialize`
for the sparq **block** as the exclusive **device period** — 64 fr ÷ 96 kHz = **666.7 µs** — while
the endpoint's `GetDevicePeriod` reports default = minimum = **10 ms** (960 fr): the number the
caps line had been printing as `hw period 10.000 ms` since increment 1.1. `IsFormatSupported` takes
no period, so no format probe could ever see this — only `Initialize` tells the truth, and on
hardware it did.

**The fix asks the driver, and the asking is pure data.** New `sparq-kernel::hal::period`
(UNGATED — the WASAPI module is `cfg(windows)` and this project is developed on Linux, so the
decision that failed on the device now unit-tests on the development machine; the null backend's
discipline applied to a negotiation): `exclusive_period_ladder(block_frames, rate, hw_default,
hw_min)` returns the candidate periods in 100 ns units — **the block period clamped up to the
driver minimum first** (on endpoints whose minimum fits the block, the ask is byte-identical to
inc 1.2's, so the fix costs low-latency devices nothing), **the driver default second** (for the
documented lying-`min` pattern), deduplicated; a default below the reported minimum is skipped,
and the raw block period is never re-asked after a clamp (a driver that said no does not get
asked twice — the #38/#46 lesson). On the UMC 204HD min = default = 10 ms, so the ladder
**collapses to a single candidate: the driver's own number**. `open_exclusive` executes the list:
a FRESH `IAudioClient` per candidate (a failed `Initialize` consumes it), the documented
`BUFFER_SIZE_NOT_ALIGNED` two-step per candidate, `INVALID_DEVICE_PERIOD` advances the ladder,
any other HRESULT aborts early (busy/invalidated is not a period problem), and total refusal
carries the **full period probe table** plus the endpoint's reported default/minimum — the #38/#46
rule kept for the second negotiation axis. `hr_text` now names `0x88890020` (windows-sys 0.61
exports the constant; checked against the SDK value in the vendored source). The pump needed **no
change**: the FIFO already decouples device period from sparq block (*Period ≠ block*), and the
2 h shared soak of 09-24 ran exactly this ratio (10 ms period, 64-fr blocks, 10 798 597 blocks,
0 xruns) — the latency report and open log keep reading the TRUE period from `GetBufferSize`, so
no number is invented. `test004.bat`'s header, [04] expectation text and failure HINT now name the
period axis (text-only edits, ASCII/CRLF/paren rules honoured).

**Defects WO-006 increment 1.3 logs:**

| # | Defect | Caught by |
|---|---|---|
| 79 | **Exclusive `Initialize` refused at every format rung with `AUDCLNT_E_INVALID_DEVICE_PERIOD` (0x88890020).** The open passed the sparq block period (64 fr = 666.7 µs @ 96 kHz) as the exclusive device period; the UMC 204HD's engine runs at 10 ms — the value its own caps line printed via `GetDevicePeriod` since inc 1.1. Unreachable by the format ladder (#77) because `IsFormatSupported` takes no period. The block↔period FIFO decoupling already existed and was soak-proven at this exact ratio in shared mode; only the exclusive open's *asking* was missing. Fixed in inc 1.3: the ladder in `hal/period.rs` (pure data, Linux-tested — the failing device's shape is now a unit test that lands on a single 10 ms candidate), fresh client + alignment two-step per candidate, early abort on non-period HRESULTs, full probe table on total refusal | `test004` on SATURN (2026-09-24, `test004.log` line `HRESULT 0x88890020`), root-caused by arithmetic in the sandbox: 64 ÷ 96 000 = 666.7 µs vs `hw period 10.000 ms` — recorded in `docs/hal/windows-notes.md` §4c |

**Measured in sandbox:** **526 tests** (was 519; +7 in `hal/period.rs`, ALL RUNNING ON LINUX —
including `the_defect_79_device_gets_its_own_period_on_the_first_and_only_try`, the exact
test004 shape: 64 fr @ 96 kHz, default = min = 10 ms ⇒ ladder `[10 ms]`); fmt clean; clippy clean
in the default workspace cell and **both MSVC `hal-wasapi` cross-lint cells** (kernel + app,
`-D warnings`, fresh re-check after touch — the only compiler the WASAPI change meets, as in
inc 1/1.2, so the wasapi.rs rewiring is compile-verified, not run-verified); goldens untouched
(the offline path never sees a device period); HAL null conformance unchanged; 4 Python gates
clean. **Hardware acceptance is unchanged in shape and closer in fact: the `test004` re-run** —
caps, the exclusive tone (the open line must name its rung AND a ~10 ms period with the 64-fr
block beneath it), unplug-in-exclusive, and the **2 h zero-xrun soak at 96 kHz/64 exclusive**.
When it passes: WO-006 acceptance closes and ADR-008's exit condition fires (cpal bootstrap
deleted, HAL becomes `play`'s default). Artefacts: `sparq-kernel::hal::period`,
`docs/hal/windows-notes.md` §4c, `WO006-INC13-RUN-SHEET.md`.

**WO-012 increment 4 — the MTA audio-control thread, the mouse, the master meters, the palette tiles (2026-09-30, sandbox-built):**
the operator's first PLAY on SATURN refused with the sentence this increment answers: *this
thread already initialised COM as single-threaded (STA); the WASAPI HAL needs MTA* — the
winit UI thread can never host the HAL's persistent per-thread MTA, so the session now spawns
ONE dedicated audio-control thread per session: the only thread that touches the backend
(enumerate → probe → open → start → stop → drop, pump/fault/capture on the manual null), over a
request-reply protocol with bounded waits (a driver that owns its thread over 5 s is a device
fault said in words, not a frozen shell); the UI thread keeps the `SharedEngine` control half,
the drain, and a health mirror the worker polls at 50 ms; `Drop` sends Stop and joins — a
session that leaked its thread would leak the device. The same increment carries the operator's
three other asks: **mouse usability** (unpressed motion dropped at the adapter — the suppression
line can now only mean a real missed Down; right-click synthesises the recogniser's `Context`;
the wheel synthesises `Pan` and reuses the panel-over-panel routing — no parallel semantics, no
modifier keys); **`out/main` meter bars like the mockup** (`MeterUpdate` grew `peak_l`/`peak_r`
in the same cache-warm publish pass — mono ports duplicate and the cross-thread bit-identity
pin grew to cover them; green DATA-class fills, amber AUDIO-class peak-hold blocks decaying on
block counts; empty wells at rest); **the palette** (dock tiles 112×56 through the token
generator, grouped by manifest top category under xs words, left stripe in the dominant signal
class — §4's rule, not accent-by-category; all seventeen tiles fit at 1920). Plan
`WO012-INC4-PLAN.md`; deviations and the mouse/meter rulings are mockup-review findings 15–17.

**Measured in sandbox:** **777 tests** (+2 behind `ui`: back-to-back sessions prove the worker
teardown, drop-without-stop proves `Drop`) · **37 smokes** (+3: right-click menu, wheel routing
+ hover silence, master bars from the live ring) · matrix 0 violations at the new element
counts (tiles are class M; the 44 px floor holds) · fmt clean · clippy clean in every runnable
cell incl. native `ui-window` (the window adapter changed) and MSVC×6 — the increment owed the
disallowed-`Mutex` lint a reasoned allow (control-path mirror, the defect-#66 precedent),
`checked_div` for the channel count, and a boxed command variant for the enum-size lint · 5
python gates · release goldens bit-identical (stress `7bb06379bd6845e5` debug AND release, the
pinned exec renders, `canvas-render.wav` `d7ad294e…`, determinism `0f5c3e86c7f117a9`) ·
selftest 9/9 · `modules --strict` 17/17 · `probe_alloc` 0/5 000 · stamp `src 94f/2188595B`.
**Hardware acceptance is one tap: PLAY makes sound (test006 J).** Artefacts:
`WO012-INC4-PLAN.md`, the reworked `ui/live.rs`, `window.rs`'s adapter gate.

**WO-012 increment 3 — mockup conformance, chrome: the shell draws like `design-mode.svg` (2026-09-29, sandbox-built):**
the operator applied all waiting bundles on SATURN and then asked for the interface itself:
*"I would like it looking like the design-mode.svg."* Slice A of the operator-approved two
(increment 4 = node inset displays + the inspector response plot). Plan of record
`WO012-INC3-PLAN.md`; the rail question went to the operator (the mockup's icon-only rail
violates look-board §8, and finding 3's tooltip remedy violates §8's hover-only ban on a touch
instrument): **glyph + permanent micro-label** inside the mockup's own 44 px box. **What
converged, measured against the mockup with a NEW instrument** — `sparq ui --svg-out PATH`
dumps the headless frame's vector shapes to SVG (a screenshot without a GPU; the review
protocol's first repeatable visual comparison; first sheet committed at
`design/mockups/convergence-wo012-inc3.png`): the rail (glyphs + words, hairline groups, the
foot's live level tick in the AUDIO class colour and the magenta ADD cross opening the browser
at canvas centre through a new driver door), the top bar (logo glyph, section dividers, the
review's "top-bar diagnostics" line — negotiated kHz · frames · xruns · swaps while live, the
build stamp at rest — status dot paired with LIVE), the wire-encoding legend top-right (drawn
by the canvas painter from the wires' OWN `class_encoding` table — legend and wires cannot
drift), the dock (MODULES tab REAL: the registry catalogue as 248×80 class-dot cards — the
review's touch table — tap spawns at canvas centre through the browser's op path, undoable;
overflow counted in words; four tabs stay honestly disabled), the inspector (port-dot summary
strip — rings in, filled out, manifest order — and control-cyan sliders with the mockup's block
thumbs), `shell::compute`'s column discipline (rail and inspector FULL HEIGHT, the dock in the
canvas column — the mockup's geometry, pinned by the moved layout test), and the demo patch
spread left-to-right like the mockup's story. Declared deviations (mockup-review findings
9–14): inspector default stays 400 (480 would force-collapse at tablet-min), top-bar keeps
word buttons (§6), the level tick is amber not green (§4), wires at rest stay dim ("silence
looks silent" — the mockup depicts a live patch). Two new tokens through the generator
(`dock_card_w/h`, the review's touch-table numbers).

**Measured in sandbox:** **777 tests** (unchanged — chrome moves no audio path) · **34 smokes**
(+2 chrome: dock-card tap spawns and one undo removes; ADD opens the browser, outside tap
cancels) · the breakpoint matrix grew with the cards (33–42 elements per Design cell, **0
violations**, 44 px floor holding, 3 dense) · fmt clean · clippy clean in every runnable cell ·
5 python gates — `token_audit` R6 caught the SVG dumper's page-ground hex on its first pass;
the dumper now reads `COLOR_GROUND_BASE.hex` · release goldens bit-identical (stress
`7bb06379bd6845e5` debug AND release, the three pinned exec renders, `canvas-render.wav`
1 920 046 B · `d7ad294e…`, determinism `0f5c3e86c7f117a9`) · selftest 9/9 · `modules --strict`
17/17 · `probe_alloc` 0/5 000 · stamp `src 94f/2155888B` · log_check BASELINE moved with the
seal. Artefacts: `WO012-INC3-PLAN.md`, the convergence sheet, `mockup-review.md` findings 9–14.

**WO-013 increment 6 — the scope screen: `dsp/scope` draws from the analysis ring (2026-09-29, sandbox-built):**
the follow-on WO-012 increment 2 named, chosen by the operator as the next build: the analysis
ring has published waveforms since WO-014 inc 5 and block-rate cv since inc 2 — now something
DRAWS them. The plan of record is `WO013-INC6-PLAN.md` (twelve decisions before the code; its
postscript records the correction the smoke caught — D3′: the prescribed per-frame SWAP of a
canvas-owned trace field alternated two sets and blanked alternate frames under manual pumping;
the shipped design has the session as the SINGLE SOURCE, read by the painter through a
`canvas_ui::draw` parameter, with an empty set at rest — a dead stream's signal has nowhere to
linger). **What shipped:** `sparq-ui::canvas::scope` — the toolkit-independent model: rolling
`TraceBuf` sized `timebase × the negotiated rate` (whole blocks, clamped to `MAX_TRACE`
65 536 — 500 ms fits at every HAL rate), a resize that ZOOMS (retains the tail) rather than
resets, rising-edge trigger with the declared free-run fallback (a blank screen pretending to
be a bug is refused), stride-decimated polyline geometry (≤ one point per pixel column; the
min/max envelope refinement is declared parked), X/Y pairing over the shorter axis, NaN
sanitised at the geometry boundary, and param clamping that does not trust its input. The
manifest's contract, honoured literally: **the wire is the binding** (a scope's x/y inputs
resolve per frame to the SOURCE `(node, port)` whose `AnalysisUpdate` they display — impossible
to stale; a rebind clears the traces so no dead tail survives), **the ring is the payload** (the
drain feeds bound traces from the SAME updates the cv wire levels read — one pass), the params
configure the DISPLAY (read by index — the order pinned by a drift-gate test — and clamped by
the model), the UI owns the buffer (session-owned display state, not project state), and
`Scope::process` stays the no-op it shipped as: zero audio-thread cost is structural, not an
optimisation. The painter draws the `scope.trace` map's own row — amber trace on `ground.inset`,
`hairline.faint` crosshair — in the WIRES' glow vocabulary (class-colour→glow lerp by the
window peak + the under-glow pass at the wires' alpha rule; the motion-token ^1.5 law and
phosphor persistence are declared parked for one data-first refinement increment that will move
wires and traces together). The well draws at Full AND Simplified (a scope reads as a scope at
a glance); the TRACE is Full-only (the Simplified contract is text-free and an unreadable trace
is decoration); Dot is untouched. Unbound or at rest: the flat rest line — "a flat line, not a
crash". No layout move: the display rides the EXISTING node box (the display-sheet's larger
tiles are a layout-token increment, parked), steals no gestures, adds no audit elements.

**Measured in sandbox:** **777 tests** (was 766; +11 scope-model gates) plus 3 session gates and
2 audit smokes behind `ui` — **32 smokes total** (the trace fills signed at amplitude from the
bound tap while the unbound spare stays flat; the LOD contract measured in SHAPE counts — this
egui tessellates a frame into ONE clipped primitive, so primitives cannot be the metric) · fmt
clean · clippy clean in every runnable cell (the increment owed `div_ceil` in three places —
MSRV-legal, clippy caught it) · 5 python gates clean (R6 accepted the painter: tokens only, the
one COUNT documented as a count) · release goldens bit-identical (the three pinned exec renders,
stress `7bb06379bd6845e5` debug AND release, `canvas-render.wav` 1 920 046 B · `d7ad294e…`) ·
selftest 9/9 · `modules --strict` 17/17 (no manifest moved) · `probe_alloc` 0/5 000 · stamp
`src 94f/2122410B`. **Hardware acceptance:** none new — the scope rides test006's steps J–K
window session (the trace animates on screen while the patch plays); the sandbox proves the
whole path hermetically on the manual null. Artefacts: `sparq-ui::canvas::scope`,
`WO013-INC6-PLAN.md`, the painter's `draw_scope_display`.

**WO-012 increment 2 — the live audio session: the HAL stream through `SharedEngine`, continuous meters (2026-09-29, sandbox-built):**
the FIRST UX increment of the operator's session-6 pivot ("start fleshing out the interface and
start building patches and sounds"), re-sequenced ahead of WO-006's exclusive acceptance because
shared mode is the device-proven path and the ADR-008 exit — not live audio — is what the
acceptance gates. The plan of record is `WO012-INC2-PLAN.md` (fifteen decisions before the code,
settling every open question checklist item 6 listed; its postscript records the one decision the
CODE corrected: `stage` never refuses — it supersedes and counts — so D8's retry flag does not
exist and the visible refusal is the BUILD's, in the executor's own words). **What shipped:** the
shell's PLAY/STOP/PANIC stubs became a real `LiveSession` (`sparq-app/src/ui/live.rs`): a
two-phase open (probe → build the executor for the NEGOTIATED config through the new
`bridge::build_with_map_at` → real open) with a callback that owns the `AudioEngine` and does
`render_block` and nothing else; the one op→sync door (`CanvasState::take_patch_changes` — an
exhaustive per-`Op` classification: param edits cross as `set_params` commands with ZERO
re-stages, structural edits and master handovers rebuild and stage at the boundary, moves and
renames are inaudible and mark nothing); the bounded per-frame drain that fills
`CanvasState.levels` from `read_meters` + `read_analysis` (the continuous half of live wire
levels — the painter path was done; this swapped the SOURCE); the ring extension it rides
(`MeterUpdate` grew `port` — one entry per audio OUTPUT port via `Executor::with_audio_out`, a
`FOLDED` sentinel for cv-only nodes so their status still crosses, single-output nodes
bit-identical to the folded reading they replace, pinned against an offline reference executor;
and block-rate cv outputs publish as one-sample analysis waveforms so a cv wire's LIVE level
reads the increment-5 magnitude rule); the honesty lines in the house shape (NEGOTIATED rate ·
ch · block, latency, and a STOP evidence line of measured counters — blocks, xruns, overruns,
callback allocations, swaps, cmds, ring drops, reclamations); and the failure discipline —
`Removed`/`Failed` ends the session in ONE line with the canvas untouched, teardown drops the
engine on the control thread after `reclaim()` drains. Backend choice: the audit and tests run
the null backend in MANUAL mode pumped by the smoke (hermetic, deterministic, `capture_frames`
proving real samples); a Windows window gets `wasapi-shared`; anywhere else gets the paced null
WITH a log line saying so. Two documented driver doors joined the headless convention
(`param_edit`, `connect_ports` — the same surface `browser_set_query`/`rename_set_text`
established). RENDER WAV did not move; the bootstrap did not move (ADR-008 gated); the executor's
semantics did not move (stress hash unchanged).

**Measured in sandbox:** **766 tests** (was 762; +4: the per-port ring gate, the block-rate
analysis gate, the ledger gate, the config-door delegation pin — plus 5 `live.rs` session tests
and 5 audit smokes behind the `ui` feature) · fmt clean · clippy clean in the default, `ui`,
`bootstrap-audio`, combined audio+hal and native `ui-window` (single-job) cells and all six
runnable MSVC cells · 5 python gates clean · `selftest --golden` **PASS (9 gates)**, golden
`ba577186c988db21`, determinism `0f5c3e86c7f117a9` · `ui --audit` **PASS, 30 smokes** (25 +
the five live-session ones) · stress `7bb06379bd6845e5` · 10 001 blocks · 2 383 swaps ·
7 617 refused · 0 allocations, debug AND release bit-identical · the cross-thread ledger holds
(2 299 staged = swaps = reclaimed, alloc 0) · `canvas-render.wav` bit-identical (1 920 046 B ·
sha256 `d7ad294e…`) · the three pinned exec renders bit-identical (`f2303f13aa0cf299`,
`1621e1f65b1b64e1`, `53de3b1f3f40e3c9`) · `modules --strict` 17/17 · `probe_alloc` 0 across
5 000 blocks · throughput 262.8× realtime (1 min at 96 kHz/64) · stamp `src 93f/2081612B`.
**Hardware acceptance (test006 steps J–K, in the bundle):** PLAY audible on `wasapi-shared` at
the negotiated latency with the wires animating WHILE it plays; a live slider edit heard without
stopping; the STOP evidence line; optionally an unplug mid-play → the Removed line and an
untouched canvas. Artefacts: `crates/sparq-app/src/ui/live.rs`, `WO012-INC2-PLAN.md`,
`scripts/test006.bat` steps J–K.

**WO-008 increment 7 — host-side `required`-unconnected enforcement, the declared baseline moved ON PURPOSE (2026-09-29, sandbox-built):**
the sandbox-track item the checklist named "the next candidate in this list" — small,
well-specified, declared in LATER.md §WO-008 since increment 3, and deliberately its own
increment because it is the one change that moves the stress baseline. The plan of record is
`WO008-INC7-PLAN.md` (eight decisions, recorded before the code). **What shipped:**
`Executor::build` refuses a patch in which any node has a `required` input port with zero
incoming edges — a new `ExecError::RequiredUnconnected { node, module, port }` rendered as ONE
sentence naming both remedies (connect a source, or remove the node; a genuinely optional port
is a manifest `required = false` — a module change, not a patch change). The gauntlet order puts
it after the edge-rules pass and before buffer allocation: an ILLEGAL wire is a more specific
defect than a MISSING one, and the check needs the resolved-edge picture. Graph order ×
manifest order — first violation wins, deterministic like every refusal in the build. The rule's
scope is the compiled contract's own words: `Port::required` = "whether an unconnected INPUT is
an error" — outputs are out (a sinkless `out/main`, the demo's undrawn `rms.level` wire, stay
legal), and one or more incoming carried edges satisfies a port (audio fan-in legal, event
multi-source legal, cv fan-in already refused upstream by the matrix rules — the check can never
legitimise a wiring the matrix refuses). Until now the field was decoded, defaulted (`true`),
validated — and consumed by NOBODY; the seventeen manifests already spoke the vocabulary
deliberately (the six single-input processors required; every matrix, display, sink and trigger
input explicitly optional), so NO manifest moved and `modules --strict` stays 17/17.
**The two declared dependents moved with it:** (1) the determinism world's unwired spare gain is
now WIRED — from the chain gain's output, not the sine, on purpose: the spare is the world's
initial master (highest-id audio-out), so every retune mutation reaches the rendered samples
and the script's hash stays sensitive to parameter edits; `the_no_mutation_script_is_the_
reference_chain` pins the new legal shape (4 nodes, 3 edges). (2) The canvas demo patch's fourth
node became a bare **`out/main`** — the one shipped module DESIGNED to sit bare (optional
input): the smokes keep their free stereo input to drag onto, `resolve_master`'s documented
handover rule ignores the unwired out/main (the master stays the chain gain), and
`canvas-render.wav` is bit-identical BY CONSTRUCTION (same master, ×1.0 path) — measured
**1 920 046 bytes**, the device baseline's exact size, sha256 `d7ad294ea0b5e6e9…` recorded for
future A/Bs. The wire-level gates gained an honest COLD subject (the bare out/main renders
`Silenced` and reads 0.0 — a level invented from canvas data could not tell "unwired" from
"silent signal", the metered one can).
**The unconnected-signal semantics survive at their declared home — OPTIONAL ports:**
`an_unconnected_input_is_silenced_and_reported` migrated in place (its probe now declares
`required = false`, which is where the behaviour legally lives), the probe manifests' audio
inputs all declare optionality (the suite run was the census: exactly 8 tests touched the
enforcement, all migrated or reworked in place), and the new gate
`a_required_input_with_no_wire_is_refused_in_words` proves the refusal sentence AND its
compliant twin (same patch + one wire → builds). `cross_thread.rs`'s status pin TIGHTENED to
`Ok` only: a bare required-input module cannot be built any more, so `Silenced` in the meter
publication would mean the enforcement leaked.
**The baseline moved on purpose and is recorded** (LATER.md's condition for this increment):
**stress `7bb06379bd6845e5` — 10 001 blocks · 2 383 swaps · 7 617 refused · 0 audio-path
allocations · 2 nodes / 0 edges final · max path latency 94 — debug AND release bit-identical**
(was `b42068ec7b206789` · 7 203 swaps · 2 797 refused). The mix flipped because add/disconnect
mutations that bare a required input are now build refusals — the refusal path (leave no trace,
live patch renders on) is exercised 2.7× harder, the swap machinery still cycles 2 383 times
under churn (acceptance floor: > 1 000), and every invariant assertion is rate-independent of
the baseline by design. The cross-thread ledger moved with it (21 509 blocks · 2 299 staged =
swaps = reclaimed · 7 701 refused · 0 allocs · 0 errors). Historical entries keep the old hash
as history; the live citations (CHECKLIST baselines, LATER.md, `cross_thread.rs`'s doc) moved.
**Docs moved in the same session (#88's lesson):** module-api-v1.md's "still declared open"
list loses the item (the comparability reason it was open is discharged by the recorded
baseline), the field-table row and the schema row grow the host-enforcement clause, the
executor header's v0 bullet is replaced by the enforcement sentence. Parked beside the
watchdog→UI hairline: a canvas badge for the bare-required node (the refusal reaches the shell
log at RENDER time; drawing the warning at DROP time is a WO-013-side painter increment).

**Defects WO-008 increment 7 logs:**

| # | Defect | Caught by |
|---|---|---|
| 93 | **The sandbox workspace silently drops any directory named `out` at message boundaries — and `modules/out/main/sparqmod.toml` is stamp-covered.** Mid-increment, `sync_check` reported the file MISSING (the snapshot exclusion list treats `out/` as a build directory; `.git` and `target/` and the `/opt` toolchain drop at the same boundaries — those are documented, this one is a PRODUCT FILE). Upstream recovery failed: GitHub 404s on the path (the repo was rebuilt/`daeda8c` or went private mid-session — the same class as defect #86, "the pushed tree can drift from the sealed one", possibly the same trap on the packaging side). The operator supplied the file; it was restored byte-exact and hash-verified against the sealed stamp (`725bf07a…`, 2 691 B). Remedies in place: a durable recovery copy at the workspace root's `sparq-recovery/modules-out-main-sparqmod.toml`, a session-start discipline in the CHECKLIST environment notes (if `sync_check` says MISSING for that path, restore from the recovery copy and verify the hash), and this row. The device tree is unaffected (bundles never deleted it; SATURN's own `synccheck.bat` would name it if it ever went missing there) | `sync_check --quiet` during increment 7 (2026-09-29), which is exactly what the stamp exists for |

**Measured in sandbox:** **762 tests** (was 761; +1 the refusal gate; the silenced-report test
migrated in place, the reference-chain pin moved in place), 0 failed, 1 ignored; fmt clean;
clippy clean in the default workspace, `bootstrap-audio`, `ui`, native `ui-window`/gles (`-j 1`)
and all six runnable MSVC cells; release goldens **bit-identical** — the three pinned exec
renders at their durations (`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`), the
wo005 + phase-b manifests, every module golden (enforcement only ADDS refusals; no render path
moved, and the demo patches were read and are compliant — their only undrawn wires are
outputs); `selftest --golden` PASS (9 gates); `ui --audit` **PASS (25 smokes)** with the demo's
new fourth node; null conformance 9/9; 5 python gates + `sync_check --self-test` 11/11 +
`log_digest --self-test` 16/16 + `log_check --self-test` 25/25 (BASELINE moved with the seal:
762 / the new stamp, #83's discipline). Artefacts: `WO008-INC7-PLAN.md`, the executor's
`RequiredUnconnected`, the wired determinism world, the out/main demo node.

**WO-006 increment 1.5 — the exclusive rate envelope is its own probe, and drift reads delivered-vs-wall, defects #91 + #76 after test004 attempt 3 (2026-09-29, sandbox-built):**
`test004` attempt 3 (2026-09-28 18:47, SATURN over RDP, inc-1.4 build — its build line proves
`sync wo006-inc14` applied: `151 files match`, `src 92f/2001521B`) **confirmed the #89 open-path
fix and falsified its remaining assumption.** The open line read exactly the predicted pass
shape — `i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr`: the
default-first ladder asked, the driver granted, no shrink note — and the stream STILL came back
`NOT CLEAN` with attempt 2's triple at the new period: **158 late wakes / 10 s (~16 stalls/s,
each ~40 ms — ~3 swallowed cadences), half throughput (7756 blocks ≈ 49.4k fr/s against a
96 kHz negotiation), jitter avg 19.44 / max 40.36 ms over 517 wakes, drift −491 161 ppm, no
clean tone** — again with 0 budget overruns and 0 FIFO starvations, and again with the same
session's shared runs green (unplug → `Removed`, recovery, the rehearsal soak, all three
conformance suites). The arithmetic (§4e of the windows-notes, in #76's style) turns the two
attempts into one identity: **delivered frames = wakes × buffer**, exact to one block on both
(517 × 960 ≈ 496 384; 1718 × 288 ≈ 494 848) — because event-driven exclusive legally CANNOT
bank more than one period (`IAudioClient::Initialize` requires `hnsPeriodicity` =
`hnsBufferDuration` under `EVENTCALLBACK`, enforced with `AUDCLNT_E_BUFDURATION_PERIOD_NOT_EQUAL`),
so the pump's per-wake write is capped at one buffer and every swallowed cadence is audio the
device never receives. The stall source (~16 Hz, ~33–40 ms wall) did not move between a 3 ms and
a 10 ms period: **it is period-independent (defect #92, open, device-side)** — while the shared
engine on the same endpoint runs 2112 fr of buffer over ~960 fr ticks (2.2× headroom) and soaked
2 h clean. And the log's own `adjusted rate 48000 Hz -> 96000 Hz` line exposed **defect #91**:
the 96 kHz was never the operator's or play's ask — `capabilities()` sieved the exclusive rate
sweep through the SHARED-supported list, and this endpoint's engine refused every shared f32
probe except at its own 96 kHz mix, so the driver — a "UMC 204HD **192k**" — was never asked
whether it does 48 kHz exclusive. Play's adjustment then consulted the same shared envelope and
bent the 48 kHz request UP into the one rate the exclusive path had proven pathological at.
**The fixes, each sandbox-verifiable:**

1. **The exclusive sweep is its own probe** (`wasapi.rs::capabilities`): every `PROBE_RATES`
   rate plus the mix rate, four rungs each, asked of the DRIVER — the shared sieve is gone, and
   `Capabilities.exclusive_rates`' doc carries the rule (each envelope has its own arbiter:
   shared asks the engine, exclusive asks the driver; `Initialize` remains the final one).
2. **Adjustment consults the envelope the open negotiates in** — new pure helper
   `Capabilities::exclusive_rate_for` (`hal/mod.rs`, Linux-pinned): nearest verified exclusive
   rate, ties to the LOWER one (the conservative clock), empty envelope → `default_rate` so the
   open ladder still refuses honestly with its full probe table instead of a manufactured rate.
   `play` and `soak` both route their `wasapi-exclusive` adjustment through it. Attempt 4's [04]
   asks 48000 and — if the driver lists it — keeps it: expected open line
   `48000 Hz · 2 ch · i24-in-32 (converting) · device period 480 fr (10.00 ms)`.
3. **#76 SHIPPED: drift is delivered frames vs wall.** The WASAPI pump counts the frames the
   device ACCEPTED (every successful `ReleaseBuffer`, preroll included) and feeds
   `record_device_clock` with that count — the `IAudioClock` binding is DELETED (vtable, IID,
   `GetService` call), with a note where it was so nobody re-adds it trusting the old story:
   its `GetPosition` advanced in buffer-sized steps per event tick (+1.2 M ppm on a soaked-clean
   shared stream, §4b). §4b's fixture requirement is honoured in the tests — both recorded
   healthy sessions read ≈ 0 ppm under the new derivation
   (`the_defect_76_shared_session_reads_zero_under_the_new_derivation`), attempt 3's shape reads
   the −49 % half-throughput verdict it is (`the_attempt_3_exclusive_shape_reads_as_half_throughput`),
   and the null backend (which always counted delivered blocks) now shares ONE metric meaning
   across backends. The SUSPECT text's falsified "implausible clock (virtual/RDP endpoint)"
   attribution is replaced by what the number now means, and `drift_suspect` waits for a 2 s
   trust window — one period of counting quantisation at 96 kHz/10 ms is ±5 000 ppm over 2 s,
   half the threshold; the arithmetic is `DRIFT_TRUST_NS`'s doc
   (`the_suspect_verdict_waits_for_the_trust_window`).
4. **The verdicts act on the measurement**: `play`'s pass line requires `!drift_suspect` (and
   the NOT CLEAN line says so when that is the reason) and `soak` fail-fasts on it — a stream at
   half its negotiated rate is not a passing stream even with quiet cadence counters.
5. **`test004.bat` is current for attempt 4**: [03] expects a MULTI-rate exclusive list, [04]
   expects the 48 kHz open with NO `adjusted` line (and says what its return would mean), the
   hints speak #92's signature words with drift-as-measurement, and [07] prompts for the soak
   rate — default **48000**, the rate [04] proves, with **96000** offered as the explicit
   stretch the WO text named. **The acceptance wording amendment is PROPOSED, not silent**: the
   WO's "2 h soak at 96 kHz" predates #91 (the lone-rate envelope was a probe bug) and #92 (the
   exclusive event path stalls at 96 kHz at every period tried); the run sheet states the
   amendment — *the 2 h zero-xrun exclusive soak at the rate the device proves* — for the next
   session to ratify or strike against attempt-4 evidence.

Parked and named, not hidden: **push-mode exclusive** is #92's remaining lever if attempt 4
comes back rate-independent (periodicity 0, no `EVENTCALLBACK`, buffer 2–4 periods, self-paced
pump — the shape the audio engine itself runs on this endpoint; LATER.md carries the design
burden: pump pacing discipline, ADR-004's "event-driven" wording, late-wake semantics when the
expected cadence is policy). An automatic reopen-on-stall stays parked with the same note:
attempt 3 proved reopening within the period ladder buys nothing while the stall is
period-independent.

**Defects WO-006 increment 1.5 logs:**

| # | Defect | Caught by |
|---|---|---|
| 91 | **The exclusive rate envelope was probed through the shared sieve, and play adjusted inside the wrong envelope.** `capabilities()` asked the exclusive rungs only at rates the shared f32 probes accepted — on SATURN's UMC 204HD that left `exclusive 96000` printed as measured truth while 44.1–192 kHz were never asked of the driver; `play`/`soak` then consulted `supports_rate` (the shared list) even for exclusive streams and bent the standing 48 kHz ask UP to 96 kHz — the one rate the exclusive path proved pathological at. Fixed in inc 1.5: the exclusive sweep asks the driver at every standard rate, and `exclusive_rate_for` (pure, pinned) keeps exclusive adjustments inside the exclusive envelope | `test004` attempt 3's own `adjusted rate 48000 Hz -> 96000 Hz` line read against its caps block — recorded in `docs/hal/windows-notes.md` §4e |
| 92 | **The exclusive event-stall phenomenon is period-independent** (OPEN, device-side): ~16 stalls/s, ~33–40 ms wall each, identical at a 3 ms and a 10 ms period at 96 kHz, on a machine whose shared engine soaks 2 h clean in the same sessions; event-exclusive's API-mandated buffer ≡ periodicity means zero bankable headroom, and delivered = wakes × buffer measured exactly — every swallowed cadence is lost audio. Not fixable from the sandbox: inc 1.5 removes the forced 96 kHz (attempt 4 discriminates rate-coupling) and ships the instrument (#76) that makes the digest self-diagnosing; the named remaining lever is push mode (LATER.md) | `test004` attempts 2+3 on SATURN (2026-09-28, `test004-digest.txt` [04] sections), joined by the wakes×buffer arithmetic in `docs/hal/windows-notes.md` §4d/§4e |

**Measured in sandbox:** **761 tests** (was 757; +3 `diag.rs` fixtures — the trust window, the
attempt-3 replay, the #76 regression pair — and +1 `hal/mod.rs` gate for `exclusive_rate_for`;
the `implausible_drift` test REPLACED IN PLACE by `implausible_throughput_is_labelled_suspect_not_hidden`,
which now asserts the new wording and can never assert the falsified one again), 0 failed,
1 ignored; fmt clean; clippy clean in the default, `bootstrap-audio`, native `ui`, native
`ui-window` (gles, `-j 1`) and all six runnable MSVC cells (module-api, music, kernel,
kernel×`hal-wasapi`, app, app×`hal-wasapi` — the WASAPI change is compile-verified, not
run-verified, as in every WASAPI increment); 5 python gates clean + `sync_check --self-test`
11/11 + `log_digest --self-test` 16/16; release goldens re-verified — the three pinned exec
evidence renders bit-identical at their durations (`1621e1f65b1b64e1` / `f2303f13aa0cf299` /
`53de3b1f3f40e3c9`; inc 1.5 touches no render path), `selftest --golden` PASS (9 gates),
`ui --audit` PASS (25 smokes), null conformance 9/9, and the paced-null play/soak smokes read
**+0.23 / −0.62 ppm drift — the new derivation at ≈0 on a healthy stream, live** (their 1–2 late
wakes are the documented Linux-scheduler simulation artefact, §3 of the windows-notes).
**Hardware acceptance is the `test004` re-run, attempt 4**: [03] multi-rate exclusive caps, [04]
the clean 48 kHz tone with no `adjusted` line, unplug-in-exclusive → `Removed`, and the 2 h
zero-xrun exclusive soak at the rate [04] proves (the proposed acceptance amendment — run sheet
§2). Artefacts: `docs/hal/windows-notes.md` §4e, `WO006-INC15-RUN-SHEET.md`, the deleted
`IAudioClock` binding's epitaph in `wasapi.rs`.

**WO-006 increment 1.4 — the default-first period ladder and the compact log digest, defect #89 (2026-09-28, sandbox-built):**
`test004` attempt 2 (2026-09-28 09:01, SATURN, inc-1.3 build under the applied
`sync wo008-inc6` chain — the build line's `sync_check: OK - 149 files` confirms both waiting
bundles landed) **opened WASAPI exclusive for the first time**: `i24-in-32 (converting)` at
96 kHz on the UMC 204HD — #77's format ladder and #79's period asking both did their job. And
then the stream stalled: the open landed at **288 fr (3.00 ms)**, the driver's reported minimum /
alignment granularity rather than the 10 ms the run sheet expected, and the 10 s tone came back
`NOT CLEAN` — **159 late wakes, half throughput (7732 blocks ≈ 49.1k fr/s against a 96 kHz
negotiation), jitter min 600 ns / avg 5.84 ms / max 33.24 ms, drift −491 656 ppm, tone not
heard**. The arithmetic (§4d of the windows-notes, in #76's style): 1718 wakes split into ~1559
at ~3.0 ms plus ~159 at ~33.2 ms — the device ran at full 96 kHz roughly half the wall time and
starved the other half, one ~33 ms stall every ~62.5 ms. Zero budget overruns and zero
FIFO-starvation writes: the pump kept every promise it could see. The control experiment ran in
the same session: the **shared 10 ms engine on the same endpoint soaked 2 h, 719 895 wakes,
0 xruns, max jitter 12.04 ms** — not a machine-wide storm, not the pump: the 3 ms exclusive period
the driver *accepted* and cannot *sustain*. **Defect #89: a third lying-`min` face** — #79's
driver refuses under-minimum asks at `Initialize` (honest, recoverable); the documented lying-min
refuses its own minimum (recoverable); this driver ACCEPTS the period and misbehaves at runtime,
where no open-path probe can see it. The attempt-1 log copy is truncated, so which door produced
the 3 ms ask (a `GetDevicePeriod` minimum now reading 3 ms, or the alignment two-step adopting a
288-fr granularity against a 10 ms ask) cannot be pinned from the evidence — **the fix closes all
three doors**, each as pure data in `hal/period.rs` where Linux unit-tests it:

1. **Default-first for coarse engines.** `exclusive_period_ladder` reorders: when the reported
   minimum sits ABOVE the sparq block period — a driver saying it needs an engine coarser than
   the block — its minimum is not a promise, and its **default period (the number its engine
   actually runs) becomes rung 1**, the min-clamped ask demoted to fallback. Sub-block engines
   keep inc 1.2/1.3's honest low-latency ask first (a liar there refuses at `Initialize`, which
   the ladder survives — #79's own shape, and four of its seven unit pins, unchanged). On the
   UMC's shape (default 10 ms, min 3 ms) the ladder is now `[10 ms, 3 ms]` — pinned by
   `the_defect_89_device_asks_its_default_before_its_min`, the test that inc 1.3's
   `a_minimum_above_the_block_clamps_the_ask_up` asserted the WRONG way round against the same
   device shape.
2. **The two-step rounds UP.** On `BUFFER_SIZE_NOT_ALIGNED`, `GetBufferSize` reports the
   driver's alignment granularity; inc 1.3 re-asked the granularity itself (the documented
   recipe's literal reading — and a way a 10 ms ask could silently become 3 ms). Now
   `align_up_frames` rounds the ORIGINAL ask up to whole granularity units: 960 fr ask, 288 fr
   granularity → **1152 fr = 12.000 ms**, aligned AND default-class. Sub-granularity asks still
   land on one unit — inc 1.2's two-step behaviour, unchanged, pinned.
3. **The silent-shrink guard.** A driver that ACCEPTS an ask and then allocates seriously less
   (< ¾ of it, `allocation_seriously_shrunk`) is reporting its granularity through the
   allocation: round up and re-ask ONCE on a fresh client; if the retry refuses, keep the
   shrunken open and print the mismatch — the open line grows `(3.00 ms, ask 960 fr)` so a
   shrunken negotiation can never be read as a clean one. `Negotiated` carries `ask_frames`
   (exclusive only; the shared engine picking its own buffer is documented behaviour, not a
   mismatch).

Expected attempt-3 open line: **`device period 960 fr (10.00 ms)`** — or **`1152 fr (12.00 ms)`**
if the driver enforces its 288 fr alignment at `Initialize`. Both are the driver's own engine
class; the 2 h shared soak says the machine sustains it. The pump needed no change (Period ≠
block; the FIFO absorbs any period the ladder lands on). **Defect #90 (the acceptance script's
own honesty):** attempt 2's summary printed `exclusive still refused — the [04] probe table … is
the diagnosis` when exclusive had OPENED and run rough, and the banner still announced
"increment 1.2" — the script that grades the acceptance misdiagnosed it in the log the operator
sends back. `test004.bat` now distinguishes refused from opened-but-not-clean (`EXCL_OPENED` via
`findstr` on the captured output), its hints name #89's runtime signature, and its expectation
text names both legal periods. **And the operator's context-window ask, shipped in the same
bundle:** `tools/log_digest.py` (+ `scripts\digest.bat`, + end-of-run hooks in `test004.bat`,
`test006.bat`, `gates.bat`) writes `NAME-digest.log` beside every device log — every line
byte-identical except three collapsed classes: `[t+ Ns]` periodic runs (first 2 + last 2 kept,
the middle becomes one count line carrying the blocks/xrun ranges), cargo build chatter, and runs
of identical lines. Attempt 2's 67 569 B log digests to 30 994 B (−54 %, 235 soak reports
collapsed); `--self-test` (16 checks) proves every class fires and every verdict line survives.
The full log stays on disk; **the digest is what travels.**

**Defects WO-006 increment 1.4 logs:**

| # | Defect | Caught by |
|---|---|---|
| 89 | **The exclusive stream opened at the driver's 3 ms minimum/granularity and stalled ~33 ms every ~62.5 ms** — 159 late wakes / 10 s, half throughput (49.1k fr/s vs 96 kHz), drift −49 %, no clean tone — while the same endpoint's shared 10 ms engine soak-ran 2 h clean in the same session. A third lying-`min` face: the driver ACCEPTS the period at `Initialize` and cannot sustain it at runtime, where no open-path check can see it. Fixed in inc 1.4 by closing all three doors the 3 ms ask could have come through: the default-first ladder for coarse engines, the round-UP alignment two-step, and the silent-shrink guard with the honest open line | `test004` attempt 2 on SATURN (2026-09-28, `test004.log` [04]/[07]/[08] sections), root-caused by arithmetic in the sandbox — recorded in `docs/hal/windows-notes.md` §4d |
| 90 | **`test004.bat`'s summary misdiagnosed attempt 2 in the log it asked the operator to send back:** it printed "exclusive still refused — the [04] probe table is the diagnosis" when exclusive had OPENED and run rough (`EXCL_OK` keyed off rc alone), and its banner still announced "increment 1.2" two increments later. Fixed in inc 1.4: `EXCL_OPENED` captured via `findstr` on the [04] output, refused vs opened-but-not-clean summary lines, hints naming #89's runtime signature, expectation text naming both legal periods (960 fr / 1152 fr) | the attempt-2 log's own SUMMARY block, read against its [04] section |

**Measured in sandbox:** **757 tests** (was 752; +6 `hal/period.rs` unit gates, 1 replaced in
place — ALL RUNNING ON LINUX, including the exact attempt-2 device shape), 0 failed, 1 ignored;
fmt clean; clippy clean in the default workspace cell and **both MSVC `hal-wasapi` cross-lint
cells** (kernel + app, `-D warnings` — the only compiler the WASAPI change meets, so the
`open_exclusive` rewiring is compile-verified, not run-verified, as in every WASAPI increment);
5 python gates clean + `log_digest --self-test` 16/16 + `log_check --self-test` 25/25 (BASELINE
moved with the seal, #83's discipline); goldens untouched **by construction** — `hal/period.rs`
feeds only the `cfg(windows)` exclusive open, and no render path moved. **Hardware acceptance is
unchanged in shape and closer in fact: the `test004` re-run** — the open line must read 960 fr
(10.00 ms) or 1152 fr (12.00 ms), the tone must be clean, unplug-in-exclusive must end `Removed`,
and the **2 h zero-xrun soak at 96 kHz/64 exclusive** closes WO-006 and fires ADR-008's exit.
Artefacts: `sparq-kernel::hal::period`, `docs/hal/windows-notes.md` §4d,
`WO006-INC14-RUN-SHEET.md`, `tools/log_digest.py`, `scripts/digest.bat`.

**WO-008 increment 3 — tasks 4–7: the swap, the latency, the watchdog, the harness (2026-09-25, sandbox-built):**
the four tasks the increment-2 status line left open, shipped as one increment because they form
one argument: *mutation is safe, latency is known, stallers are isolated, and all three are
reproducible*. The fifth remaining item — multi-port `AudioCtx` (contract v1) — stays declared
absent: non-audio edges are still REFUSED at build in words, and a contract change deserves its
own increment, not a rider.

* **Task 4 — atomic mutation + the stress test.** New `sparq-audio::engine::Engine`: one live
  executor, one staged successor, and a single meeting point — the block boundary. Staging adopts
  the live transport clock and block count (`Executor::inherit_runtime`: the timeline belongs to
  the stream, not the patch); the swap is `Option::take` at the top of `render_block`, and the
  retired executor drops at exactly that point (`ExecNode::Drop` deactivates its modules) —
  deterministic retirement, because in the single-owner shape nothing else can be inside the old
  graph. Module STATE is deliberately not carried (state transfer is the state protocol's job,
  WO-011; declared in the docs). A swap that drops the master refuses **in words** (`NoSuchNode`),
  never renders silence. **The honest scope, declared where it lives:** ADR-009 d3's literal
  cross-thread pointer swap needs an epoch-retirement primitive (allowlisted-`unsafe` kernel
  work, its own increment and proofs — parked in `LATER.md`); until then the boundary semantics
  are proven by interleaving, which is exactly what the acceptance's stress test measures:
  **`tests/mutation_stress.rs` — 10 000 seeded random mutations while rendering** (add / remove /
  connect / disconnect / retune / re-declare latency, each rebuilt control-side and swapped at
  one boundary): **10 001 blocks, 7 203 swaps, 2 797 refusals** — every refusal left no trace and
  the render never stopped — **0 allocations** across every rendered block (the counting
  allocator armed per block, defect #66's serialization discipline), 0 superseded stages, and
  the whole run takes ~1.2 s debug / 0.23 s release, so it gates every commit.
* **Task 5 — latency accounting + the raw/compensated switch.** The kernel graph grows the
  per-path map (`latency_map` / `path_latency` / `max_path_latency`): own declared latency plus
  the max over incoming arms, computed in topo order, cached on (version, **latency-edit count**,
  block_frames) — the extra key because `set_latency` is a data edit that deliberately does not
  bump the version, and a cache that serves the number it just watched change is worse than no
  cache. `UnitDelay` arms contribute 1 sample; `BlockDelay` arms are feedback and contribute
  nothing static (a loop's circulation delay is the loop's sound — documented, not averaged
  away). Three hand-computed reference graphs pin the acceptance criterion (chain 0+128+7=135;
  diamond 10+max(64,0)=74; unit-delay 5+1=6 at any block size). The switch: `LatencyMode::Raw`
  (default — byte-identical to increment 2, which the goldens pin) reports; `Compensated` aligns
  **per fan-in edge** — each plain arm into a summed input is delayed `slowest_arm − own_arm`
  through a pre-allocated integer-sample ring, so parallel paths arrive together instead of
  flamming, with **no global pipeline offset invented** (single-input nodes and chains get zero
  and render byte-identical under the switch — tested). The proof test is arithmetic against a
  module that physically does what it declares: impulse → {true 128-sample delay | thru} → sum;
  raw peaks 1.0 at blocks 0 and 2, compensated peaks **2.0 at block 2** and zero before — exactly
  the hand-computed alignment, and the delay lines show up in the printed memory budget (d4).
* **Task 6 — the watchdog.** N **consecutive** `Overrun` blocks (default 3, `Watchdog::disabled()`
  = count-only) auto-bypass the module: it is never called again (a staller cannot stall the
  block), its output becomes passthrough when shapes match and silence otherwise, the bypass is
  journaled (`WatchdogEvent {node, block}` in a bounded, build-time-reserved log — audio-path
  pushes never allocate; overflow is COUNTED as dropped, never grown), and the rest of the graph
  keeps playing (tested end to end: dc → stall → thru goes silent, bypasses at block 2, and the
  master hears the dc again from the next block). Any non-overrun block resets the streak —
  flaky is not stalled (tested). `clear_auto_bypass` re-arms from the control thread. Timing
  detection stays with the HAL pump, which owns the clock (`Instant::now` is a clippy-denied
  audio-path call); the executor acts on the status the infrastructure reports — the two halves
  are one watchdog, and the selftest's HAL gate proves the measuring half.
* **Task 7 — the determinism harness.** `sparq_audio::determinism`: a script is a seed and a
  length; the harness plays it against the registry through the SAME engine the stress uses and
  returns the FNV-1a hash of every sample plus the counters (blocks, swaps, refusals, alloc
  violations, final graph shape, max path latency). Same seed ⇒ same schedule ⇒ same graphs ⇒
  same swaps at the same boundaries ⇒ same hash — asserted twice per commit, and the 1 000-
  mutation replay runs allocation-gated. **It earned its keep on day one:** the first stress
  replay FAILED — the retune arm indexed a `HashMap`'s iteration order (per-map random), so the
  same seed chose different gains on each run. Fixed by walking the graph's node Vec (every
  collection the RNG indexes into must be deterministically ordered), and the lesson is recorded
  here because the harness is the reason it is a caught bug instead of a stage-day mystery.

**Measured in sandbox:** **549 tests** (was 526; +23: kernel latency 6, engine 4, determinism 5,
executor watchdog/compensation 6, stress binary 2) · fmt clean · clippy clean in the default,
`ui`, `bootstrap-audio` and MSVC×3 cells · release goldens **unchanged** (`ba577186c988db21`,
phase-b, module goldens) and `selftest --golden` **8/8** with determinism `0f5c3e86c7f117a9`
identical — the executor grew fields and the default path did not move a sample · `sparq ui
--audit` PASS, 0 failures, 19 smokes (no UI change; re-run because the bridge's `ExecConfig`
literal moved) · the 10 000-mutation stress hash is **bit-identical across debug and release**
(`b42068ec7b206789`) — ADR-007's portability claim, measured on the mutation path too · 4 Python
gates clean. **Not claimed:** the loaded soak (200 modules, 30 min, zero xruns) is the WO's
hardware acceptance and stays device-track; the cross-thread swap is park-and-declared until the
kernel primitive lands. Artefacts: `sparq-audio::{engine,determinism}`,
`sparq-kernel::graph::{latency_map,path_latency,max_path_latency}`, `tests/mutation_stress.rs`.

**WO-014 increment 2 — the audio-domain batch: six modules, measured (2026-09-26, sandbox-built):**
the module set triples — **9 first-party modules** — with the six whose ports the v0 executor can
actually carry: **`syn/noise`** (seeded coloured noise, five colours, 8-byte seed state, `reset`
message replays the stream — §5.6's reproducibility promise, tested), **`syn/polyblep`** (the
stable Appendix-B id kept — ids are forever, §12 — with the implementation the honest one per
defect #12: truncated additive synthesis, bandlimited by construction; the manifest header says
which is which, in words), **`flt/svf`** (the ZDF trapezoidal SVF whose defect #11 test pinned
"never silently a 1 Hz lowpass", five modes), **`util/delay`** (fractional line, damped feedback,
dry/wet, **`latency = "param:time"`** — the first parametric latency declaration in the set: the
wet path delays by `time`, the dry by zero, and a mixed delay has no single static number, which
is exactly what the parametric form exists for), **`fx/bitcrush`** (depth + rate reduction,
*deliberate aliasing as a feature*, seeded dither separate per channel), **`util/panner`** (mono→
stereo, equal-power and linear laws — the gains computed once per block, not per sample). Each is
a contract-conforming wrapper of a Phase-B primitive that already earned its measurements; each
ships manifest-on-disk + `include_str!` (the drift test is now **generic over `BUILTINS`** — a
hardcoded list was the drift it existed to catch, one meta-level up), registry factory, docs.
**The eight that did NOT ship, declared:** `syn/membrane`, `env/ad`, `mod/lfo`, `mod/clk-div`,
`util/mixer`, `ana/tap`, `dsp/scope`, `out/main` are event/cv/multi-port/display domain — they
wait for the multi-port `AudioCtx` (contract v1) and WO-009's clocks, because a module whose
trigger port cannot receive a trigger is a lie with a manifest. The v0 parameter discipline is
stated once for the batch: params read once per block; `process` allocation-free **at steady
params**; a param *change* may recompute tables (polyblep's partial rebuild) — allocation-free
transitions arrive with contract v1's control-rate design.

**The acceptance item that shipped with this batch: `syn/polyblep` aliasing measured through the
registry-built module** — frame-exact f0 (340 periods in 16 384 samples at 48 kHz, Phase B's
method with defect #13's leakage lesson baked in), FFT, content near Nyquist vs the fundamental:
**−166.8 dB** against the WO's −60 dB requirement. The number is printed by the test, into the
log, every run — measured, not inherited.

**Also in this increment: the docs generator** (`tools/module_docs.py`, the "docs for all modules
are generated from manifests (no hand-written duplicates)" acceptance): `docs/modules/*.md` (9)
are rendered from the same TOML bytes discovery and `include_str!` read — ports, params (with the
snapshot-order note), parametric latency spelled out, state schema, resources. `--check` is a
**new CI/gates stage** (wired into `justfile`, `ci.yml` and `scripts\gates.bat`), proven failable
before shipping (a hand-edit to `syn-sine.md` reads `STALE` and fails the gate), and
`check_text_io` caught its own Windows-console portability gap (defect #37's class) before the
first commit — the streams are reconfigured UTF-8 like every other tool.

**Measured in sandbox:** **559 tests** (was 549; +10: 9 in `tests/modules_batch2.rs` — six
checked-in golden renders (`44ac30042f0233a9`, `bc74dec4272b67a1`, `460cf54e6913ec41`,
`22ef1d97904b1ff5`, `cdd6232510945825`, `7ef79b30a2c7c469`) plus the full-chain golden
(`9170415cd3852736`: noise→svf→delay→bitcrush→panner in one executor graph, rendered twice
bit-identical, 256 blocks allocation-free under the counting allocator — and 5 000-process-call
zero-allocation gates per module; the delay echo landing at exactly sample 4800 = its declaration,
decaying through the damping one-pole within measured bounds; the panner's laws at their
documented curve values; +1 `set_phase` round-trip in `dsp/osc.rs`, the state-restore half the
polyblep module's 8-byte phase blob needs) · fmt clean · clippy clean in the default, `ui`,
`bootstrap-audio` and MSVC×3 cells · **goldens UNCHANGED** (`ba577186c988db21`,
`0f5c3e86c7f117a9`, `d46736fd9a1c48a1` — the demo patch renders identically with a registry three
times its size) · `selftest --golden` 8/8 · `sparq ui --audit` PASS, 19 smokes (the browser smoke
still finds exactly one "gain" — the six new names/ids/categories were checked against the fuzzy
scorer before shipping) · `sparq modules --strict` loads **9/9** from disk · 6 Python gates clean
incl. the new `module_docs --check`. **Not claimed (device-side):** the <15 %-of-a-core benchmark
at 96 kHz/64 on the stage machine (sandbox CPU ratios do not transfer — the ledger's standing
rule); per-module example patches wait for WO-015 (the study IS the example patch, and a
`.sparq` project needs WO-011's format writer). Artefacts: `modules/{syn/noise,syn/polyblep,
flt/svf,util/delay,fx/bitcrush,util/panner}/sparqmod.toml`, `docs/modules/*.md`,
`tools/module_docs.py`, `tests/modules_batch2.rs`.

**WO-008 increment 4 — contract v1: the multi-port `AudioCtx`, cv/event payloads travel, and the
rms→filter acceptance is arithmetic (2026-09-26, sandbox-built):** the increment the CHECKLIST
called "THE unblocking increment" — and per the increment-3 note, a contract change got its own
increment, not a rider. Four pieces, one argument: *the manifest's port list is now the truth the
engine presents.*

* **The contract (`sparq-module-api`).** `AudioCtx` v1: per-type port views in manifest order —
  `audio_in/audio_out` (≤ `MAX_PORTS_PER_CLASS` = 8 per class, validation-enforced as
  `E-CROSS-FIELD:ports`, the `MAX_PARAMS` pattern), `cv_in/cv_out` (`CvIn::Block(f32)` /
  `CvIn::Audio(&[f32])` / `CvIn::Unconnected` — always at the RECEIVER's declared rate),
  `events_in/events_out` (pre-sorted `&[Event]` in, bounded `EventSink` out). The v0 field pair
  survives as methods over the first audio port (`input()`/`output()`/`has_input()`), so the nine
  shipped modules migrated mechanically. Two shapes carried the design: input views escape with
  the context's lifetime (the documented two-step — input first, output second — compiles; the
  reversed order does not), and output views come in reborrowing AND `take_*` variants, because
  two `&mut self` accessors cannot coexist but two TAKEN views can — that is the multi-output
  module pattern (`util/mixer`'s shape), pinned in `api_snapshot.rs` and exercised by the
  `MultiProc` test module. New `sparq_module_api::event`: `Event { kind, sample, channel, value,
  words[4] }` over the closed six-dialect `EventKind`, `EventBuf`/`EventSink` at
  `EVENTS_PER_BLOCK` = 64 with overflow **counted, never grown** (`dropped_total` is the run's
  evidence), and `sort_by_sample` — a stable INSERTION sort, because `slice::sort_by`'s stability
  costs an allocation the audio thread may not make. `Resources` grew the per-port channel arrays
  §2's cascade rule needs. Manifest: `event_kinds` became modelled data (required on event ports,
  closed domain, `E-ENUM-UNKNOWN` per item), `cv_reduce`/`cv_interp` became typed (`CvReduce` /
  `CvInterp` — a policy the engine re-parsed from text would be a policy that can drift from the
  vocabulary), and the G3/G4 cross-field rules gained their mirrors (`cv_interp` on a block-rate
  input, either policy on an OUTPUT — the receiver owns rate changes, so an output declaring one
  is a misunderstanding worth naming).
* **The executor (`sparq-audio`).** cv/event edges EXECUTE; data/gpu/atom still refuse, now each
  with its own sentence. cv: the receiver's `cv_reduce` collapses an audio-rate source (all six
  policies, table-driven against hand-computed values on a permutation ramp — first ≠ last is the
  ADR-007 argument, measured), `cv_interp` expands a block-rate one (`linear` ramps prev→cur
  reaching cur at the NEXT boundary, prev is executor state refreshed in the wire pass; `spline`
  is REFUSED at build — the vocabulary promises it, the host has not implemented it, and holding
  instead would render differently than declared), range mismatch and fan-in refuse through
  `connect_cv` at `Phase::Zero` naming `util/range`/`util/mixer` AND their phase (never offer a
  converter that does not exist), fan-out is free. Event: `event_kinds` subset checked at build
  (the default cell's `E-EVENTKIND-UNACCEPTED` sentence), fan-in merged by a linear k-way merge
  of per-source sorted lists into build-reserved staging — ranks are host-queue first, then
  EdgeId, then insertion order (G5 made mechanical), and the two-phase sort (stable insertion
  sort per sink at its producer's dispatch, merge at the consumer) never allocates.
  `push_host_event(node, port, ev)` is the control-side door — bounded, sorted-insert, consumed
  in exactly one block; WO-009's transport will publish through the same door, and this increment
  deliberately contains no clock. Multi-port audio: per-port buffers, per-port fan-in sums and
  compensation, meters fold across a node's audio outputs (single-output nodes bit-identical to
  inc 3, which is what keeps the golden meters), the v0 convention of giving cv sources a
  1-channel AUDIO buffer is deleted — a cv payload riding in an audio buffer is a wire that lies
  about its type. New readers: `node_cv_block`, `node_cv_audio`, `node_audio_out`,
  `node_events`, `event_drops_total`.
* **The modules.** All nine migrated; two became honest: **`ana/rms`** publishes on its declared
  block-rate cv port (the output[0] hack is gone; `sparq exec`'s tap line reads the same
  `0.16621882` through `node_cv_block`), and **`flt/svf`** (0.1.0 → 0.2.0, additive) grew the
  `cutoff-mod` cv input + param 3 `mod`: `cutoff · 2^(2·mod·cv)`, clamped and **quantised to
  f32 — the parameter's own precision, deliberately**, so a hand-driven reference renders
  bit-identical. At `mod = 0` it is byte-for-byte the increment-2 filter, which every existing
  golden then proved rather than assumed.
* **The WO-014 acceptance item — rms→filter modulation — proven with arithmetic, not
  adjectives.** `tests/contract_v1.rs`: two executors run the same patch; in A the cv wire is
  live, in B it is absent and the filter's cutoff PARAMETER is set by hand, per block, to
  `(200 · 2^(2·cell)) as f32` — **200 blocks, bit-identical outputs** — so the wire carried
  exactly the declared value into exactly the declared formula. Plus the honest converse: a
  mod=0 twin proves the wire is not a no-op. The audible artefact is `sparq exec --patch
  mod-demo` (4 nodes, 4 edges, the cv line printed: `cv 0.166219 → effective cutoff 251.83
  Hz`), golden `1621e1f65b1b64e1` checked in, debug == release.
* **The compat-matrix debt (carried WO-007 → here), answered honestly.** New drift gate
  `sparq-module-api/tests/compat_matrix.rs`: parses the TOML with the crate's OWN parser and pins
  vocabularies, verdicts, adapter ids, per-type case counts and a representative outcome for
  every audio/cv `when` cell against `connect_*`. Full mirror deletion stays open WITH A NAMED
  REASON: several `when` cells are prose ("fan-out: one cv output -> many cv inputs") — shape
  rules, not pair rules — so "read at discovery" needs the ratified table restructured into
  machine predicates first (a review-packet change; `reviewed = false` is already pending).
  Recorded in the ADR-005 addendum. The gate paid for itself on first run — defects #80–#82.
* **The parser gap the gate hit immediately (defect #82's sibling, fixed):** `toml.rs` could not
  read `[[same_type.cases]]` — a nested array-of-tables under an array parent — so the matrix,
  written "as data, to be consumed by code" in WO-007, had never actually been parsed by the
  crate that ships the parser. `push_array_table` now descends into the parent array's last
  element (standard TOML), with a nesting test (each parent keeps its own children — the merge
  bug the fix could have introduced) and a standing `the_compat_matrix_itself_parses` pin.

**Defects contract v1 logs:**

| # | Defect | Caught by |
|---|---|---|
| 80 | **The compiled HOA adapter offer named the wrong module.** `Adapter::Hoa.module_id()` returned `spa/objects` for spatial ↔ non-spatial conversions — the matrix names TWO modules (`spa/hoa-encode` entering spatial, `spa/hoa-decode` leaving), and an offer that names a module which does not do the conversion is the exact failure the "never offer a converter that does not exist" rule exists to prevent. Latent since WO-007 (Phase 5 gated, so never offered on screen yet). Fixed: the variant split into `HoaEncode`/`HoaDecode` with the matrix's ids, `connect_audio` picks by direction, and — while in there — objects ↔ non-spatial stopped offering HOA at all (the table names no converter for it; it now refuses, per the rule) | the new compat-matrix drift gate, on its first run — the gate's whole reason to exist, found before it shipped |
| 81 | **The matrix's `cv→audio` adapter row was stale against its own settled decision.** The table said `syn/sine-or-offset` with "Naming to be settled"; the ADR-005 addendum (2026-09-22) settled it — `util/offset` joins Phase 1 — and `port.rs` had said `util/offset` since WO-007. The table's note invited the decision, the decision happened, the table did not move. Fixed in the table with a `decided =` line naming the gate run that caught it | the drift gate's cross-type adapter check (code id vs table id, set equality both ways) |
| 82 | **`mono → ambisonics:N` is "compatible fan-out", not an encoder insertion — in BOTH copies, consistently.** The table lists the mono fan-out case before the spatial cases and the code's match arms follow, so replicating a mono channel into an AmbiX bed is offered as a silent up-mix, which is not valid spatial audio. NOT fixed here: the two copies agree (no drift), and moving one without the other is what the gate forbids; the behaviour is pinned by `the_mono_fanout_case_precedes_the_spatial_case_in_both_copies_flagged_for_review` and flagged for the matrix review the pending `reviewed = false` owes | writing the gate's representative table — a cell that could not be represented without deciding what it means |
| 83 | **`log_check.py`'s BASELINE was frozen at WO-007 (388 tests, stamp `src 68f/1070593B`) through two sealed increments.** The CHECKLIST said the baseline "moves with" each increment's test count; nothing moved it, so SATURN's next `gates.bat` run would have hard-FAILed `stamp` and `tests_passed` against numbers three increments old — a tool that cries failure on a good run gets ignored (the defect #47 lesson, arriving anyway). Fixed: baseline now carries this increment's sealed state (614 tests + the new stamp), with the provenance comment updated | auditing the device-side baselines the CHECKLIST says move, before sealing a bundle that would have walked into the stale check |

**Measured in sandbox:** **614 tests** (was 559; +55: contract_v1 17, compat_matrix 12,
module-api validation/event/ctx 18, toml 2, executor +2 net, api_snapshot +1, port.rs +3) · fmt
clean · clippy clean in the default, `bootstrap-audio`, `ui`, native `ui-window`/gles (`-j 1`,
dev debuginfo off) and MSVC×5 cells (MSVC × `ui-window` remains unrunnable anywhere — the known
1 GB `windows`-crate OOM) · **goldens UNCHANGED and verified, not assumed**:
`ba577186c988db21` (selftest) · `0f5c3e86c7f117a9` (determinism) · `53de3b1f3f40e3c9` (exec
demo, tap value `0.16621882` now read through the cv port) · `3f325d4f99ca2a01` (sine) · the six
batch-2 module goldens + chain `9170415cd3852736` (svf at mod=0 is the inc-2 filter) · new:
mod-demo `1621e1f65b1b64e1` · **stress hash `b42068ec7b206789` UNCHANGED** (10 001 blocks ·
7 203 swaps · 2 797 refusals · 0 audio-path allocations — the determinism world never wires the
payloads that changed, which is exactly what "bit-identical" here claims) · `selftest --golden`
8/8 · `ui --audit` PASS 0 failures 19 smokes · `modules --strict` 9/9 · exec both patches ·
5 Python gates clean + module_docs regenerated (svf 0.2.0). **Not claimed:** the eight pending
modules (three — membrane, env/ad, mixer — are now unblocked and wait only on their own
increment; lfo/clk-div wait on WO-009; tap/scope on the rings; out/main on the canvas
handover), host-side `required`-input enforcement (declared v1 limit, its own increment), and
`spline` interpolation. Artefacts: `sparq-module-api::{event, module v1, manifest/decode
event_kinds, port CvReduce/CvInterp}`, `sparq-audio::executor v1`, `modules/flt/svf` 0.2.0,
`sparq_audio::modules::mod_demo_patch`, `sparq exec --patch mod-demo`,
`tests/contract_v1.rs`, `tests/compat_matrix.rs`, `tools/log_check.py` baseline.

**WO-014 increment 3 — the three modules contract v1 unblocked: membrane, env/ad, mixer
(2026-09-26, sandbox-built):** the module set reaches **twelve**, and the batch proves the
contract rather than assuming it — every module here rides a payload that did not exist yesterday,
and every claim below is a measurement in `tests/modules_batch3.rs` (12 gates), not a comment.

* **`syn/membrane`** — the Phase-B kick topology (the recipe INSIDE the phase-b demo golden:
  sine body + fixed 0.5/55 ms pitch envelope for the 909 click-then-body, gated seeded noise
  burst, amplitude AD, body lowpass) promoted to a **trigger-driven voice**. The WO's stress is
  "event-driven, sample-accurate start", and the gate is arithmetic: a trigger at sample 37
  leaves frames 0..36 at EXACT zeros and lands the hit inside frame 37's own block. Parameters:
  pitch/punch/decay/noise/damp; mono out; tail declared; `reset` replays the hit — at 96 kHz on
  purpose, because the first draft of `reset` rebuilt the voice at the constructor's 48 kHz
  reference and re-read the rate AFTERWARD, which a 48 kHz-only suite would never have caught
  (fixed pre-ship; the ordering is now a comment where the bug was).
* **`env/ad`** — contract v1 in its purest shape: an `event` input, an **audio-rate `cv` output,
  no audio ports at all**, first-class in the executor (the master render is silence; the payload
  lives on the cv port, read through `node_cv_audio`). Sample-accurate start (exact zeros before
  the trigger's frame, monotone exp attack to ≈peak by its declared 1 ms), gate-off ignored PER
  ITS MANIFEST (the module is AD, not ADSR — `AdsrEnv` exists; a gate-holding `env/adsr` is a
  later module, not a hidden mode), and `loop` free-runs without a single event. **The loop test
  earned its keep:** its first draft computed a 51 ms cycle from the parameter faces; the wrapped
  `AdEnv`'s decay segment runs to epsilon ARRIVAL (the declared ms is the five-time-constant
  point, the segment ends ≈2.3× later — the primitive's own measured semantics). The test now
  records the measured cycle and says so — the primitive's semantics won, as they should.
* **`util/mixer`** — the 4×4 stereo matrix with per-cell gains + output trims (20 of the 32
  snapshot params, which is WHY it is 4×4: declared in the manifest header, not discovered by an
  author at 3 a.m.). The first multi-port first-party module — eight audio ports, ridden through
  contract v1's `take_*` output pattern. The explicit merge the connection rules require: audio
  fan-in sums HERE, in a module the patch can see. The identity default is a **bit-exact wire**
  (the `util/gain` transparency reference, at matrix scale, golden-pinned against a bare sine
  render), and the routing gate checks the matrix against hand-computed f32 arithmetic
  (`(0.5·s + 0.25·s) · 0.5` through the f64 accumulator — dyadic, so the comparison is exact,
  not tolerant). All-unconnected inputs return `Silenced` — silence as a statement.
* **`sparq exec --patch drum-demo`** — the stack artefact: host triggers at 120 BPM (the
  control-side door, on the beat via `DrumDemoPatch::is_kick_block` — the schedule lives in ONE
  function so the command and the golden cannot drift) → membrane → mixer(in-0→out-0, trim 0.6
  for headroom: the recipe's transient peaks ≈1.5, and a demo that clips teaches the wrong
  lesson) → master. The gate locates the four kicks at EXACTLY frames 0/24000/48000/72000,
  asserts the frame before each hit is still the previous tail, the between-hits window at rest,
  and pins the golden `f2303f13aa0cf299` (2 s, debug == release, two builds bit-identical).
* **Defect #84, table-first:** `env/ad` could not declare its own `classification.top` — the
  closed TOPS domain (16 entries, identical in `manifest.rs` and `manifest-fields.toml`) has no
  `env` and no `mod`, while Appendix B's stable ids (`env/ad`, `mod/lfo`, `mod/clk-div`) and
  WO-014's own artefact paths (`modules/{...,env,mod,...}/*`) name both families. Fixed the way
  the contract says: the TABLE first (`manifest-fields.toml` domain row, with the reason inline),
  then the code (`TOPS` 16 → 18), then a **new drift pin** — `manifest-fields.toml` gains its
  first code consumer (`the_tops_vocabulary_matches_the_field_table` parses the table with the
  crate's own parser and fails if either copy moves alone; the compat-matrix gate's pattern, one
  table over). Also fixed while in there: `lib.rs`'s header referenced `tools/contract_check.py`,
  which does not exist — the sentence now names the gates that DO.
* **The executor's cv-fan-in refusal re-worded** (truth maintenance): it named `util/mixer` as
  "Phase 1, not in this build" — the module now ships, as the AUDIO matrix; the refusal says
  exactly that and points at LATER.md for the cv side. A refusal that lies about why is worse
  than no refusal.

**Defect WO-014 increment 3 logs:**

| # | Defect | Caught by |
|---|---|---|
| 84 | **`classification.top`'s closed domain was missing the `env` and `mod` families in BOTH copies** (`manifest.rs`'s `TOPS` and `manifest-fields.toml`'s domain row — 16 entries, identical), while Appendix B's stable ids (`env/ad`, `mod/lfo`, `mod/clk-div`) and WO-014's own artefact-path list (`modules/{syn,flt,env,mod,util,fx,ana,dsp,out}/*`) name them. `env/ad` literally could not declare its own top: the module that unblocking contract v1 was built to ship was refused by its own taxonomy. Fixed table-first (the table row carries the reason inline), then the code (TOPS 16 → 18), then a drift pin so the copies move together: `the_tops_vocabulary_matches_the_field_table` is `manifest-fields.toml`'s first code consumer | trying to write `modules/env/ad/sparqmod.toml` — validation refused `top = "env"` with the closed-domain sentence, which is the error message doing exactly its job: it named a vocabulary that the plan had outgrown |

**Measured in sandbox:** **627 tests** (was 614; +12 batch-3 gates, +1 field-table pin) · fmt
clean · clippy clean in the default, `bootstrap-audio`, `ui`, native `ui-window`/gles and MSVC×5
cells · **ALL goldens unchanged** (`ba577186c988db21`, `0f5c3e86c7f117a9`, `53de3b1f3f40e3c9` +
tap `0.16621882`, `3f325d4f99ca2a01`, batch-2's six + chain, mod-demo `1621e1f65b1b64e1`) ·
NEW goldens: membrane `1d1c84bd37c99b91`, drum-demo `f2303f13aa0cf299` (both debug == release) ·
stress/determinism `b42068ec7b206789` unchanged · zero allocations: 15 000 `process` calls across
the three new modules and 1000 drum-demo blocks with in-loop host-event pushes, counted ·
`selftest --golden` 8/8 · `ui --audit` PASS 0 failures 19 smokes (three new module names checked
against the browser's fuzzy scorer — the "exactly one gain" smoke still passes) ·
`sparq modules --strict` **12/12** · exec all three patches · 5 Python gates + module docs
regenerated (12). **Not claimed:** the five remaining modules (`mod/lfo`, `mod/clk-div` →
WO-009; `ana/tap`, `dsp/scope` → the rings; `out/main` → the canvas handover), mixer's cv side,
per-module example patches (WO-015/WO-011), the stage-machine benchmark. Artefacts:
`modules/{syn/membrane,env/ad,util/mixer}/sparqmod.toml`, `sparq_audio::modules`
(Membrane/EnvAd/Mixer + `drum_demo_patch`), `sparq exec --patch drum-demo`,
`tests/modules_batch3.rs`, `docs/modules/*.md` (12), TOPS 18.

**WO-009 increment 1 — clocks + transport v0: the map, the broker, the queue, and a cross-check
that says it all in one hash (2026-09-26, sandbox-built):** the WO's risk line — "clock-math bugs
that only appear at tempo automation extremes; write the continuity test before the feature" —
was taken literally: the kernel's continuity/monotonicity/bisection tests were written against
the map spec before the transport existed, and they caught the two finite-difference traps
(half-increments at ramp edges; a step anchor's jump appearing one sample LATE) inside the first
hour. The ADR-006 addendum carries the scored acceptance table; the headline:

* **Kernel `Clock` v2** — the piecewise map rule 2 specifies: anchored segments, linear bpm
  ramps (derivative continuous by INHERITANCE — a mid-ramp edit glides from the rate reached),
  quadratic closed-form integral inside ramps, `sample_at_tick` by exact integer bisection with
  nearest-rounding consistent with the forward direction, constant-segment fast path
  **bit-identical to v0** (the phase-b goldens' protection), surface kept except the declared
  `Copy` → `Clone`. A hostile 64-sample-anchor alternating sweep (step AND ramp edits) keeps the
  map strictly monotone with deltas inside the fastest tempo's envelope.
* **`sparq-music`** — the planned crate's first contents (ADR-000's "no empty crates" honoured:
  it arrived with 13 unit tests + 5 acceptance tests). `ClockBroker`: position, tempo-anchored
  edits, the wall side (windowed-ratio ppm estimator, one-pole smoothed, re-anchored every 1024
  observations and on locate; **backwards wall readings refused and counted, never applied** — a
  non-monotone OS clock is a fact about the machine, not a licence to corrupt the map).
  `Transport`: play/stop (stop FREEZES — offline tape semantics, which is what makes criterion 4
  exact rather than approximate), tempo as segments (a stream by construction), loop region
  (block-granular fold, declared), tap tempo (≤4-tap mean, 2 s memory, clamped 20..300), the
  tick-scheduled queue (absolute domain — the map is the truth, so a tempo edit MOVES a scheduled
  event to where its tick now lives, tested), bar/beat emission on the absolute grid, and
  `advance_block` → a bounded 64-slot `BlockEvents` collector, **zero allocations measured
  across 10 000 blocks** with a live schedule, beats, bars and a loop.
* **Executor integration is a door, not a dependency:** `set_musical_position((tick, ppqn))` —
  the transport computes, the executor carries, and `sparq-audio` never learned what a tempo is.
  Never-set renders the declared static `tick = 0`: the stress hash `b42068ec7b206789` and every
  golden are unchanged **by construction**, then verified. `inherit_runtime` carries the musical
  position across a boundary swap (the timeline is the stream's — task 4's own sentence).
* **The ±0 acceptance, against an independent implementation:** `sparq-music/tests/wo009.rs`
  carries its OWN segment-integral and its OWN bisection, written from the ADR's description —
  1 000 seeded ticks at constant tempo, and 1 000 across a 10 ms glide + a step, all firing at
  the test-computed samples exactly. Two implementations agreeing to the sample is the criterion;
  one implementation agreeing with itself would be theatre.
* **The cross-check:** `sparq exec --patch drum-demo` is now transport-driven (beat AND bar
  triggers, tick positions live) and hashes to **`f2303f13aa0cf299` — the identical golden the
  block-counter schedule produced in WO-014 inc 3**. Two independent trigger mechanisms, one bit
  pattern: the clocks, the event door and the executor agree about where a beat lives. The stop/
  start criterion runs through the same graph: freeze at block 500, resume at 900, kicks at
  frames [0, 24000, 73600] — arithmetic, twice, identically.
* **One defect caught by measurement before shipping (recorded in the transport header):** the
  first draft placed beats inside a loop region PROPORTIONALLY (folded domain, no map) and a
  boundary beat clamped to sample 23999 instead of 24000 — one sample early, every cycle. The
  fix is the design now declared: beats ride the absolute map grid (`sample_at_tick`, exact);
  for beat-aligned loops — the sane case — the grids coincide, and loop-relative beat phase for
  unaligned regions is a LATER.md entry instead of a silent one-sample lie.

**Measured in sandbox:** **655 tests** (was 627; +7 kernel clock, +13 music unit, +5 WO
acceptance, +1 executor door, +2 app cross-check, +7 broker) · fmt clean · clippy clean (default,
bootstrap-audio, ui, ui-window gles, MSVC×5 — the new crate is pure Rust and cross-lints free) ·
**every pre-existing golden unchanged** incl. the stress hash and `f2303f13aa0cf299` (now proven
from two directions) · `selftest --golden` 8/8 · audit PASS · exec all three patches · Python
gates clean. **Not claimed:** the 30-minute drift measurement (device), listening to a tempo
sweep (needs an editing surface), external sync (Phase 4), sub-block transport and unaligned-loop
beat phase (declared limits). Artefacts: `sparq-kernel::clock` v2, `sparq-music::{broker,
transport}`, `Executor::set_musical_position`, transport-driven `drum-demo`, ADR-006 addendum,
`tests/wo009.rs`, `sparq-app/tests/transport_drum.rs`.

**WO-014 increment 4 — the two modules the clocks unblocked: `mod/lfo`, `mod/clk-div`
(2026-09-26, sandbox-built):** the module set reaches **fourteen**, and the `mod` taxonomy
family defect #84 added to the closed domain ships its first citizens. Both ride WO-009's
machinery end to end; `clk-div` is the set's first event-PROCESSING module (events in, events
out), which is the contract-v1 wire proven from the consuming end.

* **`mod/lfo`** — four shapes (sine/tri/saw/square) mapped into 0..1, an **audio-rate unipolar
  `cv` output**, and phase reset from the `phase-reset` message OR a `sync` event at its EXACT
  sample (the membrane's onset discipline on the modulation side — the transport's beat triggers
  are the intended clock). Two decisions recorded in the manifest header where they are visible:
  **unipolar, not bipolar** — the matrix REFUSES a bipolar→unipolar wire rather than silently
  rescaling (G2), and its named converter (`util/range`) is Phase 1 and not built, so a bipolar
  port today would be a module that cannot connect to a single shipped consumer; and **sync =
  event-driven phase reset** — continuous tick-derived phase (rate in beats) needs a per-frame
  tick view or a module-side bpm derivation, declared in LATER.md rather than approximated.
  State is the 8-byte phase (the `syn/sine` promise). Gates: a saw at a **binary-exact rate**
  (48000/2^15 Hz, so every phase is a dyadic rational) lands quarter-cycle values as
  EQUALITIES and wraps on frame 32768 to the bit — the first draft used 2 Hz and its wrap
  assert was one accumulated-ulp coin-flip from failing, which is the kind of thing an
  arithmetic gate should not be; square is exactly its two levels; the sync reset is an exact
  zero ON the event's frame; the state blob round-trips phase-continuous.
* **`mod/clk-div`** — the WO's "event ports, transport as source" stress. Semantics stated in
  the manifest and pinned by exact sample lists: DIVIDE counts from the first input (outputs on
  inputs 1, 1+N, 1+2N… — a divider never swallows the downbeat of its own count); MULTIPLY
  schedules M−1 sub-triggers evenly across the interval to the next passing input, MEASURED from
  the previous arrival, fired from a bounded 32-slot internal schedule on exact samples in later
  blocks (overflow dropped-and-counted internally — a multiplier over a stream faster than its
  own capacity is a patching error, and the module degrades by dropping, never by growing on the
  audio thread); PROBABILITY is an independent seeded draw per candidate (xorshift64 from the
  state blob — same seed, same decision stream, tested both ways; p=0 and p=1 are exact, not
  statistical). Gates: ÷4 of a 10 Hz stream fires at [0, 19200, 38400, 57600]; ×2 turns quarters
  into eighths at [0, 4800, 7200, 9600, …]; seed 42 twice is the same stream, seed 43 is not,
  and p=0.5 passes ~50 % of 500 inputs.
* **The two acceptance-shaped gates:** *lfo → svf cutoff* renders **BIT-IDENTICAL to a
  hand-driven reference** over 200 blocks (the contract-v1 acceptance pattern reused: audio-rate
  cv reduced `last` into the block-rate input, the module's formula reproduced per block — final
  effective cutoff 289.445 Hz and rising, the sweep real); and **host 16ths → clk-div ÷4 →
  membrane** — event wire to event wire to audio — lands kicks on exactly frames
  0/24576/49152/73728 with the chain golden **`914d9063ce9d8a0f`** pinned (2 s, debug ==
  release). Both new modules: 5 000-call zero-allocation gates, and the divided-drum chain
  renders 1 000 blocks allocation-free with in-loop host pushes.

**Measured in sandbox:** **665 tests** (was 655; +10 batch-4 gates) · fmt clean · clippy clean
in the default, `bootstrap-audio`, `ui`, native `ui-window`/gles and MSVC×6 cells · **ALL
pre-existing goldens unchanged** (`ba577186c988db21`, `0f5c3e86c7f117a9`, `53de3b1f3f40e3c9`,
`3f325d4f99ca2a01`, batch-2's six + chain, `1621e1f65b1b64e1`, `1d1c84bd37c99b91`,
`f2303f13aa0cf299`) · stress hash `b42068ec7b206789` unchanged · NEW goldens: chain
`914d9063ce9d8a0f` · `selftest --golden` 8/8 · `ui --audit` PASS 19 smokes (the browser smoke
meets "LFO" and "Clock Divider" and still finds exactly one "gain") · `modules --strict`
**14/14** · exec all three patches · 5 Python gates + docs regenerated (14). **Not claimed:**
per-module example patches (WO-015/WO-011), the stage-machine benchmark, an exec patch for the
lfo sweep (its gate is the bit-identical test; the audible artefact rides with WO-015's study).
Artefacts: `modules/{mod/lfo,mod/clk-div}/sparqmod.toml`, `sparq_audio::modules` (Lfo/ClkDiv,
BUILTINS 14), `tests/modules_batch4.rs`, `docs/modules/*.md` (14).

**WO-008 increment 5 — the cross-thread hot-swap primitive: ADR-009 d3's literal pointer swap
with epoch retirement, and d8's first publication (2026-09-27, sandbox-built):** the CHECKLIST's
"only remaining increment with a proofs burden this heavy" — the allowlisted-`unsafe` kernel work
that makes the engine cross-thread, shipped with the proofs it was scheduled with. Four pieces:

* **The primitive (`sparq-kernel::sync::hotswap`, allowlist entry 6).** `HotSwap::split(initial)`
  returns the two halves: `HotSwapControl` (stage complete boxed payloads, reclaim retired ones,
  read counters) and `HotSwapAudio` (once per block, `boundary(inherit)` — the swap point). The
  mechanism is exactly decision 3's sentence: staging is one `AtomicPtr::swap` (a stage onto an
  occupied slot SUPERSEDES the waiter — dropped control-side immediately, counted); the boundary
  takes the staged pointer, runs the inherit hook at the swap point, `mem::replace`s the live
  payload, and publishes the outgoing box into one of **eight epoch-tagged retirement slots** —
  then stores `epoch + 1` with Release. The grace period is mechanical: the controller may only
  re-box a retirement whose tag is strictly below its Acquire-loaded epoch, i.e. only after the
  audio thread has demonstrably passed the boundary that let the payload go — and the drop
  happens control-side, so `ExecNode`'s deactivate-on-Drop never runs inside a device callback
  (the contract's own "deactivate is control-thread" sentence, finally true by construction).
  Slot exhaustion DEFERS the swap and counts it — the audio thread's every fallback is "keep
  rendering the live patch": no locks, no allocation after construction, no blocking, no drops.
  The reclaim read order (pointer first, then tag) is load-bearing and commented: the Acquire on
  a published pointer also acquires the `UNCLAIMABLE` tag write that precedes it, so a stale tag
  can never authorise taking a fresh box one boundary early. Ships at `sync/hotswap.rs`, not the
  ADR's `graph/hotswap.rs` — the primitive is payload-generic (the kernel cannot name the
  executor type above it); the allowlist's status paragraph and the auditor's alias table both
  carry the move, the entry-4 precedent.
* **The contract grows one bound: `Module: Send`.** A module is BUILT on the control thread,
  RENDERED on the audio thread and RETIRED back on the control thread — the handover moves the
  executor, so everything inside it must be movable. Additive (all 41 shipped/test implementors
  are data-only and satisfy it automatically), pinned by a new `api_snapshot` test, `Sync`
  deliberately NOT required (a module is never touched by two threads at once — the executor's
  `&mut` proves it). Recorded here because it touches the contract crate: a thread-affine module
  now fails to COMPILE at registration instead of racing at runtime — the refusal-as-design rule
  applied to concurrency.
* **The cross-thread engine (`sparq-audio::engine`).** `LivePatch {executor, master}` is the
  payload — a staged patch carries its own master, so the audio thread never has to be told
  twice which node the listener hears. `SharedEngine::new` splits into the control half (stage,
  `send`/`set_params`/`set_musical_position`, reclaim, `read_meters`, `stats`) and `AudioEngine`
  (the audio half: per block — boundary swap with `inherit_runtime` as the hook, then commands,
  then render, then meter publication; `shutdown()` hands the final patch back so teardown drops
  where it belongs). **Decision 8's first consumer:** every block publishes one `MeterUpdate`
  per node (peak/rms/status + the stream's block count — `Copy`, fixed-size) into an `SpscRing`
  the UI side drains; the `EngineCmd` ring carries `Copy` commands the other way (`ParamSet` was
  made `Copy` in WO-007 for exactly this trip). Both rings refuse-and-count when full — an
  absent reader is counted refusals, never memory growth, never a wait on the audio thread. The
  single-owner `Engine` (task 4) stays untouched: it remains the determinism harness's vehicle
  and the offline shape; the header docs now tell the two shapes apart in words, including WHEN
  inheritance runs (v0: at stage; cross-thread: at the swap point, strictly more current).
* **The proofs.** Kernel suite (9 tests): handoff at exactly the next boundary; the hook sees
  both payloads; the epoch gate white-boxed in its three states (tag ≥ epoch refuses,
  `UNCLAIMABLE` refuses at any epoch, tag < epoch takes); supersede dropped-and-counted; a
  retirement crosses to the controller only AFTER the audio thread let it go (two real threads);
  slot exhaustion defers without dropping anything; teardown frees in-flight payloads exactly
  once (drop-counted, both orders); stats; and a paced two-thread exchange stress (5 000
  payloads natively, `cfg!(miri)` scales it to 200) checking order, the pre-staged pattern
  (tearing) and the inherited chain. Audio suite (`tests/cross_thread.rs`, 7 tests): the staged
  patch crosses at exactly one boundary (exact peaks, transport clock continues, retirement
  returns control-side); commands apply between blocks without a swap and a bad one is
  refused-and-counted; `SetMusical` reaches the module context across the thread boundary (and
  `ClearMusical` freezes rather than rewinds — documented, tested); the clock survives the swap;
  meters publish per block with an absent reader counted (40 publications → 8 kept + 32
  refusals); the cross-thread audio path allocates NOTHING across 100 paced swaps and a
  free-running render; and **the acceptance — 10 000 seeded random mutations staged from a
  control thread while the audio thread renders continuously**: 7 400 consumed swaps, 2 600
  refusals that left no trace, ~43 000–48 000 rendered blocks (scheduling-dependent, hence a
  range in words and equalities on everything structural), zero failed blocks, zero audio-thread
  allocations under the counting allocator, `deferred == superseded == 0`, retirements == swaps
  == reclaimed, the clock never regressing. No golden hash here ON PURPOSE: which block a
  mutation lands on is a scheduling fact, and a golden over scheduling is a flake generator —
  the determinism claim stays where it is provable (the single-owner stress, hash unchanged).
  Plus **selftest gate 9**: 64 paced swaps across two threads in-process, so SATURN measures the
  primitive on its own atomics and scheduler (MSVC evidence for a concurrency increment).

**Defects increment 5 logs:**

| # | Defect | Caught by |
|---|---|---|
| 85 | **The exec evidence hashes were recorded without their durations.** CHECKLIST/SYNC ask the device for `sparq exec --patch drum-demo` and name `f2303f13aa0cf299` — but the CLI default renders 5 s (3750 blocks) and hashes `de216da86b2a1869`; the golden is the 2 s render (as pinned by `transport_drum.rs`, and the demo's `53de3b1f3f40e3c9` is likewise the 2.8 s render vs the default's `d46736fd9a1c48a1`). Nothing renders wrong — all three goldens reproduce EXACTLY at their pinned durations (verified this session, debug == release) — but an operator comparing a default-run hash against the recorded golden would read a pass as a failure (or worse, the reverse). Fixed in the standing order: the evidence commands now carry explicit `--seconds` values everywhere they appear | running the evidence commands in the sandbox before re-quoting them — the hashes did not match the prose, and the pristine tree proved the mismatch predates the increment |
| 86 | **The GitHub tree drifted from the sealed tree: the history rebuild lost bytes.** `sync_check.py --quiet` on the PRISTINE clone fails its own stamp three ways: `crates/sparq-module-api/src/lib.rs` is 3449B on disk vs 3679B stamped, `tools/sync_check.py` is 26387B vs 26542B, and `fp` computes 86f/1738511B against a stamped 89f/1782456B. Mechanism: the sealed sync_check covered SIX src roots (the 143-file covered set and the 89f fp both include sparq-music, exactly as build.rs and build.bat do — defect #68's three-definitions rule); the committed copy lost `sparq-music` from `CRATE_DIRS`/`FP_ROOTS`, and lib.rs lost 230B of doc prose somewhere in the "Fresh start — rebuilt git history" squash. The zips are truth; GitHub is a copy of a copy. Fixed what is recoverable: sync_check.py restored to the six-root definition (arithmetically proven: sealed 143 = this tree's 140 − hotswap.rs + music's 3 src + music's Cargo.toml), and the bundle carries BOTH files as declared drift repairs — the run sheet copies the device's sealed versions to `logs\` BEFORE extraction and sends them back for the diff. All 682 gates pass against the 3449B lib.rs (api_snapshot pins the surface), so the lost bytes are prose; the returned diff will say exactly what they were, and a later bundle can restore them | running `sync_check.py --quiet` against the pristine clone before sealing — the recipe's verify step, pointed at the tree the bundle is built from |

**Measured in sandbox:** **682 tests** (was 665; +9 kernel hotswap, +7 cross_thread, +1
api_snapshot Send pin) · fmt clean · clippy clean in the default, `bootstrap-audio`, `ui`,
native `ui-window`/gles (`-j 1`, dev debuginfo off, 2 m 24 s) and MSVC×6 cells · **ALL
pre-existing goldens unchanged and re-verified, not assumed**: `ba577186c988db21` (selftest),
`0f5c3e86c7f117a9` (determinism), `53de3b1f3f40e3c9` (exec demo 2.8 s), `3f325d4f99ca2a01`
(sine), batch-2's six + chain `9170415cd3852736`, `1621e1f65b1b64e1` (mod-demo 1 s),
`1d1c84bd37c99b91` (membrane), `f2303f13aa0cf299` (drum-demo 2 s), `914d9063ce9d8a0f` (16ths
chain) · **stress hash `b42068ec7b206789` UNCHANGED** (debug AND release — the determinism
refactor that exposed `World::candidate_mutation` preserved the RNG draw order exactly) ·
`selftest --golden` **9/9** (gate 9 = the hot-swap cell) · `ui --audit` PASS 0 failures 19
smokes · `modules --strict` 14/14 (unchanged) · 5 Python gates clean (unsafe_audit lists the
new module through its alias) · `sync_check --self-test` 11/11 · stamp written and verified: **144-file covered set, `src
90f/1842678B`** (the six-root fp — defect #86's repair puts sync_check back in step with build.rs
and build.bat; `--quiet` on the sealed tree is clean, which the pristine clone's was not). **Miri:** the CI cell widened
from `sync::rings` to `sync::` (both modules, hotswap scales itself down under `cfg!(miri)`);
the sandbox could not run it — the nightly sysroot build is OOM-killed at 1 GB on `core`
(environment, not code; same class as the MSVC×`ui-window` cell, tried twice). **Not claimed:**
the paced zero-xrun half of the allowlist's stress sentence (real-time = device-track, the
loaded soak), multi-reader epochs, HAL integration (`play` still pumps the static WO-005 graph —
routing the live stream through `SharedEngine` waits on WO-006's exclusive acceptance), per-port
meters, the UI-side consumption (WO-013 inc 4), and the `ana/tap`/`dsp/scope` contracts
(WO-014 inc 5 — unblocked, not shipped). Artefacts: `sparq-kernel::sync::hotswap`,
`sparq-audio::engine::{SharedEngine, AudioEngine, LivePatch, MeterUpdate, EngineCmd}`,
`Module: Send`, `determinism::World` (pub, `candidate_mutation` extracted behaviour-preserving),
`tests/cross_thread.rs`, selftest gate 9, CI miri cell, ADR-009 addendum, allowlist status.

**WO-014 increment 5 — the last three modules; the first-party set is COMPLETE at seventeen
(2026-09-27, sandbox-built):** the three that waited on contracts, not on effort, shipped on the
analysis-payload contract this increment also defined. **`ana/tap`** (top `ana`, kind `analysis`) —
the generic signal tap for displays: an audio-rate BIPOLAR `wave` output (the per-frame mono monitor
mix) plus block-rate UNIPOLAR `peak` and `rms` (the `ana/rms` convention, so a tap drives modulation
too). Range decisions recorded in the manifest header, not buried (the `mod/lfo` precedent): `wave`
is bipolar because a display wants the sign, and `dsp/scope`'s inputs are bipolar to match
(`connect_cv` bipolar→bipolar is compatible; the matrix refuses bipolar→unipolar rather than
rescaling, so the ranges line up ON PURPOSE). `gain` scales the WAVE only — peak/rms report the TRUE
signal, so a tap used for modulation stays honest about level while a tap used for a scope can be
zoomed. Memoryless (empty state). **The cross-validation, not a coincidence:** the tap's wave golden
is **`3f325d4f99ca2a01`** — the IDENTICAL hash of `syn/sine`'s 1 s golden (`modules_golden.rs`). The
mono mix of a mono signal fanned to the tap's stereo input is `(s + s)/2 × 1.0 == s`, so the waveform
it publishes is the sine itself, bit for bit: two mechanisms (a direct render, an analysis tap), one
bit pattern — the tap colours nothing. **`dsp/scope`** (top `dsp`, kind `display`) — the first real
visual module and the set's ONLY module whose `process` is a deliberate **NO-OP**. Zero audio-thread
cost is the WO's acceptance box (*"≥ 60 fps alongside audio with zero audio-thread cost"*), met by
ARCHITECTURE not optimisation: the waveform a scope shows is published by its source tap onto the
analysis ring and drawn by the UI/visual thread FROM THE RING — the audio thread computes no pixel
and the scope touches no sample. Rendering inside `process` would be a lie against its own acceptance
box, so it does not happen. Its two audio-rate bipolar cv inputs (`x`, `y`) are the display BINDING:
they record in the patch GRAPH which taps the scope shows, and the UI resolves each input wire to its
source tap and reads that tap's published waveform. Parameters (timebase, mode, trigger, gain, colour
map) configure the DISPLAY, read by the UI, not by `process`; colour-map index 0 is `scope.trace`
(the identity token). **The zero-cost PROOF is a patch-level golden:** adding a tap+scope to a
`sine → out/main` render leaves the master output BIT-IDENTICAL over 2 s, so the scope-rig golden
**EQUALS the out/main golden `75bc7f2f18cac9d5`** — the display path takes nothing from the signal
and adds nothing to the render. **`out/main`** (top `out`, kind `io`) — the master output with the
metering hook: a unity-**BIT-EXACT** pass-through (`v × 1.0 == v`) with a trim and a hard mute that
writes EXACT zeros (never a near-zero, so a muted master is bit-exactly silent and its meter reads
0). Because the executor meters every node, `out/main`'s peak/rms ARE the master meters — that is
the "metering hook". It has an audio OUTPUT because the executor renders the MASTER node's first
audio output to the device; a pure sink would render silence. The input is `variable` (accepts the
mix bus at whatever width it arrives), the output stereo (the v0 device target; a multichannel out
grows the channel_set when the HAL's channel map does — declared, not faked). **The analysis-payload
CONTRACT** (`ana/tap`/`dsp/scope` waited on this, not on effort): `sparq-audio::engine::AnalysisUpdate`
(node, port, block, a bounded `[f32; 128]` waveform, `Copy`) extends ADR-009 d8's publication from
meters to WAVEFORMS. `AudioEngine::publish_analysis` runs after `publish_meters`, pushing every
audio-rate `cv` output onto a bounded `SpscRing` once per block through the new allocation-free
`Executor::with_audio_rate_cv_out` hook; `SharedEngine::read_analysis` drains it control-side and
`EngineStats` grew `analysis_refusals`. Publication is GENERIC (every audio-rate cv output — a tap's
wave, an envelope, an LFO) because the executor does not retain per-node manifests post-build, the
ring is bounded and refuses-and-counts, and any audio-rate cv signal is legitimately scopable; a
consumer filters by the `(node, port)` its display is bound to. `tests/analysis_pub.rs` (3 gates):
the waveform crosses to the control side and is the signed signal, an absent reader is
counted-not-queued (20 published → 8 kept → 12 refusals), and the audio thread allocates NOTHING
while publishing. **The LFO beat-rate DECISION** the WO demanded before building: **module-side bpm
derivation, NOT a per-frame tick view** — a per-frame `BlockContext` tick is a CONTRACT CHANGE
rippling through all 41 implementors and the `api_snapshot` pins to buy sample-accurate tempo almost
no module needs, whereas the module-side derivation reads the block-start `tick`/`ppqn` the executor
ALREADY carries (WO-009's door, and the cross-thread `SetMusical` command that feeds it) and derives
`bpm = Δtick/ppqn/Δseconds` from consecutive blocks — no contract change, and one block of lag on a
tempo EDIT is 1.3 ms at 96 kHz/64, below any musical threshold. The delay's tested-but-unreachable
`set_tempo_sync` rides the same door. Recorded in LATER.md with its reasoning — **and shipped the
same session**: `mod/lfo` grew to 0.2.0 (`sync-mode` 0 Hz/1 beat + `division` cycles-per-beat) and
`util/delay` to 0.2.0 (`tempo-sync` + `division` in beats), both deriving bpm through a shared
private `TempoFollower` (the block-start-tick diff, `bpm = Δtick/ppqn/Δseconds×60`, holding the
estimate on a frozen/backwards tick and re-anchoring so a seek recovers), both falling back to their
free-running parameter when no transport feeds a tick (a beat-locked module never silently stops),
both ADDITIVE (at the defaults they are byte-for-byte the prior versions, which the unchanged
batch-2/batch-4 goldens prove), and the delay now reaches `DelayLine::set_tempo_sync` — the
"tested-but-unreachable" path the decision named. `tests/tempo_sync.rs` (7 gates, block size 750 so
120 bpm is an exact integer tick delta): the beat-locked rate is the transport's (2 Hz at 120 bpm,
NOT the 7 Hz fallback — measured 20 cycles/10 s), it tracks a tempo change (120→60 halves the rate),
the no-transport fallbacks hold, the tempo-synced delay (120 bpm, 0.5 beats = 0.25 s) renders
BIT-IDENTICAL to a hand-timed 250 ms delay, a beat-locked golden (`db4013f41d1fa678`), and zero
allocations across 5 000 driven process calls. **Measured in sandbox:** **713 tests** (was 682; +31
across WO-014 inc 5, WO-013 inc 4 and this follow-on) · `modules --strict` **17/17** · batch-5's
13 gates (bit-exact unity wire, mute-exact-zeros, the metering hook, the two goldens, mono-mix/peak/rms
arithmetic, gain-scales-wave-only, unconnected-is-silenced, the no-op scope, the bit-identical scope
rig, and two zero-allocation gates) + analysis_pub's 3 · docs regenerated (17) · ALL pre-existing
goldens unchanged (stress hash `b42068ec7b206789` in debug AND release) · selftest 9/9 · clippy clean
(default / bootstrap-audio / ui / native ui-window check / MSVC×(audio,ui)). Sealed in
**`sync wo014-inc5+wo013-inc4`** (stamp `src 91f/1889639B`). Artefacts: `modules/{ana/tap,dsp/scope,
out/main}/sparqmod.toml`, `sparq-audio::modules::{Tap, Scope, OutMain}` + factories,
`sparq-audio::engine::{AnalysisUpdate, ANALYSIS_WAVE_LEN, ANALYSIS_RING_SLOTS}`,
`Executor::with_audio_rate_cv_out`, `sparq-audio::modules::TempoFollower` (the shared module-side
bpm derivation), `mod/lfo` 0.2.0 + `util/delay` 0.2.0 (beat-lock / tempo-sync),
`tests/modules_batch5.rs`, `tests/analysis_pub.rs`, `tests/tempo_sync.rs`,
`docs/modules/{ana-tap,dsp-scope,out-main}.md`.

**WO-013 increment 4 — live wire levels, the "signature sparq image" (2026-09-27, sandbox-built):**
the increment-3 park note said wires drew in class colour at rest *until the executor taps existed,
because faking them from canvas data would be a lie in motion*. The taps exist now (WO-008's
`Executor::meter`, and the cross-thread `SharedEngine::read_meters`), so the painter half shipped —
and it does NOT fake them. **`sparq-ui::canvas::levels`** (toolkit-independent, *computed not drawn*,
the discipline the whole canvas crate holds): `NodeLevels` (per-node 0..1, clamped, NaN-to-rest, a
deterministic `BTreeMap` so any evidence line printed from it is stable) + `wire_level(graph, levels,
wire)` — a wire carries its SOURCE node's level, because a wire carries the signal its source
published. 3 unit tests. **`bridge::node_levels`** is the ONLY place a level comes from: it renders
a short preview (`LEVEL_PREVIEW_BLOCKS = 64`, ~85 ms) and reads each node's PEAK meter through the
executor, mapping kernel nodes back to canvas nodes via the new `build_with_map`. Peak (not rms) is
the honest "is signal flowing right now" reading for a wire glow. Two bridge tests prove the levels
are REAL and FOLLOW the signal: the demo's sine and in-chain gain read hot (~0.5), the analysis node
and the unwired spare read at rest, and turning the gain down makes the level fall — a value
synthesised from canvas data could not track the rendered signal. **The painter** (`canvas_ui.rs`)
modulates each wire from its `class_colour` (at rest) toward its `class_glow` (hot) by level, plus a
soft alpha-scaled under-glow — BOTH endpoint colours are tokens and the per-channel blend
(`lerp_colour`, `with_alpha`) invents no third colour. `CanvasState.levels` is a transient,
NON-undoable field (a level is a fact about the last render, not an edit) that the shell refreshes
from `bridge::node_levels` after RENDER WAV; empty until the first render, so wires draw exactly as
they did before this increment until there is a real signal to show. **Audit smoke 20** (`ui --audit`
now 20 smokes): RENDER WAV populates live wire levels from the executor's meters, hot where signal
flowed. **The master handover** (coupled to `out/main`, so the MASTER badge never lies):
`interact::resolve_master` now prefers a WIRED `out/main` node over the default highest-id-terminus
rule — an explicit SET MASTER still wins (a user's statement the rule must not contradict), and an
unwired out/main does not count (it would render silence). `OUT_MAIN_ID` is a named constant in
`canvas/mod.rs` so the rule and the id cannot drift. 3 new tests. **DECLARED LIMIT (not faked):** a
node's meter folds its AUDIO outputs, so a `cv` wire out of a cv-only source (`mod/lfo`, `env/ad`,
`ana/rms`) reads at rest — the animation tracks AUDIO signal flow, and per-port/per-cv meters (the
rings can carry port ids) are the LATER.md item that would light cv wires from their own values.
**NOT in this increment** (the task list's "Else" column, declared for the next pass): rename text
entry, inspector scrolling, LOD visual iteration vs `design-mode.svg`. **Measured in sandbox:** the
live-levels data path is toolkit-independent and fully tested (3 levels + 2 bridge gates); the
painter compiles clean in the `ui` cell and the audit passes 20/20; `resolve_master`'s handover is
3 of the 116 sparq-ui tests. Sealed in **`sync wo014-inc5+wo013-inc4`**. Artefacts:
`sparq-ui::canvas::levels`, `canvas::OUT_MAIN_ID`, `CanvasState.levels`, `bridge::{build_with_map,
node_levels, LEVEL_PREVIEW_BLOCKS}`, the `canvas_ui.rs` wire modulation, audit smoke 20.

**WO-013 increment 5 — the "Else column" pass: rename, inspector scroll, per-cv levels, the LOD pass (2026-09-27, sandbox-built):**
increment 4 declared, in words, what it did not ship; this increment ships exactly that list and
nothing else (risk R1). **The rename text entry**: NEW `sparq-ui::canvas::entry` —
`TextEntry` (end-caret, printables-only, `ENTRY_MAX_CHARS` = 32, the node header's budget, so a
name you can type is a name the canvas can show) + `RenameState` (the sheet geometry on the
browser's clamping rule: three token rows, `clamp_origin`); the node menu grows **RENAME** (first
row — the WO's context list names it first; the one existing test that hard-coded DUPLICATE at
row 0 now finds its row BY ACTION); commit runs `Graph::op_rename` — undoable since increment 1 —
with two honest edges: an EMPTY buffer commits `None` (the module default returns, the hint row
states the rule) and an UNCHANGED buffer is a stated no-op that pushes no history entry, catching
the case `op_rename`'s equality cannot (committing the default name verbatim on a node with no
custom name would pin a rename that changes nothing visible). Escape, an outside tap, a drag or a
long-press all cancel **stated in words**. The shell's keyboard path is now ONE feed: `read_keys`
parses egui raw events into a normalised `KeyBatch` (text / backs / nav / enter / escape) that the
rename sheet consumes first (deepest modal) and the browser second — the shared text-entry
surface the provisional feed was declared to be waiting for; the wrap-egui rule holds (no widget
owns input). **Inspector scrolling**: `inspector::compute_at(node, rect, scroll)` — the offset
clamps IN the layout (the one place that knows the content height), the title is a FIXED header
the rows slide under, and increment 3's honesty rule survives the offset in both directions: a
bottom-clipped row stays touchable through its visible sliver, a row under the header is refused
by `row_at` AND skipped by the painter (new `row_visible`/`visible_rows`, one clip rule for draw,
hit-test and audit). A hairline `thumb()` draws only when the panel scrolls (a scrollbar that
cannot move is chrome pretending to be a control) and is deliberately NOT an audit element (the
gesture, not the thumb, is the v0 input). The gesture: one finger on a row is a slider edit, so
scrolling rides the TWO-finger pan — `GestureIntent::Pan` grew `center` (the centroid the
recogniser already tracked; `Zoom` has carried its centre since WO-012): centre in the panel ⇒
scroll (content follows the fingers, transient view state, reset on selection change), centre
anywhere else ⇒ the camera pans exactly as before. A pinch over the panel is DECLINED — the panel
owns the gesture and has no zoom, and the canvas behind it must not move; that rule also keeps
the recogniser's sequential-contact span wobble (two fingers processed one event at a time make a
straight drag momentarily read as a pinch — measured: reciprocal 1.667/0.6 factors) off the
camera. The panel speaks only at the ends ("scrolled to the first/last row") — mid-scroll the
moving rows are the feedback, the camera-pan precedent. **Per-cv wire levels** (increment 4's
DECLARED LIMIT, retired): `NodeLevels` grew per-port entries `(node, port)`; `wire_level` prefers
the source PORT's own level and falls back to the node's fold — audio wires keep the inc-4
semantics bit for bit (no audio port levels are published yet; a test pins that). The level
SOURCE stays the executor: `bridge::node_levels` now also reads every cv OUTPUT port's real
published value after the preview render (`node_cv_block` for block rate, `node_cv_audio`'s peak
for audio rate), as a MAGNITUDE (a bipolar swing lights by absolute value — declared), clamped by
the one sanitising rule. Two new bridge gates prove it real AND following: on the rms→svf thesis
rig the cv wire lights at the metered rms (~0.35) while the SAME node's folded audio meter stays
at rest, and dropping the sine's amp takes the cv level down with it. The executor was not
touched — stress hash `b42068ec7b206789` unchanged in debug AND release. Per-port AUDIO meters
and the ring extension (port ids on `MeterUpdate`) stay parked for the live-HAL increment,
pinned by a test rather than a sentence. **The LOD pass**: the renderings now match the contracts
`camera.rs` always stated — **Dot** wires are HAIRLINES (selection keeps its emphasis weight),
the SUM conversion word is suppressed at Dot (unreadable at that scale by definition; the class
encoding survives and the word returns on zoom-in — declared, not silently dropped);
**Simplified** is truly text-free — flag badges draw at Full only — and states ride the
look-board's pattern-first language at EVERY level: BYPASS hatches the body (the mockup's own
`hatch8`: 45°, 8 px pitch, hairline at 0.35 over the fill), MUTE dashes the border (space-token
dash), LOCK doubles it (inset hairline), MASTER chips the header at reduced LOD (a filled square
in the badge accent) and keeps its word at Full — redundant encoding, never either alone, which
is exactly the mockup's bypass pairing. At Dot the shapes carry it: hollow = bypassed, dimmed =
muted, concentric ring = locked, accent hairline ring = master (selection's ring stays
emphasis-WEIGHT, so weight distinguishes them). The side-by-side human review against
`design-mode.svg` stays a device box (test006 step H asks the operator to write what differs).
**Five new audit smokes (25 total)**: rename commit + one-undo restore; outside-tap cancel
stated; the cv wire lit from its own port's value (read through the bridge DIRECTLY — a second
menu-driven render would overwrite `canvas-render.wav`, the file test006's step [05] hashes as
the manifest-defaults baseline, and turn the device A/B into a graph comparison; smoke 13 owns
the menu path); the inspector two-finger scroll at 1280×800 with a 20-param mixer (a hidden row
becomes touchable, the camera stays put); the LOD walk (Simplified then Dot logged; at Dot a
port-to-port drag wires NOTHING). **Measured in sandbox:** **733 tests** (was 713; +20 — entry 5,
inspector 4, levels 2, interact 7, bridge 2), every pre-existing golden re-verified unchanged
(`ba577186c988db21`, `0f5c3e86c7f117a9`, the three pinned exec hashes at their `--seconds`, the
phase-B demo values), selftest **9/9**, `ui --audit` PASS **25 smokes**, `modules --strict`
**17/17**, fmt clean, clippy clean in default / bootstrap-audio / ui / native ui-window-gles /
MSVC×7 cells (MSVC×ui-window and MSVC×bootstrap-audio remain environment-OOM, the documented
`windows`-crate class), 5 python gates clean, `log_check --self-test` 25/25 (BASELINE moved WITH
the seal: 733 / `src 92f/1973870B`), `sync_check` 11/11 failable. Sealed as **`sync wo013-inc5`**
(149-file stamp). Artefacts: `sparq-ui::canvas::entry`, `inspector::{compute_at, row_visible,
visible_rows, thumb, visible_range}`, `levels::NodeLevels::{set_port, port, port_len}`,
`GestureIntent::Pan.center`, `MenuAction::Rename`, `CanvasState::{rename*, inspector_scroll,
scroll_inspector}`, `CanvasEvent::Rename`, `bridge::node_levels`'s cv-port pass, the
`canvas_ui.rs` pattern/hatch/dot renderings + `draw_rename`, the shell's `read_keys`/
`feed_modal_keys`, audit smokes 21–25, test006 steps F–I.

**WO-014 increment 6 — `util/mixer` 0.2.0, the cv merge side (2026-09-27, sandbox-built):**
the checklist's sandbox-track item 8, first entry — LATER's spec was one sentence ("adding
cv-in/cv-out ports to the mixer … is a small increment") and the plan of record
(`WO014-INC6-PLAN.md`) recorded the decisions before the code: the MIXER grows the ports rather
than a new `util/cv-mix` (the compat-matrix cell, the drift gate and `Adapter::Merge` all name
`util/mixer`; a new module would fork that name and make the set eighteen against the WO's
seventeen); **4 cv in → 1 cv out**, because a 4×4 cv matrix is 36 params > MAX_PARAMS(32) and the
fan-in rule's need is N→1; **block rate, unipolar, `cv_reduce = "mean"` DECLARED** (every shipped
modulation source and consumer is unipolar — a bipolar side would refuse the whole ecosystem
today, so it waits for `util/range`, Phase 1; `mean` is the honest block summary of a merge, and
G3 says the receiver declares while the host performs); inputs clamp to the declared range (the
svf receiver precedent) and the f64 sum clamps to 0..1 on publish (the module's own documented
rule, like the lfo's "mapped into 0..1" — never a silent host transformation); gains 0..2 like the
audio cells, **identity default** (`cvm0 = 1`, rest 0 — an untouched cv side is a bit-exact wire
from `cv-0`); params **appended LAST** (20..23) and ports after `out-3`, so every v0.1.0 snapshot
index and every audio golden keeps its bits — and short snapshots read 0.0 past their end
(`ParamSet` zero-fills), which is why batch-3's 20-value tests render unchanged audio BY
CONSTRUCTION. `BlockStatus` stays the AUDIO contract: all audio inputs unconnected ⇒ zeros +
`Silenced` while the cv side merges and publishes — a cv-only mixer is a legitimate patch citizen,
pinned by a test. A wrong-rate presentation (`Audio` slot on a declared-block port) is `Failed`,
the env/ad precedent. **The executor's fan-in refusal is RE-WORDED, not removed** — no implicit
summing, ever — and now carries the working remedy: "wire each source to its own `cv-0`…`cv-3`
input and feed this input from the mixer's `cv-out`"; `contract_v1`'s pin ("names the merge
module") passes on the new text, and the refusal test in `mixer_cv.rs` pins the port names too.
`Adapter::Merge.first_phase` stays **Phase 1 on purpose**: the MODULE exists at Phase 0, but the
compiled connect functions never return `Merge` (the fan-in cell is prose) and the canvas's
one-tap insert fits INLINE pair adapters, not a three-wire merge re-patch — the auto-insert OFFER
waits, recorded as a decision, not an oversight. **`tests/mixer_cv.rs` (9 gates):** identity cv
wire bit-exact; the merge sums by the declared gains against hand-computed f64→f32 arithmetic;
the output clamps at exactly 1.0; an audio-rate lfo arrives reduced by the DECLARED mean —
checked against the arithmetic mean of the source's own published buffer in the same block
(`node_cv_audio`, f64 sum / len / one f32 write — measured, not trusted); the audio side
re-proven under a full 24-value snapshot (and batch-3's goldens untouched); status decoupling
(`Silenced` + exact-zero audio + hot cv-out on the same render); the refusal names the remedy;
zero allocations over 1 000 blocks with connected cv; and the **snapshot-order pin** (ports 0..7
audio / 8..12 cv, params 0..19 v0.1.0 index-for-index / 20..23 `cvm0..3`, defaults `[1,0,0,0]`,
version 0.2.0) — a future edit that inserts instead of appends fails here first, in words.
**A gate caught a real fragility mid-increment (defect #87, logged and fixed in-increment):** the WO-013 inc-5 inspector-scroll smoke failed
when the mixer's param count moved 20 → 24 (its fixed drag distance had been tuned to the old
`max_scroll`); the smoke now asserts the CONTRACT — the panel overflowed before, and after the
drag some previously-hidden row is visible AND touchable where drawn — not the arithmetic of one
module's panel. **Measured in sandbox:** **742 tests** (was 733; +9), every pre-existing golden
re-verified unchanged (`ba577186c988db21`, `0f5c3e86c7f117a9`, drum-demo `914d9063ce9d8a0f`, the
three pinned exec renders at their `--seconds`, the phase-B demo values), stress hash
`b42068ec7b206789` unchanged (debug AND release — the render path did not move), selftest
**9/9**, `ui --audit` PASS **25 smokes**, `modules --strict` **17/17** (mixer now "13 ports, 24
params"), docs regenerated (17) + `--check`, fmt clean, clippy clean in default /
bootstrap-audio / ui / native ui-window-gles / MSVC×8 cells, 5 python gates clean. Sealed as
**`sync wo014-inc6`**. Artefacts: `modules/util/mixer/sparqmod.toml` (0.2.0),
`modules.rs::Mixer` (the merge), the executor's remedy sentence, `tests/mixer_cv.rs`,
`docs/modules/util-mixer.md` (regenerated), the hardened scroll smoke.

**WO-008 increment 6 — `cv_interp = "spline"`: the host performs the last word of the G4
vocabulary (2026-09-27, sandbox-built):** the checklist's sandbox-track item 8, second entry —
named there as "the next candidate in this list", and LATER's spec was one sentence ("declared in
the vocabulary and REFUSED at build until the host implements it … faking it with a hold is
exactly the silent transformation the refusal exists to prevent"). The plan of record
(`WO008-INC6-PLAN.md`) recorded eight decisions before the code. **The curve:** when block N
expands, frame `i` of `n` reads `t = i/n` on the unique parabola through the last three block
values — `v(t) = prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2` — equivalently a cubic Hermite
whose start tangent is the central difference `(cur−prev2)/2` and whose end tangent is the
second-order backward estimate `(3cur−4prev+prev2)/2` (four constraints, one cubic, the same
curve; the equivalence is stated in `CvInterp::Spline`'s doc, the one compiled copy). Chosen
properties, each gated: **exact for any signal quadratic in block index** (three knots determine
the parabola — the strongest claim a causal three-knot scheme can make); **the exact line for
collinear knots** (a block-rate ramp renders BIT-IDENTICALLY to the same wire declaring
`linear`); **constant in, constant out** (the Lagrange coefficients sum to 1); and **`linear`'s
arrival contract kept** — `v(0) = prev` exactly, the curve reaches `cur` at the next block
boundary, so declaring `spline` changes the shape of the ride, never its timing. True
Catmull-Rom (needs `v[N+1]`) and the natural cubic spline (a global solve over future knots)
were REJECTED: both buy smoothness with a block of latency, which would make `spline` time its
wire differently from `linear`; a monotone-preserving PCHIP variant was rejected too — its
limiter bends parabolas near extrema (losing the exactness property) and the safety it buys is
already provided where it matters by decision 4's clamp. **State:** `CvPlan` grew `prev2: f32`
beside `prev` and `range: CvRange` (the receiver's declaration, captured at build where G2
already proved the source compatible); both history slots start at 0.0 — the same declared
zero-history ramp `linear` has always had — and the first two blocks are a documented,
deterministic startup transient, gated. The audio path allocates nothing, as before (measured
over 1 000 blocks). **Range discipline (decision 4):** the parabola carries momentum from its
history and can locally exceed the knot span — all interpolating splines do; the inertia IS the
smoothness. The overshoot is HOST-made, so the host bounds it: the spline branch clamps to the
wire's declared range (`CvRange::clamp_f64`, new). `hold` and `linear` provably never exceed
their knots (a repeat and a convex combination) and deliberately do NOT clamp — an out-of-range
frame there can only come from an out-of-range SOURCE, a module bug that must stay visible
rather than be silently rounded away. The clamp is documented in words in all four copies of the
rule (port.rs, the matrix cell, §17, the author guide), which makes it part of performing the
declared interpolation, not a silent transformation — the mixer's own publish-clamp precedent
from WO-014 inc 6. **Arithmetic (decision 6):** the parabola evaluates in f64 with ONE rounding
per frame — host-side cv transformations accumulate in f64, the rule inc 6 of WO-014 established
for the mixer's sum; Rust never contracts into FMA, so every operation is IEEE-exact per
instruction and the render is bit-identical sandbox ⇔ SATURN (ADR-007). **One signature, one
copy (decision 5):** `expand` widened to `(prev2, prev, cur, range, dst)` — clamping in the
executor instead was rejected because the rule would live apart from the math it bounds and any
second consumer would re-implement it ("answering it twice, in two places, is how the table and
the code drift" — the matrix's own G4 gap note); the api-snapshot pin moved on purpose with the
reason at the pin. **The refusal retired; its philosophy is kept** — it existed because the
vocabulary promised what the host did not perform, and every other cv refusal (range, fan-in,
delay-kind) stands untouched; `contract_v1`'s refusal gate was REPLACED in place by a behaviour
gate, and the G4 drift gate grew a pin that the matrix cell carries the curve's definition in
the same words as the compiled copy. **`tests/cv_spline.rs` (8 gates):** the hand-computed
parabola over a scripted dyadic sequence (every frame of every block `==` against independently
simplified polynomials — the sequence visits the clamped overshoot, the momentum dip and the
plain descent); the constant wire flat bit-exactly; spline ≡ linear bit-for-bit on a ramp (with
a hand-computed anchor so "equal" cannot mean "equally wrong"); a `v[N] = N²/64` sweep
reproduced exactly at fractional frames from block 2 on; frame 0 == the previous knot bit-exact
over an LCG-seeded pseudo-random sequence (the house seed 0xA17E); the startup transient
hand-computed AND a rebuild byte-identical (determinism at wire scale); both polarities' clamps
at exactly ±1.0 with the un-clamped interior frames keeping their hand-computed values (the
clamp bounds, never reshapes); zero allocations over 1 000 blocks. Plus two `port.rs` unit pins
(function level, including the bipolar mirror) and **nothing else moved, by the shape of the
diff** (decision 8): the hold/linear branches are byte-identical, no shipped module declares
`spline` (grep-verified), no demo patch contains a block→audio cv edge, and the stress /
determinism generators build from registry manifests and never synthesise an interp. **Defect
#88 (docs-only, found and fixed in passing):** the author guide's §11 bullet still called
`util/mixer` "Phase 1, not yet built" — stale since WO-014 inc 6 shipped the cv merge; the
bullet had to be rewritten for `spline` anyway, so both halves now tell the truth. **Measured in
sandbox:** **752 tests** (was 742; +8 cv_spline, +2 port.rs units; contract_v1's refusal gate
replaced 1:1), every pre-existing golden re-verified unchanged (`ba577186c988db21`, drum-demo
`914d9063ce9d8a0f`, the three pinned exec renders at their `--seconds` — `1621e1f65b1b64e1`,
`f2303f13aa0cf299`, `53de3b1f3f40e3c9`), stress hash `b42068ec7b206789` unchanged in debug AND
release with identical counters (10 001 blocks · 7 203 swaps · 2 797 refused), selftest **9/9**,
`ui --audit` PASS (25 smokes — the canvas never saw the refusal; interp lives in the executor,
not in `connect_cv`), `modules --strict` **17/17**, fmt clean, clippy clean in default /
bootstrap-audio / ui / native ui-window-gles / all six runnable MSVC cells (the two documented
`windows`-crate OOM classes stay out), 5 python gates + `module_docs --check` (17) clean.
Sealed as **`sync wo008-inc6`**. Artefacts: `port.rs` (the curve, `clamp_f64`, the unit pins),
`executor.rs` (the refusal retired, the two-slot history, the widened call),
`tests/cv_spline.rs`, the api-snapshot pin moved with its reason, the matrix cell + its widened
drift pin, §17 / the schema row / the author guide re-worded, `WO008-INC6-PLAN.md`.

---

## 3. Work orders

> Template used throughout: **Objective · Depends on · In scope · Explicitly out of scope · Tasks · Acceptance criteria · Tests/evidence · Artefacts · Risks.**
> "Out of scope" is as important as "in scope" — it is the anti-scope-creep mechanism (risk R1).

---

### WO-000 — Repository, workspace, CI, ADR scaffold
**Objective.** A repo that enforces the engineering conventions of §20 from commit #1.
**Depends on.** —
**In scope.** Workspace layout per §20 (`crates/`, `modules/`, `design/`, `docs/`, `reference/`, `tools/`); Rust toolchain pinning (`rust-toolchain.toml`); `rustfmt` + `clippy` config with `#![forbid(unsafe_code)]` default and an explicit allowlist for HAL/rings/FFI; CI (Windows runner primary, Linux + macOS secondary) running fmt/clippy/test/debug+release; golden-file and screenshot-diff directory conventions; `.gitattributes` (LFS for `reference/` audio); ADR folder + template; issue/branch conventions; `LATER.md` parking-lot file (R1 mitigation).
**Out of scope.** Any DSP, any UI, any module implementation. No dependency on a rendering crate yet.
**Tasks.**
1. Create the workspace with empty crates: `sparq-kernel`, `sparq-module-api`, `sparq-audio`, `sparq-ui`, `sparq-app` (others added when needed — empty crates are debt).
2. Toolchain + lint config; deny-warnings in CI, `unsafe` requires a `SAFETY:` comment and an entry in the allowlist file.
3. CI matrix + caching; a `just`/`cargo xtask` command surface (`xtask test`, `xtask render-golden`, `xtask bench`).
4. ADR template (context / decision / consequences / alternatives / status) + ADR index.
5. Write `LATER.md` with the first 20 ideas you're tempted to build now.
**Acceptance criteria.**
- [ ] `cargo xtask ci` passes locally and in CI on all three OSes.
- [ ] A crate that introduces `unsafe` without an allowlist entry **fails** the build.
- [ ] Adding a new first-party module crate requires **zero** edits to CI config (globbed discovery).
- [ ] ADR-000 exists documenting the conventions themselves.
**Tests/evidence.** CI run log; a deliberately-broken PR screenshot showing the unsafe gate firing.
**Artefacts.** Repo, `docs/adr/*`, `LATER.md`, `xtask` command list.
**Risks.** Over-engineering the scaffold (cap: 1.5 days). Empty crates becoming permanent stubs.

---

### WO-001 — Stage hardware decision & Windows audio baseline
**Objective.** Lock the physical target (D-11, D-12) and characterise it, so every later ergonomic and latency claim is measured on the real thing.
**Depends on.** —
**In scope.** Choosing and configuring: the **touchscreen device** (D-11), the **audio interface** (D-12), monitoring (headphones + at least stereo speakers; 4 channels if available now), and the dev machine power plan. Producing a written baseline report: device enumeration, supported rates/channel counts, WASAPI exclusive vs shared vs ASIO capability, measured round-trip latency, measured DPC/ISR latency, and a **"stage mode" system preset** (documented, scriptable) per §5.2.
**Out of scope.** Multichannel/surround hardware (Phase 7), sensor hardware (Phase 5), buying a speaker array.
**Tasks.**
1. Shortlist 2–3 touch devices; test palm rejection, ≥10-point multitouch, glove/sweaty-finger response, 120 Hz support, brightness in a dark room, and whether it can be the *only* screen.
2. Install/verify interface drivers; enumerate with a diagnostic tool; record which sample rates and channel counts are actually available in exclusive mode and under ASIO.
3. Measure round-trip latency (loopback impulse) at 44.1/48/96 kHz × block sizes 32/64/128/256; record a table.
4. Measure DPC/ISR latency for 10 minutes idle and 10 minutes under GPU load; identify offenders (network, GPU, USB power management).
5. Write and save the stage-mode preset: high-performance power plan, core parking off, NIC power saving/EEE off, USB selective suspend off, notifications/focus-stealing suppressed, GPU scheduling mode, sparq process priority class.
6. Decide D-11 and D-12; record as ADR-004.
**Acceptance criteria.**
- [ ] One touch device is designated **the** stage device and Phase 0 runs on it (not on a dev monitor).
- [ ] Round-trip latency table exists; **< 10 ms** achieved at 96 kHz / 64 samples, or the gap is documented with a plan.
- [ ] DPC worst case < ~500 µs in stage mode, or offenders are documented with mitigations.
- [ ] Stage-mode preset is reproducible (script or documented checklist), not tribal knowledge.
- [ ] ADR-004 records the hardware decisions and the reasoning.
**Tests/evidence.** Latency table, DPC capture, photos of the rig, the preset script/checklist.
**Artefacts.** `docs/hardware/stage-baseline.md`, ADR-004.
**Risks.** Buying the wrong touch device (mitigate: test before committing, or buy used first). ASIO driver quality varies wildly by vendor — record which drivers behave. Windows "modern standby"/power throttling silently raising latency.

---

### WO-002 — Design tokens v0 + typography + colour maps
**Objective.** Author the visual identity as **data**, independent of any UI toolkit, so the Phase 6 renderer swap (D-1) can't destroy it.
**Depends on.** —
**In scope.** The token set of §3.3/§14.1: colour roles (ground, structure hairlines at 10/25/60 %, text tiers), **signal-class accents** (audio amber, CV cyan, event magenta, data green, spatial violet), type scale (variable monospace with true tabular numerals + geometric grotesque), stroke widths, corner policy, 8 px spacing grid, motion curves and durations, glow/bloom parameters, and **scientific colour maps** for spectrogram/scope rendering (perceptually uniform, colour-blind-safe variants). Tokens serialised as data (TOML/JSON) with a generator that emits Rust constants *and* a CSS/SVG export for mockups.
**Out of scope.** Widget implementation, layout system, any rendering code, the full module panel spec (§6.6 — Phase 1+).
**Tasks.**
1. Select the two typefaces (licence-checked for desktop + app embedding + web thin client later); verify tabular numerals, superscripts, Greek/math glyphs, and unit symbols (Hz, dB, ms, LUFS, φ, ×, ≈, →).
2. Define the token schema and roles; write the v0 values; build the generator + a token preview page (static HTML/SVG).
3. Define 4 colour maps: `phosphor` (default), `thermal`, `greyscale`, `cb-safe`; document the mapping rule (magnitude → colour, with perceptual uniformity note).
4. Define the **stroke language**: what a 1 px hairline means vs 2 px (structure vs signal), how glow intensity maps to amplitude, how "selected" and "active" are shown without colour alone (accessibility, §14.3).
5. Accessibility checks: contrast ratios at 3 levels, 200 % type scale behaviour, high-contrast theme values.
**Acceptance criteria.**
- [ ] Tokens live in data files; a change to one value regenerates Rust constants + preview with no manual edits.
- [ ] All 5 signal-class accents pass contrast against the ground colour at hairline weight, and each has a non-colour redundant encoding (dash pattern / shape / label).
- [ ] Colour maps are documented with their intended use and a colour-blind-safe alternative exists for every default.
- [ ] A one-page "sparq look" board exists that a stranger could use to judge whether a new surface conforms.
**Tests/evidence.** Token preview page; contrast-ratio report; the look board.
**Artefacts.** `design/tokens/*`, `design/look-board.md`, generated `sparq-ui/src/tokens.rs`.
**Risks.** Falling in love with a look that's unreadable in a dark club at 3 m (test at distance, on the projector/target screen, in WO-004). Typeface licence costs.

---

### WO-003 — `docs/VISION.md` + ADR-001…007
**Objective.** Write down the *why* and the locked decisions before the codebase is big enough to argue with you.
**Depends on.** WO-002 (identity informs the vision page).
**In scope.** One-page vision (§1–3 compressed, re-readable when discouraged); ADRs: **001** language = Rust, **002** module tiers = T1+T3 now / T2 in Phase 5, **003** UI = egui scaffold → custom shell, **004** hardware (from WO-001), **005** port type system (the closed set of §6.3), **006** clock model (three clocks, §5.1), **007** determinism & journaling policy (§5.6).
**Out of scope.** Any ADR that isn't needed to start Phase 0/1 work. No speculative future architecture.
**Acceptance criteria.**
- [ ] `VISION.md` is ≤ 1 page and states the seven pillars verbatim.
- [ ] Each ADR names the alternatives considered and the reason for rejection (this is what stops you re-litigating in month 8).
- [ ] ADR-005 lists the closed port type set and states the rule for changing it (never remove; deprecate + alias).
**Artefacts.** `docs/VISION.md`, `docs/adr/001…007`.
**Risks.** Writing essays instead of decisions. Cap: 1 page each.

---

### WO-004 — Static canvas mockups (identity before code)
**Objective.** See sparq before it exists, and validate A3 cheaply.
**Depends on.** WO-002.
**In scope.** Three static images (SVG or high-res raster) produced from the tokens, not from a UI toolkit: **(1)** Design mode — graph canvas with ~12 modules, wires colour-coded by port type, inspector with parameters, dock with module browser; **(2)** Perform mode — scene matrix, 8 macro pads, transport/clock readout, meters, full-bleed visual; **(3)** a display-module sheet — scope, spectrogram, phase portrait, rig map, DNA tree at three breakpoints (tablet / laptop / 4K wall).
**Out of scope.** Interaction, animation, real data, any code beyond token consumption.
**Tasks.**
1. Draw at true pixel dimensions for the stage device and for 4K.
2. Print/view at 1:1 and at 3 m distance; annotate what fails.
3. Test the "no title" rule (§3.3): show it to someone; ask what kind of software it is.
4. Test a dark-room and a bright-room variant.
5. Revise tokens; freeze v0.
**Acceptance criteria.**
- [ ] All touch targets ≥ 44 px at the stage device's true resolution; primary performance controls ≥ 64 px.
- [ ] Numerals are legible at 3 m in the Perform mockup.
- [ ] An uninformed viewer says something in the family of "scientific / lab / engineering software".
- [ ] The three mockups are visually consistent with each other and with the look board.
- [ ] Every accent colour in use maps to a signal class (no arbitrary colours).
**Artefacts.** `design/mockups/*.svg|png`, `design/mockup-review.md` (findings + token changes).
**Risks.** Mockups that can't be built (catch this now, not in Phase 6). Over-detailed mockups becoming a straitjacket — keep them structural, not illustrative.

---

### WO-005 — First sound out (bootstrap device layer)
**Objective.** Hear the machine within days, using a throwaway audio path.
**Depends on.** WO-000, WO-001.
**In scope.** Minimal window + audio output via a bootstrap crate (`cpal` class), a fixed 48 kHz stereo stream, a sine generator, a gain control from the keyboard/UI, and an underrun counter printed to the console.
**Out of scope.** The real HAL, multichannel, exclusive mode, ASIO, any module system. **This code is explicitly disposable** (D-13).
**Acceptance criteria.**
- [ ] Audible sine within one working day of starting the WO.
- [ ] Underrun counter visible; you can make it underrun deliberately (by loading the callback) and see the count rise.
- [ ] The bootstrap path is behind a feature flag so it can't leak into later phases.
**Artefacts.** `sparq-app` binary, `docs/adr/008-bootstrap-device.md` (short: what it is, when it dies).
**Risks.** Letting `cpal`'s capability model shape the real HAL. Mitigation: WO-006 defines its own trait first, then implements.

---

### WO-006 — Real-time HAL: WASAPI exclusive + ASIO, MMCSS, diagnostics
**Objective.** Prove A1's foundation: a trustworthy, low-latency, multichannel Windows audio path with real-time discipline and honest diagnostics.
**Depends on.** WO-005 (bootstrap, to be replaced), WO-001 (device baseline).
**In scope.**
* HAL trait: `enumerate → capabilities → open(config) → start/stop → process(frames, in, out) → latency_report → error_report`.
* Backends: **WASAPI exclusive (event-driven)**, **ASIO**, **WASAPI shared**, **null/virtual** (deterministic, for offline + CI).
* Config: 44.1/48/96 kHz, block sizes 32–2048, 2–32 channels, full duplex.
* RT discipline per §5.2: MMCSS `Pro Audio` + critical priority, ideal-processor assignment, working-set lock + pre-touch, zero allocation in the callback (custom counting allocator in debug/CI), no locks, no syscalls.
* Diagnostics: xrun count, block-time histogram (p50/p99/max), callback jitter, clock drift vs `t_wall`, reported hardware latency, and a **round-trip latency measurement** utility.
* Stage-mode integration: apply/verify the WO-001 preset at startup in Perform mode.
**Out of scope.** Device aggregator (Phase 7, but the trait must allow it), PipeWire/JACK/CoreAudio backends (Phase 1), spatial routing, resampling between devices.
**Tasks.**
1. Write the HAL trait + capability struct first; review it against Phase 7 needs (aggregation, per-channel latency) before implementing.
2. Implement the null device fully (it's the CI workhorse).
3. Implement WASAPI exclusive event-driven path; then shared; then ASIO.
4. Add the counting allocator + debug assertions that fire on alloc/lock/syscall in the audio callback.
5. Build the diagnostics ring + a console/overlay readout.
6. 2-hour soak at 96 kHz / 64 samples with a deliberately heavy dummy callback graph; log everything.
7. Document driver quirks per interface tested.
**Acceptance criteria.**
- [ ] **Zero xruns** in a 2-hour soak at 96 kHz / 64 samples on the stage device (idle system, stage mode applied).
- [ ] Audio callback performs **0 allocations, 0 locks, 0 syscalls** (verified by the debug gates, not by inspection).
- [ ] Round-trip latency measured and displayed; matches WO-001 table within ±1 block.
- [ ] Multichannel open succeeds at ≥8 out if the interface supports it; otherwise the capability report correctly says it can't, and the null device proves the code path.
- [ ] Switching backend (WASAPI↔ASIO↔null) at runtime without a restart is *not* required — but switching on stop/start **is**, and must not leak.
- [ ] Device removal/unplug mid-run produces a clean error state and a recoverable stop, not a crash.
- [ ] Diagnostics are visible without a debugger (overlay or console).
**Tests/evidence.** Soak log with histograms; allocator-gate test; unplug test; latency table regenerated from inside sparq.
**Artefacts.** `sparq-kernel::hal`, `docs/hal/windows-notes.md`, CI job using the null device.
**Risks.** WASAPI exclusive quirks (channel masking, format negotiation, hijacking by other apps). ASIO driver variance. MMCSS not being honoured under some power settings. Windows audio session policy surprises. **Mitigation:** the null device keeps all higher layers testable while backend bugs are chased.

---

### WO-007 — Module contract v0 (manifest, ports, params, lifecycle)
**Objective.** Prove A2's foundation: define the contract that will be *frozen* at end of Phase 1. Getting this shape right early is worth more than any DSP work.
**Depends on.** WO-000.
**In scope.** The v0 subset of §6:
* **Manifest** schema (TOML): identity, category, classification, ports, params, voices (stub), state schema id, resources (latency, tail, cpu/mem hints), ui descriptor (stub), lifecycle hints.
* **Port types** — the closed set from ADR-005 (`audio`, `cv`, `event`, `data`, `gpu`, `atom`), with `ChannelSet` for audio and rate tags for cv.
* **Parameter descriptor**: id, name, unit, type, range, default, curve, smoothing, modulation depth, automatable, randomisable, per-voice flag.
* **Lifecycle**: `discover → validate → instantiate → configure → prepare → activate → process → deactivate → dispose`, with the rule that only `prepare`/`configure` may allocate.
* **Param delivery**: immutable double-buffered snapshots swapped at block boundaries + in-audio-thread smoothing.
* **Discovery & registration**: filesystem scan of `modules/` + built-in registry; manifest validation with helpful errors.
* **Connection rules**: type compatibility matrix, explicit channel-set rules (mono→multi fan-out, multi→mono sum-with-warning, spatial refusal without encoder/decoder), adapter suggestions.
**Out of scope.** Hot reload (Phase 1), wasm tier (Phase 5), state migration chains (Phase 1 — but the schema id + version field must exist now), full UI descriptor (Phase 1), voice management beyond the policy enum.
**Tasks.**
1. Write the manifest schema and the compatibility matrix as **data/tables first**, review, then code.
2. Define the Rust traits (or trait + enum-dispatch hybrid — measure the dispatch cost; per-sample virtual calls matter).
3. Implement 3 conforming modules as contract tests (`util/gain`, `syn/sine`, `ana/rms`).
4. Write the "new module" documentation: what you must implement, what you get free, what you must never do.
5. Add a machine-checked **API surface snapshot** (so accidental breaking changes fail CI from day 1).
**Acceptance criteria.**
- [ ] A new module can be added by creating a crate + manifest, with **zero** edits to engine code (verified by a CI test that adds a throwaway module).
- [ ] Manifest validation rejects: unknown port types, duplicate ids, missing required ports, undeclared latency, unknown units — each with an actionable error message.
- [ ] `process()` signature makes it structurally impossible to allocate/lock (types enforce it, not documentation).
- [ ] Param snapshot swap is atomic at block boundary; a param change during a block is provably deferred to the next block.
- [ ] API snapshot test exists and fails on an intentional signature change.
- [ ] Dispatch overhead measured: a graph of 100 null modules processes a 64-sample block in < 20 µs (indicative target — measure and record the real number).
**Tests/evidence.** Contract tests for 3 modules; the throwaway-module CI test; snapshot diff; dispatch benchmark.
**Artefacts.** `sparq-module-api`, `docs/module-author-guide-v0.md`, ADR-005 addendum (dispatch strategy chosen + measured cost).
**Risks.** Designing the contract for today's 3 modules and having it collapse at 40. **Mitigation:** write the *Phase 3* module list (Appendix B) on the wall and check every candidate against the contract on paper before implementing — especially granular (voices), convolution (latency/tail), HOA (channel sets) and data modules (typed records).

---

### WO-008 — Graph + executor (typed edges, atomic mutation, RT-safe)
**Objective.** Prove A1: a graph executor that is real-time-safe, deterministic and mutation-safe.
**Depends on.** WO-006, WO-007.
**In scope.** Graph data structure (nodes, typed edges, versions); topological ordering with caching and dirty flags; cycle detection with `unit_delay`/`block_delay` as the only legal cycle-closers (§5.4); buffer allocation/pooling per block; audio + cv + event propagation; atomic graph mutation from the control thread applied at block boundaries (RCU-style swap); per-module latency declaration and per-path latency computation with a global raw/compensated switch; per-module block-time watchdog with auto-bypass and UI flag; memory arenas pre-reserved at graph-activate.
**Out of scope.** Polyphony/voice manager (Phase 1), sub-block processing, feedback-delay-network-level optimisation, GPU ports (declared but not executed), degradation ladder beyond watchdog auto-bypass.
**Tasks.**
1. Define graph + version types; implement topo sort and validation.
2. Implement buffer pool and channel-set negotiation at connect time.
3. Implement the executor loop: read param snapshots → process in order → write outputs → publish analysis taps + meter ring.
4. Implement atomic mutation (build new graph on control thread, swap pointer, retire old after a grace period).
5. Implement latency accounting + the raw/compensated switch.
6. Implement watchdog + auto-bypass.
7. Determinism harness: run the same graph twice with the same inputs/seeds; assert bit-identical output.
**Acceptance criteria.**
- [ ] 200-module graph, 96 kHz / 64 samples: **zero xruns** in a 30-minute soak, p99 block time < 40 % of budget.
- [ ] Adding/removing/reconnecting a module **while audio is running** produces no xrun, no glitch beyond the intended one-block boundary, and no crash — under a 10 000-iteration random-mutation stress test.
- [ ] Cycles without a delay edge are rejected at connect time with a clear error.
- [ ] Determinism harness passes: two runs are bit-identical (hash equality).
- [ ] Latency readout per path is correct against a hand-computed reference for a 3 test graphs.
- [ ] Watchdog auto-bypasses a deliberately stalling module within N blocks (N configured) and flags it; the rest of the graph keeps playing.
- [ ] Zero allocations on the audio thread (allocator gate from WO-006 still passing).
**Tests/evidence.** Soak log, mutation stress test, determinism hash test, latency reference tests, watchdog test.
**Artefacts.** `sparq-kernel::graph`, `docs/adr/009-executor.md` (ordering, mutation, latency policy).
**Risks.** Mutation-during-playback races (the classic modular-system bug) — mitigate with the stress test as a first-class deliverable, not an afterthought. Cache-unfriendly traversal order hurting CPU at high module counts — measure early with the 200-module graph.

---

### WO-009 — Clocks + transport v0
**Objective.** The three-clock model of §5.1 working, with sample-accurate events — the prerequisite for all sequencing later.
**Depends on.** WO-008.
**In scope.** `ClockBroker` maintaining `t_sample ↔ t_musical ↔ t_wall` as a smoothed piecewise-linear map; transport v0 (play/stop, tempo, tick position, loop region, tap-tempo); 960 PPQN default; **sample-accurate event queue** (events scheduled by tick, dispatched at the exact sample); transport as an event source (bar/beat triggers into the graph); tempo as a settable value now and as a *stream* by design (interface must allow it).
**Out of scope.** External sync (Link/MIDI clock — Phase 4), metric modulation UI, per-track time signatures, count-in/metronome, recording.
**Acceptance criteria.**
- [x] A trigger scheduled at tick T fires at exactly the corresponding sample index — verified for 1 000 random ticks against an analytically computed expectation (±0 samples). **Done 2026-09-26 (inc 1), twice**: constant tempo, and across a 10 ms glide + a step, with the expectation computed from an independent implementation of the segment integral inside the test.
- [x] Tempo change during playback produces **no discontinuity** in either direction (tick↔sample mapping stays monotonic and continuous); verified by a sweep test and by listening. **Sweep done 2026-09-26 (inc 1)**: 6 000 blocks of 60→200→70 bpm at one target per block, plus the kernel's derivative-continuity tests (half-increments at ramp edges, worst-case 64-sample anchors). Listening needs a tempo-editing surface — device track, with `drum-demo` as the artefact.
- [ ] `t_wall` drift vs device clock is estimated and reported; over 30 min the reported drift matches an independent measurement within tolerance. **Mechanism done 2026-09-26 (inc 1)** — estimator shipped and synthetic-verified (+1000 ppm converges to ±2; backwards readings refused and counted); **the 30-minute hardware box stays open** (device track: the HAL pump feeding `observe_wall` between blocks).
- [x] Transport stop/start is sample-accurate and repeatable (same event twice ⇒ same output). **Done 2026-09-26 (inc 1)**: stop FREEZES the timeline; the same play/stop/play script twice renders bit-identical through the real drum-demo graph, beats at the arithmetic positions the freeze implies (`sparq-app/tests/transport_drum.rs`).
**Tests/evidence.** Tick-accuracy test, tempo-sweep continuity test, drift report.
**Artefacts.** `sparq-music::transport`, ADR-006 addendum (measured drift behaviour).
**Risks.** Clock-math bugs that only appear at tempo automation extremes — write the continuity test before the feature.

---

### WO-010 — Offline render + golden-reference test harness
**Objective.** Prove A4's first half and establish the permanent quality floor of §9.1.
**Depends on.** WO-008.
**In scope.** Virtual-device-driven offline render (same executor, no real-time constraint); deterministic single-threaded path; WAV export (mono/stereo/multichannel interleaved, 16/24/32-bit int + 32-bit float, BWF metadata); optional global `f64` mode; the **golden harness**: store reference renders + hashes, compare with a relative tolerance, and fail CI on deviation.
**Out of scope.** Multi-threaded segmented render (Phase 1), stem/bus bouncing (Phase 6), MP3/FLAC export, loudness-normalised renders.
**Acceptance criteria.**
- [ ] Rendering the same project twice yields **identical bytes** (hash match), across runs and across machines in CI.
- [ ] Offline render of a 3-minute dense patch is ≥ 20× real time on the dev machine.
- [ ] Multichannel (≥8 ch) WAV export round-trips correctly (channel order and count verified).
- [ ] Golden harness: 10 reference renders checked in; an intentional 1-sample change to a DSP module **fails CI** with a readable diff (max abs error, first differing sample, hash).
- [ ] `f64` mode changes results only where expected (documented per-module which are bit-identical, which differ).
**Tests/evidence.** Hash-equality test, CI failure demo, multichannel round-trip test.
**Artefacts.** `sparq-kernel::render`, `reference/golden/*`, `xtask render-golden`.
**Risks.** Golden files becoming a maintenance burden (mitigate: small, meaningful set; regenerate with an explicit reviewed command). Platform FP differences — pin the reference to one CI platform and record tolerances per module.

---

### WO-011 — Journal v0 + project file + deterministic replay
**Objective.** Prove A4's second half: the patch + seeds + journal *is* the composition (§5.6).
**Depends on.** WO-009, WO-010.
**In scope.** Append-only checksummed binary journal; event schema (graph mutation, param change, transport, seed allocation, module versions, asset hashes, device config, xrun/degradation); autosave interval; atomic project write with rollback copy; `.sparq` container v0 (graph + seeds + module versions + asset refs, text-diffable graph representation); **replay**: re-execute a journal to reproduce the session; **re-render from journal** matching the live render.
**Out of scope.** Stream recording (Phase 5), full undo/redo history UI (WO-013 does graph-level undo), cross-session time-travel UI, encryption.
**Acceptance criteria.**
- [ ] Killing the process arbitrarily (task-kill) during a 30-minute session never corrupts the project; on restart sparq offers recovery to the last good state in **< 10 s**.
- [ ] Replaying a recorded 5-minute session reproduces the audio **bit-exactly** (hash match against the original render).
- [ ] `.sparq` graph representation is text-diffable (two versions diff legibly in a code review).
- [ ] Project records every module version and asset hash; opening with a missing module produces a **repair dialog** (stub / substitute / cancel), never a crash and never a silent wrong sound.
- [ ] Journal size for a 1-hour session is reasonable (< 50 MB with no stream recording) and bounded by design.
**Tests/evidence.** Kill-recovery test, replay hash test, missing-module repair test, diff sample.
**Artefacts.** `sparq-kernel::journal`, `.sparq` format spec v0 (`docs/formats/project.md`).
**Risks.** Format churn — keep v0 minimal and versioned from the start; never write an unversioned container.

---

### WO-012 — egui shell: window, DPI, touch input, panels
**Objective.** A touch-usable application shell in the token language, without yet investing in the custom renderer (D-1).
**Depends on.** WO-002, WO-004.
**In scope.** `winit` window + DPI/fractional scaling on Windows (per-monitor DPI awareness — a classic Windows trap); egui integration; input abstraction that normalises **touch, pen and mouse** into one gesture model (tap, drag, long-press, two-finger pan/pinch, multi-touch tracking); the shell layout of §14.2 (rail / canvas / inspector / dock) with collapsible panels and 8 px grid snapping; token consumption (no hard-coded colours/sizes anywhere in widget code); a **layout audit** that fails if any interactive element is < 44 px.
**Out of scope.** The custom renderer (Phase 6), second-display output (Phase 6), theming beyond tokens + high-contrast, command surface (Phase 1).
**Tasks.**
1. Per-monitor DPI: verify at 100/125/150/200 % and on mixed-DPI multi-monitor; touch coordinates must map correctly in all cases.
2. Input normalisation layer: one gesture API consumed by all widgets; long-press = context; two-finger = pan/zoom; second-finger-down during a drag = ×10 fine resolution (§14.3.5).
3. Panel/shell system with token-driven styling; an `egui` style adapter generated from tokens so a token change restyles everything.
4. Layout audit as an automated test (walk the widget tree, assert minimum sizes).
5. Palm-rejection and accidental-touch testing on the stage device.
**Acceptance criteria.**
- [ ] Correct layout and correct touch hit-testing at 100/125/150/200 % DPI and on a mixed-DPI dual-monitor setup.
- [ ] Every interactive element ≥ 44 px (automated audit passes).
- [ ] No hard-coded colour/size/spacing values in widget code (grep-clean audit in CI).
- [ ] 60 fps sustained on the stage device with the full shell + a 50-node canvas (measure; record the number).
- [ ] Long-press, two-finger pan/pinch and fine-resolution drag all work with a real finger, including with a hand resting on the screen.
- [ ] High-contrast theme switch works and is generated from tokens.
**Tests/evidence.** DPI matrix screenshots, layout audit report, frame-time histogram, palm-rejection notes.
**Artefacts.** `sparq-ui` shell, `docs/ui/input-model.md`, `docs/ui/windows-dpi-notes.md`.
**Risks.** egui's immediate-mode model fighting touch gestures (no hover, grab semantics). **Mitigation:** wrap it — all widgets go through your own gesture layer so the Phase 6 swap only replaces the drawing half. If egui proves unworkable for touch by end of Week 3, escalate: this is the one decision that could pull the custom renderer forward (record as a Phase 0 finding either way).

---

### WO-013 — Graph canvas: nodes, wires, touch gestures, undo
**Objective.** The signature sparq surface: editing a patch by touch, in the token language.
**Depends on.** WO-012, WO-008.
**In scope.** Canvas with pan/zoom + LOD (three levels: full panel detail → simplified → colour-coded dot); node rendering from the module's UI descriptor; port rendering with signal-class colours; wire drawing (bézier or orthogonal, per token spec) with live level indication; drag-to-connect with compatibility affordances (compatible ports glow, incompatible dim, "adapter needed" one-tap insert); long-press context (rename, duplicate, bypass, mute, inspect, randomise, lock); module browser with fuzzy search (command surface v0); graph-level undo/redo (3-finger tap = undo); zoom-to-fit double-tap; selection and multi-select; a minimal inspector showing the selected module's params with touch sliders and numeric entry.
**Out of scope.** Custom module panels with novel widgets (Phase 2+), mapping canvas (Phase 5), DNA tree (Phase 4), sub-graph/patch nesting (Phase 1), keyboard shortcut coverage beyond undo/zoom.
**Acceptance criteria.**
- [ ] Building a 12-module patch **entirely by touch** takes < 3 minutes for a first-time user (timed test with someone who hasn't seen it).
- [ ] Port capture radius ≥ 24 px; a connection can be made one-handed on the stage device.
- [ ] 200-node graph stays at ≥ 60 fps while panning/zooming (LOD engages automatically).
- [ ] Undo/redo restores graph *and* param state correctly for 20 random operations.
- [ ] Connecting incompatible ports is impossible; the "insert adapter" path works in ≤ 2 touches.
- [ ] A screenshot of the canvas next to the WO-004 mockup is visually consistent (side-by-side review).
**Tests/evidence.** Timed first-user test, frame-time histogram at 200 nodes, undo fuzz test, mockup-vs-reality comparison.
**Artefacts.** `sparq-ui::canvas`, `docs/ui/gestures.md`.
**Risks.** LOD making the graph unreadable when zoomed out (iterate on the mockup first). Wire spaghetti at scale — decide the routing style now and keep it.

---

### WO-014 — Module set v0 (17 modules incl. scope display)
**Objective.** Enough instrument to make real music, and enough variety to stress the contract from WO-007.
**Depends on.** WO-007, WO-008 (WO-009 for the clock-driven ones).
**In scope.** The **original** 12 Phase-1 modules (§17 / Appendix B, before WO-007 added the three adapters — those are Phase 1, not Phase 0) plus **five** that Phase 0 specifically needs (the count below is 17 = 12 + 5; this line previously said "four", defect #57):

| ID | Purpose | Contract stress it exercises |
|---|---|---|
| `syn/sine` | exact-frequency oscillator | param smoothing, f64 phase accumulation |
| `syn/noise` | white/pink/brown + impulse/dust | seeded RNG from the seed tree (§5.6) |
| `syn/polyblep` | bandlimited saw/square/pulse | aliasing discipline, oversampling hook |
| `syn/membrane` | drum-membrane model (pitched impulse + resonance + noise burst) | the techno kick, physical-model latency/tail |
| `flt/svf` | state-variable filter, ZDF | nonlinear param modulation without zipper noise |
| `env/ad` | attack/decay + loopable | event-driven, sample-accurate start |
| `mod/lfo` | shapes + phase reset + sync-to-transport | clock coupling |
| `mod/clk-div` | clock divider/multiplier + probability gate → triggers | event ports, transport as source |
| `util/gain` | gain/trim | trivial baseline |
| `util/mixer` | N×M matrix with per-cell gain | channel sets, multichannel readiness |
| `util/panner` | stereo pan + law | spatial precursor |
| `util/delay` | tempo-synced delay, feedback with unit-delay safety | declared latency + legal cycles |
| `fx/bitcrush` | depth/rate reduction | deliberate aliasing as a feature (documented) |
| `ana/rms` | RMS/peak → cv + data | analysis-as-control-source principle |
| `ana/tap` | generic signal tap for displays | gpu/display plumbing |
| `dsp/scope` | waveform + X/Y display from tokens/colour maps | the first real visual module |
| `out/main` | output with metering hook | device channel mapping |

Each module ships: manifest, implementation, golden reference render, unit tests (property-based where relevant: DC-free, bandlimited within tolerance, no denormal stalls), example patch, and docs generated from the manifest.
**Out of scope.** Granular, spectral, convolution, reverb, physical models beyond membrane, spatial (HOA/VBAP), samplers, sequencers (Phase 2–4).
**Acceptance criteria.**
- [ ] All 17 modules load from manifests via discovery, with no engine edits (contract test from WO-007 passes for each).
- [ ] Every module has a golden render checked in and passing.
- [ ] `syn/polyblep` aliasing measured: harmonics above Nyquist at least 60 dB below the fundamental at 48 kHz (record the actual spectrum plot).
- [ ] A "typical Phase 0 patch" (16 modules, 3 voices of activity) uses **< 15 %** of one core at 96 kHz / 64 samples on the stage machine.
- [ ] `dsp/scope` renders at ≥ 60 fps alongside audio with zero audio-thread cost.
- [x] `ana/rms` output demonstrably modulates a filter cutoff (the analysis-as-control-source principle proven end to end). **Done 2026-09-26, contract v1 (WO-008 inc 4):** `tests/contract_v1.rs` renders the cv-wired `flt/svf` bit-identical to a hand-driven reference over 200 blocks; `sparq exec --patch mod-demo` is the artefact, golden `1621e1f65b1b64e1`.
- [ ] Docs for all modules are generated from manifests (no hand-written duplicates).
**Tests/evidence.** Golden renders, aliasing spectrum plots, CPU benchmark, the rms→filter demo patch.
**Artefacts.** `modules/{syn,flt,env,mod,util,fx,ana,dsp,out}/*`, `examples/phase0/*`.
**Risks.** DSP quality rabbit-holing (the membrane model can eat a week). Cap each module at 0.5 day; "good enough for the study" is the bar, Phase 2 is where they get excellent.

---

### WO-015 — The Phase 0 study (60 s of music + capture)
**Objective.** Prove A5. The most important work order in Phase 0 and the one most likely to be skipped.
**Depends on.** WO-013, WO-014.
**In scope.** Compose and render a 45–90 second piece using **only** sparq, in the techno/breakcore/experimental idiom. Requirements: at least one generative or clock-driven element (so it isn't a static drone), at least one `ana/*` → parameter modulation path, a multichannel or binaural-check render, and a full-bleed `dsp/scope` visual recorded from the app itself. Deliver: audio render (WAV), the `.sparq` project, the journal, a screen capture, and 300 words of notes on **what was painful**.
**Out of scope.** Mixing/mastering polish, releasing it, adding features to make it work (pain points go to `LATER.md` and the Phase 1 backlog — not into Phase 0).
**Acceptance criteria.**
- [ ] The piece is complete, rendered offline, and **bit-exactly reproducible** from project + journal (WO-011 gate, applied for real).
- [ ] Screen capture shows the patch and the visual running together, in the token language.
- [ ] A "pain log" lists every moment you wanted a feature that didn't exist, ranked by how much it blocked the music.
- [ ] Verdict written: *would I have enjoyed making this in sparq?* (yes/no + why). A "no" is a legitimate and valuable Phase 0 result — it re-scopes Phase 1.
**Artefacts.** `examples/phase0/study-01/`, `docs/studies/phase0-study-01.md` (pain log + verdict).
**Risks.** Skipping it because the software isn't ready. Mitigation: it is scheduled, and WO-016 cannot pass without it.

---

### WO-016 — Phase gate: exit criteria, soak, devlog, Phase 1 scope
**Objective.** Close Phase 0 honestly and convert findings into Phase 1 tickets.
**Depends on.** All.
**In scope.** Run the full gate (§4); a **12-hour soak** in Perform-like conditions (touch interaction, mutations, journal writing, scope rendering) with zero xruns; publish the first devlog artefact (mockups + screenshots + study audio/video); write the Phase 1 backlog from the pain log + §17; confirm or revise the egui→custom-renderer decision (D-1) with evidence from WO-012/013.
**Acceptance criteria.**
- [ ] Every gate in §4 measured and recorded (not asserted).
- [ ] 12-hour soak: zero xruns, no memory growth beyond the documented arena budget, journal intact, recovery tested.
- [ ] Devlog artefact published (this is the start of the audience-building asset, §17).
- [ ] Phase 1 backlog exists, estimated, and ordered; ADR-003 updated with the D-1 evidence.
- [ ] `LATER.md` reviewed: everything parked is still parked, or explicitly promoted with a phase.
**Artefacts.** `docs/phase0-report.md`, public devlog post, Phase 1 backlog.

---

## 4. Phase 0 exit gate (all measured, all recorded)

| Gate | Target | Measured how |
|---|---|---|
| Xruns, 2 h soak, 96 kHz / 64 samples | **0** | HAL diagnostics log (WO-006) |
| Xruns, 12 h interactive soak | **0** | WO-016 |
| Audio-thread allocations / locks / syscalls | **0** | counting allocator + debug gates |
| Controller→sound / input→output round trip | **< 10 ms** | loopback impulse measurement |
| p99 block time, 200-module graph | < 40 % of budget | histogram |
| Typical Phase 0 patch CPU | < 15 % of one core | benchmark |
| Offline render determinism | bit-exact, twice | hash comparison |
| Journal replay determinism | bit-exact vs live render | hash comparison |
| Crash recovery | < 10 s to last good state | task-kill test |
| Missing-module open | repair dialog, no crash, no silent wrong sound | test |
| UI frame rate, shell + 50-node canvas, stage device | ≥ 60 fps | frame-time histogram |
| UI frame rate, 200-node canvas while panning | ≥ 60 fps (LOD) | frame-time histogram |
| Touch target compliance | 100 % ≥ 44 px | automated layout audit |
| DPI correctness | 100/125/150/200 % + mixed-DPI | screenshot matrix |
| Add a module without editing engine code | passes | CI throwaway-module test |
| Golden harness catches a 1-sample change | fails CI | demo |
| `polyblep` aliasing | ≥ 60 dB below fundamental above Nyquist | spectrum plot |
| Build a 12-module patch by touch, first-time user | < 3 min | timed test |
| Identity check ("what kind of software is this?") | "scientific/lab/engineering" | blind viewer test |
| **A5: a piece of music made only in sparq** | **exists, reproducible, and you'd do it again** | WO-015 verdict |

---

## 5. Windows-first setup checklist (run in WO-001, re-verify in WO-016)

**System**
- [ ] High-performance power plan; processor performance boost mode set explicitly; core parking disabled; sleep/hibernate off during sessions.
- [ ] Windows Update pause policy for performance machines; no automatic reboots.
- [ ] Notifications, focus assist, and "foreground window stealing" suppressed for stage mode.
- [ ] NIC power saving / Energy-Efficient Ethernet off; Wi-Fi adapter power management off (or use wired).
- [ ] USB selective suspend off; hubs powered, not bus-powered, for audio/MIDI/sensors.
- [ ] GPU driver: known-good version pinned; hardware-accelerated GPU scheduling tested both ways (it changes frame pacing).
- [ ] Antivirus/Defender exclusions for the project directory, asset store and module directories (scan-on-access is a classic source of hiccups).
- [ ] Audio engine/service set to the performance device; sample rate matched across OS mixer and sparq to avoid hidden SRC.

**Devices & drivers**
- [ ] Interface driver version recorded; ASIO and WASAPI exclusive both tested; channel count/rate matrix captured.
- [ ] Exclusive-mode access verified (no other app holding the device); document what happens when another app does.
- [ ] Touch device: multitouch point count, palm rejection, glove response, refresh rate, per-monitor DPI behaviour all recorded.
- [ ] DPC/ISR latency measured idle and under GPU load; offenders listed with mitigations.

**sparq**
- [ ] Stage-mode preset applies all of the above and *verifies* (report pass/fail per item) at startup in Perform mode.
- [ ] Pre-flight checklist (plan §12.4) prints device config, sample-rate match, channel count, disk space, CPU headroom estimate.
- [ ] Process priority class and MMCSS task confirmed at runtime in the diagnostics overlay.

---

## 6. Definition of done (applies to every work order)

A work order is done only when **all** are true:
1. Acceptance criteria are demonstrated, not asserted — with the evidence artefact named.
2. Tests are in CI and would fail if the behaviour regressed.
3. Docs exist where the WO lists them (generated from source/manifest where possible).
4. An ADR is written if the WO made an architectural choice (one page, alternatives included).
5. Nothing out of scope was built; anything tempting went to `LATER.md`.
6. The numbers it produced are recorded in `docs/phase0-report.md` (so the gate is a table, not a memory).
7. It was used, at least once, to make sound or music — not just to pass tests.

---

## 7. Recommended schedule (6 weeks at ~25–30 h/week)

| Week | Work orders | End-of-week checkpoint |
|---|---|---|
| 1 | WO-000, WO-001, WO-002, WO-003 | Repo + CI green; hardware decided and measured; tokens v0; ADRs 001–007 |
| 2 | WO-004, WO-005, WO-006 (start) | Mockups reviewed at 3 m; first sound out; HAL trait + null device done |
| 3 | WO-006 (finish), WO-007 | **Zero-xrun soak on the real device**; module contract + 3 conforming modules |
| 4 | WO-008, WO-012 (start) | Executor with mutation stress test passing; shell + touch input working |
| 5 | WO-009, WO-010, WO-012, WO-013 (start) | Clocks sample-accurate; golden harness in CI; canvas editable by touch |
| 6 | WO-011, WO-013, WO-014, WO-015, WO-016 | Journal + replay; 17 modules; **the study**; gate report + devlog + Phase 1 backlog |

**If you must cut** (to fit 4 weeks), cut in this order and never the other way:
1. WO-011 journal → reduce to "project save/load + seeds" (replay slips to Phase 1). **Keep the seed tree** — determinism is cheaper to add early than late.
2. WO-009 transport → reduce to a fixed-tempo clock + triggers (no loop region, no tap tempo).
3. WO-014 → drop `syn/membrane`, `util/panner`, `fx/bitcrush` (10 modules is enough for the study).
4. WO-004 → one mockup instead of three.
**Never cut:** WO-001 (hardware reality), WO-006 (zero-xrun proof), WO-007 (the contract), WO-008 (mutation safety), WO-012/013 (touch), WO-015 (the music).

---

## 8. Phase 0 risk watchlist

| Risk | Early sign | Response |
|---|---|---|
| WASAPI/ASIO driver variance eats the phase | Underruns only on one backend; channel-count negotiation failures | Null device keeps upper layers moving; file backend bugs separately; consider ASIO-first for the stage device |
| egui fights touch gestures | Long-press/drags conflicting; no hover-equivalents; hit targets wrong at DPI scale | Escalate the input abstraction; if unresolved by end of Week 5, pull the custom shell forward (documented as a D-1 revision) |
| Windows DPI/scaling surprises | Touch coordinates offset from visuals on a second monitor or at 150 % | Per-monitor DPI awareness from day 1; test the full matrix in WO-012, not later |
| Module contract designed for 3 modules, breaks at 40 | Granular/convolution/HOA candidates don't fit the port or latency model | Paper-check every Appendix B module against the contract in WO-007 before implementing |
| DSP perfectionism | One module takes 4 days | 0.5-day cap per module in Phase 0; excellence is scheduled for Phase 2 |
| The study gets skipped | Week 6 slips and WO-015 is "next week" | WO-016 cannot pass without it. Book the recording session in the calendar in Week 1 |
| Solo momentum loss | Two consecutive weeks with no artefact | Weekly artefact rule (§17.1); publish the devlog even when it's ugly |

---

*Next documents after Phase 0: `docs/module-api-v1.md` (the frozen contract), `docs/adr/*` from the phase, and the Phase 1 backlog derived from the WO-015 pain log.*
