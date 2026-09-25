# SPARQ — one-page brief

**What it is:** a self-built, real-time, touch-first modular instrument where audio, video, numerical and sensor streams are the sound source, the control source *and* the visual output. The patch is the composition; there is no timeline at the centre.

**What it is not:** a DAW, a plugin host, a randomiser.

---

### The seven pillars
1. Real-time correctness above features. 2. Deterministic & reproducible (same seeds ⇒ same samples). 3. Everything is a typed stream. 4. The module boundary is a frozen contract. 5. The UI is data-ink — scientific instrument, not skin. 6. Touch first, no modifiers, nothing hover-only. 7. Every phase ships a piece of music.

### The eight planes
Kernel (time/threads/graph) · Module system (contract, 3 tiers) · Stream bus (ingestion spine) · Audio (synth/sample/FX/analysis/spatial) · Music logic (transport/seq/harmony/mutation/form) · Graphics (custom WebGPU shell + display modules) · Library/network (content-addressed, local-first, registry) · Performance (modes/scenes/macros/capture/recovery).

### Signature capabilities (why it isn't generic)
* **Mutation as a subsystem**: L-systems, cellular automata, chaos attractors, Markov, spectral-of-sequence, genetic, geometry-as-score — composing into chains and a navigable **DNA tree**.
* **Geometry → sound**: draw a function or a surface and hear it (waveshaping, wavetermaining, wave terrain, lattice rhythms).
* **Analysis as control source**: every analyser output is a stream you can modulate with and display.
* **Live data**: MIDI 2.0/MPE, OSC, serial/IMU/EMG/BLE, camera optical flow, network feeds — with staleness handling, recording and rehearsal replay.
* **Spatial**: VBAP/VBAP3D, HOA ambisonics (order 1–7), binaural + HRTF, object mixer, room calibration workflow, up to 64 outs.
* **Journal + replay**: any performance can be re-rendered offline, bit-exact, because the score is the patch + seeds + journal.

### Recommended stack (locked)
**Rust core** · HAL over **WASAPI exclusive / ASIO** (Windows primary; PipeWire/JACK + CoreAudio in CI from Phase 1) · **egui scaffold now → custom retained-mode `wgpu` WebGPU shell in Phase 6** · native modules first + separate processes for ML/video, WebAssembly sandbox tier in Phase 5 · SQLite + content-addressed asset store · Faust as an optional module codegen path.

### Roadmap (solo, ~25–35 h/wk → 74–92 weeks)
| Phase | Weeks | Ships | Musical deliverable |
|---|---|---|---|
| 0 Proof of concept | 1–6 | zero-xrun HAL, module contract, graph executor, touch canvas, journal + replay | 60 s study |
| 1 Kernel + module contract | 7–14 | frozen API v1, hot reload, full journal, offline render | 2–3 min techno sketch |
| 2 Audio identity | 15–26 | full synth/granular/spectral/FX/analysis toolkit | **EP #1** |
| 3 Library & sample wrangler | 27–36 | asset store, search, slicing workbench | own sample pack + track |
| 4 Music systems | 37–50 | sequencers, harmony/tuning, mutation, DNA tree | **EP #2** (fully generative) |
| 5 Data plane | 51–62 | all stream adapters, Field Kit, wasm tier | documented data-driven set |
| 6 Interface & visuals | 63–76 | custom shell, display modules, Perform mode | **first live performance** |
| 7 Spatial audio | 77–88 | VBAP/HOA/binaural/calibration | surround release + spatial set |
| 8 Cloud, hardening, v1.0 | 89–102+ | registry/sync, soak tests, docs | **Album #1** |

### Phase 0 exit criteria (the gate that matters)
Zero xruns in a 2 h soak at 96 kHz / 64 samples on the stage device · a 12-module patch built entirely by touch in <3 min · bit-exact offline render **and** journal replay · 100 % of touch targets ≥44 px at 100–200 % DPI · a new module addable with zero engine edits · a screenshot that reads as "scientific software" · and 60 seconds of music only sparq could have made.

→ Fully ticketed in **`PHASE0-WORKORDERS.md`** (WO-000…WO-016, acceptance criteria, Windows setup checklist, cut list if time runs short).

### Top three risks
**Scope creep** (mitigate: phase gates + musical deliverables + API freeze). **Building a tool nobody plays** (mitigate: make music from week 1, keep a sketch environment as spec). **Stage reliability** (mitigate: watchdog, degradation ladder, journal recovery, physical fallbacks for every critical control).

### Decided (v0.2)
**Rust core** · **native + process-bridge modules now, WebAssembly tier in Phase 5** · **egui scaffold → custom `wgpu` shell in Phase 6** · **Windows primary** (WASAPI exclusive/ASIO, MMCSS, working-set lock, DPC discipline; Linux + macOS in CI from Phase 1).

### Still to decide (Phase 0–1)
Sample-rate & block-size defaults · library backend (rec: local-first) · distribution intent (rec: open core) · dedicated appliance (rec: post-v1.0) · stage touch device & interface channel count · bootstrap audio crate (rec: `cpal` for one work order only, then replace).

*Full spec: `SPARQ-PLAN.md`.*
