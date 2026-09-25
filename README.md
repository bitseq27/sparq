# sparq

> A real-time, touch-first modular instrument in which mathematical structures, geometry and live data streams are simultaneously the sound source, the control source and the visual output. Techno · breakcore · experimental. **The patch is the score.**

**Status:** WO-000…WO-005, **Phase B (DSP toolkit)** and **Phase C1 (music systems)** built and green · **WO-006** real-time HAL in progress (WASAPI verified on hardware over RDP — acceptance needs the physical machine) · **WO-012** increment 1 (the zero-dependency gesture core + the egui shell) · **WO-013** increments 1–2 (the graph canvas: nodes, wires, touch gestures, undo — `sparq-ui::canvas` + the egui painter — and **the bridge**: a drawn patch renders to WAV through the registry + executor, master designation included, no audio device needed) · **WO-008** increments 1–2 (the structural graph core — `sparq-kernel::graph` — and the executor: modules run in graph order, allocation-free, deterministic — `sparq-audio::executor`) · **WO-014** increment 1 (the first-party module batch — `sparq-audio::modules` + `modules/**/sparqmod.toml` + `sparq exec`: registry → executor → WAV, goldens checked in) · **WO-007** task 1 done (the contract, as data) and tasks 2–3 built (`sparq-module-api` + three conforming modules) · stack locked (Rust · egui→custom WebGPU shell · Windows primary)
**Code:** 5 crates, **474 tests passing** (+1 ignored), `clippy -D warnings` clean in the default and `ui` cells (the `ui-window` cell compiles; the sandbox OOMs on `naga` at `debuginfo=2`), zero third-party dependencies in the default build
**Design:** 3 token-driven mockups · 5 token files + generator + 2 auditors · 10 ADRs · module contract · manifest schema · **the contract as data** (compatibility matrix + manifest field table) · project format · input model

**On Windows, start with [`WINDOWS.md`](WINDOWS.md)** — `scripts\setup.bat` installs the toolchain, `scripts\devices.bat` lists your audio devices, `scripts\run.bat` makes a sound, and `scripts\diag.bat` captures everything needed to diagnose a silent run.

```
cargo test --workspace                                    # 388 tests, no sound card needed
cargo run --release -p sparq-app -- demo --out demo.wav   # 4 bars, no sound card needed
cargo run --release -p sparq-app -- demo --list-patterns   # every style and mutation preset
cargo run --release -p sparq-app -- demo --pattern breakcore --mutation grid-break --seed 7 --show-dna
cargo run -p sparq-app --features bootstrap-audio -- play  # live output
cargo run -p sparq-app -- render --out out.wav --seconds 10
cargo run -p sparq-app -- exec --out exec.wav --seconds 5    # registry → executor → WAV, no sound card
cargo run -p sparq-app -- modules                            # discover modules/**/sparqmod.toml
cargo run -p sparq-app -- selftest --golden               # Phase 0 gate table, measured
cargo run -p sparq-app -- soak --minutes 120              # long-run reliability gate
just gates                                                # everything CI runs
```

---

## Read in this order

| # | Document | What it gives you |
|---|---|---|
| 0 | [`WINDOWS.md`](WINDOWS.md) | **Get it running on your machine**: setup, devices, run, and what to do when there's no sound |
| 1 | [`docs/VISION.md`](docs/VISION.md) | One page. The seven pillars, the four signature ideas, the test that settles arguments |
| 2 | [`SPARQ-BRIEF.md`](SPARQ-BRIEF.md) | The whole project on one page |
| 3 | [`SPARQ-PLAN.md`](SPARQ-PLAN.md) | The master plan: 22 sections + 5 appendices. Architecture, subsystem specs, quality gates, roadmap, risks, budget |
| 4 | [`PHASE0-WORKORDERS.md`](PHASE0-WORKORDERS.md) | WO-000…WO-016: buildable tickets with acceptance criteria, tests, artefacts, a Windows setup checklist and a cut list |
| 5 | [`LATER.md`](LATER.md) | The parking lot. If it isn't in a work order, it lives here |

## Decisions

