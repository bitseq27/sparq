# SPARQ
### A self-built, non-linear, data-driven instrument for techno, breakcore and experimental electronic music

**Document:** Master Plan v0.2 (no code — architecture, scope, roadmap)
**Date:** 2026-09-20
**Status:** Core decisions locked (D-1 Rust · D-2 native+process tiers first · D-3 egui scaffold → custom shell · D-4 Windows primary). Phase 0 work orders: see `PHASE0-WORKORDERS.md`.
**Owner:** you (solo artist-engineer), with optional collaborators per phase

> **v0.2 changes:** decisions D-1…D-4 resolved and folded into §5.2, §5.5, §16.1 and §22; Windows is now the primary platform (with a dedicated real-time and device section); Phase 0 broken out into a separate work-order document.

---

## 0. How to read this document

| Section | What it answers |
|---|---|
| 1–3 | What sparq *is*, who it's for, what it must sound and look like |
| 4 | The whole system on one page |
| 5–15 | Deep spec of each subsystem (this is the buildable part) |
| 16 | Stack choice + the three realistic build strategies |
| 17 | Roadmap, milestones, exit criteria |
| 18–20 | Risks, budget, repo layout |
| 21–22 | First 30 days, decisions you must make |
| A–E | Naming, formats, module taxonomy, glossary, references |

Everything in §5–15 is written so a single engineer can turn any subsection into a ticket without further design work. Where a choice is genuinely open, it is marked **[DECISION]** and collected in §22.

---

## 1. Executive summary

**sparq** is not a DAW. It is a *scientific instrument for making music*: a real-time modular system where audio, video, numerical and sensor data streams are ingested, conditioned, routed and rendered through a graph of user-extensible modules, and where the interface itself is a visual performance surface.

Four things make it different from anything you can buy:

1. **Non-linear by construction.** There is no timeline at the centre. The centre is a *living patch* — a graph that runs continuously, mutates, and is constrained rather than arranged. Composition = designing and steering a system. Arrangement happens as *scenes*, live.
2. **Everything is a stream.** Audio, control voltage, MIDI 2.0, OSC, serial/IMU/EMG sensors, camera video, network feeds, CSV/time-series, ML latents. All normalised into one typed stream bus with timestamps, so any stream can modulate any parameter and any parameter can be displayed.
3. **The UI is the identity.** A cohesive, scientific, touchscreen-first graphical language — hairline traces, phosphor palette, monospaced numerals, oscilloscope/spectrogram/attractor displays — that works identically as an editing tool and as the projected visual for a live set. The instrument looks like the music sounds.
4. **Open at the edges, closed at the core.** A frozen, versioned module API with three execution tiers (native / WebAssembly / external process) means you can add modules forever without destabilising the engine or breaking existing patches.

**Target quality bar:** 96 kHz capable, 64-sample blocks with zero xruns under full performance load, up to 64 output channels with HOA ambisonics + VBAP + binaural, offline render bit-reproducible, sub-10 ms controller-to-sound round trip.

**Realistic effort:** 74–92 focused weeks solo (~15–20 months at a sustainable part-time pace), to v1.0. Crucially, **you make music with it from week 3** — every phase ends with a musical deliverable, not just a technical one. The tool and the art are the same project.

---

## 2. Vision and product definition

### 2.1 One-sentence definition

> sparq is a real-time, touch-first modular synthesis and performance environment in which mathematical structures, geometry and live data streams are simultaneously the sound source, the control source and the visual output.

### 2.2 What it is / what it is not

| sparq **is** | sparq **is not** |
|---|---|
| An instrument you play | A DAW you edit in |
| A patch/graph-first environment | A clip/timeline-first environment |
| A generative system with constraints | A randomiser |
| A performance surface (UI = visuals) | A plugin host with a mixer |
| A personal, evolving standard | A commercial product for a market |
| Sample-accurate, deterministic, reproducible | "Good enough" live coding |
| Extensible by you, forever | Extensible by a vendor's roadmap |

### 2.3 Users

1. **You, the composer-performer** (primary). Every design decision optimises for: *can I get the sound in my head, and can I play it on stage, tonight?*
2. **You, the builder** (same person, different hat). The module API must be pleasant enough that writing a new module is a 2-hour task, not a 2-week task.
3. **Future: a small circle of collaborators** who can share presets, modules and sample packs through the library plane (§13) without you maintaining their machines.

### 2.4 Core use cases (these drive every architectural decision)

* **UC-1 — Sound design session.** Build a kick from membrane model + noise burst + spectral resynthesis of a metal hit; mutate it 200 ways; audition by morphing; save the whole family to the cloud library.
* **UC-2 — Break construction.** Load a break, auto-slice on transients, scatter slices across a polyrhythmic 7-over-4 grid with per-slice probability, ratchets and microtiming; run a mutation operator chain seeded from a Lorenz attractor; capture the result as a phrase.
* **UC-3 — Generative patch.** A patch that never repeats in 3 hours: chord engine over a 19-TET scale, granular voices, L-system phrase expansion, cellular automaton gating, all modulated by a slow data stream (tide level / CPU load / a random walk).
* **UC-4 — Live set, 45 min, quad or 8-channel.** Performance mode on a touchscreen tablet: scenes, 8 macros, one big transport readout, full-bleed visualiser mirrored to the projector. No editing surface visible. Recovery from any crash in <10 s.
* **UC-5 — Data performance.** Wear an IMU glove + EMG armband; arm angle drives a filter cutoff and the spatial trajectory of a granular cloud; heart-rate variability drives mutation probability; camera optical flow drives reverb size. The visuals show the streams live.
* **UC-6 — Installation / kiosk mode.** Unattended, deterministic-ish, self-healing, runs for 72 h on a mini-PC, drives a 12-speaker array and a wall projection.
* **UC-7 — Extending the system.** You invent a new synthesis idea on a Sunday; by Sunday night there's a `syn/xyz` module in the graph, hot-loaded, and it appears in every existing patch as an optional insert with zero breakage.

---

## 3. Design pillars, identity and influences

### 3.1 The seven pillars (arbitration rules for every future decision)

1. **Real-time correctness above features.** If a feature risks an xrun, it is deferred or moved off the audio thread. No exceptions.
2. **Deterministic and reproducible.** Same patch + same seeds + same inputs + same time ⇒ same samples. Sessions are journals, not just files.
3. **Everything is a typed stream.** No special cases. Audio is just a high-rate float stream; a sensor is just a low-rate one.
4. **The module boundary is a contract.** Additive evolution only. Old patches load forever.
5. **The UI is data-ink.** Every pixel represents a real signal or a real control. No decoration. Scientific instrument, not skeuomorph.
6. **Touch first, mouse tolerated, keyboard optional.** Nothing may require a modifier key, a right-click or a hover to be usable on stage.
7. **Music ships every phase.** A phase without a track made with the current build is an incomplete phase.

### 3.2 Influence → capability map

This is the most important table in the document. It converts taste into engineering requirements, so the build never drifts into generic synth territory.

| Influence | Signature technique | Required sparq capability | Subsystem |
|---|---|---|---|
| **Autechre** | Bespoke Max patches as the *master recording*; systems that compose themselves; FM/granular textures; metallic inharmonic timbres; alien but *logical* rhythmic grids | Patch-as-score: the project file must be the composition. Generative graph runtime, FM operator graphs, granular clouds, inharmonic/partial-based synthesis, non-standard subdivision lattices, deterministic replay | §5, §6, §9, §10 |
| **Aphex Twin / Drukqs-era** | Extreme DSP chains, prepared-piano-ish acoustics mangled, glitch, granular | Deep per-module FX insert chains, convolution & spectral mangling, physical models, resampling artefacts as features | §9 |
| **Venetian Snares** | Sliced breaks at absurd BPM in odd metres; orchestral samples collided with snares; chaotic-but-intentional | Sub-sample-accurate slicing, polyrhythm/polymetre per track, microtiming, huge randomised sample access, spectral resynthesis of acoustic sources, fast auditioning | §9, §10, §11 |
| **Squarepusher** | Acoustic ↔ synthetic collision; jazz harmony under breakcore; expressive bass; live bass + electronics | Chord/voicing engine with real theory + alternate tunings, MPE and MIDI 2.0 per-note expression, physical-model bass, live input monitoring with sub-10 ms latency, record-to-phrase | §8, §10 |
| **Richard Devine** | Maximalist IDM/glitch sound design; enormous preset ecosystems; intricate generative arpeggiation; surgical sound sculpture | Morphing between presets/parameters, huge fast preset library with similarity search, generative arp/phrase mutation, batch render, deep FX rack | §11, §13 |
| **Ryoji Ikeda / Alva Noto** | Data as material; sine/impulse minimalism; audiovisual unity; scientific presentation | Pure sine/impulse generators at exact frequencies, numeric data sonification modules, ultra-clean typography-led visuals, A/V lock to the sample | §7, §14 |
| **Rastellini / Pan Sonic / Russell Haswell** | Brutal loudness, distortion as structure, analogue chaos | Wavefolder/feedback path support, high-headroom internal bus, true-peak-safe limiting, feedback with unit-delay safety | §9, §5 |

**Synthesis of the above:** sparq is a *mutation machine + sample engine + spatialiser + live-data instrument* whose score is its patch. That is the product.

### 3.3 Visual identity (this is a deliverable, not a skin)

* **Reference frame:** laboratory instrumentation — oscilloscope, spectrum analyser, seismograph, technical drawing, astronomical plate — reinterpreted in software.
* **Palette:** near-black ground (`#07090C`), phosphor traces. One accent per *signal class*, never per module:
  * amber `#FFB347` — audio
  * cyan `#4FD8E8` — control / CV
  * magenta `#FF5FA2` — events (MIDI, triggers)
  * green `#8BE36A` — data streams (sensor, network, numeric)
  * violet `#A98CFF` — spatial / multichannel
  * neutral greys for structure, hairlines at 1 px, 10 % / 25 % / 60 % opacity tiers
* **Typography:** one variable monospace with true tabular numerals for *all* numbers, labels and units; one geometric grotesque for headings and long text. Numerals are the most important glyph set in the product.
* **Graphic language:** 8 px base grid; hairline rules; tick marks and crosshairs; SI units and prefixes everywhere (Hz, dB, ms, LUFS, ch); no drop shadows, no gradients-as-decoration, no rounded-corner-everything; glow only on signal traces (bloom pass), intensity ∝ amplitude.
* **Motion:** traces redraw at frame rate; transitions are 120–200 ms with a critically-damped curve; nothing bounces.
* **Rule of thumb:** if a screenshot of sparq were shown without a title, it should read as "scientific software," not "music software."

### 3.4 Brand and naming

* **Name:** `sparq` — lowercase, always. Evokes ignition, a discharge across a gap, and the discrete sample/quantum of sound.
* **Mark:** a node-graph intersection rendered as a four-point spark, drawn in hairline strokes with one vertex highlighted. Must be legible at 16 px and at 3 m on a projector.
* **Product family naming** (for later modules/companions): `sparq core`, `sparq field` (sensor kit), `sparq stage` (performance appliance), `sparq cloud` (library).

---

## 4. System overview

### 4.1 Layer diagram

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  PERFORMANCE PLANE (§12)   design mode · perform mode · install mode          │
│                            scenes, macros, controllers, capture, safe-mode    │
├──────────────────────────────────────────────────────────────────────────────┤
│  GRAPHICS / INTERFACE PLANE (§14)                                             │
│   ┌─────────────────────────┐  ┌──────────────────────────────────────────┐   │
│   │ Shell: canvas, panels,   │  │ Display modules: scope, spectrogram,     │   │
│   │ inspector, browser,      │  │ phase portrait, vector field, geometry,  │   │
│   │ mapping, transport       │  │ attractor, DNA tree, stream inspector    │   │
│   └─────────────────────────┘  └──────────────────────────────────────────┘   │
│            custom retained-mode renderer on WebGPU (wgpu) + design tokens     │
├──────────────────────────────────────────────────────────────────────────────┤
│  MUSIC / LOGIC PLANE (§10, §11)                                               │
│   transport · clocks · sequencers · chord engine · mutation engines ·         │
│   phrase/DNA history · form engine · tuning                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│  AUDIO PLANE (§9)                                                             │
│   synthesis · sampling · FX · analysis · dynamics · SPATIAL (VBAP/HOA/binaural)│
├──────────────────────────────────────────────────────────────────────────────┤
│  STREAM BUS (§7)  — the spine of the whole system                             │
│   typed channels + timestamps: audio / cv / event / data / gpu-texture        │
│   sources: audio in, cameras, serial, HID, BLE, MIDI 2.0, OSC, network, files │
│   conditioning: rate convert, smooth, calibrate, map, scale, condition, record│
├──────────────────────────────────────────────────────────────────────────────┤
│  MODULE SYSTEM (§6)  — the extension contract                                 │
│   manifest · ports · params · state · voices · tiers (native / wasm / process)│
│   discovery · hot reload · versioning · sandbox · registry                    │
├──────────────────────────────────────────────────────────────────────────────┤
│  REAL-TIME KERNEL (§5)                                                        │
│   graph executor · scheduler · clocks · memory arenas · audio device HAL      │
│   offline render · journaling · deterministic replay                         │
├──────────────────────────────────────────────────────────────────────────────┤
│  LIBRARY / NETWORK PLANE (§13)                                                │
│   content-addressed asset store · metadata DB · fingerprints & embeddings ·   │
│   sync · registry · signing & provenance · sharing                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 The six planes and what each owns

