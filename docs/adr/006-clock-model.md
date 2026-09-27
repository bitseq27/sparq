# ADR-006 — Three-clock model and latency policy

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-005, ADR-007, ADR-009

## Context

sparq must reconcile three incompatible notions of time: the audio device's sample count (which must never jump), musical position (which must respond to tempo changes and external sync), and the wall clock (which sensors, network feeds, video and journals live on). Getting this wrong produces glitchy tempo automation, misaligned sensor data and unreproducible performances.

## Decision

**Three clocks, one master, one broker.**

| Clock | Unit | Master | Used by |
|---|---|---|---|
| `t_sample` | integer sample index at device rate | audio device (or virtual device offline) | all DSP, all event dispatch |
| `t_musical` | ticks (960 PPQN default), with bar.beat.tick view | transport | sequencers, harmony, form, tempo-synced FX |
| `t_wall` | monotonic microseconds since epoch | OS | streams, journal, external sync, diagnostics |

Rules:

1. **`t_sample` is the master.** Nothing else can move it. Offline render uses a virtual device that advances it deterministically.
2. A single **`ClockBroker`** owns the mapping `t_sample ↔ t_musical ↔ t_wall` as a *piecewise-linear function with a smoothed derivative*. Tempo change therefore never produces a discontinuity in either direction; a continuity test is part of the acceptance criteria (WO-009).
3. **External sync is a slave PLL**, never a hard jump. MIDI clock / Link / OSC transport / (later) MTC-LTC feed a rate+phase estimator with configurable stiffness; manual override wins within one block. sparq can also be master and transmit.
4. **Streams carry `t_wall`** (and `t_sample` when they originate inside sparq). The stream bus converts onto the audio timeline with a per-source **jitter buffer** (default 4 ms, range 0–50 ms) and a declared interpolation policy (`none`/`step`/`linear`/`spline`). A stream's latency budget is explicit and visible in the UI.
5. **Staleness is a first-class state.** A source that stops publishing marks its stream `stale`; downstream modules apply their declared policy (`hold`, `decay`, `zero`, `error`). No consumer ever silently reads garbage — this is what makes live data performance survivable.
6. **Module latency is declared, not inferred.** Every module reports latency (samples) and tail (samples). The executor computes per-path latency and compensates **only** where the path is marked `compensate: true`, plus a global raw/compensated switch and a per-path readout. Latency is a timbral resource in this genre; silently "fixing" it is a bug.
7. **Sample accuracy.** Events scheduled at a tick fire at the exactly corresponding sample index (verified for 1 000 random ticks, ±0 samples).

## Consequences

* Tempo automation, metric modulation and probabilistic arrangement all become safe operations rather than glitch sources.
* The stream bus needs a resampler/interpolator per source — a small, well-tested component rather than per-module ad-hoc handling.
* Determinism (ADR-007) depends on this: with `t_sample` master and the broker pure-functional, replay is reproducible.
* PLL stiffness is a musical parameter as much as an engineering one; it gets a UI control and a documented default per sync source.

## Alternatives considered

* **Single global clock (samples only), with musical time derived ad hoc.** Rejected: tempo automation becomes discontinuous and sequencing code scatters clock math everywhere.
* **Wall clock as master.** Rejected: OS clock jitter is audible, and offline render/determinism become impossible.
* **Silent global latency compensation (DAW-style).** Rejected as a default: it destroys deliberate delay/phase relationships. Offered as an explicit per-path switch instead.

## Review trigger

When external sync lands (Phase 4) — the PLL parameters will need real-world tuning; and when video/NDI sync requirements appear (Phase 5).

---

## Addendum — WO-009 increment 1 (2026-09-26): what is built, what is measured, what stays open

