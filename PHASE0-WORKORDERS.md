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
| WO-006 | **in progress — increment 1.1d; the FIRST PHYSICAL session ran (2026-09-24, `test001`/`test002`)** | HAL trait + diagnostics + null backend + conformance + WASAPI exclusive/shared, 242 tests, four clippy matrix cells clean. **Physical SATURN session (operator present, no RDP, High-performance scheme):** the endpoints are RESOLVED by name — the default is **OUT 1-2 (BEHRINGER UMC 204HD 192k)**; shared mode is proven on the real interface: **first sound ever through the HAL** (tone audible), 10 s plays + a 5-min soak @96k/64 all **0 xruns / 0 allocations** (450 307 blocks, p50 1.0 µs / p99 2.0 µs / max 108 µs, 3 outliers), 4 ch f32 @96k negotiated on OUT 1-4 via the two-shapes ladder (#38/#46 fix working on hardware), **the unplug acceptance criterion PASSED** (mid-run removal → `Removed` state, dev-err 1, 0 xruns, clean stop, recovery play + tone after re-plug), reopen-leak 0 on hardware. **Multichannel criterion resolved honestly** (interface maxes at 4 out; caps say so; null proves ≥8). Remaining for acceptance: **the 2 h zero-xrun soak at 96 kHz/64 EXCLUSIVE — blocked by defect #77**: the exclusive ladder probes f32 only and the Behringer driver refuses f32 exclusive (`AUDCLNT_E_UNSUPPORTED_FORMAT`, not policy — both checkboxes ticked); increment 1.2 adds integer rungs (i32/24-in-32/i16 + pump conversion), then test004 runs the acceptance. Also found: **#75** (friendly-name `E_ACCESSDENIED` on REAL endpoints with no remote session — #40's "RDP quirk" attribution falsified; registry reads names fine → fallback path proven) and **#76** (the `SUSPECT` drift decoded: `(buffer_frames ÷ event_period) ÷ rate − 1` = +1.2 M ppm on BOTH the RDP and Behringer sessions to four digits — `IAudioClock` advances in buffer steps per event tick; the #45 guard did its job, throughput stayed truthful). `docs/hal/windows-notes.md` §4/§4b carry the rows and the arithmetic. |
| WO-012 | **in progress — increment 1 built and headless-green** | Toolkit-independent UI core in `sparq-ui` (pointer model, gesture recogniser with the full §14.3 table, shell layout computation, touch-target audit — 37 tests, zero dependencies) + the egui shell in `sparq-app` behind `ui`/`ui-window` features (token-generated style adapter incl. the derived high-contrast theme, window host with per-monitor DPI via winit, headless driver). `sparq ui --audit` is a gate: 5 viewports × 4 DPI scales × 2 modes + DPI-invariance + 9 synthetic-gesture smokes — **PASS (0 failures)** in sandbox; frame logic med 94 µs headless. Defects #50–#53 found and fixed (table below). **Awaiting device:** the DPI matrix on real monitors, touch with a real finger, palm rejection (needs WM_POINTER contact area — winit reports none), 60 fps on the stage device. |
| WO-007 | **task 1 done — the contract is data, and ten decisions are taken** | `docs/api/compat-matrix.toml` (6 port types, 21 same-type cases, 5 verdicts, 4 adapters, 1 cell still open) and `docs/api/manifest-fields.toml` (82 field rows, 22 required, an error code per violation) — the tables-first artefact task 1 asks for, both parsing, neither consumed by code yet. `WO007-TASK1-REVIEW.md` carries the ten ratified decisions and the two acceptance blockers found. §16's five open questions are all answered, Q1 **by measurement** (`tools/dispatch-bench`: enum dispatch is within noise of monomorphised; per-sample trait objects cost 27–36 µs for 100 null modules against a 20 µs budget, so `process(block)` may be a trait object and nothing per-sample may be). Amended: `manifest-schema.md`, `module-api-v1.md` (§2 cascade rule, §3, §14, §16), ADR-005 addendum, **ADR-009 executor decision 7 + Consequences**, Appendix B and §17 Phase 1 (12 → 15 modules). Defects #54–#65 below. **Tasks 2–3 built** (increment below): `sparq-module-api` — the closed port vocabulary as types, the connection rules as functions, the derived error catalogue (19 legacy kinds + 5), the manifest with every required field an `Option` so absence is reportable, `validate()` returning either a `ValidatedManifest` the executor may use or *every* failure at once, `Copy` param snapshots through the kernel's lock-free ring, and the dyn-compatible `Module` trait whose `AudioCtx` exposes no allocator, clock, filesystem or lock. Three conforming modules as contract tests (`util/gain`, `syn/sine`, `ana/rms`). **368 tests** (was 280 before WO-007), clippy clean in seven of eight cells, **0 allocations across 15 000 `process` calls through `Box<dyn Module>`**. Increment 2 added the **TOML reader** — `toml.rs`, a dependency-free subset parser, and `decode.rs`, text → `ValidatedManifest` reporting syntax and type failures before semantic ones — so a real `sparqmod.toml` can now be read; sections v0 does not decode are still *key-checked*, because accepting `[ui]` while ignoring a typo inside it would be an invisible failure. New stamp **`src 65f/1034459B`**. Defects #60–#63 below. Increment 3 added the **registry and discovery** and task 5's two gates — the API-surface snapshot and the throwaway-module test that walks every engine source file to prove criterion 1 mechanically — so **all six acceptance criteria are now addressed** (3 only as far as Rust allows, and the log says so). **388 tests**, stamp `src 67f/1063788B`, defects #64–#65. Increment 4 closed the last two: **task 4** (`docs/module-author-guide-v0.md`, written from the three reference modules) and the **app-side scan** (`sparq modules [--root DIR] [--strict]` in `crates/sparq-app/src/modules.rs` — the only place in the workspace that reads a directory; the contract crate still holds no I/O). **WO-007 is complete.** The one piece deliberately left open is reading `compat-matrix.toml` at discovery instead of mirroring it in `port.rs` — carried to WO-008, because until then the mirror is the only copy of the matrix that can drift. **P1 device run (2026-09-23, SATURN physical): the first real MSVC build of the increment is green where it counts** — clippy audio+hal clean, release build 58 s, golden bit-identical (`ba577186c988db21`), selftest 8/8, `ui --audit` PASS, 1059× realtime, exe stamp `src 68f/1070593B` — and found six defects, #66–#71 below, fixed in `sync-p1-fixes.zip` (tests + scripts only; stamp unchanged) |
| WO-013 | **in progress — increment 2 (the bridge) built and headless-green; increment 2b closed the delivery gap (#78) that test005's first SATURN run found** | The graph canvas: `sparq-ui::canvas` (model with invertible ops + undo/redo, camera + LOD, computed layout + hit-testing, connect verdicts delegated to `sparq-module-api`'s own `connect_*`, the intent→op interaction table) + the egui painter in `sparq-app/src/ui/canvas_ui.rs` (nodes/wires/ports in the token signal-class language, glow/dim affordances, marquee, long-press menu). Shell routes canvas intents and binds the WO-012 `DoubleTap`/`Context`/`Undo` stubs. Demo patch = the reference modules that actually exist (#58 honoured). **474 tests**, `sparq ui --audit` PASS (16 smokes incl. 7 canvas; Design cells audit 24 touch targets), goldens unchanged, `ui-window` compiles. **Increment 2 shipped the bridge:** `sparq-app/src/bridge.rs` (ungated — the default CI test path exercises canvas graph → registry → executor → WAV), master resolution (SET MASTER + MASTER badge + the documented default rule), the RENDER WAV menu row, the registry-driven demo graph, `scripts/test005.bat` for SATURN. Artefacts `sparq-ui::canvas`, `docs/ui/gestures.md`. Defect #73 (recogniser release position) found + fixed. **Increment 2b (no product code):** test005's first run on
SATURN compiled the new `sparq-app` against the *previous* increment's `sparq-ui` — cargo answered
`Fresh` for an rlib whose source had been replaced by a zip with archive-restored mtimes (#41's
mechanism, on the side of the failure the stamp guard cannot see). Now `SYNC-STAMP.txt` +
`tools/sync_check.py` verify the tree by sha256 *before* cargo runs, and `build.bat` purges the
first-party fingerprints every build; `test005` reports both as step [00b]. Defect #78. **Awaiting device:** 60 fps at 200 nodes, real palm rejection (WM_POINTER, increment 2), full DPI matrix — none claimable over RDP. **Increment 3:** module browser + fuzzy search, inspector with touch sliders (per-node param state — the bridge renders manifest defaults until then), wire endpoint re-patch, live wire levels (needs WO-008 taps) |
| WO-008 | **in progress — increments 1–2 built and sandbox-green: the structural graph core + the executor** | `sparq-kernel::graph` (task 1): stable never-reused `NodeId`/`EdgeId`; `EdgeKind {Plain, UnitDelay, BlockDelay}` — §5.4's cycle vocabulary as a type; a topology **version** bumped on committed mutations only (latency edits are data, not topology); the **cached deterministic topological sort** (Kahn, smallest-id frontier — the same graph state always yields the identical order, which is what ADR-007 replay demands; `order_computes()` makes "recomputed only when the version changes" observable, ADR-009 decision 1); plain-edge **cycle refusal carrying the loop's path** in the error, rendered as a sentence that names the delay-edge remedy; delay edges close loops legally and stay out of the ordering; `remove_node` returns the detached edges for the journal/undo layer; per-node `latency_samples` stored for task 5. **Layering decision, recorded:** the kernel graph is *structural* — module-api sits above the kernel in the dependency order, so typed verdicts stay with `connect_*`'s single copy of the matrix and only validated edges are offered to the kernel. 12 tests incl. a 200-node/399-edge order at the acceptance scale; **430 tests total**, clippy/fmt/4 python gates clean, no new unsafe. **Increment 2 (same day):** `sparq-audio::executor` — builds a runnable patch from a kernel graph + contract modules and renders it block-by-block: channel negotiation (mono↔multi, buffer pool pre-allocated and pre-touched at build, budget reported per ADR-009 d4), one `Box<dyn Module>` dispatch per node per block (d7's hybrid), block-delay/unit-delay feedback memories refreshed at block end, `Failed` → silenced + flagged (never unwound), relaxed-atomic meters, deterministic fan-in sums, bit-identical renders across builds (hashed). **Measured: 0 allocations across 1000 blocks and across a 201-node block under the counting allocator**; 13 integration tests + 7 unit; the kernel gained the increment-2 cycle refinement (a `unit_delay` edge is in-block, so it ORDERS like a plain edge and cannot close a loop — loops must contain a `block_delay`; §5.4's own "keeps the executor a simple topological sort" clause, made structural, 2 new tests). **452 tests total**, clippy clean in three cells (workspace + both MSVC cross), goldens unchanged. **Remaining:** task 4 RCU swap + the 10 000-mutation stress test, task 5 latency accounting, task 6 watchdog (overruns are counted, not yet acted on), task 7 the determinism harness proper, multi-port `AudioCtx` (contract v1) so cv/event payloads travel — until then a non-audio edge is REFUSED at build in words, never silently ignored |
| WO-014 | **in progress — increment 1 built and sandbox-green: the first-party module batch** | The three reference modules promoted from contract tests to library code (`sparq-audio::modules`: `syn/sine`, `util/gain`, `ana/rms`), with their manifests as real files under `modules/` **and** compiled in via `include_str!` — one copy, two consumers (disk discovery + built-in registry), and a test that fails if the file and the binary ever drift. `sparq modules` discovers all three from disk; §11 precedence demonstrated live (the disk copies shadow against the built-ins, reported not errored). New **`sparq exec`** command: registry → factories → kernel graph → executor → WAV with no device — prints the ADR-009 d4 budget line, the analysis-tap value, the master meters and the render's golden hash. **Goldens checked in** (demo patch 2.8 s = `53de3b1f3f40e3c9`, sine 1 s = `3f325d4f99ca2a01`); zero allocations measured per module (5 000 `process` calls each) and through the executor path (1 000 blocks); the rms tap value and the master's metered rms agree to the bit (0.16621882 — analysis-as-control-source, cross-validated). **468 tests**, clippy clean (workspace + ui + MSVC audio cross), existing goldens unchanged. **Remaining:** the other 14 of the WO's 17 modules (Appendix-B order), per-module example patches + generated docs, polyblep aliasing measurement, the rms→filter modulation demo, and the <15 %-of-a-core benchmark on the stage machine |
| WO-009…WO-011, WO-015…WO-016 | not started | WO-009/010 follow WO-008 on the plan's own gate order. WO-013 builds directly on the shell: the gesture layer it needs (drag/pan/pinch/context/undo) is implemented and tested — but see #58: the canvas may not offer a converter module that does not yet exist |

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
- [ ] A trigger scheduled at tick T fires at exactly the corresponding sample index — verified for 1 000 random ticks against an analytically computed expectation (±0 samples).
- [ ] Tempo change during playback produces **no discontinuity** in either direction (tick↔sample mapping stays monotonic and continuous); verified by a sweep test and by listening.
- [ ] `t_wall` drift vs device clock is estimated and reported; over 30 min the reported drift matches an independent measurement within tolerance.
- [ ] Transport stop/start is sample-accurate and repeatable (same event twice ⇒ same output).
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
- [ ] `ana/rms` output demonstrably modulates a filter cutoff (the analysis-as-control-source principle proven end to end).
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
