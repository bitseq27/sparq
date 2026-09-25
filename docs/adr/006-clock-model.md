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
