# ADR-002 — Module execution tiers (decision D-2)

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-001, ADR-005, `docs/api/module-api-v1.md`

## Context

The project's core promise is *"add modules forever without breaking anything else."* That requires an execution model with a real boundary, and a decision about when to pay for sandboxing. Building a WebAssembly ABI before the port/param model has been proven on real modules is the classic way to spend six months and still have no instrument.

## Decision

Three tiers, adopted in this order:

| Tier | Runtime | When | Used for |
|---|---|---|---|
| **T1 — Core** | Native, statically linked, first-party | **Phase 0** | All DSP on the critical path: oscillators, filters, granular, spatial, analysis, displays |
| **T3 — Bridged** | Separate OS process, shared-memory rings + typed IPC | **Phase 0 design / Phase 1–2 implementation** | ML inference, video/camera analysis, network scrapers, hardware daemons, rapid prototyping in Python/Rust |
| **T2 — Sandboxed** | WebAssembly (WASI 0.2 + Component Model), `wasmtime`, capability-scoped, fuel-metered | **Phase 5** | Third-party and experimental modules; anything untrusted; anything authored in another language; registry/marketplace path |

Supporting rules:
* **The manifest is tier-independent.** One declarative schema (plan §6.2) describes identity, ports, params, state, resources, UI, lifecycle and capabilities. The host validates it identically for all tiers, so promoting a module between tiers is a packaging change, not a redesign.
* **Port payloads must be serialisable** (ADR-005) precisely so T2/T3 can carry them across a boundary without special cases.
* **T1 admission rule:** a module is T1 only if it is on the critical DSP path, first-party, and covered by golden tests. Everything else starts in T3 and graduates if it needs to.
* **T3 is non-real-time by contract.** It may block, allocate and crash. The host treats a dead T3 module as a *stale stream* with an explicit hold/decay policy (plan §7.2) — never as an audio failure.
* **Resource isolation everywhere:** per-module CPU budget, memory cap, audio-time watchdog with auto-bypass (ADR-009).

## Consequences

* Phase 0–2 stay simple and fast: no wasm runtime, no ABI negotiation, no fuel accounting.
* T3 gives immediate access to Python/OpenCV/ONNX ecosystems for the data plane without endangering the set.
* The registry/sharing story (plan §13.4) is deferred to Phase 5+, which is exactly when it becomes useful.
* **Risk accepted:** the wasm tier may later want a contract change. Mitigation: keep the manifest declarative, the port set closed and serialisable, and no raw pointers in the public contract. A `sparq-module-api` v1 that is wasm-expressible is a design constraint from day 1, verified by paper-reviewing a granular synth, a convolution reverb, an HOA encoder and a data module against it (see `docs/api/module-api-v1.md` §14).
* T3 IPC needs a versioned wire format and a shm ring implementation in the `unsafe` allowlist.

## Alternatives considered

* **T2 first (wasm from Phase 0).** Rejected: pays the ABI cost before knowing the model; slows every DSP experiment.
* **T1 only, forever.** Rejected: no third-party story, no safe home for ML/video, and every experiment risks the whole process.
* **VST3/CLAP as the module format.** Rejected as the *native* module system (wrong abstraction: fixed in/out, parameter model too narrow for streams, geometry and DNA; no `data`/`gpu` ports). Retained as an **optional T3 wrapper module** so other people's plugins can be used inside sparq.
* **Lua/JS scripting inside the audio thread.** Rejected: GC pauses and unbounded execution are incompatible with pillar 1.

## Review trigger

At the start of Phase 5, or earlier if a collaborator needs to ship a module before then.