| Plane | Owns | Must never do |
|---|---|---|
| Kernel | Time, threads, memory, graph execution, device I/O, offline render, journal | Know anything about music theory or visuals |
| Module system | Contract, discovery, lifecycle, sandboxing, versioning | Execute DSP itself |
| Stream bus | Type normalisation, timestamps, rate conversion, fan-out, recording of streams | Generate sound |
| Audio | Synthesis, sampling, FX, analysis, spatialisation | Allocate memory or block |
| Music/logic | Transport, patterns, harmony, mutation, form | Touch audio buffers directly (it emits *events*) |
| Graphics | Rendering, hit-testing, gestures, visual identity | Run on the audio thread; block on I/O |
| Library/network | Assets, metadata, sync, registry, trust | Ever be on a critical path for playback |
| Performance | Modes, scenes, macros, capture, recovery | Expose destructive editing on stage |

### 4.3 Process model

```
┌────────────────────────── sparq (single process by default) ─────────────────────────┐
│                                                                                      │
│  [AUDIO THREAD]  pinned, RT priority, lock-free, zero-alloc                          │
│     │  reads: param snapshots, event queue, stream snapshots                         │
│     │  writes: output buffers, analysis taps, meter ring, journal events             │
│     ▼                                                                                 │
│  [CONTROL THREAD]  param interpolation, automation, scene morph, graph mutation      │
│     ▲                             (applies changes at block boundaries only)         │
│  [UI THREAD]  60–120 fps, input, layout, render submit                               │
│     ▲                                                                                 │
│  [IO THREADS]  audio device, MIDI, OSC, serial, HID, BLE, network, camera            │
│     ▲                                                                                 │
│  [WORKER POOL]  analysis, library indexing, ML inference, offline render, file decode│
│                                                                                      │
│  [TIER-3 MODULES]  separate sandboxed processes (Python/Node/etc.) ← shm rings       │
│                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

All cross-thread communication uses bounded lock-free ring buffers (SPSC/MPMC) with explicit overflow policy (`drop-oldest` for streams, `block-never` for audio, `drop-and-count` for UI telemetry). Every ring has a counter exposed in the diagnostics panel — invisible failures are forbidden.

---

## 5. Real-time kernel

### 5.1 Time model — three clocks, one master

| Clock | Unit | Owner | Purpose |
|---|---|---|---|
| `t_sample` | integer sample count at device rate | audio device / offline renderer | **master**. All DSP scheduling |
| `t_musical` | ticks (default 960 PPQN), with bar.beat.tick view | transport | Sequencing, chord engine, form |
| `t_wall` | microseconds since epoch (monotonic) | OS | Streams, journal, sync to external world |

* A single `ClockBroker` maintains the mapping `t_sample ↔ t_musical ↔ t_wall` as a piecewise-linear function with *smoothed* derivative, so tempo change never produces a discontinuity in either direction.
* **External sync** (MIDI clock, Ableton Link, OSC transport, LTC/MTC later) is a *slave PLL*: it estimates rate + phase and drives `t_musical` with a critically-damped correction (configurable stiffness), never a hard jump. Manual override always wins within one block.
* **Stream alignment:** every stream sample carries `t_wall` (and optional `t_sample` if it originates inside sparq). The Stream Bus resamples/holds onto the audio timeline with an explicit, per-source latency budget (jitter buffer, default 4 ms, configurable 0–50 ms).
* **Latency accounting:** each module declares its latency in samples (lookahead filters, convolution, granular). The executor computes per-path latency and auto-compensates *only* where the user marks a path as `compensate: true` — deliberately, because in experimental music latency is a timbral resource, not a bug. There is a global "raw / compensated" switch and a per-path latency readout in the UI.

### 5.2 Threading and real-time discipline

Hard rules (enforced by lint + review + runtime assertions in debug builds):

1. The audio thread performs **no** allocation, **no** lock, **no** syscall, **no** page fault, **no** unbounded loop.
2. All parameter changes reach audio as **immutable double-buffered snapshots** swapped at block boundaries, with per-parameter smoothing applied *inside* the audio thread.
3. Graph mutations (add/remove/reconnect module) happen on the control thread and are applied atomically at a block boundary via an RCU-style pointer swap. The audio thread never sees a half-built graph.
4. Per-module **watchdog**: block processing time is measured; a module exceeding its budget N times in a row is auto-bypassed, flagged in the UI with a red hairline, and logged to the journal. The set survives.
5. **Graceful degradation ladder** (auto, then manual): reduce analysis FFT size → reduce display refresh → reduce voice count → reduce oversampling → increase block size. Each rung is logged and reversible.
6. Audio thread pinned to a core (configurable), RT priority, memory pre-faulted and locked, all arenas pre-reserved at patch load.

**Windows-specific real-time discipline (primary platform, D-4):**

* Audio thread runs under **MMCSS** (`Pro Audio` task, `AVRT_PRIORITY_CRITICAL`), with `AvSetMmcThreadCharacteristics` + explicit ideal-processor assignment; *never* rely on `SetThreadPriority` alone.
* **Working set locked** (`SetProcessWorkingSetSizeEx` with `QUOTA_LIMITS_HARD_WORKING_SET`), pages pre-touched at patch load so no fault occurs on the audio path.
* **Timer resolution**: do not globally raise the system timer; use `waitable timer` with `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` where needed and keep the audio thread free of waits entirely.
* **DPC/ISR latency** is the main xrun source on Windows: ship a diagnostics routine that surfaces suspected DPC stalls (use LatencyMon-class measurement during setup), and document the standard mitigation set — disable NIC power saving/EEE, disable USB selective suspend, high-performance power plan, core parking off, GPU driver scheduling mode, disable unnecessary services, and a "stage mode" preset that applies them and verifies.
* **Process priority**: sparq runs as a foreground high-priority process; Perform mode optionally raises process priority class and suppresses Windows notifications/focus-stealing (a real stage risk).
* **Audio session policy**: exclusive mode for the performance device; keep a separate shared-mode path for UI cues/monitoring so a browser or system sound can never hijack the performance device.
* **ASIO** support as a first-class alternative to WASAPI exclusive (many multichannel interfaces only behave properly under ASIO); the HAL abstracts both behind one interface and reports the same capability set.
* Linux (PipeWire/JACK) and macOS (CoreAudio) backends remain in CI from Phase 1 — Windows-primary does not mean Windows-only.

### 5.3 Memory model

* **Arenas per subsystem**, reserved at load, never grown on the audio path: voice arenas, delay-line pool, FFT scratch, per-module work buffers, ring buffers.
* **Delay lines** are the biggest memory consumer; a dedicated pool with reference-counted shared buffers, size quantised to powers of two, allocated at patch-load with a printed budget (`delay pool: 412 MB / 1024 MB`).
* **Zero-copy everywhere possible:** sample data is `mmap`'d from the asset store; analysis results are read via atomic slices; GPU uploads use staging buffers.
* **Object pooling** for voices, grains, events, particles.
* Everything measurable: a `Memory` panel shows per-arena, per-module, per-tier usage live.

### 5.4 Graph executor

* Modules = nodes; ports = typed edges. **Acyclic by default**; explicit feedback allowed via `unit_delay` / `block_delay` edges which are the *only* legal cycle-closers (this keeps the executor a simple topological sort and makes feedback audible-but-safe).
* Execution order computed once per graph version (dirty flag), cached, and validated on every mutation.
* **Block size:** configurable 32 / 64 / 128 / 256 / 512 / 1024 / 2048 samples, default **64** for performance work, 256 for editing on weaker machines. Sub-block processing allowed per module (e.g. a granular module may internally run at 16).
* **Channel model:** every audio edge carries a `ChannelSet` (count + layout tag: `mono`, `stereo`, `quad`, `5.1`, `7.1.4`, `ambisonics:N`, `objects:K`, `raw:M`). Connection rules are explicit and *never* silently guess: mono→multi fans out, multi→mono sums with a warning hairline, ambisonic edges refuse to connect to non-spatial ports unless a decoder/encoder node is inserted (the UI offers to insert it).
* **Sample formats:** internal processing `f32` per-sample with `f64` accumulators in summing buses, filters, meters and analysis; optional global `f64` mode for offline render and for scientific accuracy. Output dithered/limited per master chain config.
* **Polyphony:** a node declares a `VoicePolicy` (mono, poly-N, MPE, multitimbral). Polyphonic nodes are executed by a voice manager with allocation, stealing (`oldest`, `quietest`, `highest`, `round-robin`, `priority`), unison (n, detune curve, spread), and per-voice parameter snapshots. Voices are pooled; no allocation on note-on.
* **Offline render:** identical executor, driven by a virtual device, multithreaded by splitting the timeline into segments *only* where the graph is stateless-per-segment; otherwise single-threaded for determinism. Bit-reproducible output is a test assertion, not a hope.

### 5.5 Device / HAL layer

* **Backend priority (Windows primary, D-4):** 1) **WASAPI exclusive** (event-driven, lowest latency, multichannel), 2) **ASIO** (required for many multichannel interfaces and for reliable 8–32 out operation), 3) WASAPI shared (fallback, and for the non-performance monitoring path), 4) **null/virtual device** (offline render + CI, deterministic), 5) PipeWire/JACK (Linux), 6) CoreAudio (macOS).
* One HAL trait over all backends: `enumerate → open(config) → start/stop → callback(frames, in, out) → latency report → error/xrun report`. Capability flags per device (channel count, sample rates, exclusive mode, event-driven, hardware latency) are surfaced in the UI, not guessed.
* Aggregated/multi-device operation: Windows has no native aggregate device, so sparq implements its own **device aggregator** (clock-master selection, drift compensation via a resampling PLL, per-device latency alignment) to reach 16/32/64 outs from several interfaces. This is a Phase 7 requirement for large surround rigs and must be designed for from the start.
* Per-output-channel routing matrix with rename, invert, delay (samples), trim (dB), mute, solo — this is the *speaker rig* definition, saved per-venue as a **Rig Preset** (§9.6).
* Full duplex with independent input/output latency compensation; input monitoring path guaranteed shortest-possible (direct `in → out` copy node with a fast path).
* Diagnostics: xrun counter, block time histogram (p50/p99/max), callback jitter, device clock drift estimate vs `t_wall`, and a **round-trip latency measurement** utility (loopback impulse) — displayed in the UI and asserted in CI.

### 5.6 Journaling, determinism, reproducibility

Every session writes an append-only **journal** (binary, content-addressed chunks):

* graph mutation events, parameter automation, scene changes, transport events, stream *recordings* (optionally, for any stream), RNG seed allocations, module versions, asset hashes, device config, xruns and degradation events.

This buys you:

* **Deterministic replay** of a performance for debugging and for regenerating a render exactly.
* **Undo across sessions** and **time-travel** to any point in a performance.
* **"Score" export:** the journal + patch is the composition. This is the Autechre lesson — the patch *is* the master.
* **Crash recovery:** on restart, sparq offers "resume at last good block" in <10 s.

Randomness is managed by a **seed tree**: every random consumer registers and gets a deterministic sub-seed derived from (session seed, module id, consumer id). Re-seeding one module never perturbs another. This is what makes generative patches *reproducible* rather than merely random.

---

## 6. Module system — the extension contract

This is the part that guarantees "build on it without affecting other modules."

### 6.1 Three execution tiers

| Tier | Runtime | Latency budget | Use for | Safety | Hot reload |
|---|---|---|---|---|---|
| **T1 — Core** | Native, statically linked, first-party | Tightest (per-sample DSP) | Oscillators, filters, granular, spatial, analysis | Language-level safety | Rebuild required |
| **T2 — Sandboxed** | WebAssembly (WASI 0.2 + Component Model), linear memory, fuel/instruction metering, no ambient authority | ~1.2–2× native | Third-party and experimental modules, anything untrusted, anything you want to write in another language | Strong sandbox, crash-isolated | **Yes**, at block boundary |
| **T3 — Bridged** | Separate OS process, shared-memory ring + typed IPC | Non-real-time to ~1 ms | ML inference, Python/Rust prototyping, network scrapers, video analysis, hardware daemons | Process isolation | **Yes**, restart-in-place |

Design consequences:
* T2 gives you a *marketplace/registry* possibility later and lets collaborators contribute modules in Rust, C, Zig, AssemblyScript, Python (compiled) without ever crashing your set.
* T3 keeps heavy and slow things (a neural vocoder, a Python data scraper, OpenCV) completely off the audio thread while still feeling like "modules."
* T1 stays lean and fast. **Rule:** a module is T1 only if it is on the critical DSP path *and* first-party *and* covered by golden tests.

**[DECISION D-2]** Start with T1 + T3 (simplest, fastest to value), add T2 in Phase 5–6 once the manifest/port types have stabilised. Building the wasm ABI first is a classic way to burn six months before making a sound.

**[D-2, amended 2026-10-04 → ADR-010 / D-14]** The third-party instrument requirement pulled T2 forward: **T2 is the instrument tier** — contract v1.1 freezes in Phase 1 (WO-017), the loader/validator ship in Phase 2 (WO-018), and the first handed-off instruments land in Phase 2 (WO-019). Backbone modules stay T1 under the unchanged admission rule; heavy non-real-time work stays T3. The original warning stands — the wasm ABI is frozen *after* the native contract has been proven on real modules, which by Phase 1 it has been.

### 6.2 Manifest (declarative, versioned, signed)

Each module ships a manifest (`sparqmod.toml` / embedded in the wasm component) declaring:

```
identity      : id (reverse-DNS or namespace/name), display name, version (semver),
                min/max host API version, author, license, signature, repo, docs url
category      : taxonomy path, e.g. "synth/oscillator/wavetable"
classification: realtime | analysis | source | processor | utility | spatial | data | display
ports         : typed list (see 6.3) with direction, channel set, rate, optional/required
params        : typed list (see 6.4)
voices        : policy, max polyphony, per-voice param set, allocation hints
state         : schema id + version, migration chain, size class, serialisable? (must be yes)
resources     : cpu hint, memory hint, gpu hint, latency (samples), tail (samples),
                requires (features: fft, midi2, spatial, ml), assets (bundled files)