| Doc | Content |
|---|---|
| [`docs/adr/`](docs/adr/README.md) | ADR-000…009: conventions, Rust, module tiers, renderer path, Windows-primary, closed port types, three-clock model, determinism/journal, bootstrap audio, executor + HAL shape |
| [`docs/api/module-api-v1.md`](docs/api/module-api-v1.md) | **The module contract** — ports, params, state, voices, resources, the real-time rules, versioning, and a paper test of seven hard modules |
| [`docs/api/manifest-schema.md`](docs/api/manifest-schema.md) | `sparqmod.toml` field reference + the validation error catalogue |
| [`docs/api/compat-matrix.toml`](docs/api/compat-matrix.toml) | **What may connect to what, as data** — the single table host validation and the canvas affordances both read (ADR-005 addendum) |
| [`docs/api/manifest-fields.toml`](docs/api/manifest-fields.toml) | **Every manifest field, its domain and the error code it produces** — the validator is generated from this, so a new field brings its own codes |
| [`docs/formats/project.md`](docs/formats/project.md) | `.sparq` container, journal record types, and the whole format family |

## Design

| Doc | Content |
|---|---|
| [`design/token-spec.md`](design/token-spec.md) | Tokens as data, the generator, the ten hard CI-enforced rules, the font selection gate |
| [`design/look-board.md`](design/look-board.md) | **The conformance authority**: stroke language, numeric discipline, forbidden list, review checklist, five test protocols |
| [`design/tokens/*.toml`](design/tokens) | `colors` · `typography` · `layout` · `motion` · `colormaps` |
| [`design/mockups/design-mode.svg`](design/mockups/design-mode.svg) | Static mockup, 2560 × 1600 — the graph canvas, inspector and dock |
| [`design/mockups/perform-mode.svg`](design/mockups/perform-mode.svg) | Static mockup, 2560 × 1600 — the stage surface: full-bleed sonogram, hero clock, scenes, 8 macros, meters, panic |
| [`design/mockups/display-sheet.svg`](design/mockups/display-sheet.svg) | Static mockup — 12 display modules: scope, spectrum, sonogram, phase portrait, vector field, geometry, rig map, graph view, DNA tree, streams, meters, matrix |
| [`design/tokens/preview.html`](design/tokens/preview.html) | Generated token preview: every colour with contrast ratio, type specimens, ramps, live conformance report |
| [`design/mockups/mockup-review.md`](design/mockups/mockup-review.md) | Review protocol, measurement tables, colour-map region register, findings log |
| [`docs/hardware/stage-baseline.md`](docs/hardware/stage-baseline.md) | WO-001 measurement sheets: latency matrix, DPC budget, stage-mode checklist, touch ergonomics log |

## Identity in one breath

Near-black ground. Hairline structure in three opacity tiers. Five accents, one per **signal class** — amber audio, cyan control, magenta events, green data, violet spatial — each with a non-colour redundant encoding. Tabular monospace numerals, always with units. Stroke widths 1/2/3 (4 for Perform-mode hero glyphs). Corners 0/2/4 px. No shadows, no gradient decoration, no pure white, nothing that bounces. A screenshot with no title should read as *scientific software*.

## Tooling

`tools/` holds the design-time machinery (Python 3.11, stdlib only — see [`tools/README.md`](tools/README.md)). Every gate is **proven to fail on bad input**, because a gate that cannot fail is decoration:

| Tool | Purpose | Gate | Proven to fail? |
|---|---|---|---|
| `token_gen.py` | TOML tokens → `tokens.rs` / `.json` / `.css` / `.svg` / baked LUTs / `preview.html`, plus 10 conformance checks | `--check` fails on stale output or a failing check | ✅ caught a text-contrast failure and a missing `.cb` map |
| `token_audit.py` | Hard rules R1–R6 on mockups and widget source | exit 1 on any violation | ✅ all 6 rules fire on a bad fixture |
| `unsafe_audit.py` | ADR-000: `unsafe` only where allowlisted, exempted and justified | exit 1 on any violation | ✅ 3 negative fixtures (stray unsafe, missing SAFETY, unexplained lint relaxation) |
| `make_display_sheet.py` | Regenerates the display-sheet mockup from the tokens | run after any token change, then re-audit | ✅ caught an invented greyscale ramp |

**CI** (`.github/workflows/ci.yml`): design gates → rust matrix (Windows primary, Linux, macOS) with fmt, `clippy -D warnings` in each feature configuration, tests, golden reference, double-render determinism check, selftest and allocation probe → nightly Miri over the lock-free ring.

