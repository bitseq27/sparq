# ADR-008 — Disposable bootstrap audio path

**Status:** accepted · **Date:** 2026-09-20 · **Kill date:** end of WO-006 · **Related:** ADR-001, ADR-009

## Context

Hearing sound within days matters more than hearing it through the right abstraction. But a bootstrap crate's capability model (device enumeration, format negotiation, latency reporting, channel counts) tends to colonise the design of whatever comes after it.

## Decision

Use a `cpal`-class crate for **WO-005 only**, behind a Cargo feature flag (`bootstrap-audio`) that is off by default and removed at the end of WO-006.

Constraints:
* No sparq type may depend on a bootstrap type. The callback converts at the boundary.
* The real HAL trait is written and reviewed **first** (WO-006 task 1), against Phase 7 requirements (multichannel, per-channel latency, device aggregation), before any backend is implemented.
* The bootstrap path may not be used to justify a design decision. If a capability question arises, it is answered against the HAL trait, not the crate.

## Consequences

* First sound in about a day; motivation and momentum preserved.
* A small amount of throwaway code, explicitly labelled and scheduled for deletion.
* The underrun counter and callback-shape lessons carry over; the device model does not.

## Alternatives considered

* **Write the WASAPI/ASIO HAL first.** Correct but slow: days of COM/FFI before a sound. Rejected on sequencing only.
* **Keep the bootstrap crate as the permanent shared-mode backend.** Rejected: it does not expose exclusive mode, ASIO, per-channel latency or aggregation, which sparq needs (ADR-004).

## Review trigger

At the end of WO-006: confirm the flag is gone and no dependency remains.

**Status note (WO-006 increment 1, 2026-09-21).** The HAL is built (null + WASAPI
exclusive/shared), but "the end of WO-006" means its **hardware acceptance**, not its
compilation: the flag is removed when the 2-hour zero-xrun soak has passed on the stage device
and `sparq play --backend wasapi-exclusive` has made audible sound there. Until then both paths
ship side by side and `--backend wasapi` (bare) keeps routing here — a proven path is never
rerouted by an unproven one. The `Mutex<Wo005Graph>` violation documented in `bootstrap.rs`
dies with this flag; the HAL path never had it.
