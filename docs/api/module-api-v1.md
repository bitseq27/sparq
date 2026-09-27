# sparq module API — v1 draft (v0.1)

**Status:** draft for WO-007; the runtime surface described here is **implemented as contract v1**
(WO-008 increment 4, 2026-09-26 — see §17). **Freeze target:** end of Phase 1 as `v1.0-rc`, then `v1.0`.
**Governing ADRs:** 002 (tiers) · 005 (port types) · 006 (clocks/latency) · 007 (determinism) · 009 (executor/HAL).
**Rule of this document:** everything here must be expressible by a *granular cloud*, a *partitioned convolution reverb*, an *HOA encoder*, a *MIDI 2.0 MPE input*, a *camera optical-flow analyser*, a *CSV player* and a *DNA-tree mutation module* — without inventing a new port type. §14 runs that test.

No code in this document. Signatures are written in prose + tables so the shape can be argued before it is typed.

---

## 1. Goals and non-goals

**Goals**

1. A new module can be added with **zero** edits to engine code (CI-enforced).
2. It is structurally impossible for `process()` to allocate, lock, block or perform a syscall.
3. The same manifest describes a native module, a wasm module and a process-bridged module.
4. Old patches load forever: additive evolution, deprecation windows, alias tables, migration chains.
5. Authoring a simple module is a **two-hour** task; a complex one is a two-day task.
6. Every module is testable offline, deterministically, with a golden render.

**Non-goals**

* Compatibility with VST3/CLAP as the native format (those are wrapped by an optional T3 host module).
* A visual programming language inside modules (the graph is the visual language).
* Runtime scriptability on the audio thread (see ADR-002).

---

## 2. The module surface (what a module is)

A module is a package consisting of:

| Part | Required | Notes |
|---|---|---|
| `sparqmod.toml` manifest | yes | the contract's front half; validated at discovery (§11) |
| Implementation (native binary / wasm component / process) | yes | the contract's back half |
| Golden reference render | yes (T1) | `reference/golden/<id>/<case>.wav` + expected hash |
| Unit tests | yes | properties from §13 |
| Example patch | yes | `.sparqpatch` showing the module doing its job |
| Docs | generated | produced from the manifest; hand-written notes optional |
| Panel descriptor | optional | if absent, the shell auto-generates a panel from params (§10) |

Behavioural surface (the six things a module does):

| Stage | Thread | May allocate? | May block? | Deterministic? |
|---|---|---|---|---|
| `instantiate` | control | yes | yes | must be |
| `configure(state)` | control | yes | yes | must be |
| `prepare(resources)` | control | **yes (all of it here)** | yes | must be |
| `process(block)` | **audio** | **no** | **no** | **must be** |
| `deactivate` / `dispose` | control | no | yes | must be |
| `message(atom)` | control | yes | yes | must be |

`prepare(resources)` receives everything that can change the module's memory or timing shape: sample rate, block size, channel sets per audio port, oversampling factor, voice count, GPU device handle (if declared), arena budget. Any change to these ⇒ `deactivate → prepare → activate`. Nothing in `process` may assume a value that was not given in `prepare`.

