# ADR-009 — Graph executor and audio HAL shape

**Status:** **draft** — ratify at the end of WO-006 (HAL) and WO-008 (executor) · **Date:** 2026-09-20 · **Related:** ADR-002, ADR-005, ADR-006, ADR-007

## Context

Two structures decide whether sparq is trustworthy on stage: how the graph executes, and how samples reach the device. Both are written early, both are hard to change later, and both must be shape-checked against modules that don't exist yet (granular, convolution, HOA, data sources).

## Decision — executor

1. **Nodes and typed edges** (ADR-005); a graph has a **version** number. Execution order is a cached topological sort, recomputed only when the version changes.
2. **Cycles are legal only through `unit_delay` / `block_delay` edges.** Any other cycle is rejected at connect time with an actionable error. This keeps the executor a simple sort and makes feedback audible-but-safe.
3. **Atomic mutation, RCU-style.** The control thread builds a complete new graph, then swaps a pointer at a block boundary; the audio thread retires the old graph after a grace period. The audio thread never observes a half-built graph. Verified by a 10 000-iteration random-mutation-while-playing stress test.
4. **Buffers are pooled** per block size and channel set, allocated at `prepare`, never on the audio path. Arenas are pre-reserved and pre-touched; a memory budget is printed at patch load.
5. **Latency accounting** per path with opt-in compensation (ADR-006.6).
6. **Per-module watchdog**: block processing time measured; N consecutive overruns ⇒ auto-bypass + red hairline in the UI + journal entry. Above that, the **degradation ladder** (plan §5.2): analysis FFT size → display refresh → voice count → oversampling → block size. Each rung logged and reversible.
7. **Dispatch strategy — DECIDED BY MEASUREMENT (WO-007 task 1, 2026-09-22).** Measured with the 100-null-module graph this item asked for (`tools/dispatch-bench`, sparq's exact release profile, four distinct implementors so the vtable pointer genuinely varies, three runs, module body identical in every variant so the difference is dispatch and nothing else). **Per block** (one dispatch per module): `Box<dyn Module>` 1.65–1.95 µs · `enum + match` 1.26–1.30 µs · monomorphised 1.23–1.31 µs — all three inside the < 20 µs target, at 10–16× under it. **Per sample** (6400 dispatches per block): `Box<dyn Module>` **27.1–35.6 µs** · enum 2.08–2.22 µs · monomorphised 2.05–2.17 µs. **Choice: the hybrid.** `process(block)` is a trait object; nothing called per sample is — per-sample paths are enum-dispatched or generic. Enum dispatch measured *within noise of fully monomorphised code*, so the generated enum costs nothing and there is no argument against it. *Caveat:* measured on a 2-core virtualised Linux sandbox at ~1 GB RAM, not SATURN and not Windows — the ordering and ratios transfer, the absolute nanoseconds do not. Re-measure on the stage device before this is treated as final.
8. **Analysis taps and meters are published, not polled**: the executor writes into lock-free rings that the UI/visuals consume; visual work never touches the audio thread.

## Decision — HAL

One trait, five backends:

```
enumerate() -> [DeviceInfo]
capabilities(DeviceId) -> {rates, channel_counts_in/out, exclusive, event_driven, hw_latency, asio?}
open(DeviceId, Config{rate, block, in_ch, out_ch, exclusive|shared|asio}) -> Stream
Stream::{start, stop, process(&mut FrameCtx), latency_report, error_report}
```

* **Backend priority:** WASAPI exclusive (event-driven) → ASIO → WASAPI shared → **null/virtual** (CI + offline) → PipeWire/JACK and CoreAudio from Phase 1.
* **Real-time discipline:** MMCSS `Pro Audio` critical + ideal-processor assignment; working-set lock and pre-touch; counting allocator in debug/CI that fails on any allocation, lock or syscall inside `process`.
* **Diagnostics:** xrun count, block-time histogram (p50/p99/max), callback jitter, device clock drift vs `t_wall`, reported hardware latency, and a round-trip latency measurement utility (loopback impulse).
* **Aggregator-ready:** the trait must be able to express a composite device (clock master, per-device latency alignment, drift-compensating resampling PLL) even though the aggregator is implemented in Phase 7.
* **Failure semantics:** device removal, format hijack by another app, and driver reset all produce a clean error state and a recoverable stop — never a crash, never a hang.

## Consequences

* The mutation stress test and the allocator gate become two of the most valuable tests in the repo; they are deliverables, not afterthoughts.
* **Enum dispatch was chosen for the per-sample path** (Decision item 7), so the mitigation below is a requirement rather than a hedge: the enum is **generated from module registration, never hand-written**. A hand-written enum would make WO-007's first acceptance criterion — "a new module can be added with **zero** edits to engine code" — false by construction, which is the whole point of measuring before choosing. Per ADR-002 that generation is bounded: **T1** modules are statically linked and appear as generated variants; **every T3** module appears as a single *bridged* variant, because the executor reaches it through shared-memory rings rather than a Rust call; **T2** does not exist until Phase 5. Keeping T1 categories few still applies.
* The null device is load-bearing: it keeps every layer above the HAL testable in CI on machines with no audio hardware.

## Alternatives considered

* **Dataflow/actor model with per-module threads.** Rejected: unpredictable scheduling and cache behaviour; impossible to bound block time.
* **Compile-time graph (const generics / monomorphised chains).** Fastest, but incompatible with runtime patching, hot reload and touch editing. Rejected; may be revisited for fixed sub-chains inside a module.
* **JUCE AudioProcessorGraph.** Rejected with ADR-001; also assumes plugin-shaped nodes.
* **Lock-free graph mutation without a grace period.** Rejected: the audio thread can still hold a reference mid-block.

## Ratification notes — WO-006 increment 1 (still draft: hardware numbers outstanding)

The HAL half of this ADR shipped as sketched, with four recorded deviations/extensions:

1. **`actual_config()`** joined the stream surface: negotiated format ≠ requested format must be
   readable, not inferred (Phase A telemetry lesson). `latency_report` and diagnostics ride on it.
2. **`pump()` / `inject_fault()`** were added for the null backend so determinism and the failure
   acceptance criteria are testable in CI; device backends refuse both honestly.
3. **`windows-sys` + hand-written vtables** instead of the high-level `windows` crate: the crate
   cannot be compiled under the project's 1 GB CI/sandbox memory budget (defect #30). The vtable
   slot order is the SDK header order, ABI-frozen since Vista, each slot commented with its method.
4. **No `AvSetMmMaxThreadCharacteristicsW`** (defect #31: wrong metadata binding in windows-sys
   0.61); the critical boost is MMCSS + `TIME_CRITICAL`, each step separately reported via
   `RtReport` — "MMCSS not being honoured" stays observable, which was the risk-register demand.

Diagnostics split **late wakes / budget overruns / device errors** into separate counters: the
sketch's single "xrun count" could not distinguish our DSP overrunning from the system missing an
interrupt, and the first paced-null run proved the distinction is real (defect #34: the simulation
itself was the noisy party). Ratification waits for the stage-machine numbers (2 h soak, latency
table) per the review trigger below.

## Addendum — WO-008 increment 5 (2026-09-27): decisions 3 and 8 ship as sandbox mechanics

**Decision 3's swap is now a kernel primitive** (`sparq-kernel::sync::hotswap`, allowlist
entry 6): the control thread stages a *complete* boxed patch into one atomic slot; the audio
thread's once-per-block `boundary` takes it with a single pointer `swap` — the literal swap
this decision named — runs the caller's inherit hook at the swap point (the transport clock
crosses here, not at stage time), and publishes the outgoing patch into a fixed array of
**epoch-tagged retirement slots**. The grace period is mechanical, not argued: the boundary
stores `epoch + 1` with Release *after* publishing, and the controller may only convert a
retirement back into a `Box` once its Acquire-loaded epoch exceeds the slot's tag — a retired
patch becomes droppable only after the audio thread has demonstrably passed the boundary that
let it go, and the drop happens control-side, so module deactivation never runs inside a
device callback. When every retirement slot is occupied the swap is **deferred and counted**,
never forced: the audio thread's fallback is always "keep rendering the live patch". No locks,
no allocation after construction, no blocking, single control + single audio thread.

