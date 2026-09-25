# ADR-001 — Implementation language: Rust (decision D-3)

**Status:** accepted · **Date:** 2026-09-20 · **Supersedes:** — · **Related:** ADR-000, ADR-002, ADR-009

## Context

sparq is a long-lived, single-maintainer, real-time system that must: (a) never crash or xrun on stage, (b) render a bespoke GPU UI at 4K, (c) host third-party/user modules safely, (d) target Windows first with Linux and macOS supported, and (e) still be maintainable by one person in year five. Language choice is the least reversible decision in the project, so it is decided first and in writing.

## Decision

**Rust for the engine, the first-party modules, the UI and the tooling.** Stable channel, pinned toolchain, `forbid(unsafe_code)` with a small audited allowlist (ADR-000).

Supporting choices that follow:
* **Audio HAL written by us** over WASAPI exclusive / ASIO / shared / null, with PipeWire-JACK and CoreAudio later (ADR-009). Bootstrap via a `cpal`-class crate for one work order only (ADR-008).
* **DSP** in Rust with portable SIMD; `rustfft`-class transforms plus GPU FFT in compute shaders for analysis/visuals; `rubato`-class resampling; `symphonia`-class decoders; FFmpeg wrapped only for exotic/video paths.
* **Faust as an optional authoring path** (Faust → wasm/native → wrapped in a sparq manifest) evaluated in Phase 2, not adopted now.
* **ML and video always out-of-process** (Tier 3) via `ort`/`candle` in a bridge process — never in the audio binary.

## Consequences

* Memory safety removes an entire class of stage-failure modes (use-after-free in a voice pool, data race in graph mutation). This is the single biggest argument: the system's job is to not die in front of an audience.
* Ownership makes the real-time discipline *expressible*: `process()` can take types that cannot allocate or lock, so the rule is enforced by the compiler rather than by review.
* One language across engine, modules, UI and CLI; wasm target gives Tier-2 sandboxing and a browser thin client for free later.
* **Costs accepted:** smaller audio ecosystem than C++; more plumbing (HAL, MIDI 2.0 UMP, ASIO FFI) written by hand; borrow-checker friction when writing arena/pool code; fewer existing DSP references to port directly.
* ASIO SDK is C++ → an FFI shim is required and lands in the `unsafe` allowlist. Budget for it in WO-006.

## Alternatives considered

* **C++ / JUCE.** Proven in pro audio, huge DSP/plugin ecosystem, excellent multichannel support. Rejected for a solo multi-year project: memory-safety burden, slower iteration, and the UI work (which is half this product) is JUCE's weakest area for a bespoke touch-first scientific look.
* **C++ / Qt or custom + miniaudio.** Same safety argument; less audio-specific infrastructure than JUCE. Rejected.
* **Zig.** Real-time-friendly, C interop, no hidden control flow. Rejected: smaller ecosystem, weaker GPU/UI story, and less mature tooling for a 5-year horizon.
* **Extend SuperCollider / Csound / Faust + custom UI.** Fastest to sound, real DSP depth. Rejected as the *product* (it doesn't meet "my own software," and the UI/streams/touch layer would still be built from scratch) — but retained as the **sketch environment** for developing musical ideas and generating golden reference renders (plan §16.3).
* **Max/MSP + TouchDesigner.** Rejected for the same reason; excellent for prototyping ideas this week, wrong for the instrument.

## Review trigger

Only if a hard blocker appears that Rust cannot solve: e.g. WASAPI/ASIO multichannel proves impossible without an existing C++ framework, or WebGPU tooling stalls. Any such finding gets its own ADR superseding this one — not a quiet language switch.
