# ADR-005 — Closed port type system

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-002, ADR-006, `docs/api/module-api-v1.md`

## Context

Modules must interoperate forever, across tiers (native / wasm / process) and across years of version drift. That is only possible if the set of connection types is **closed**: a fixed vocabulary with precise semantics, so any two modules written a decade apart still connect. An open type system ("just add a port kind") is how modular ecosystems become a thousand incompatible islands.

## Decision

Six port types. Nothing else is ever a port type; new *kinds of content* are expressed as subtypes or `atom` payloads, never as new ports.

| Port | Payload | Clock/rate | Notes |
|---|---|---|---|
| `audio` | float frames | per sample | carries a `ChannelSet` (count + layout tag) |
| `cv` | float, declared range `[-1,1]` or `[0,1]` | per sample **or** per block | the universal modulator; broadcasts |
| `event` | MIDI 2.0 UMP, OSC, trigger/gate, note (+MPE) | timestamped in `t_sample` | sample-accurate inside the audio clock domain |
| `data` | typed record (named channels with dtype/unit) | timestamped in `t_wall`, declared rate class | sensors, network, numeric, files, ML, telemetry |
| `gpu` | texture/buffer handle | per frame | video frames, compute results, visual feedback |
| `atom` | opaque versioned blob | on demand, never on the audio thread | host↔module messages, asset refs, structured config |

`ChannelSet` layout tags: `mono`, `stereo`, `quad`, `5.1`, `7.1.4`, `ambisonics:N` (AmbiX, N = order 1–7), `objects:K`, `raw:M`.

**Connection rules (host-enforced, surfaced as UI affordances):**
* Same type required. Cross-type connections are refused; the UI offers a one-tap **adapter insertion** (`data→cv` via a Mapper, `cv→audio` via an oscillator/offset, `audio→cv` via an analyser, `event→cv` via a gate/trigger converter).
* `audio`: layout must be compatible or explicitly convertible. `mono→multi` fans out; `multi→mono` sums **and draws a warning hairline**; `ambisonics:*` refuses to connect to non-spatial ports unless an encoder/decoder is inserted. `raw:M` connects only to `raw:M` with equal M.
* `cv`: one output → many inputs (broadcast); many outputs → one input requires an explicit merge/mix module (no implicit summing).
* `event`: sample-accurate within the audio clock domain; events from the wall clock (OSC, sensors) are quantised per the stream's declared latency budget and interpolation policy (ADR-006).
* `data`: consumers declare a read policy — `latest`, `interpolated-at(t)`, `window(n)`, `accumulate`, `on-change` — and a staleness policy — `hold`, `decay(ms)`, `zero`, `error`.
* `gpu`: display modules and GPU-processing modules only; **never** touched from the audio thread.
* `atom`: never on the audio thread; used for configuration, asset references and structured messages.

**Evolution rule:** the set is closed. Adding a type requires a new ADR *and* a host major version. Removing or renaming a type is forbidden; deprecate with an alias table and keep loading old patches forever.

## Consequences

* Adapter modules become first-class, small, and numerous — this is a feature (explicit conversion is visible in the patch).
* Everything that must cross a process or wasm boundary is serialisable by construction (`data` records, `atom` blobs, `event` packets); `audio`/`cv`/`gpu` use shared buffers or handles.
* The UI's "compatible ports glow / incompatible dim / adapter offered" behaviour is a direct rendering of this table, which is what makes the canvas feel coherent.

## Paper test (run before implementing — see `docs/api/module-api-v1.md` §14)

Every one of these must be expressible without a new port type: granular cloud (many voices, per-grain `cv`), partitioned convolution reverb (latency + tail), HOA encoder/decoder (`ambisonics:N`), MIDI 2.0 MPE input (per-note controllers), camera optical flow (`gpu` in → `data` out), CSV playback (`data` out), DNA tree mutation (`event`/`atom`), scope display (`audio`/`cv` in, `gpu` out).

## Alternatives considered

* **Everything-is-audio (Max/PD style).** Elegant and permissive, but loses type safety, makes spatial layouts and data streams second-class, and pushes all validation to runtime. Rejected.
* **Open/extensible port types.** Rejected for the island problem above.
* **Separate `midi` and `osc` port types.** Rejected: both are `event`; the payload carries the dialect. One type keeps routing uniform.

## Review trigger

When the first module cannot be expressed (that is a bug in this ADR, and it gets superseded — but only with a paper test showing why `atom`/`data` subtyping genuinely fails).

---

## Addendum — WO-007 task 1 (2026-09-22)

The connection rules above are now also **data**: `docs/api/compat-matrix.toml`. That file is the
single source both consumers read — host validation, and the canvas affordance layer whose behaviour
the Consequences section above specifies. The prose here explains; the table is what gets checked.
Where this addendum and the table disagree, the table wins and this ADR is superseded.