**Channel-set resolution is a resource change, never a graph mutation** *(rule added 2026-09-22, WO-007 task 1 — this is what §14's HOA row asked for)*. When `prepare` resolves a `variable` port to a concrete channel set, every module whose effective channel set depends on that port is re-prepared in topological order within the same control-thread pass. The graph structure, its version and its mutation counter are untouched. Resolution is idempotent, and a pass that changes no resolved set terminates it — a neighbour that flips its own set back is a livelock, which the executor must detect and refuse rather than spin on. Test, owned by WO-008: `channel_set_change_reprepares_neighbours_without_bumping_graph_version`.

---

## 3. Ports (ADR-005)

A port declaration:

| Field | Values / meaning |
|---|---|
| `id` | unique within the module, stable forever (renaming requires an alias) |
| `name` | human label |
| `direction` | `in` \| `out` |
| `type` | `audio` \| `cv` \| `event` \| `data` \| `gpu` \| `atom` |
| `required` | bool — optional inputs receive an explicit *unconnected* signal, never silence-by-accident |
| `channel_set` | for `audio`: fixed tag, `variable`, or a list of accepted tags (§3.1) |
| `rate` | for `cv`: `audio` (per sample) \| `block` (one value per block) |
| `range` | for `cv`: `bipolar` (−1..1) \| `unipolar` (0..1) |
| `cv_reduce` | for `cv` at `block` rate fed by an `audio`-rate source: `last \| first \| mean \| min \| max \| peak` (default `last`) — **declared in the manifest, never implicit** (ADR-007) |
| `cv_interp` | for `cv` at `audio` rate fed by a `block`-rate source: `hold \| linear \| spline` (default `hold`) — the host performs it, the module declares which |
| `data_schema` | for `data`: schema id + version (§4) |
| `event_kinds` | for `event`: subset of `ump`, `osc`, `trigger`, `gate`, `note`, `clock` |
| `multiplicity` | `single` \| `multi-in` \| `multi-out` (fan-in requires an explicit merge; fan-out is free) |
| `latency_contribution` | samples this port path adds (usually 0; used by convolution/granular) |

### 3.1 Channel sets

`mono · stereo · quad · 5.1 · 7.1.4 · ambisonics:N (N=1..7, AmbiX) · objects:K · raw:M`

A port may declare `variable` and be told the actual set at `prepare`. Modules that accept several sets declare a list; the host picks the intersection with the connected neighbour, and reports the choice in the UI. **Nothing is silently up/downmixed** except the two documented cases: `mono→multi` (fan-out) and `multi→mono` (sum, with a warning hairline).

---

## 4. `data` records

A `data` port carries a stream of timestamped records over a declared schema:

* **Schema** = ordered list of channels, each `{ id, dtype (f32|f64|i32|i64|bool|vecN|enum|str|blob), unit, range, interpolation (none|step|linear|spline) }`, plus `rate_class` (`dc | slow ≤10 Hz | mid ≤200 Hz | fast ≤2 kHz | bursty`) and `clock_domain` (`wall | audio`).
* Records are `{ values, t_wall, t_sample?, seq, flags(ok|stale|discontinuity|estimated) }`.
* Schemas are registered by id+version in the manifest; unknown schemas are a load-time error, not a runtime surprise.
* Standard schemas ship with sparq (`sparq/imu9@1`, `sparq/optical-flow@1`, `sparq/loudness@1`, `sparq/pitch@1`, `sparq/onset@1`, `sparq/timeseries@1`, `sparq/pose2d@1`, …) so third-party modules interoperate without negotiation.

---

## 5. Parameters

| Field | Meaning |
|---|---|
| `id`, `name` | stable id; display label |
| `type` | `float \| int \| bool \| enum \| text \| blob` |
| `unit` | from the closed unit vocabulary (`Hz, s, ms, samples, dB, %, st, cents, ratio, ch, LUFS, deg, m, bpm, ticks, x`) — no invented units |
| `range`, `default` | inclusive range; default must be inside it |
| `curve` | `lin \| exp \| bipolar \| custom:<lut id>` |
| `smoothing` | `none \| one_pole:<ms> \| lin:<ms> \| lag:<ms>` — applied **in the audio thread** by the host unless the module declares `self_smoothed` |
| `mod_depth` | default modulation depth when mapped |
| `automatable` | can be written by automation/journal |
| `randomisable`, `lockable` | participate in seeded randomise |
| `per_voice` | value differs per voice (§7) |
| `hidden` | not shown in the auto panel (still automatable) |

**Universality rule:** every parameter is a modulation target. Any `cv` output, `data` channel, event CC or LFO can drive it, with `amount`, `curve`, `bipolar` and `smoothing`. This is one system, not three (plan §6.4).

**Morphability rule:** parameter vectors must be *semantically continuous* wherever possible, because scene crossfades, preset morphs and DNA-tree auditioning all interpolate between two vectors. A module that cannot interpolate a parameter declares it `morph: discrete`, and the host holds it constant during a morph instead of interpolating garbage.

---

## 6. State

* State is a versioned, serialisable blob whose **schema id + version** are declared in the manifest.
* Every module must be serialisable (no exceptions) — a module that cannot save its state cannot be in a project.
* **Migration chain:** a module declares ordered migrations `vN → vN+1`. Loading an old project runs the chain, re-saves, and preserves the original.
* State may reference assets **only by hash** (never by path), so projects remain portable and verifiable.
* Assets a module needs (wavetables, IRs, tables, models) are declared in the manifest with hashes and sizes; the host loads them before `prepare` and hands over immutable references.

---

## 7. Voices (declared in Phase 0, implemented Phase 1)

| Field | Meaning |
|---|---|
| `policy` | `none \| mono \| poly:<max> \| mpe \| multitimbral:<n>` |
| `per_voice_params` | which parameters are per voice |
| `allocation` | `oldest \| quietest \| highest \| lowest \| round_robin \| priority` |
| `unison` | `{count, detune_curve, spread}` |
| `note_events` | which `event` kinds start/stop/modulate a voice |

Rules: voices come from a **host-managed pool**; a module never allocates on note-on. Voice state is part of the module's state for serialisation, but voice *count* is a `prepare` resource. MPE per-note controllers arrive as `event` payloads and are exposed to the module as per-voice `cv` values.

---

## 8. Resources, latency and the watchdog

Declared in the manifest, verified by tests:

* `latency_samples` (may be a function of parameters — declared as `latency: param:<id>` so the executor recomputes on change)
* `tail_samples` (how long output continues after input stops — reverb, delay)
* `cpu_class` (`trivial | light | medium | heavy | very_heavy`) — used for budgeting and the degradation ladder
* `mem_class`, `gpu_class` (arena budget hints)
* `oversampling` (`none | x2 | x4 | x8`, per-module, host-provided)
* `requires` feature flags (`fft`, `midi2`, `spatial`, `ml`, `gpu_compute`)

Enforcement: the host measures real block time per module; `N` consecutive overruns ⇒ **auto-bypass + red hairline + journal entry** (ADR-009.6). A module that lies about `cpu_class` shows up immediately in the diagnostics panel — dishonesty is visible, which is the point.

---

## 9. Real-time contract (the hard rules)

Inside `process(block)` a module **must not**:

1. allocate or free memory (no `Box`, `Vec`, `String`, `format!`, closures that capture by value into heap);
2. lock a mutex, take a reference-counted write, or touch shared mutable state;
3. perform any syscall, file, network, GPU-submit or IPC operation;
4. read the wall clock (use the block's timestamp provided by the host);
5. use unseeded randomness (all RNG comes from the seed tree, ADR-007);
6. depend on iteration order of any hash-map-like structure;
7. throw/panic — errors are reported via the return status and the watchdog, never by unwinding across the boundary;
8. read/write outside the buffers and arenas handed to it;
9. assume denormal flushing is off (it is on: denormals-are-zero on the audio path);
10. exceed its declared block-time budget.

The host guarantees: contiguous aligned buffers, valid for the whole block; parameter snapshot immutable for the block; sample rate/block size/channel sets unchanged since `prepare`; all events for the block pre-sorted by sample offset; a monotonic `t_sample` block start; and that any mutation of the graph takes effect only at a block boundary.

**Type-level enforcement:** the audio context handed to `process` exposes no allocation or blocking capability at all. The rule is not documentation; it is the absence of an API. Debug builds add a counting allocator and assertion gates so violations fail tests rather than shows.

---

## 10. UI descriptor (panels are data)

A module may declare a panel; otherwise the shell auto-generates one from the parameter list. Declared panels use the shared widget vocabulary so 140 modules look like one product (plan §14.5):

* **Widgets:** `slider`, `knob`, `xy_pad`, `matrix`, `enum_select`, `toggle`, `numeric_entry`, `label`, `meter_attach`, `display_slot`, `button`, `waveview`, `grid_pad`.
* **Layout:** positions on the 8 px grid, sizes in grid units, grouping into rows/sections, visibility conditions on parameters.
* **Touch class:** every interactive widget declares `S | M | L | XL`; the shell refuses to render a widget below its declared minimum (the ≥44 px audit fails the build).
* **Display slots:** which analysis outputs to render, with which display module (`dsp/*`) and which colour map token.
* **Escape hatch:** a T1 module may supply a custom GPU draw routine, but it must consume design tokens and pass the visual conformance checklist (`design/look-board.md`).

---

## 11. Manifest schema and validation

Full schema: `manifest-schema.md` (companion document). Validation happens at **discovery**, before instantiation, and every failure produces an actionable message naming the field and the fix.

Rejected at validation: unknown port type · unknown unit · duplicate ids · param default outside range · undeclared latency for a module that reports non-zero measured latency · missing state schema · asset hash mismatch · `requires` feature not available on this host · unsupported host API version · bad or missing signature when the host is in Perform mode.

Discovery order: built-in registry → user `modules/` directory → project-local modules → registry cache. Conflicts resolve by explicit version pin in the project, never by "latest wins".

---

## 12. Versioning and compatibility

* **Host API version** is separate from engine semver and is declared as a min/max range by every module.
* **Freeze rule:** `v1.0-rc` at end of Phase 1. After that, only additive changes within a minor; removals/renames require a deprecation window of ≥2 minor versions plus an alias table.
* **Behavioural changes** require a `compat` flag so an old patch can request old behaviour explicitly.
* **API snapshot test** in CI fails on any unreviewed change to the public surface.
* **Repair, never crash:** a project whose modules are missing or incompatible opens a repair dialog offering *stub* (preserves routing so the rest of the patch keeps working), *substitute* (a suggested equivalent), or *cancel*. Stubs are visible in the canvas with a distinct hairline pattern.
* **Signature/provenance:** manifests and binaries are signed; unsigned modules are badged in the UI and **disabled by default in Perform mode** (plan §15).

---

## 13. Required tests per module

1. **Golden render** (deterministic, hashed, tolerance documented).
2. **Properties** as applicable: DC-free output for AC-coupled modules · bandlimited within declared tolerance (spectrum plot checked in) · no denormal stalls (worst-case input test) · monotonic filter response · silence in ⇒ silence out for processors with no internal source · latency and tail equal to declared values (measured, not asserted) · parameter extremes do not produce NaN/Inf.
3. **State round-trip**: save → load → identical output; and migration from the previous schema version.
4. **Real-time discipline**: runs under the counting allocator with zero allocations; zero locks (TSan clean).
5. **Stress**: declared `cpu_class` verified at the reference machine's measured cost.
6. **Panel conformance**: screenshot at three breakpoints; ≥44 px audit; token-only colours (grep audit).

---

## 14. Paper test — seven hard modules against this contract

Run this before implementing anything (WO-007 task 1). Each must be expressible with no new port type and no contract change.

| Module | What it stresses | Verdict with v0.1 |
|---|---|---|
| **Granular cloud** (512 grains) | voice-like pool without notes; per-grain `cv`; `prepare`-time grain arena; declared latency | ✅ `policy: poly` with `note_events: none` + host pool; grain params per voice; `latency_samples` declared. *Resolved 2026-09-22:* grain **onsets are** sample-accurate — §9 delivers the block's events pre-sorted by sample offset and the module renders into the block buffer. Sub-block *processing* is forbidden in v0 (§16 Q2), so per-grain parameter movement inside a block is derived by the module from the one immutable snapshot; if that ever proves musically insufficient the remedy is a smaller host block, not a contract change. |
| **Partitioned convolution reverb** | large latency + long tail; FFT requirement; IR asset by hash; heavy CPU class | ✅ `latency: param:<size>`, `tail_samples`, `requires: fft`, IR declared as asset hash. |
| **HOA encoder / decoder** | `ambisonics:N` channel sets; order as a parameter that changes channel count | ⚠️ Order changes the channel set ⇒ must trigger `deactivate → prepare → activate`. Confirm the executor treats a channel-set change as a resource change, not a graph mutation, and that neighbours are re-prepared. **Done 2026-09-22:** the rule is now in §2 and the test is named there. |
| **MIDI 2.0 MPE input** | `event` with UMP payloads; per-note controllers; MIDI-CI discovery off the audio thread | ✅ `event_kinds: [ump, note]`; per-note controllers surfaced as per-voice `cv`; CI discovery via `message(atom)` on the control thread. |
| **Camera optical-flow analyser** | `gpu` in, `data` out, T3 process, staleness | ⚠️ **PROVISIONAL — do not build against this row.** It passes on the payload side (`gpu` input handle; `sparq/optical-flow@1` output; `rate_class: mid`; staleness flags per ADR-006.5), but this module is **T3**, and whether a bridged/third-party module may hold a `gpu` port at all is §16 Q5 — **deferred to Phase 5** (2026-09-22). The `gpu` port *type* is not deferred: Phase 0 ships `ana/tap` and `dsp/scope`, both first-party T1 |
| **CSV / time-series player** | `data` out from a file asset; interpolation policy; transport coupling | ✅ asset by hash; `sparq/timeseries@1`; `interpolation: linear`; sync via `event` `clock` input. |
| **DNA-tree mutation module** | `event`/`atom` in and out; state = lineage; must be deterministic | ✅ lineage in state blob; `atom` for host messages (keep/kill/branch); seeded RNG from the seed tree. |

**Result: the contract holds.** The one rule it asked for (channel-set change ⇒ re-prepare cascade) was added to §2 on 2026-09-22, and one row is now provisional (camera optical-flow, pending §16 Q5). That is exactly the kind of finding this test exists to produce before code does.

---

## 15. Author experience (the two-hour test)

A new module author must be able to:

1. `sparq mod new syn/my-osc` → scaffold with manifest, skeleton, golden-test harness, example patch.
2. Implement `prepare` + `process` in one file.
3. `sparq mod test` → validation, golden render, RT-discipline gates, panel screenshot, ≥44 px audit.
4. Drop the build into `modules/` → discovered, hot-loaded, appears in the browser with generated docs.

If any step needs engine knowledge, the contract has failed and this document gets amended.

---

## 16. Open questions — ANSWERED (WO-007 task 1, 2026-09-22)

1. **Dispatch: trait objects vs generated enum (ADR-009, executor decision 7) — ANSWERED BY MEASUREMENT.** Harness
   `tools/dispatch-bench`: 100 modules × 64-sample block, sparq's exact release profile, four
   distinct implementors so the vtable pointer genuinely varies, three runs, module body identical
   in every variant so the difference is dispatch and nothing else. Per **block** (one dispatch per
   module): `Box<dyn Module>` 1.65–1.95 µs · `enum + match` 1.26–1.30 µs · monomorphised
   1.23–1.31 µs. Per **sample** (6400 dispatches per block): `Box<dyn Module>` **27.1–35.6 µs** ·
   enum 2.08–2.22 µs · monomorphised 2.05–2.17 µs. Two results carry the decision: enum dispatch is
   **within noise of fully monomorphised code** (the `match` compiles away, so there is no cost
   argument against it), and per-sample trait objects **exceed the entire < 20 µs acceptance budget
   on their own**. Decision: **`process(block)` may be `Box<dyn Module>`; nothing called per sample
   may be** — per-sample paths are enum-dispatched or generic. ADR-009 executor decision 7 records the same numbers, and its Consequences section makes the generated enum a **requirement**: a hand-written one would falsify the zero-engine-edits criterion. *Caveat, stated plainly:* measured on
   a 2-core virtualised Linux sandbox at ~1 GB RAM, not SATURN and not Windows. The ordering and the
   ratios transfer; the absolute nanoseconds do not. Re-run on the stage device before the ADR-005
   addendum is treated as final.
2. **Sub-block processing — FORBIDDEN in v0.** No `internal_rate` field (removed from
   `manifest-schema.md` §8). Grain and event onsets remain sample-accurate because §9 pre-sorts the
   block's events and the module renders into the block buffer; what v0 gives up is parameter
   movement inside a block. The remedy is a smaller **host** block — `BlockContext.frames` is a
   runtime field, and 100 modules at 16 frames costs ~0.52 % of a core against ~0.13 % at 64 frames
   (measured). Roughly 0.4 % CPU buys 4× the time resolution, which is why forbidding it now is safe:
   the fix does not require reopening a frozen document.
3. **Block-rate `cv` — the host interpolates; the module declares `cv_interp`** (`hold` / `linear` /
   `spline`, default `hold`). Symmetrically, an audio-rate source feeding a block-rate input is
   reduced by the receiver's declared **`cv_reduce`** (`last` / `first` / `mean` / `min` / `max` /
   `peak`, default `last`). One rule covers both directions: **the receiving module owns any rate
   change on its own input.** Both policies are manifest data and never code, because `first` and
   `last` render differently and an undeclared choice would break journal replay (ADR-007).
4. **Oversampling — the host provides it; the module declares the need** (`resources.oversampling`),
   as recommended.
5. **`gpu` ports and tiers — DEFERRED to Phase 5**, to be answered when the wasm tier (T2) is
   scoped. Interim rule: no `gpu` module may be T2 or T3, and §14's camera-optical-flow row is
   provisional. The `gpu` **port type is not deferred** — Phase 0 ships `ana/tap` ("gpu/display
   plumbing") and `dsp/scope` (`audio`/`cv` in, `gpu` out), both first-party T1, which every
   candidate answer permits.

Each answer is recorded here and, where architectural, in the ADR-005 addendum. The machine-readable
form of every connection rule — the table host validation and the canvas affordance layer both read
— is `docs/api/compat-matrix.toml`.

---

## 17. Implementation status — contract v1 (WO-008 increment 4, 2026-09-26)

What the engine actually presents today, so this document cannot drift from the code silently (the
drift gate for the connection table is `sparq-module-api/tests/compat_matrix.rs`; the drift gate for
the Rust surface is `tests/api_snapshot.rs`):

* **`AudioCtx` is multi-port** (§2/§3's port model, carried): per-type port views in manifest order
  — `audio_in/audio_out` (≤ 8 per class, `MAX_PORTS_PER_CLASS`), `cv_in/cv_out` (block value or
  audio-rate buffer, always at the RECEIVER's declared rate — the host performs §16 Q3's
  `cv_reduce`/`cv_interp` before the module sees a value), `events_in/events_out` (§9's guarantee
  is mechanical: merged pre-sorted by sample offset, ties stable by host-rank → connection id →
  insertion order; sinks bounded at `EVENTS_PER_BLOCK` = 64 with counted, never-grown overflow).
  Multi-output modules use the `take_*` accessors (documented in the author guide §8.1).
* **`cv` edges execute** with every matrix rule enforced at build: range mismatch refused (G2,
  naming `util/range` and its phase), fan-in refused (naming `util/mixer`), fan-out free,
  `spline` refused until implemented — refusals in words, never silent no-ops.
* **`event` edges execute**; the producer's `event_kinds` must be a subset the consumer accepts
  (the default cell's rule, `E-EVENTKIND-UNACCEPTED`'s sentence). Host-side event injection exists
  as the executor's control-thread door; scheduling BY TICK is WO-009's clock broker and is
  deliberately not pre-empted here.
* **`data`/`gpu`/`atom` payloads are still not carried**; their edges refuse at build with
  type-specific sentences (§4's schema machinery and the ring publication have not shipped;
  `atom` is the control-thread `message()` door by design, never a wire payload).
* **`Resources`** now carries the per-port resolved channel counts §2's cascade rule needs.
* The **rms→filter acceptance** (WO-014) is proven with arithmetic, not adjectives:
  `tests/contract_v1.rs` renders a cv-wired `flt/svf` **bit-identical** to a hand-driven reference
  whose cutoff is set per block to the value the contract declares — and `sparq exec --patch
  mod-demo` is the audible artefact.
* **The musical clock is live (WO-009, 2026-09-26):** `ctx.block.tick`/`ppqn` carry the
  transport's position for the block — the transport computes (the piecewise map of ADR-006),
  the executor carries (`set_musical_position`), and a driver that never calls it renders the
  static `tick = 0` the goldens were built against. Transport triggers reach modules through the
  same `event` ports and the same host-event door as any other producer; a scheduled tick fires
  at its exact sample (±0, measured against an independent implementation of the map).
* Still declared open: audio-only delay edges, single-pass `variable` channel-set resolution,
  unenforced `required`-unconnected inputs (the module sees the explicit unconnected signal and
  answers with its status; host-side enforcement is a declared open item so the determinism
  harness's refusal counters stay comparable), and sub-block transport positions (module-api
  §16 Q2's forbidden territory — loops fold at block granularity for the same reason).