**The map (rule 2) is implemented, in the kernel.** `sparq_kernel::clock::Clock` is now the
piecewise tempo map its own Phase-0 header promised: segments anchored at exact `(sample, tick)`
pairs, each with a **linear bpm ramp** — position continuous by anchoring, derivative continuous
by inheritance (a mid-ramp change starts from the rate the previous ramp had *reached*). Inside a
ramp the tick integral is quadratic in closed form; `sample_at_tick` inverts by **exact integer
bisection** with nearest-sample rounding consistent with the forward direction's `round()`. The
public surface was kept as promised, with one declared change: `Copy` → `Clone` (a segment list
is not `Copy`; every consumer in the tree held the clock by value-once-and-reference-thereafter).
A single constant segment still takes the v0 closed-form path **bit-for-bit**, which is why the
phase-b goldens did not move.

**The broker and the transport are `sparq-music`** (the crate's first contents): `ClockBroker`
owns position + the wall side; `Transport` is play/stop (stop FREEZES the timeline — offline tape
semantics, which is what makes criterion 4 exact), tempo now and as a stream by construction
(every change is a segment), loop region, tap tempo, the tick-scheduled queue, and bar/beat
trigger emission. `advance_block` fills a bounded 64-slot collector with **zero allocations
measured** — it is pump-side code. The executor gained `set_musical_position((tick, ppqn))`: the
transport computes, the executor carries, and a driver that never calls it renders the static
`tick = 0` every golden was built against (the stress hash `b42068ec7b206789` did not move).

**The acceptance criteria, scored honestly:**

| Criterion | Status |
|---|---|
| Trigger at tick T fires at the exact sample, 1 000 random ticks, ±0 | **MET, sandbox** — twice: constant tempo, and across a 10 ms glide + a step, against an expectation computed from an INDEPENDENT implementation of the segment integral inside the test (`sparq-music/tests/wo009.rs`) |
| Tempo change: no discontinuity in either direction; sweep test + listening | **Sweep MET, sandbox** — 6 000 blocks of 60→200→70 bpm, one target per block: playhead strictly monotone, deltas inside the fastest tempo's envelope, effective tempo inside the sweep's hull; kernel tests pin the derivative continuity (half-increments at ramp edges) and a worst-case 64-sample-anchor sweep. **Listening is device track** — `sparq exec --patch drum-demo` with a tempo edit mid-render is the artefact when a transport UI exists to make one |
| t_wall drift estimated and reported; 30 min matches independent measurement | **Mechanism MET, sandbox** — windowed-ratio estimator, one-pole smoothed, re-anchored every 1024 observations and on locate; synthetic +1000 ppm converges to ±2 ppm; backwards wall readings are refused and counted, never applied. **The 30-minute-on-hardware box stays OPEN (device track)** — the HAL pump feeding `observe_wall` between blocks is the wiring the stage run must prove |
| Stop/start sample-accurate and repeatable | **MET, sandbox** — the same play/stop/play script twice renders bit-identical through the real drum-demo graph, with the beats landing at the arithmetic positions the freeze implies (`sparq-app/tests/transport_drum.rs`) |

**The cross-check this increment is proudest of:** the transport-driven drum demo hashes to
`f2303f13aa0cf299` — **the identical golden** the block-counter schedule produced in WO-014 inc 3.
Two independent trigger mechanisms, one bit pattern: the clocks, the event door and the executor
agree about where a beat lives, to the sample.

**Declared limits (v0, in the crate headers and LATER.md):** loops fold at BLOCK granularity
(sub-block transport is module-api §16 Q2's forbidden territory — the remedy is a smaller host
block); beats/bars ride the ABSOLUTE tick grid (for beat-aligned loops the grids coincide; a
loop-relative beat phase for unaligned regions is a later refinement — the first draft's
proportional placement put boundary beats one sample early, and the measurement is recorded in
the transport's header); the schedule speaks absolute ticks; external sync (rule 3's slave PLL)
is Phase 4 — `observe_wall` is the door it will knock on; rules 4/5 (stream jitter buffers,
staleness) are `sparq-stream`'s, not this increment's.