ui            : panel descriptor (see 6.6), display outputs, colour class
lifecycle     : init / reset / suspend / resume / dispose semantics, thread affinity
safety        : allowed capabilities (fs read scope, net, device) — enforced for T2/T3
```

Manifests are **validated at discovery time**; a bad manifest never loads. Manifests are also the source of truth for documentation generation and for the module browser UI.

### 6.3 Port type system

Ports are strongly typed and the type system is *closed* (fixed set), which is what keeps modules interoperable forever.

| Type | Payload | Rate | Notes |
|---|---|---|---|
| `audio` | float frames, `ChannelSet` | per sample | layout-tagged; spatially aware |
| `cv` | float, normalised −1..1 or 0..1 (declared) | per sample or per block | the universal modulator |
| `event` | MIDI 2.0 UMP, OSC bundles, triggers, gates, notes w/ MPE | timestamped in `t_sample` | sample-accurate |
| `data` | typed record stream (see §7.2) | timestamped in `t_wall` | sensors, network, numeric, files |
| `gpu` | texture / buffer handle | per frame | video, visual feedback, compute results |
| `atom` | opaque serialised blob (for host↔module messages) | on demand | config, file refs, structured messages |

Connection rules are enforced by the host and surfaced in the UI as *affordances*: compatible ports glow when a drag is in progress; incompatible ones dim; "requires adapter" offers one-tap insertion of the right converter (e.g. `data→cv` offers a Mapper node).

### 6.4 Parameter model

Every parameter declares: `id, name, unit, type (float/int/bool/enum/text/blob), range, default, curve (lin/exp/bipolar/custom), smoothing (none/lin/one-pole/lag ms), modulation depth, automation-writable, randomisable, per-voice?, hidden?`.

* **Modulation matrix is universal:** any param can be a target; any `cv`, `data` channel, `event` CC or LFO can be a source, with `amount`, `curve`, `bipolar`, and `smoothing`. This is one system, not three.
* **Morphing:** every module exposes a *parameter vector*; sparq can interpolate between two vectors (used by preset morph, scene crossfade, and DNA-tree audition). Modules must therefore keep parameters *semantically continuous* where possible — a documented guideline for module authors.
* **Macros:** a macro is a named bundle of (target, curve, amount) mappings, assignable to a touch surface, controller, or data stream. Macros are how you play a patch.
* **Randomise with lock:** per-parameter lock flags, "randomise unlocked," "nudge," and "seeded randomise" (reproducible).

### 6.5 Module lifecycle

`discover → validate → sign-check → instantiate → configure(state) → prepare(resources, sample rate, block size, channel sets) → activate → process(blocks) → deactivate → dispose`

* `prepare` may allocate; `process` may not.
* `configure` is called on the control thread; `process` on the audio thread. State crossing that boundary is only ever the double-buffered snapshot.
* **Version migration:** state blobs carry a schema version; a module declares a migration chain. Loading a 2027 patch in 2031 must work — the chain runs, the result is re-saved, and the original is preserved.

### 6.6 Module UI descriptor (panels are data, not code)

A module's front panel is described declaratively so the shell renders it consistently (this is what makes the UI *cohesive* rather than 200 different plugin skins):

* widget list (slider/knob/xy-pad/matrix/enum/label/scope-attach), geometry on a 8 px grid, label, unit, param binding, visibility conditions, touch target class (`S/M/L/XL`), and *display slots* (which analysis outputs to render, with which display module and colour map).
* **Graphical displays are host-rendered scene data (ADR-010).** Beyond display slots, a module may declare *displays* that emit a **display list** (2D: paths, polylines, rects, arcs, glyph runs, point clouds, heat cells) or a **scene descriptor** (3D: camera, points, lines, triangle meshes, heightfields) per frame; the host renders them with its own pipeline against the current design tokens. Instruments never own pixels — that is what makes app design changes propagate into every third-party instrument with zero intervention. Literal colours/sizes in declared UI or emitted lists are a validation error.
* Escape hatch: a custom `gpu` draw routine remains available to **first-party T1 modules only**, and must consume the same design tokens and pass the visual conformance checklist (`design/look-board.md`). A badged third-party pixel surface is parked in `LATER.md`, not in this contract.

### 6.7 Hot reload and non-breakage guarantees

* Watch `modules/` for changes. On change: build/load a **shadow instance**, run `configure` with migrated state, A/B it silently for N blocks (comparing output against the live instance for a sanity metric), then swap the pointer at a block boundary. On failure: keep the old instance, surface a non-modal warning with the diff of what broke.
* **Backwards compatibility contract:**
  * Port and param *additions* are always safe (defaults fill in).
  * Removals/rename require a deprecation window of ≥2 minor versions and an alias table.
  * Behavioural changes require a `compat` flag so old patches can request old behaviour.
  * The host API is versioned and *frozen* per minor; there is a machine-checked API surface snapshot test in CI so accidental breaking changes fail the build.
* **Patch provenance:** every project records the exact module versions and asset hashes it needs. Opening a project with a missing/incompatible module yields a *repair dialog* (substitute / stub / load offline), never a crash and never a silent wrong sound. A stub module preserves routing so the rest of the patch keeps working.

### 6.8 First-party module taxonomy (the initial library)

Naming convention: `<category>/<name>` (see Appendix A). Phase-relevant counts:

* Phase 1: ~12 modules (enough for a real track skeleton)
* Phase 3: ~40 modules (full audio toolkit)
* Phase 6: ~90 modules (music + data + display)
* v1.0: ~140 modules + templates + example patches

The first-party set is the **backbone layer** (§6.9): utilities, control and basic sources on the critical DSP path. The count above grows by accretion of small modules; the performance layer grows by *instrument*, most of them not first-party.

### 6.9 The two-layer library: backbone modules and instruments (ADR-010)

**The library is two layers, and the difference is a role, not a size.**

| | **Backbone modules** | **Instruments** |
|---|---|---|
| Role | the substrate: utilities, control, basic sources, analysis | the performance and control layer: what a set is played on |
| Manifest | `classification.layer = "backbone"` (default) | `classification.layer = "instrument"` |
| Tier | T1 — native, first-party, golden-tested (ADR-002 admission rule unchanged) | T2 — wasm sandbox; third parties never ship native code into the live process |
| UI | auto-generated panel or declared widget layout | declared widget layout **+ graphical displays**: display lists (2D) and scene descriptors (3D), host-rendered with current tokens (§6.6) |
| Author | first-party | first-party reference instruments + anyone, via the hand-off system below |
| Complexity bar | small, single-purpose, composable | "simple or very complex" — a two-hour module to a two-day instrument (the guide's own test) |

Instruments are built *from* backbone modules conceptually — but they run as single graph nodes; patching remains the composition mechanism. An instrument whose inner DSP is too hot for the sandbox factors the hot path into a backbone utility or petitions for T1 promotion, which is a packaging change, not a redesign (the manifest is tier-independent).

**The hand-off / hand-in system.** Third parties build instruments against the frozen contract face and hand them in as a package of **no more than five files**:

```
instruments/                    ← dropped into the project root; discovered at launch (§11 order), no intervention
└─ my-fm-terrain/
   ├─ sparqmod.toml             ← manifest: identity, layer, ports, params, panel, displays, capabilities, asset hashes
   ├─ fm-terrain.wasm           ← the component; small assets embed in its data section
   ├─ example.sparqpatch        ← the instrument doing its job
   ├─ preview.svg               ← library-card render, validator-generated, never hand-drawn
   └─ README.md                 ← what it is, what it needs, licence