**Measured** (x86-64 Linux): **474 tests passing, 0 failed, 1 ignored** (2026-09-24, dev profile), including **0 allocations across 15 000 `process` calls** through `Box<dyn Module>` in the contract crate, **0 allocations across 1000 executor blocks and a 201-node block** (`sparq-audio::executor`, WO-008 increment 2), **34 canvas unit tests** (`sparq-ui::canvas`, incl. the master-resolution rule) and **3 bridge tests** (canvas graph → registry → executor → a real WAV file, WO-013 increment 2) and **14 graph-core tests** (`sparq-kernel::graph`); `sparq ui --audit` PASS with 16 smokes (6 chrome + 7 canvas + 3 breakpoint) — the two new canvas smokes are the full touch-driven render chain (double-tap → long-press → RENDER WAV → `canvas-render.wav` on disk with an evidence line) and SET MASTER, the two golden hashes unchanged (`ba577186c988db21`, `dd975a24f03b19c1` — no audio path touched). The release-profile figures that follow are from the Phase B/C1 ledger and were **not** re-measured on 2026-09-24: 0 allocations across 5 000 blocks · 656× realtime on the WO-005 chain · 20 853× realtime for the 4-bar Phase B patch · oscillator aliasing below the noise floor (measured against an analytic bandlimited reference) · two golden hashes reproducible in debug and release.

## Crate map

| Crate | Contents | `unsafe` |
|---|---|---|
| `sparq-kernel` | block context, three-clock model, **the seed tree (ADR-007)**, allocation-counting RT discipline, lock-free SPSC ring, null device, counters | **yes** — allowlisted, 2 modules |
| `sparq-audio` | DSP nodes (sine, gain, SVF filter, additive osc, AD/ADSR envelopes, 5-colour noise, dust, bitcrush, delay), a test FFT, the WO-005 chain, the demo patch, **patterns + Euclidean generation + polymeter, 25 mutation operators, the DNA lineage tree, style presets**, WAV I/O, FNV-1a golden hashing | no |
| `sparq-ui` | generated design tokens (295 constants) | no |
| `sparq-app` | CLI: `render` / `selftest` / `soak` / `golden-values` / `play` (feature-gated bootstrap) | no |

## Where the work starts

**Done:** WO-000 (workspace, lints, CI, `unsafe` policy and its auditor, golden-file conventions) · WO-002 (tokens, generator, preview, look board) · WO-003 (`VISION.md`, ADR-000…009) · WO-004 (all three mockups + review protocol) · WO-005 (offline render path, DSP chain, diagnostics, feature-gated bootstrap live path) · plus WO-007's contract draft, WO-011's format draft, WO-012's input model, and a soak tool that WO-016 will need.

**What still needs you, in order:**

1. **WO-001 — hardware.** Choose the stage touch device and interface (D-11/D-12), then fill in `docs/hardware/stage-baseline.md`. Nothing ergonomic after this is valid unless measured on that device. Windows-specific: WASAPI exclusive and ASIO latency tables, and the DPC/ISR budget — the Linux numbers measured so far do not transfer.
2. **WO-005 — hear it.** `cargo run -p sparq-app --features bootstrap-audio -- play`. Needs a real sound card, so it has never been run: it compiles and lints clean on Linux/macOS/Windows in CI, but nobody has heard it yet.
3. **WO-002 — fonts.** Fill `chosen = ""` in `typography.toml` against the 8-point gate.
4. **WO-004 — viewers.** Run the blind identity test on the three mockups; log verbatim answers.

**Phase A (sound out of Windows) is built and awaiting hardware:** the bootstrap path now enumerates devices on every backend, walks an attempt ladder (requested format → device default → 48k stereo → 44.1k stereo) reporting each failure, handles F32/I16/U16, prints live callback telemetry, and can capture to WAV in parallel so a silent run is diagnosable in one pass. It has never produced sound — no audio device exists in the build environment.

**Next build step: WO-006 hardware acceptance, then increment 2.** The HAL is built — `sparq devices`, `sparq play --backend wasapi-exclusive`, `sparq soak --backend …`, the conformance suite, MMCSS + working-set discipline, the whole ADR-009 shape. What remains: run `scripts\hal.bat` + `scripts\soak.bat 120 wasapi-exclusive` on the stage machine (zero-xrun acceptance, latency table, unplug test), then ASIO + full duplex + the round-trip measurement utility. When the acceptance soak passes, the cpal bootstrap is deleted (ADR-008) and the HAL becomes `play`'s default.