**Dispatch — measured, because it decides how every module is called.** 100 modules × 64-sample
block, sparq's release profile, four distinct implementors, three runs. Per block: `Box<dyn Module>`
1.65–1.95 µs, `enum + match` 1.26–1.30 µs, monomorphised 1.23–1.31 µs. Per sample (6400 calls per
block): `Box<dyn Module>` **27.1–35.6 µs**, enum 2.08–2.22 µs, monomorphised 2.05–2.17 µs. Enum
dispatch is within noise of monomorphised; per-sample trait objects exceed the whole < 20 µs budget
alone. **Decision: `process(block)` may be a trait object; nothing called per sample may be.**
Sandbox measurement — ratios transfer, absolute nanoseconds do not; re-run on SATURN.

**`cv` range mismatch (bipolar ↔ unipolar): REFUSED, converter offered** (`util/range`). A silent
rescale is precisely the invisible transformation this ADR exists to prevent.

**Cells this addendum closes**, all of them unspecified above:

| Edge | Rule |
|---|---|
| `objects:K → objects:J` | compatible iff K == J; otherwise convert via `spa/objects`. Never silently renumbered — the object count *is* the spatial meaning |
| audio-rate `cv` → block-rate input | reduced by the receiver's declared `cv_reduce` (default `last`). Declared in the manifest, never in code: `first` and `last` render differently (ADR-007) |
| block-rate `cv` → audio-rate input | the host interpolates per the receiver's declared `cv_interp` (default `hold`) |
| `event` fan-in | free, like `cv` fan-out — merged into one list pre-sorted by sample offset, ties broken stably by connection id then insertion order, so a replay is identical to the original |
| `data` schema version mismatch | loads, but every record is flagged `stale` and the consumer's own `stale_policy` decides what that means. A canvas badge **and a journal entry** are mandatory, so a replay records that the project opened mismatched. Residual risk: this is the one rule that can produce wrong sound with no error — if a bug ever traces here, mismatched schemas default to `error` instead |

**A promised adapter is only a promise if the module exists.** Three of the four converters named in
the Decision section were not scheduled anywhere that could honour them: `data→cv` "a Mapper" is
`dat/mapper`, Phase 5–7; "an oscillator/offset" has no `util/offset` module at all; "a gate/trigger
converter" has no module and no phase. Decision: **`util/range`, `util/offset` and `util/gate-to-cv`
join Phase 1**; `dat/mapper` stays Phase 5–7, and until it ships `data→cv` degrades from
*adapter offered* to *refused*. The rule this establishes for the canvas: **never offer a converter
that does not exist** — an offer that does nothing is exactly the failure `sparq-ui`'s suppression
log was built to make impossible.

**Still open, with a trigger:** whether a T2/T3 module may hold a `gpu` port. Deferred to Phase 5,
when the wasm tier is scoped. Interim: no `gpu` module may be T2 or T3. The port type itself is not
deferred — Phase 0's `ana/tap` and `dsp/scope` are first-party T1 and use it.

## Addendum — contract v1 (WO-008 increment 4, 2026-09-26)

The table now has **two consumers in code and a gate between them**. The canvas affordance layer
already delegated to `sparq-module-api::port`'s `connect_*` (WO-013); the executor's new `cv`/`event`
build rules delegate to the same single compiled copy — range mismatches, fan-in, and the four
cross-type adapters all resolve through it, at `Phase::Zero`, so an offer that does not exist in this
build degrades to a refusal that names the remedy and its phase. `tests/compat_matrix.rs` parses the
TOML with the crate's own parser and pins vocabularies, verdicts, adapter ids, case counts and
representative outcomes against the code: the mirror carried since WO-007 can no longer drift
silently. Its first run found two real drifts — the compiled HOA offer named `spa/objects` instead of
the table's `spa/hoa-encode`/`spa/hoa-decode` (defect #80, fixed: the offer must name the module it
would insert), and this table's `cv→audio` adapter still said "syn/sine-or-offset … naming to be
settled" after the 2026-09-22 addendum had settled it as `util/offset` (defect #81, fixed here).
One ordering question is pinned rather than decided: the mono fan-out case precedes the spatial
cases in both copies, so `mono → ambisonics:N` is "compatible fan-out" rather than an encoder
insertion — flagged for the matrix review the pending `reviewed = false` owes (defect #82). Full
deletion of the code mirror stays open and now has a named reason: several `when` cells are prose
("fan-out: one cv output -> many cv inputs"), and no interpreter can evaluate a shape rule against a
port pair — the table must be restructured into machine predicates (a review-packet change) before
"read at discovery" can mean more than "pinned at test time".