```

Large asset packs (samples, tables, models) do not break the cap: they are referenced by hash from the content-addressed library (§13.1) and fetched on first load. The author's *source* project is an ordinary cargo/wit project against the `sparq-module-guest` SDK; the five-file cap governs the distributed package.

The properties that make this a system rather than a folder convention:

* **Loads at launch, without intervention.** Discovery walks `instruments/` like every other source (§11): a bad manifest never loads, one broken package cannot hide the others, and every failure is surfaced verbatim in the module browser.
* **Design updates propagate, without intervention.** The host owns the look: widgets and displays are declared as data (§6.6), colours/strokes/metrics resolve from the runtime token bundle at draw time (§14.1, `design/token-spec.md`). When the app's design changes, tokens change, and every instrument follows — there is no instrument-side copy of the aesthetic to go stale. Literal appearance values in a package are a validation error, machine-checked.
* **Hand-in is a gate, not a review queue.** `sparq mod validate` runs schema → sandboxed smoke-instantiation → golden render → fuel/real-time budget → panel+display screenshots against the look-board audit → preview regeneration → file-count check, and emits the actionable report style the contract already uses. A package that passes validation *is* a package that loads. Signing/badging per §15: unsigned instruments load badged and are disabled by default in Perform mode.
* **Authorship is parallelisable.** `sparq mod dev` runs the same chain headless — no core repo, no sound card — and a nightly CI matrix runs every handed-in package against latest core. Core and instruments are built simultaneously behind the frozen contract; integration is mechanical, never merge-based (ADR-010 decision 7).
* **The author-facing document is [`MODULE-BUILD-GUIDE.md`](MODULE-BUILD-GUIDE.md)** at the repo root, with the skeleton package in `reference/instrument-template/`. The guide carries the contract's own rule: if any step needs engine knowledge, the contract has failed and the guide gets amended.

---

## 7. Stream bus — data ingestion plane

The spine. Nothing is special-cased: everything becomes a stream.

### 7.1 Source adapters (each is a Tier-3 or Tier-1 module)

| Family | Sources | Notes |
|---|---|---|
| Audio in | Interface inputs, loopback, files, network audio (jacktrip-class later) | Auto-resample to device rate, DC offset removal, per-input gain/impedance notes |
| MIDI | MIDI 1.0 & **2.0 UMP**, MPE, MIDI-CI discovery, Property Exchange, clock, timecode, Ableton Link | Per-note controllers (pressure, pitch, timbre) are first-class; auto-fallback to 1.0 |
| OSC | OSC 1.0/1.1 UDP/TCP, OSC over WebSocket, SLIP/serial OSC | Address-pattern → stream-channel mapping table, bundle-aware (sample-accurate when possible) |
| Serial / MCU | USB CDC, UART, CAN, Teensy/ESP32/STM32, Arduino | Framed binary protocol with CRC; hot-plug re-enumeration |
| HID | Gamepads, controllers, custom HID, TouchMIDI-class, keyboards, trackpads, MIDI-learn-anything | Unified "controller" abstraction |
| Wireless | BLE (IMU/HR/EMG wearables), Wi-Fi UDP, optionally UWB for position | Latency budget explicit; BLE advertised as *slow* stream (10–100 Hz) |
| Video | Cameras (V4L2/AVFoundation/MF), NDI, screen capture, video files, image sequences | GPU decode → frame bus; feature extraction on worker/GPU |
| Network / numeric | HTTP/HTTPS polling, WebSocket, MQTT, SSE, REST APIs (weather, transit, finance, seismic, air quality, astronomy), RSS, NTP-derived drift | Scheduler with jitter/backoff; cached last-good value; explicit staleness flag |
| Files / tables | CSV, TSV, JSON, Parquet, HDF5, Markdown tables, spreadsheets | Time-column detection, interpolation policy, loop/one-shot, playback rate |
| Internal | Any module's analysis output, any parameter, transport state, diagnostics, RNG | "Everything is a source" — you can modulate from your own CPU load |
| ML / latents | Embeddings from audio/video encoders, latent vectors, classification scores, transcription, pose keypoints | Tier-3, non-realtime-tolerant, publishes `data` streams |
| Generative math | L-systems, CA, chaos, fractals, random walks, stochastic processes, geometry (see §10.5) | These are *sources*, not effects — a core sparq idea |

### 7.2 Normalised stream model

A **Stream** = `{ id, name, source, type, channels[], rate, clock domain, staleness, unit hints, colour class }`

A **Channel** = `{ id, dtype (f32/f64/i32/bool/vec/enum/string/blob), unit, range, interpolation (none/step/linear/spline), smoothing }`

Sample = `{ channel values, t_wall, optional t_sample, seq, flags }`

Guarantees:
* Consumers never block. They read the latest N samples from a lock-free ring, with an explicit *read policy*: `latest`, `interpolated-at(t)`, `window(n)`, `accumulate`, `on-change`.
* Every stream has a **quality readout**: rate, jitter, dropouts, staleness, min/max/mean. If a sensor disconnects, its stream goes `stale` and downstream modules get an explicit `hold`/`decay` policy rather than garbage. **This is the single most important robustness feature for live data performance.**
* Every stream can be **recorded** into the journal and **replayed** as a file source. This means a data performance can be rehearsed, debugged and reproduced — and that you can compose against a recording of a live stream.
* Every stream can be **displayed** by any display module without extra wiring.

### 7.3 Conditioning toolkit (`dat/*` modules)

`resample · hold · smooth (1-pole, SVF, median, Hampel) · deadband · hysteresis · calibrate (2/3/5-point, per-channel curve) · invert · range map · curve (exp/bipolar/custom LUT) · scale/offset · quantise (to scale, to step, to grid) · differentiator · integrator · trigger-from-threshold · edge detect · peak follower · envelope follower · gate/smooth noise · random walk · slew limiter · normalise (auto-gain) · unit converter · expression evaluator (tiny safe math DSL) · record · replay`

### 7.4 Mapping and modulation surfaces

* **Map mode:** press *Map*, touch a control, wiggle a source → binding created with auto-detected range. Works for MIDI/OSC/HID/sensor/data. Bindings are inspectable, editable and exportable as a **Controller Map** asset.
* **Mapping canvas:** a dedicated grid view showing all sources × all targets, with amount, curve and bipolar flags — the "patch sheet" of a performance. Touch-friendly, printable, projectable.
* **Learn-from-performance:** record a mapping session and turn it into static bindings (or keep it as live automation).

### 7.5 The sparq Field Kit (reference hardware, optional but recommended)

A small, documented reference sensor rig so data performance is repeatable rather than improvised:

* **Body:** 9-DOF IMU ×2 (wrist, chest), EMG armband, heart-rate (BLE), foot switch, 2× rotary encoders, 4× pressure/FSR pads, 2× capacitive touch strips.
* **Environment:** distance (ToF), light (lux + spectrum-ish), temperature/humidity, microphone (sound pressure), optional magnetometer.
* **Position:** 2× joystick, 1× XY touch pad, optional camera-based hand tracking.
* **Firmware:** one sketch per board, framed binary out at 200–1000 Hz, USB CDC, auto-identify string so sparq maps it without configuration.
* **Enclosure:** 3D-printed, Velcro/magnet mount, single USB hub, one power bank. Total target cost ≤ $350.
* Delivered as `sparq field` — a documented kit + calibration routine + example patches. (This is also a compelling part of the public identity of the project.)

---

## 8. MIDI 2.0, controllers and expression

* Full UMP parsing/serialisation; MIDI-CI Profile Configuration so sparq can auto-configure a controller's pads/knobs to the current scene (huge for live work).
* **Per-note controllers** (pressure, pitch, timbre, articulation) routed as `cv` per voice — enables Squarepusher-style expressive bass lines and MPE granular playing.
* Property Exchange for bulk preset/scale transfer with hardware.
* MIDI 1.0 fallback with 14-bit NRPN support; MIDI clock in/out; MTC; Ableton Link in/out; OSC transport.
* **Controller-agnostic mapping** (§7.4): no module may hard-code a controller. Every mapping lives in the Controller Map asset.
* Low-latency path: controller → mapping → audio in ≤1 block + device latency. Target **<10 ms** round trip on a wired interface, measured and displayed.

---

## 9. Audio plane — synthesis, sampling, FX, analysis, spatial

### 9.1 Quality principles

* Internal buses `f32` with `f64` accumulation at sums; global `f64` mode for offline/scientific renders.
* **Anti-aliasing is not optional:** polyBLEP/BLAMP or bandlimited wavetables for all classic shapes; oversampling (2×/4×/8×, per-module) for nonlinear stages (waveshaping, folding, FM index > threshold); explicit `aliasing` meter so you can *see* when you're cheating.
* Headroom: internal bus nominally −18 dBFS ref, true-peak safe to +6 dB internally; clipping only ever at the master limiter.
* Every processor declares latency and tail; the graph can compensate or not, per path (§5.1).
* **Testable:** each module has a golden reference render + property tests (DC-free, bandlimited, monotonic filter response, no denormal stalls).

### 9.2 Synthesis modules (`syn/*`)

* **Oscillators:** polyBLEP saw/square/triangle/pulse (with BLAMP for PW), wavetable (variable-length tables, 2D morph, spectral warping, per-cycle interpolation), additive/partial engine (up to 512 partials, per-partial amp/freq/phase envelopes — the *inharmonic metallic* sound), phase-modulation/phase-distortion, sine/pure-tone bank (Ikeda-grade exactness), noise (white/pink/brown/blue/violet, LFSR, impulse/crackle, sample-and-hold, dust).
* **FM:** 2–8 operator graphs with arbitrary routing matrices (the DX-with-a-scalpel approach), per-operator envelopes, feedback operators with unit delay, ratio & index modulation, key tracking, unison.
* **Physical modelling:** membrane/drum (2D waveguide + exciter), plate/string (Karplus-Strong extended, bidirectional), tube/bore (wind), modal synthesis from measured or generated mode lists, spring, and a generic `resonator bank` (formant/comb) that can be driven by any impulse — this is the fastest route to "acoustic object mangled" territory.
* **Granular:** clouds (density, size, position, jitter, window, overlap up to 512 grains), time-stretch/pitch-shift (granular + phase-vocoder hybrid), freeze, scattering with per-grain probability and spatial spread, grain source from sample/live input/synthesis output/resynthesis buffer.
* **Spectral:** FFT resynthesis (magnitude/phase with per-bin manipulation), phase vocoder, cross-synthesis (convolve spectra of two sources), spectral filtering/morphing/freezing, harmonic-percussive separation, bin-shifting, sonification of arbitrary `data` streams into spectra (a signature sparq capability: **play a spreadsheet, play a sensor, play an image row**).
* **Mathematical/geometric sources (`syn/math`):** waveshaping from arbitrary transfer functions; **wavetermaining** (function-of-two-variables surface scanned by a trajectory — Lissajous, spiral, random walk, attractor path); **waveTerrain synthesis**; Chebyshev/Legendre polynomial shaping; fractal curves (Koch, Hilbert, Peano) as waveforms; cellular automaton (Rule 30/90/110, 2D Life) as wavetables and as gates; **function plotter → wavetable** (draw a curve on the touchscreen, hear it — a killer touch-native feature).
* **Wavefolding / nonlinearity:** fold, wrap, soft/hard clip, diode/ladder emulation, bit depth & sample-rate reduction with proper anti-aliasing option, glitch (drop/stutter/repeat/hold/reverse at sample or block granularity).

### 9.3 Sampling (`smp/*`)

* Formats: WAV (incl. float/BWF), AIFF, FLAC, OGG, MP3, CAF; import via resampling to project rate with high-quality polyphase SRC.
* **On import, automatically compute and cache:** peak/RMS envelope, waveform overview (multi-resolution), transient/onset map, beat grid estimate, pitch contour (perceptual + autocorrelation), spectral centroid/bandwidth/rolloff/flatness, loudness (LUFS short/long), duration, key/chroma estimate, silence trim, DC removal, normalisation option, fingerprint + embedding (§13).
* Playback: multi-voice, one-shot/loop (with crossfade points), ping-pong, slice engine (transient/equal/beat/manual slices, per-slice offset/gain/pitch/reverse/probability), stretch algorithms (granular HQ / phase vocoder / transient-preserving), key-mapped multisample with crossfade zones, round-robin & random groups, velocity layers, choke groups (drum-kit semantics), reverse, playback-rate modulation with proper pitch independence, live input capture to a scratch buffer (record-and-play-in-place, loop recorder with overdub — the live-performance essential).
* **Spectral morph** between any two samples; **convolution** with any impulse (from library, from analysis, from a generated geometry).

### 9.4 Effects (`fx/*`)

* **Filters:** SVF (LP/HP/BP/notch/peak) zero-delay-feedback, ladder (24 dB, drive), formant, comb (positive/negative, modulated), allpass, state-space, brickwall linear-phase FIR, Moog-style, diode, and a **filter bank / vocoder** (16–64 bands, FFT vocoder, with `data`-driven band gains — sonification again).
* **Delay/time:** tap delay, ping-pong, multitap (up to 64 taps, positions from any stream), diffuser, **feedback delay network** (4/8/16 order, damping, modulation), freeze/infinite, granular delay, tape (wow/flutter/saturation/head bump), tempo-synced with polyrhythmic tap ratios.
* **Reverb:** FDN (large, dense), plate, hall, room, **convolution reverb** with partitioned FFT (long IRs at low CPU), shimmer/pitched reverb, gated/reverse reverb, spatialised reverb (tail rendered into the ambisonic bus — important for surround work).
* **Distortion/color:** soft/hard clip, wavefolder, bitcrush, sample-rate reduction, tube/transistor/diode models, frequency shifter, ring mod, phase shifter, flanger/chorus, ensemble, exciter, transformer saturation, **circuit-bend-ish feedback path** (deliberate instability with a safety limiter).
* **Dynamics:** compressor (feed-forward/feed-back, look-ahead), multiband (2–6), sidechain (from any stream, not just audio), gate/expander, transient shaper, upward/downward comp, limiter, true-peak limiter, clipper, **LUFS-normalising gain stage**.
* **Glitch/IDM specials:** stutter/repeat with rhythmic subdivision, slicer/gater synced to transport with polyrhythmic patterns, buffer shuffler, tape-stop, vinyl noise & crackle generator, impulse/burst generator (Ikeda), "data mosh" (corrupt a buffer according to a stream), sample-and-hold resampler, reverse-granular, and a **probability engine** that gates *any* effect per event (the "random but intentional" tool).
* **Utility:** gain, pan/law, matrix mixer (N×M with per-cell gain, essential for surround), DC/phase tools, mid-side, mono-compat checker, test signal generator (sine sweep, pink, impulses, M-sequence for room measurement), channel router/splitter/merger.

### 9.5 Analysis (`ana/*`) — real-time, GPU-friendly

RMS/peak/true-peak · LUFS (momentary/short/integrated) · FFT magnitude/phase (sizes 256–65536, windowing set) · mel/ERB/bark bands · spectral centroid/spread/skew/kurtosis/rolloff/flatness/flux · pitch tracking (YIN/pYIN-class, plus harmonicity) · onset/transient detection with strength curve · zero-crossing rate · autocorrelation · chroma · **MFCC + embeddings** for similarity and for ML modules · loudness histogram · stereo correlation/width · **phase-space reconstruction** (delay embedding → attractor display, a scientific and beautiful control source) · zero-crossing-entropy and other oddities as *modulation sources*.

Every analysis output is a `cv`/`data` stream by default. **Analysis is a control source, not a meter.**

### 9.6 Spatial audio (`spa/*`) — the surround requirement

* **Speaker rig definition:** arbitrary layouts in 2D or 3D (up to 64 outputs), named presets per venue (`rig/quad-4`, `rig/8-ring`, `rig/7.1.4`, `rig/12-dome`, `rig/binaural`), with per-speaker distance, azimuth, elevation, gain, delay, polarity, and a comment field.
* **VBAP / VBAP3D:** amplitude panning with 2D and 3D triplets/pairs, spread control, ghost-image reduction, and trajectory smoothing.
* **HOA Ambisonics:** encode mono/objects to **AmbiX order 1–7** (1..64 channels), rotate/sphere functions (rotation, directivity, maximiser, proximity effect), decode to any rig with in-phase / basic / energy-vector decoders, near-field compensation, and **binaural rendering from HOA** with HRTF datasets (SOFA import) + head-tracking input from the Field Kit.
* **Object-based mixing:** up to K simultaneous objects, each with position (driven by any stream), size/spread, distance model (air absorption, level law, early-reflection delay), Doppler, and direct/reverb balance. Objects can be *emitters* (a granular cloud moving through the room) or *zones* (a reverb return placed in space).
* **Spatial tools:** trajectory generator (Lissajous, spiral, random walk, attractor path, drawn-by-touch path, sensor-driven path), decorrelator, up-mixer/down-mixer, matrix encoder, **spatial reverb** (FDN with per-speaker taps), Doppler, distance/air-absorption model, room simulator (image-source for early reflections in a shoebox), **ambisonic microphone input decode** (record with a tetra mic, decode later).
* **Calibration workflow:** built-in sine sweep / M-sequence generator + measurement-mic input + analysis (per-speaker impulse response, delay, level, room modes) → generates a **correction preset** (per-channel FIR + delay + trim) saved with the rig. This is how you get consistent quad/8-channel sets in unfamiliar rooms.
* **Monitoring:** stereo binaural downmix for headphone checking, mono-compat check, per-channel meters, "which speaker is which" identify pings, and an overhead **rig map display** (§14.5) that is itself a performance visual.
* **Recording:** multichannel bounce to interleaved WAV/FLAC/BWF with embedded layout metadata; separate stem busses; ambisonic master + decoded rig master in one pass.

### 9.7 Master chain and loudness

Configurable per project: `bus matrix → per-bus dynamics → master EQ (linear phase optional) → true-peak limiter → dither → output matrix → correction preset`, with LUFS-M/S/I meters, loudness range, true-peak, per-channel and correlation metering, and a **target-loudness** assist (techno/breakcore realities: −9 to −6 LUFS integrated for club, −14 for streaming; sparq shows both and can render both).

---

## 10. Music & logic plane — sequencing, harmony, mutation, form

### 10.1 Transport

Play/stop/record/punch, loop regions, tempo (with tap-tempo, and tempo as a *stream* — tempo can be modulated by data), time signature **per track** (polymetre), global tick resolution 960 PPQN default (configurable), count-in, metronome (with polyrhythmic click patterns), master/slave sync, and **transport as an event source** (bar/beat/section triggers into the graph).

### 10.2 Sequencers

* **Step grid** (per track): 1–256 steps, arbitrary step *ratio* (so a "16-step" pattern can be 16 steps of a 7/8 bar), per-step: probability, velocity (+random range), note/param value, ratchet (n, curve), roll, microtiming offset (ticks, ±), gate length, sample/slice selection, choke group, accent, and *mutation lock*.
* **Piano roll / event lane**: sample-accurate events with MPE data, per-note controllers, drawn curves, and note-level mutation.
* **Euclidean/polyrhythm generator**: `E(k,n)` with rotation, offset, and per-hit velocity shapes; nested polyrhythms (k over n over m) — the Venetian Snares engine.
* **CV recorder/lanes**: record and draw automation at sample or block resolution for *any* parameter, including data-driven lanes.
* **Trigger/patch-style sequencer** (modular idiom): clock dividers/multipliers, probability gates, logic (AND/OR/XOR of triggers), clock随机 jitter, swing/humanise curves, accumulator/counters, Turing-machine-style shift registers, and **sub-pattern recursion** (a pattern can contain a pattern).
* **Phrase/clip system**: any region of any sequencer can be captured as a **phrase** with metadata (length, key, density, timbre tag) and stored in the library; phrases are themselves mutable objects.

### 10.3 Rhythm concepts (explicitly non-standard, because that's the genre)

* Polymetre and polyrhythm per track; **non-integer and irrational ratios** (a 1:φ grid is a first-class citizen).
* Microtiming as a *field*: a per-step offset curve that can be drawn, generated, or driven by a stream (this is where "feel" and "alien groove" both come from).
* **Subdivision lattices**: instead of a fixed grid, a pattern can live on a lattice defined by any set of ratios; the UI shows the lattice geometrically (see §14.5) — geometry as rhythm, rhythm as geometry.
* Metric modulation: retarget the tick-to-sample ratio smoothly, with the transport clock broker handling it without glitches (§5.1).
* Stochastic timing: Poisson, Gaussian, lognormal, and "grid-with-jitter" generators with density control.

### 10.4 Chord & harmony engine

* **Scale/tuning**: 12-TET default, plus **Scala `.scl` / `.kbd` import**, arbitrary N-EDO (5, 7, 11, 19, 22, 31, 53…), just intonation, Harry Partch-style custom scales, per-key tunings, and **dynamic retuning** (tuning as a stream — microtonal glides between tuning systems).
* **Chord model**: chords as *pitch-class sets + voicing rules*, not strings. Voice-leading engine with configurable priorities (smoothness, spacing, avoid-parallel, bass motion, tension target), inversion/voicing strategies (drop-2/3, close/open, spread, cluster), and **tension curves** over time.
* **Progression generation**: functional/roman-numeral progressions, modal interchange, negative harmony, chromatic mediants, cycle-of-N root motion, Markov over chord graph, constraint solver (declare rules → get valid progressions), and *progression mutation* (transpose, invert, rotate, substitute by tritone/relative, re-voice, rhythmically displace).
* **Arpeggiator/generative player**: strum patterns, order (up/down/random/Markov/constrained-random), velocity contours, per-note probability, "ghost notes," and **pattern-level** mutation. For Devine-style intricate arps, the arpeggiator takes its order from any `data` stream.
* **Output**: chords/notes are *events* into the graph, and can also be exposed as `cv` (root, tension, density, brightness) for modulation — harmony as a control source.

### 10.5 Mutation engines (`gen/*`) — the heart of sparq

Mutation is a **first-class subsystem**, not a "randomise" button. Every mutation operator is a module with typed inputs (pattern / param vector / sample selection / geometry) and typed outputs, so operators compose into **chains** and **trees**.

Operator families:

| Family | Operators |
|---|---|
| Deterministic transforms | reverse, rotate, mirror, transpose, invert, scale, quantise, interpolate (morph) between two patterns, stitch, splice, dilate/contract, offset, interleave, decimate, duplicate, palindrome, canon |
| Stochastic | per-element probability, weighted choice, Bernoulli/Poisson gates, random walk on values, shuffle with constraints, jitter, dropout |
| Markov | n-order chains over notes/intervals/timbres/slices, trained from a corpus or a phrase, with temperature and forbidden-transition matrices |
| **L-systems** | rewrite grammars with turtle interpretation → rhythms, melodies, and *geometry* (same engine feeds visuals) |
| **Cellular automata** | 1D (rules 0–255, incl. 30/90/110/184), 2D (Life, Seeds, Brian's Brain, totalistic), used as gates, as pitch fields, as wavetables, and as visuals |
| **Chaos / attractors** | logistic map, tent, Hénon, Lorenz, Rössler, Chua, standard map, circle map; sampled onto parameter trajectories with rate, quantisation and folding controls |
| Fractal / noise | Perlin/simplex/worley (1D–3D), fBm with octaves/lacunarity, white/pink, wavelet noise; used as continuous modulation fields |
| Spectral-of-sequence | treat a pattern as a signal: FFT it, filter/threshold/scramble bins, inverse it → musically surprising but structured variation |
| Genetic | population of patterns + fitness function (user-defined: density, similarity to target, tension curve, "surprise"), selection/crossover/mutation, and a **fitness from listener input** (you tap "keep/kill" during auditioning — human-in-the-loop evolution) |
| Geometry | project a pattern onto a shape (polygon, spiral, torus knot, Voronoi cell graph, Lissajous); traversal order and step length determine rhythm and pitch; rotate the shape → rotate the music. **This is the signature sparq idea: geometry as score.** |
| Corpus / similarity | sample-slice selection by embedding similarity to a target; phrase interpolation via DTW alignment; "find me 8 slices that sound like this one but get progressively more metallic" |
| Data-driven | any `data` stream selects among operators, sets operator parameters, or gates application — e.g. mutation probability = heart rate, operator choice = seismic feed |

**DNA tree:** every mutation creates a child node in a persistent, visualised tree (parents, operator, seed, timestamp, "kept" flag). You can navigate the family tree by touch, audition branches without committing, collapse to a "best path," and export a lineage as a **generative score** (the operator chain + seeds, reproducible forever). This single feature turns random exploration into *directed* exploration and is the strongest defence against "generative mush."

### 10.6 Form engine (macro structure without a timeline)

* **Sections** as scene groups with a directed graph of possible transitions (weighted, conditional on transport position, energy level, or a stream).
* **Energy/density/tension curves** as global `cv` streams that many modules subscribe to — so the whole patch "breathes" together. Draw the curve, or drive it from data.
* **Arrangement by probability**: define a Markov graph of sections and let the performance take different paths each night (recorded in the journal, so you can replay the exact path).
* **Clock-free mode**: for installation/ambient work, form driven purely by slow streams and long envelopes.

### 10.7 Tuning, timing and theory utilities

Scala/KBM import-export · frequency ↔ note conversion with arbitrary reference (A=432/440/442…) · cent math · harmonic series tools · interval/consonance analysis · key detection · beat/tempo detection · polyrhythm visualiser · metric modulation calculator. All exposed as `data`/`cv` where useful.

---

## 11. Sample wrangler & library UX (the "10 000 samples at my fingertips" requirement)

* **Browser**: touch-first, huge tiles + dense list toggle, waveform/spectrogram thumbnails rendered from cached analysis, instant search (substring + tag + facet), **similar-to-this** (fingerprint + embedding), "random from filtered set," audition-on-drag with automatic key/tempo matching to the project.
* **Facets**: category, timbre tags (auto + manual), key, BPM, duration, loudness, brightness, "graininess", licence, provenance, last used, usage count, folder, project, cloud/local, hash.
* **Sets/packs**: named collections with their own analysis, versioned, shareable through the registry (§13).
* **Slicing/workbench**: a dedicated full-screen touch surface for chopping a break — transient detection with sensitivity, manual slice by touch-drag on the waveform, per-slice pad assignment, instant scatter to a sequencer, and "explode into N variants" (mutation applied to the whole set).
* **Loop recorder**: live input capture with overdub, quantise-to-bar, reverse, and immediate availability as a sample (essential for Squarepusher-style live interplay).
* **Prep pipeline**: batch normalise/trim/silence-strip/resample/analyse/tag, with a preview and undo. Runs on the worker pool, never blocks.
* **Cloud integration** (§13): local-first, background sync, bandwidth caps, offline-complete guarantee (a performance never depends on the network), and an explicit "on-device only" flag for anything you don't want uploaded.

---

## 12. Performance plane

### 12.1 Three modes

| Mode | Surfaces | Purpose |
|---|---|---|
| **Design** | Graph canvas, inspector, browser, mapping, diagnostics | Building and sound design. Mouse/keyboard friendly, touch usable |
| **Perform** | Scene matrix, 8 macros (touch), transport+clock readout, metering, full-bleed visual, one "panic" button | Playing live. No editing. Everything ≥44 px. Readable at 3 m |
| **Install** | Nothing (or a minimal status page over HTTP) | Unattended, self-healing, watchdog, auto-restart, log rotation |

Mode switching is a single gesture and is *stateful* (your perform layout is saved per project).

### 12.2 Scenes and macro play

* **Scene** = a snapshot of: module parameter vectors, routing (optional), pattern/phrase selection, spatial rig + object positions, visual preset, and the transition curve.
* **Scene matrix** (touch grid): launch modes `instant / crossfade(N ms) / quantised-to-bar / conditional`, with per-scene morph curves per parameter group (so a filter opens over 4 bars while the pattern swaps on the beat).
* **Scene chains**: ordered playlists with probability branches — the "arrangement" of a non-linear system.
* **Macros**: 8 (or 16) big touch targets per scene, each a bundle of parameter mappings with curve and range; assignable to controllers, sensors or data streams. Macros are the primary expressive surface.
* **Gestures**: swipe to morph between scenes continuously (X/Y over a 2×2 scene quad), pinch to zoom a visual, three-finger tap for panic (all sound off, then restore), five-finger hold for the recovery menu.

### 12.3 Live capture and recall

* Record **multitrack stems** (per bus), the **automation journal**, the **stream recordings**, and a **video capture of the UI** (optional, for documenting performances) — all synchronised to `t_sample`.
* **Take marker**: one touch drops a marker with a screenshot of the rig map + macro positions + seed. After the set you can browse takes visually.
* **Re-render**: because everything is journaled and deterministic, you can re-render any take offline at higher quality/`f64`/multichannel, exactly as performed. This is the "patch is the master" payoff.

### 12.4 Reliability and stagecraft

* Autosave every N seconds + on every scene change; journal flushed continuously.
* **Watchdog**: audio thread liveness, block-time budget, memory high-water, device error → automatic degradation ladder (§5.2) then, if unrecoverable, silent restart into the last good state with a visual "RECOVERED" stamp.
* **Rehearsal mode**: replay recorded streams and MIDI so you can practise a data-driven set without the sensors.
* **Blind mode**: all labels hidden except numerals — for playing by muscle memory in the dark.
* **Two-screen/two-device**: primary = instrument; secondary = visuals (audience). Optional: a tablet running a *thin client* (web view over WebSocket) showing only macro pads, in case the main machine is out of reach.
* **Pre-flight checklist** generated per project: device config, channel count vs rig, sample-rate match, missing assets, unsigned modules, disk space, CPU headroom estimate, "have you calibrated the room?"
* **Physical redundancy**: every critical function (play, stop, panic, next scene) has a hardware mapping in the Controller Map, independent of the touchscreen.

---

## 13. Library & network plane ("preset and sample cloud")

### 13.1 Storage model

* **Content-addressed store**: `assets/<blake3-hash>` for immutable blobs (samples, IRs, images, module binaries, preset state). Deduplication is automatic; corruption is detectable; sync is trivially idempotent.
* **Metadata DB**: SQLite (with FTS5 text index + a vector index) holding asset records, tags, analysis, provenance, licence, usage stats, embeddings.
* **Local-first**: the entire library is usable with no network. Cloud is an accelerator, never a dependency.
* **Project bundle** `.sparq`: a container referencing assets by hash, embedding the graph, seeds, journal head, module versions, and rig presets. Openable on any machine that has the modules and can fetch the assets (bundled or from cloud).

### 13.2 What's in the library

`samples · slices · IRs · wavetables · presets (module) · patches (subgraphs) · projects · phrases/patterns · chord progressions · scales/tunings · controller maps · rigs (spatial) · visual presets · scenes · mutation lineages (DNA exports) · field recordings · render bounces`

Everything has: hash, created-at, source (project/import/generation), lineage (what it was derived from), tags (auto + manual), analysis summary, licence, and share state (`private / team / public`).

### 13.3 Search and discovery

* Text + facets (fast, offline).
* **Query-by-example**: hum/tap/drag a slice → nearest neighbours by embedding.
* **Query-by-description** (optional, local model): "metallic inharmonic hit, 200 ms, bright" → ranked results. Runs on the worker pool or Tier-3.
* **Serendipity tools** (deliberately non-obvious, because discovery is a compositional tool): "random from a filtered set," "similar but more X" (slide along an embedding axis), "the 20 least-used samples matching this tag," "lineage siblings."

### 13.4 Cloud sync and registry

* **Sync**: background, resumable, bandwidth-capped, prioritised (project-critical assets first). Conflict model: assets are immutable and content-addressed so they never conflict; *metadata* uses per-field last-writer-wins with a monotonic logical clock, and tags/notes are set-merged.
* **Backend** (when you want it): S3-compatible object store + Postgres + a thin API; OIDC/JWT auth; presigned uploads/downloads; per-user quota; audit log. Can be self-hosted (a single small server) or run on managed infra. Optional end-to-end encryption for private libraries.
* **Registry** (`registry.sparq.*` or self-hosted): namespaces, semver modules and packs, signed manifests, dependency resolution, download counts, and a `sparq mod add ns/name@1.2` style CLI. This is the mechanism by which your module ecosystem grows without you hand-installing anything.
* **Sharing**: public packs with licence metadata (default CC-BY-NC for your own uploads; explicit choice at publish), private team spaces, and "share this patch with exactly the assets and module versions it needs" (a reproducible share, not a broken zip).

### 13.5 Provenance, licence and ethics

* Record provenance for every sample (source, date, licence, whether it's a derivative). This matters for released music and for shared packs.
* Flag and quarantine anything of unknown provenance; provide a "clearance report" per project so you can prove what's in a release.
* No scraping of copyrighted content into the public registry. Personal use is your business; publishing is gated behind an explicit provenance check.

---

## 14. Graphics & interface plane

### 14.1 Rendering architecture decision

**[DECISION D-1]** Recommended: **custom retained-mode renderer on WebGPU (`wgpu`)**, with your own layout + widget layer, built on `winit` for window/input.

Why:
* The UI *is* the visual identity and the stage visual — you need full control of every stroke, glow and trace, and you need the same renderer to drive a projector at 4K.
* WebGPU gives one portable GPU abstraction (Vulkan/Metal/DX12) and a *compute shader* path, which you need for FFT-driven visuals, particle systems, attractors and vector fields at high frame rates.
* Retained mode keeps a 2 000-node graph smooth (incremental redraw, dirty regions, LOD).
* Same code compiles to a browser build later (thin client, demo, documentation), and wasm modules can render into your canvas.

Fallback if you want velocity over control: start Phase 0–2 with **egui** (immediate mode, wgpu-native, trivially fast to prototype) behind your own thin abstraction, then swap the shell in Phase 6 when the visual language is settled. This is a legitimate, low-regret path — the *module API and audio engine are unaffected*, which is what matters.

Non-negotiables either way: 60 fps UI minimum (120 on capable panels), render thread separate from UI logic, GPU-side trace generation, no per-frame allocation, and a **design token system** (colours, stroke widths, radii, type scale, spacing, motion curves, colour maps) that every widget and every module panel consumes. Tokens are the mechanism by which 140 modules look like one product.

### 14.2 Shell layout

```
┌───────────────────────────────────────────────────────────────────────────┐
│  RAIL (56 px)  │                    CANVAS                        │ INSPECTOR │
│  ─ transport   │   graph / geometry / lattice / DNA tree / visual   │ params    │
│  ─ modes       │            (full-bleed, zoomable, LOD)             │ ports     │
│  ─ browsers    │                                                    │ mapping   │
│  ─ diagnostics │                                                    │ analysis  │
├────────────────┴────────────────────────────────────────────────────┴───────────┤
│  DOCK (collapsible): module browser · library · streams · scenes · journal       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

* Responsive breakpoints: **tablet ≥10"** (touch-primary), **laptop 13–16"**, **large/display wall 4K+**. The layout is the same object, re-flowed; no separate "mobile app."
* Panels are resizable, collapsible, and can be torn off to a second display (visuals out).
* **Everything reachable in ≤2 touches** from any state. A persistent search/"command" surface (summoned by a two-finger tap) finds modules, samples, presets, actions and settings by fuzzy text — the power-user escape hatch that doesn't clutter the touch UI.

### 14.3 Interaction model (touch-first rules)

1. Minimum hit target **44×44 px**; primary performance controls **≥64 px**; grid snapping at 8 px; magnets on ports with a 24 px capture radius.
2. **No modifier keys, no right-click, no hover-dependence.** Every hover affordance has a long-press equivalent.
3. Gestures: 1-finger drag = move/connect; 2-finger = pan/zoom (with a *zoom-to-fit* double-tap); long-press = context (rename, duplicate, bypass, mute, solo, inspect, randomise, lock); 3-finger tap = undo; 5-finger hold = panic/recovery.
4. **Direct manipulation everywhere**: drag a waveform edge to change slice points; drag a filter curve; draw an envelope with a finger; draw a wavetable by stroking the screen; sketch a spatial trajectory on the rig map; draw a tempo curve.
5. **Precision assist**: after a touch-drag begins, a second finger adds fine resolution (×10), and a numeric field is always available for exact entry. No "you can't be precise on touch" excuses.
6. **Haptics** where available (detents on encoders-in-software, confirmation on latch).
7. **Mouse/keyboard parity**: everything touchable is also keyable (documented shortcuts), for design-mode speed.
8. **Accessibility**: 200 % type scale option, high-contrast theme, colour-blind-safe colour maps for spectrograms and stream classes (shape/pattern redundancy, not colour alone), full keyboard navigation, adjustable motion (reduce-traces mode), and a minimum-contrast guarantee for stage lighting conditions.

### 14.4 Display modules (`dsp/*`) — the visuals

| Display | Shows | Use |
|---|---|---|
| Scope | waveform, X/Y (Lissajous), rolling, persistence/phosphor decay | Timbre, stereo image, chaos |
| Spectrum | magnitude/phase, mel bands, waterfall, peak-hold, sonogram | Spectral work, analysis |
| Phase portrait | delay-embedded attractor of any signal | "Is this system chaotic?" — and it's beautiful |
| Vector field | 2D/3D field from any two/three streams | Sensor/geometry visualisation |
| Geometry | L-system turtle, CA grid, Voronoi/Delaunay, lattices, torus knots, subdivision surfaces | Geometry-as-score made visible |
| Rig map | top-down/isometric speaker layout with object trajectories and levels | Spatial work + a great stage visual |
| Graph view | the patch itself, with live signal levels on edges | The signature sparq image |
| DNA tree | mutation lineage, navigable | Directed exploration |
| Stream inspector | every stream: value, rate, jitter, history sparkline, quality flags | Live-data debugging on stage |
| Metering | per-channel, LUFS, true-peak, correlation, loudness histogram | Mixing/mastering |
| Matrix | N×M routing/mapping grid with live values | Mapping surface |
| Text/numeric | large monospaced readouts (tempo, bar, seed, clock, CPU) | Stage legibility |
| Visualiser chains | particle systems, feedback (ping-pong buffer) effects, shader playground driven by streams | Full-bleed projected art |

Every display module can be: full-bleed, scaled to a second output, recorded, or exported as a still/clip. Visual presets are library assets (§13). **Rule:** a display must never cost audio-thread time; all visual work is GPU/worker.

### 14.5 Cohesion mechanisms (why it won't look like 140 plugins)

* Design tokens (§14.1) consumed by every panel.
* Declarative panel descriptors (§6.6) rendered by one shell widget set.
* **Instruments render through the host** (ADR-010): display lists and scene descriptors resolved against the runtime token bundle, so a third-party instrument cannot drift from the look even in principle — and a token change re-themes every instrument at once.
* One type scale, one stroke language, one colour-class system, one motion curve set.
* A **visual conformance checklist** in the module review process, and an automated screenshot-diff test for every module panel at three breakpoints — for handed-in instruments, run mechanically by `sparq mod validate` against `design/look-board.md`.
* A single "sparq look" reference board (a canonical screenshot set) that any new surface must be able to sit next to without obvious dissonance.

---

## 15. Security, trust and safety

* **Module signing**: manifests and binaries signed; the registry publishes attestations; local install of unsigned modules is allowed but *badged* in the UI and disabled by default in Perform mode (so an experiment can't break a set).
* **Sandboxing**: T2 wasm modules — the instrument tier from Phase 1–2 (ADR-010), not Phase 5 — get capability-scoped access (fs read of declared asset paths only, no net unless granted, fuel limits, memory limits, instruction budgets). T3 processes get an OS sandbox profile (seccomp/AppContainer/sandbox-exec) and a resource cap.
* **Resource isolation**: per-module CPU budget, per-module memory cap, per-module audio-time watchdog with auto-bypass.
* **Data privacy**: sensor/camera/network streams are opt-in per source, with a persistent indicator when a camera or mic is live; stream recordings are encrypted at rest if the library is marked private; nothing leaves the machine without an explicit sync/publish action.
* **Physical safety**: a master SPL ceiling with a hard limiter and an audible/visual warning; "venue mode" caps output level and disables feedback-path modules that can runaway (a real risk with FDN + wavefolders at 8 channels).
* **Data integrity**: asset hashes verified on load; journals checksummed; projects atomic-write with a rollback copy.

---

## 16. Technology stack and build strategy

### 16.1 Recommended stack (opinionated)

| Layer | Choice | Why |
|---|---|---|
| Language (core) | **Rust** | Memory safety in a system that must not crash on stage; fearless concurrency for a 5-thread real-time design; first-class wasm target (both for T2 modules and a browser client); excellent GPU ecosystem; single language across engine, UI and modules |
| Audio device | **Windows primary: WASAPI exclusive → ASIO → WASAPI shared**, all behind one HAL trait; plus a **null/virtual device** for offline render and CI from day 1; PipeWire/JACK (Linux) and CoreAudio (macOS) follow in Phase 1 CI | `cpal` is acceptable for the Phase 0 bootstrap only — it does not give you exclusive mode, ASIO, real multichannel, or a device aggregator, all of which sparq needs |
| Windows RT plumbing | MMCSS (Avrt), working-set lock, high-resolution waitable timers, DPC-latency diagnostics, "stage mode" system preset | The difference between a demo and an instrument you can trust on stage (§5.2) |
| DSP math | Rust `num`/`simd` (std portable SIMD or `packed_simd`-class), `rustfft` (+ GPU FFT in compute shaders for analysis/visuals), `rubato` for SRC | Deterministic, fast, testable |
| Audio codecs | `symphonia` (WAV/FLAC/OGG/MP3), `libsndfile`-class for BWF; `ffmpeg` (via a wrapped lib) for exotic/video | Broad coverage without reinventing decoders |
| Video/camera | GStreamer or `ffmpeg` decode → GPU textures; MediaPipe-class models in T3 for pose/flow | Keeps heavy work off the audio and UI threads |
| GUI/render | `winit` + **`wgpu`** (custom retained shell) or `egui` as the Phase 0–2 scaffold | See §14.1 |
| MIDI | Custom UMP parser + platform MIDI (CoreMIDI/ALSA/WinRT) | MIDI 2.0 support is still uneven in off-the-shelf libs; owning this is a feature |
| OSC / net | `rosc`-class + custom high-perf path; WebSocket for thin clients | Mature enough |
| ML inference | `ort` (ONNX Runtime) or `candle` (pure Rust), always in T3 | Real-time-safe by isolation |
| Data/metadata | SQLite (+ FTS5 + vector index), `serde` + `postcard`/bincode for state, `blake3` for hashes | Boring, fast, proven |
| Wasm runtime (T2) | `wasmtime` with WASI 0.2 + Component Model | The 2026-standard sandbox; stable component model makes cross-language modules practical |
| Plugin interop | Host **CLAP** and **VST3** (via existing Rust/C++ bridges) as *optional* Tier-3 wrapper modules | Use other people's effects without making them core |
| Faust (optional) | Faust as a **module codegen path**: write DSP in Faust → emit wasm/native → wrap in the sparq manifest | Lets you (and others) author modules in a DSP language; excellent for filters and classic DSP |
| CI/CD | GitHub/Gitea + self-hosted runner with a real audio device; deterministic offline render tests; sanitizers; screenshot diffs | Audio correctness needs hardware in CI |
| Platforms | **Windows primary** (WASAPI exclusive/ASIO, touchscreen devices, venue-standard laptops), then **Linux** (PipeWire/JACK, kiosk/install mode, future ARM appliance) and **macOS**; all three in CI from Phase 1 | Windows matches where live sound hardware, touch devices and venue machines actually are; Linux stays close because install/kiosk mode and the appliance path want it |

### 16.2 Alternative stacks (honest comparison)

| Strategy | Speed to first sound | Ceiling | Risk | Verdict |
|---|---|---|---|---|
| **A. Build in Max/MSP + TouchDesigner/vvvv** | Days | Limited: DSP depth, polyphony, multichannel, custom UI cohesion, distribution | You inherit someone else's architecture; the "own software" goal is only partly met | Great for *prototyping musical ideas now*, wrong for the product |
| **B. Extend an existing engine (SuperCollider / Csound / Faust + custom UI)** | Weeks | High for DSP, medium for UI/streams | Two languages, awkward integration, harder to make a cohesive touch UI | Good DSP accelerator; poor shell |
| **C. C++ / JUCE (or Qt + miniaudio)** | Months | Very high, proven in pro audio | Memory safety burden over a multi-year solo project; UI work is heavy; slower iteration | The industry default; defensible but riskier solo |
| **D. Rust + wgpu (recommended)** | Months (Phase 0 = days) | Highest for your specific goals (safety, wasm, GPU visuals, one language) | Smaller audio ecosystem; you write more plumbing | **Recommended** |
| **E. Hybrid: Rust core + Faust DSP codegen + optional C++ plugin bridges** | Months | Highest | Integration complexity | Recommended *within* D once the API is frozen |

### 16.3 Pragmatic hybrid path (strongly recommended)

Do **not** wait for sparq to make music. From week 1:
* Use a temporary "sketch environment" (Max, Pure Data, SuperCollider, or even a DAW) to develop *musical* ideas and to build a reference corpus of techniques.
* Port each proven technique into sparq as a module. The sketch environment becomes your **spec and your test corpus** (golden reference renders to match).
* Every sparq phase produces an actual track; every track exposes what's missing, which becomes the next phase's backlog. This closes the loop between art and tool and is the single best defence against building an instrument nobody plays.

---

## 17. Roadmap

**Assumption:** solo developer, ~25–35 productive hours/week → **~74–92 weeks** to v1.0. Full-time compresses this by ~40 %, not 100 % — the hard parts (DSP tuning, UI ergonomics, spatial calibration) don't parallelise with hours. Phase 0 is sized at 6 weeks rather than 4; see the cut list in `PHASE0-WORKORDERS.md` §7 if you want it shorter.

### Phase 0 — Proof of concept (weeks 1–6) → ticketed in `PHASE0-WORKORDERS.md`
* Repo/CI/engineering gates, Windows audio + touch hardware baseline, design tokens v0, static canvas mockups.
* HAL over WASAPI exclusive/ASIO (+ null device for CI), MMCSS + working-set lock, zero-alloc audio callback, diagnostics.
* Module contract v0 (manifest, closed port type set, param snapshots, lifecycle) with a machine-checked API snapshot.
* Graph executor with atomic mutation + a 10 000-iteration mutation stress test; three clocks + sample-accurate transport.
* Offline render + golden-reference harness; journal, `.sparq` project, deterministic replay.
* egui shell with normalised touch/pen/mouse gestures and per-monitor DPI; touch-editable graph canvas; 17 modules incl. the first scope display.
* **Exit criteria:** zero xruns in a 2 h soak at 96 kHz / 64 samples on the stage device; 12-module patch built by touch in <3 min; bit-exact render **and** replay; 100 % touch targets ≥44 px; a new module addable with zero engine edits; identity check passes.
* **Musical deliverable:** the Phase 0 study — 45–90 s made only in sparq, reproducible from project + journal, with a ranked pain log that becomes the Phase 1 backlog.

### Phase 1 — Kernel & module contract (weeks 7–14)
* Threading model, clock broker (3 clocks), transport v1, double-buffered param snapshots, arenas, watchdog, degradation ladder.
* Module API v1 (manifest, ports, params, state, lifecycle) + module browser/discovery + hot reload for T1.
* **Contract v1.1 — the instrument freeze set (WO-017, ADR-010):** the `layer` field, the display-list + scene-descriptor vocabularies, the runtime token-bundle hand-off, the WIT binding of the v1 module API, the five-file package spec, `MODULE-BUILD-GUIDE.md` v1 and the skeleton template. Frozen *before* any instrument authorship starts, so core and instruments can then be built in parallel.
* Journal v1 + deterministic replay + project file format v1.
* Offline renderer (bit-reproducible), WAV import/export.
* 15 modules: `syn/sine`, `syn/noise`, `syn/polyblep`, `flt/svf`, `env/ad`, `mod/lfo`, `util/gain`, `util/mixer`, `util/delay`, `ana/rms`, `ana/tap`, `out/main`, plus the three WO-007 adapters (`util/range`, `util/offset`, `util/gate-to-cv`) that make the canvas's one-tap converter offer real rather than a button that does nothing.
* **Exit criteria:** API reviewed and *frozen at v1.0-rc*; a patch survives 8 hours; a module can be added without editing engine code; replay reproduces a render bit-exactly; hot reload works mid-playback.
* **Musical deliverable:** a 2–3 minute techno sketch, all in-engine.

### Phase 2 — Audio identity (weeks 15–26)
* Full synth toolkit (wavetable, FM, additive/partial, physical models), granular, spectral (FFT resynthesis, phase vocoder, convolution), FX suite, dynamics, analysis suite.
* Sample engine v1 (import, analysis cache, slicing, stretch, multisample, loop recorder).
* `syn/math` (function→wavetable, wavetermaining, CA wavetables, attractor trajectories).
* Golden-reference test corpus established (this is your quality floor forever).
* **The instrument host (WO-018/WO-019, ADR-010):** the `wasmtime` loader behind the `instrument-host` feature, drop-in discovery of `instruments/` at launch, `sparq mod validate` + `sparq mod dev`, and **two first-party reference instruments built through the public path** (proving display-list v1 and scene-descriptor v1) — then the hand-off opens to external authors, who build in parallel against the frozen contract with the nightly conformance matrix as the integration point.
* **Exit criteria:** you can design a kick, a break-mangling chain and an inharmonic metallic texture entirely in sparq; blind ABX vs a reference plugin chain passes; CPU budget for a "typical dense patch" < 40 % on the dev machine at 96 k/64.
* **Musical deliverable:** **EP #1 (3–4 tracks)** — breakcore/IDM, made and mixed in sparq.

### Phase 3 — Library & sample wrangler (weeks 27–36)
* Content-addressed asset store, metadata DB, analysis pipeline, browser UX, faceted + similar-to search, slicing workbench, batch prep, packs.
* Fingerprints + embeddings; local-only first (no server yet).
* **Exit criteria:** 100 000-asset library indexes in background without audio impact; search < 50 ms; "similar to this" returns musically sensible results; a full project reloads with all assets verified by hash.
* **Musical deliverable:** a sample-pack of your own sounds, organised and analysed, used in a new track.

### Phase 4 — Music systems (weeks 37–50)
* Sequencers (step, piano roll, euclidean/polyrhythm, trigger-logic), microtiming fields, subdivision lattices, CV lanes.
* Chord/harmony engine, tuning (Scala/KBM, N-EDO), arpeggiator/generative player.
* Mutation engine v1 (deterministic + stochastic + Markov + CA + chaos + spectral-of-sequence), operator chains, **DNA tree** UI.
* Form engine (sections, energy curves, probabilistic arrangement), Link/clock sync.
* **Exit criteria:** a 6-minute track composed *entirely* with generative + mutation tools (no hand-drawn notes), with a reproducible seed; DNA tree navigation is a pleasure, not a chore.
* **Musical deliverable:** **EP #2** — generative/experimental, plus a public devlog of the DNA trees (this is your audience-building asset).

### Phase 5 — Data plane (weeks 51–62)
* Stream bus hardening, adapters: OSC, MIDI 2.0 (+MPE, CI, Property Exchange), serial/HID/BLE, network/numeric, files/tables, video (frames + optical flow + brightness), internal telemetry.
* Conditioning toolkit, map mode, mapping canvas, stream recording/replay, rehearsal mode.
* **sparq Field Kit** hardware v1 + firmware + calibration + 3 example patches.
* T3 process bridge polish (the T2 instrument tier moved forward to Phase 1–2 — ADR-010; what remains here is registry/marketplace plumbing for instruments, on the way to §13.4).
* **Exit criteria:** a 20-minute set driven by wearables + camera + a network feed with zero dropouts; a disconnected sensor degrades gracefully and is visible in the UI; recorded streams replay identically.
* **Musical deliverable:** a documented **data-driven performance** (video), and a track made from sonified data.

### Phase 6 — Interface & visual identity (weeks 63–76)
* Custom retained-mode shell (or the egui→custom swap), full design system, responsive breakpoints, gesture set, command surface, accessibility pass.
* Display modules (all of §14.4), visual presets, second-output/projector path, GPU compute visuals.
* Perform mode, scene matrix, macros, capture/recall, pre-flight checklist, panic/recovery.
* **Exit criteria:** a 45-minute live set played on a touchscreen tablet with visuals projected from the same app; 60 fps sustained with a dense patch; a stranger can launch a scene and move a macro within 60 seconds without instruction.
* **Musical deliverable:** **first live public performance** (recorded, mixed from stems + journal).

### Phase 7 — Spatial audio (weeks 77–88)
* Rig definitions, VBAP/VBAP3D, HOA encode/rotate/decode (order 1–7), binaural + HRTF, object mixer with distance/Doppler, spatial reverb, trajectory generators, room calibration workflow, multichannel bounce.
* **Exit criteria:** a quad and an 8-channel set delivered in an unfamiliar room, calibrated in <30 min with the built-in measurement workflow; localisation accuracy verified by a listener test; binaural downmix sounds right on headphones.
* **Musical deliverable:** **a surround/ambisonic release** (quad or 7.1.4 + binaural) and a spatial live set.

### Phase 8 — Cloud, hardening, v1.0 (weeks 89–102+)
* Cloud sync + registry (namespaces, signing, packs), sharing, quota, self-hostable backend.
* Reliability campaign: sanitizers, fuzzing parsers, 72-hour install-mode soak, CI on real hardware, accessibility audit, docs/onboarding, module author guide + templates.
* **v1.0 exit criteria:** the full UC-1..UC-7 use cases pass; a fresh install on a new machine reproduces a shared project exactly; 72 h install run with no intervention; API documented and stable.
* **Musical deliverable:** **Album #1**, entirely sparq, released with a public "how it was made" (patch + journal + DNA trees). That artefact *is* the marketing and the artistic statement.

### 17.1 Ongoing disciplines (every phase, no exceptions)

* **Weekly:** one finished 1–3 minute piece or study; a devlog note; a golden test added.
* **Monthly:** a public artefact (track, video, screenshot set, module release); a performance/reliability review; a backlog prune.
* **Per phase:** a listening panel (blind ABX vs references), a stage-readiness test (play a set from a cold boot), a documentation pass.
* **Continuous:** the *instrument-fitness question* — "did I want to play this today?" If the answer is no for two weeks, stop feature work and fix playability. This is the most important metric in the project.

---

## 18. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | **Scope creep** — the plan is huge | Very high | Fatal | Phase gates with musical deliverables; API freeze after Phase 1; "no new subsystem until the current one shipped a track"; keep a `LATER.md` parking lot |
| R2 | **Building a tool nobody plays** (incl. you) | High | Fatal | Music from week 1; hybrid sketch environment (§16.3); instrument-fitness metric; perform mode early (Phase 2 dogfooding) |
| R3 | Real-time bugs (xruns, denormals, memory growth) | High | High | RT discipline enforced by lint/review; arenas; watchdog + degradation; sanitizers; 8 h soak tests in CI; deterministic replay |
| R4 | Touch UI ergonomics fail on stage | Medium | High | Prototype on the *actual* device in Phase 0; blind/dark testing; ≥44 px rule; physical controller fallback for every critical function; test with gloves/sweaty hands |
| R5 | Module ABI churn breaks old patches | Medium | High | Freeze + semver; alias tables; migration chains; stub/repair dialog; API snapshot tests in CI |
| R6 | Spatial audio complexity explodes | Medium | Medium | Ship VBAP + stereo/quad first; HOA order ≤3 initially; calibration workflow before higher orders; per-venue rig presets |
| R7 | Library/cloud becomes a second product | Medium | Medium | Local-first forever; server optional and self-hostable; defer registry to Phase 8; no social features |
| R8 | Copyright/provenance issues with samples | Medium | Medium–legal | Provenance tracking, clearance report, licence metadata, no scraping into public registry |
| R9 | Solo burnout | High | High | Sustainable cadence, public artefacts as motivation, phase-end celebrations, a "no build, only music" week each quarter, optional collaborators for art/hardware |
| R10 | GPU/renderer rabbit hole | Medium | Medium | Use `egui` scaffold until Phase 6; visuals must never cost audio time; cap shader complexity; LOD |
| R11 | Hardware/sensor flakiness on stage | Medium | Medium | Rehearsal mode with recorded streams; stale-stream policies; wired fallback for every sensor; pre-flight checklist |
| R12 | Analysis/ML features underdeliver | Medium | Low | All ML is T3 and optional; core musical value never depends on it |

---

## 19. Budget & resources (indicative)

| Item | Notes | Est. |
|---|---|---|
| Dev machine | 8–16 core CPU, 32–64 GB RAM, discrete GPU (WebGPU compute), fast NVMe, Linux + dual-boot macOS/Windows for testing | $2.5–4 k (or existing) |
| Touchscreen device | 11–13" tablet or convertible, high-refresh, palm-rejection; the *performance* surface | $0.6–1.5 k |
| Audio interface | ≥8 outs (16 ideal) for spatial work, low latency, good drivers | $0.5–1.5 k |
| Speaker test array | 4–8 monitors + subs, amp, cabling, stands | $1–4 k (start with 4) |
| Measurement mic + calibration | Measurement-grade mic, interface gain staging | $0.2–0.5 k |
| Controller | Grid controller / MIDI 2.0 keyboard / MPE controller | $0.3–1.2 k |
| **sparq Field Kit** | IMUs, EMG, FSRs, encoders, MCU boards, enclosure, hub | ≤ $0.35 k |
| Cloud backend | Object store + small DB/API, minimal traffic to start | $0–30 /mo |
| Test devices | Second tablet, a weak laptop (performance floor), an ARM SBC (appliance path) | $0.3–0.8 k |
| Reference/inspiration | Records, books (composition + DSP), occasional plugins for ABX reference | ongoing |

Time is the dominant cost: **~1 900–3 300 hours** to v1.0.

---

## 20. Repository layout and engineering conventions

```
sparq/
├─ Cargo.toml / workspace members
├─ crates/
│  ├─ sparq-kernel/        # clocks, executor, threading, arenas, HAL, journal
│  ├─ sparq-module-api/    # the frozen contract: manifest, ports, params, state (v1)
│  ├─ sparq-stream/        # stream bus, types, conditioning, adapters (host side)
│  ├─ sparq-audio/         # DSP primitives, filters, transforms, spatial, analysis
│  ├─ sparq-music/         # transport, sequencers, harmony, tuning, mutation, form
│  ├─ sparq-sample/        # sample engine, slicing, stretch, asset-backed playback
│  ├─ sparq-assets/        # content-addressed store, metadata DB, fingerprints, sync
│  ├─ sparq-ui/            # shell, widgets, design tokens, renderer abstraction
│  ├─ sparq-visual/        # display modules, GPU compute visuals, colour maps
│  ├─ sparq-perform/       # modes, scenes, macros, capture, recovery
│  ├─ sparq-io/            # midi2/ump, osc, serial, hid, ble, net, video, camera
│  ├─ sparq-host-wasm/     # wasmtime runtime + component bindings (Phase 1–2 — the instrument tier, ADR-010)
│  ├─ sparq-bridge/        # T3 process bridge, shm rings, IPC protocol
│  └─ sparq-app/           # binary, config, CLI, kiosk/install mode
├─ modules/                # first-party T1 backbone modules (one crate or module per family)
│  ├─ syn/ flt/ fx/ smp/ seq/ gen/ ana/ spa/ dat/ dsp/ util/
├─ instruments/            # drop-in instrument packages (≤5 files each, ADR-010); discovered at launch
├─ adapters/               # source adapters (network, video, sensors, files)
├─ field-kit/              # firmware, enclosures, calibration docs
├─ reference/              # golden renders, test corpora, ABX material
├─ design/                 # tokens, type, colour maps, panel specs, visual boards
├─ docs/                   # architecture, module author guide, decisions (ADRs)
├─ tools/                  # codegen (Faust→module), packager, registry CLI
└─ examples/               # patch templates, demo projects, tutorials
```

Conventions:
* **ADR (architecture decision record)** for every significant choice — you will forget why in 18 months. One file per decision, ~1 page.
* Rust: `#![forbid(unsafe_code)]` everywhere except the audio HAL, ring buffers and FFI (those get their own reviewed unsafe module with SAFETY comments and Miri coverage).
* CI matrix: Linux (real audio device runner), macOS, Windows; debug + release; sanitizers on nightly jobs; golden-render diff on every DSP change; screenshot diff on every UI change; `cargo deny`/audit for supply chain.
* Versioning: engine semver; module API version separate and frozen per minor; project format version with migration chain.
* Docs-as-code: module docs generated from manifests; every module ships an example patch.

---

## 21. The first 30 days

Fully specified as buildable tickets in **`PHASE0-WORKORDERS.md`** (WO-000 … WO-016, with acceptance criteria, tests, artefacts, a Windows-first setup checklist and a recommended 6-week schedule). Summary:

**Week 1 — Foundations and taste.** Repo/workspace/CI + engineering gates (WO-000) · stage hardware decision and Windows audio/DPC/touch baseline (WO-001) · design tokens v0 and colour maps (WO-002) · `VISION.md` + ADR-001…007 (WO-003).

**Week 2 — Identity and first sound.** Static canvas mockups at true resolution, reviewed at 3 m and in the dark (WO-004) · first sound out via a disposable bootstrap path (WO-005) · real HAL: WASAPI exclusive + ASIO + null device, MMCSS, working-set lock, zero-alloc audio callback, diagnostics (WO-006).

**Weeks 3–4 — The contract, the graph, the pixel.** Module contract v0 with machine-checked API snapshot (WO-007) · graph executor with atomic mutation and a 10 000-iteration mutation stress test (WO-008) · three clocks + sample-accurate transport (WO-009) · offline render + golden-reference harness (WO-010) · journal, `.sparq` project, deterministic replay (WO-011) · egui shell with normalised touch/pen/mouse gestures and per-monitor DPI (WO-012) · graph canvas editable entirely by touch (WO-013) · 17 modules including the first scope display (WO-014).

**Weeks 5–6 — Music and the gate.** The Phase 0 study: 60 s of music made only in sparq, reproducible from project + journal, with a ranked pain log (WO-015) · Phase gate: 12 h soak, all numbers measured, devlog published, Phase 1 backlog written from the pain log (WO-016).

**Definition of done for Phase 0:** zero xruns in a 2 h soak at 96 kHz / 64 samples on the stage device · a patch buildable entirely by touch in under 3 minutes · bit-exact offline render and journal replay · a screenshot that reads as "scientific software" · and a piece of music that only sparq could have made.

---

## 22. Decisions

**Locked in v0.2 (2026-09-20):**

| ID | Decision | **Chosen** | Consequences folded into the plan |
|---|---|---|---|
| **D-3** | Implementation language | **Rust core** (Faust codegen optional later) | §16.1 stack table; `#![forbid(unsafe_code)]` except HAL/rings/FFI (§20); wasm target available for T2 + browser thin client |
| **D-2** | Module tiers at start | **T1 native + T3 process bridge now; T2 wasm in Phase 5** | §6.1, §17 Phase 5. Manifest and port types stabilise on native modules before a sandboxed ABI is frozen |
| **D-1** | UI rendering strategy | **egui scaffold (Phase 0–2) → custom retained-mode `wgpu` shell (Phase 6)** | §14.1, §17 Phase 6. All shell code sits behind a thin renderer/layout abstraction so the swap is contained; design tokens are authored independently of the toolkit from day 1 |
| **D-4** | Primary platform | **Windows** (Linux + macOS first-class, in CI from Phase 1) | §5.2 Windows RT discipline; §5.5 WASAPI-exclusive/ASIO HAL + device aggregator; §16.1. Phase 0 targets Windows 11 on a real touch device |

**Locked in v0.3 (2026-10-04):**

| ID | Decision | **Chosen** | Consequences folded into the plan |
|---|---|---|---|
| **D-14** | Instrument layer & third-party hand-off (amends D-2) | **Two-layer library (`layer = backbone \| instrument`); instruments run in the T2 wasm sandbox pulled forward to Phase 1–2; displays are host-rendered scene data resolved against the runtime token bundle; five-file drop-in packages in `instruments/`; `sparq mod validate` as the hand-in gate** | ADR-010; §6.1 amendment, §6.6, §6.9, §14.5, §15, §17 Phases 1/2/5, §20 layout, Appendix A; `MODULE-BUILD-GUIDE.md`; WO-017…019 |

**Still open (resolve during Phase 0–1):**

| ID | Decision | Options | Recommendation |
|---|---|---|---|
| **D-5** | Sample rate policy | 44.1 / 48 / 96 default; per-project or global | **48 kHz default, 96 kHz for design/mastering; project-level setting with SRC on import** |
| **D-6** | Block size policy | Fixed 64 / adaptive | **Adaptive with a floor of 32; default 64 in Perform, 256 in Design** |
| **D-7** | Library backend | Local-only / self-hosted / managed cloud | **Local-only until Phase 8, then self-hosted-first with a managed option** |
| **D-8** | Distribution intent | Private tool / open core / commercial | Decide before Phase 3 — changes signing, docs and licensing effort. **Recommend open core (engine + module API public, personal library private)** |
| **D-9** | Dedicated hardware appliance | Never / later (`sparq stage`) | **Later (post-v1.0).** Keep the app headless-capable + thin-client so an appliance is a packaging exercise. Note: with Windows primary, a future appliance would most likely be a Linux box — don't let Windows-isms leak into the kernel |
| **D-10** | Web presence | None / thin client / full web app | **Thin client (visuals + macro pads) in Phase 6; full web app never** |
| **D-11** | Stage touch device | Surface-class 2-in-1 / large touch panel + mini-PC / separate touch controller beside a laptop | Decide in Week 1 (WO-001). **The Phase 0 device must be the actual stage device** — ergonomics findings don't transfer |
| **D-12** | Interface channel count | 2-in/2-out now vs 8+ outs immediately | Validate WASAPI exclusive + ASIO on a small interface in Phase 0; **buy ≥8 outs before Phase 7** |
| **D-13** | Rust audio HAL bootstrap | `cpal` then replace / write WASAPI+ASIO HAL immediately | **`cpal` for WO-005 only** (get sound out in a day), then the real HAL in WO-006. Do not let `cpal` set the capability model |

---

## Appendix A — Naming, taxonomy and conventions

**Module IDs:** `namespace/category/name@semver`, e.g. `sparq/syn/wavetable@1.4.0`, `you/fx/granulator@0.3.1`.

**Categories (fixed top level):**
`syn` synthesis · `smp` sampling · `flt` filtering · `fx` effects · `dyn` dynamics · `ana` analysis · `spa` spatial · `seq` sequencing · `gen` generative/mutation · `harm` harmony/tuning · `dat` data & streams · `io` device/network I/O · `dsp` displays · `util` utility · `ml` machine learning · `out` outputs

**Parameter naming:** lowercase snake_case, SI units in the display (`Hz`, `ms`, `dB`, `%`, `st`, `ch`, `LUFS`), normalised internal ranges with a declared curve, and human-readable labels in the UI.

**Project/asset formats:**

| Extension | Contents |
|---|---|
| `.sparq` | Project bundle: graph, journal head, seeds, module versions, asset refs (hashes), rig presets, scene data |
| `.sparqpatch` | A subgraph (reusable instrument/effect chain) with port surface |
| `.sparqpreset` | Parameter vector for one module (+ morph-compatible siblings) |
| `.sparqpack` | A shareable collection (samples/presets/patches/visuals) with manifest + licences |
| `.sparqrig` | Spatial rig definition (layout, calibration, correction FIRs) |
| `.sparqmap` | Controller map (source → target bindings) |
| `.sparqjournal` | Session journal (append-only, checksummed) |
| `.sparqbounce` | Render metadata sidecar (multichannel layout, loudness targets, provenance) |
| `.sparqmod` | Module package (manifest + binary/wasm + assets + docs + example patch). **Instrument packages are capped at five files** — manifest, wasm, example patch, validator-generated preview, README (ADR-010); bulk assets ride by hash in the library |

All are containers with a version header, content-addressed internals where practical, and a documented text-diffable representation for the graph (so patches can be version-controlled and reviewed as text).

---

## Appendix B — Initial module backlog (the first 60, by phase)

**Phase 1 (15):** `syn/sine` `syn/noise` `syn/polyblep` `flt/svf` `env/ad` `mod/lfo` `util/gain` `util/mixer` `util/delay` `ana/rms` `ana/tap` `out/main` **+ the three WO-007 adapters (added 2026-09-22):** `util/range` `util/offset` `util/gate-to-cv` — each trivial, and each one the thing a refused connection offers to insert (ADR-005 addendum)

**Phase 2 (+28):** `syn/wavetable` `syn/fm-ops` `syn/additive` `syn/membrane` `syn/resonator-bank` `syn/math-func` `syn/math-terrain` `syn/ca-table` `gra/cloud` `gra/stretch` `gra/freeze` `spc/fft-resynth` `spc/phase-vocoder` `spc/cross-synth` `spc/convolve` `flt/ladder` `flt/comb` `flt/brickwall` `fx/bitcrush` `fx/wavefolder` `fx/freq-shift` `fx/ringmod` `fx/stutter` `fx/slicer` `fx/fdn-reverb` `fx/tape-delay` `dyn/comp` `dyn/limiter`

**Phase 3–4 (+20):** `smp/player` `smp/slicer` `smp/multisample` `smp/loop-rec` `smp/morph` `seq/step` `seq/piano` `seq/euclid` `seq/polyrhythm` `seq/trigger-logic` `seq/cv-lane` `seq/phrase` `harm/chords` `harm/scale-tuning` `harm/arp` `gen/markov` `gen/lsystem` `gen/chaos` `gen/ca` `gen/dna-tree`

**Phase 5–7 (+~40):** `dat/osc` `dat/midi2` `dat/serial` `dat/hid` `dat/ble` `dat/net-http` `dat/net-ws` `dat/csv` `dat/video-frames` `dat/optical-flow` `dat/pose` `dat/telemetry` `dat/condition` `dat/mapper` `dat/recorder` `dat/replay` `ml/embed` `ml/neural-synth` `ml/classify` `spa/vbap` `spa/hoa-encode` `spa/hoa-rotate` `spa/hoa-decode` `spa/binaural` `spa/objects` `spa/trajectory` `spa/spatial-reverb` `spa/calibrate` `spa/matrix` `dsp/scope` `dsp/spectrum` `dsp/sonogram` `dsp/phase-portrait` `dsp/vector-field` `dsp/geometry` `dsp/rig-map` `dsp/graph-view` `dsp/dna-view` `dsp/streams` `dsp/meters` `dsp/particles`

---

## Appendix C — Quality gates (the numbers that must hold)

| Metric | Target | Measured by |
|---|---|---|
| Xruns in 2 h at 96 k / 64, dense patch | **0** | CI soak on real hardware |
| Block processing p99 | < 40 % of block budget | Histogram in diagnostics |
| Controller→sound round trip | **< 10 ms** | Loopback measurement + reported |
| UI frame rate, dense patch, 4K | **≥ 60 fps** | Frame-time histogram |
| Visual work on audio thread | **0 ms** | Static + runtime assertion |
| Allocations on audio thread | **0** | Custom allocator counter in debug/CI |
| Offline render reproducibility | **bit-exact** | Hash comparison in CI |
| Golden-render deviation | ≤ 1e-6 relative (f64 mode: exact) | Per-module test |
| Library search (100 k assets) | < 50 ms | Benchmark |
| Project cold load (large) | < 10 s | Benchmark |
| Crash recovery | < 10 s to last good state | Soak test with injected kills |
| Install mode unattended | ≥ 72 h | Soak test |
| Multichannel | ≥ 32 outs, ≤ 64 | Device + rig tests |
| HOA order | ≤ 7 (64 ch) | Spatial test suite |
| Startup to first sound | < 5 s | Benchmark |
| Touch target compliance | 100 % ≥ 44 px | Automated layout audit |

---

## Appendix D — Glossary

**BLAMP/BLEP** — band-limited step/parabolic interpolation for aliasing-free discontinuities. **DNA tree** — sparq's mutation lineage graph. **FDN** — feedback delay network (reverb). **Field Kit** — the reference sensor hardware rig. **HOA** — higher-order ambisonics. **HRTF** — head-related transfer function (binaural). **Journal** — append-only session record enabling replay/re-render. **LUFS** — loudness units relative to full scale. **MPE** — MIDI polyphonic expression. **Rig** — a saved speaker layout + calibration. **Scene** — a snapshot of parameter/routing/spatial/visual state. **Stream** — normalised typed time-series from any source. **T1/T2/T3** — module execution tiers (native / wasm / process). **UMP** — Universal MIDI Packet (MIDI 2.0). **VBAP** — vector-base amplitude panning.

---

## Appendix E — Study and reference list

**Technical**
* Udo Zölzer (ed.), *DAFX: Digital Audio Effects* — the FX/DSP canon.
* Curtis Roads, *Microsound* and *The Computer Music Tutorial* — granular/pulsar synthesis.
* Julius O. Smith, *Physical Audio Signal Processing* (free online) — waveguides, FDN reverbs, filters.
* Miller Puckette, *Theory and Technique of Electronic Music* (free) — the practical DSP + patching grounding.
* Faust documentation & libraries (GRAME) — DSP codegen, HOA library.
* Web Audio API / AudioWorklet and WASM component-model docs — for the T2 tier and the thin client.
* VBAP (Pulkki) and Ambisonics/AmbiX + SOFA/HRTF specifications — for the spatial plane.
* MIDI 2.0 specification (UMP, Profiles, Property Exchange), Ableton Link protocol, OSC 1.1.
* *The Scientist and Engineer's Guide to DSP* (Smith) — approachable reference.

**Musical/aesthetic (study the systems, not just the sounds)**
* Autechre — *Confield*, *Draft 7.30*, *NTS Sessions*, *PLUS* (systems, generative form, inharmonic timbre).
* Aphex Twin — *Drukqs*, *Syro*, *Collapse EP* (DSP extremes, prepared acoustics).
* Venetian Snares — *Rossz Csillag Alatt Született*, *Winter in the Belly* (slicing, polyrhythm, orchestral collision).
* Squarepusher — *Hard Normal Daddy*, *Go Plastic*, *Ufabulum*, *Be Up a Hello* (acoustic↔synthetic, harmony, live bass).
* Richard Devine — *Hemispheres*, *Eros*, *Sort/Loop* (maximalist sound design, generative arpeggiation).
* Ryoji Ikeda — *dataplex*, *test pattern*, *supercodex* (data as material, audiovisual unity, scientific presentation).
* Autechre's own public statements about building their tools (the patch-as-master-record principle) — read and re-read when tempted to add a feature that doesn't serve the music.

---

*End of plan v0.1. Next artefact to produce: `docs/VISION.md` (one page), `design/tokens.md`, and ADR-001 (stack) — then Phase 0 begins.*