Decision 3's verification sentence is scored honestly against the sandbox: the
**10 000-iteration random-mutation-while-playing stress** ran with two real threads
(`sparq-audio/tests/cross_thread.rs`: 10 000 seeded mutations staged from a control thread
while the audio thread rendered continuously — 7 400 consumed swaps, 2 600 refusals, ~43 000
rendered blocks, zero failed blocks, zero audio-thread allocations under the counting
allocator, every retirement reclaimed and dropped exactly once, clock never regressing) plus
the kernel-level suite (handoff order, the epoch gate white-boxed, deferral under slot
exhaustion, teardown drop hygiene, a paced two-thread exchange stress) with Miri over the
`sync::` modules in CI. The **zero-xrun** half of the sentence is a real-time claim and stays
device-track: the loaded soak (200 modules, 30 min, SATURN) is where a clock exists to miss.

**Decision 8's first consumer ships with it:** the cross-thread engine publishes per-node
meter snapshots (`peak`/`rms`/`status` + the stream's block count) into a lock-free
`SpscRing` at the end of every block, and a second ring carries `Copy` commands the other way
(param snapshots — `ParamSet` was made `Copy` for exactly this trip — musical position,
bypass clears), applied to the patch that renders the block they precede. Both rings
refuse-and-count when full; the audio thread never waits for a reader. This is the
publication `ana/tap`, `dsp/scope` and WO-013's live wire levels wait on; their own increments
consume it.

**Scope declared, not hidden:** one control thread + one audio thread (the slot's invariants
are written for that pair; multi-reader epochs are a later shape); per-node meters fold a
node's audio outputs (per-port meters ride the same rings later); a patch swap still does not
carry module state (that is WO-011's state protocol — unchanged); and the primitive ships at
`sparq-kernel/src/sync/hotswap.rs` rather than this ADR's `graph/hotswap.rs` sketch, because
the swap is payload-generic — the kernel cannot name the executor type that lives above it.
The allowlist and its auditor carry the alias, as they do for the arena allocator.

## Review trigger

End of WO-006 and WO-008 (ratify with measured numbers), then at Phase 1 hot reload (which stresses the RCU swap), and at Phase 7 (aggregator).
