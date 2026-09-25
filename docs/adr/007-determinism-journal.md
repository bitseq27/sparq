# ADR-007 — Determinism, seed tree and journaling

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-006, ADR-009, `docs/formats/project.md`

## Context

The artistic thesis of this project is that **the patch is the score** — the Autechre lesson. That is only true if a performance can be reproduced exactly: same patch, same seeds, same inputs, same time ⇒ same samples. It is also the foundation of debugging (a crash you can replay is a crash you can fix) and of re-rendering a live set offline at higher quality.

## Decision

1. **Seed tree.** Every random consumer registers with `(session_seed, module_id, consumer_id)` and derives a deterministic sub-seed. Re-seeding one module never perturbs another; re-seeding the session reproduces a different but equally deterministic performance. Seeds are recorded in the journal and displayed in the UI (a seed is part of the instrument's voice).
2. **Append-only journal**, binary, chunk-checksummed, flushed continuously. Records: graph mutations, parameter automation, scene changes, transport events, seed allocations, module versions, asset hashes, device config, xruns, degradation events, and (optionally, per stream) full stream recordings.
3. **Deterministic offline render.** The identical executor driven by a virtual device, single-threaded by default; bit-exact across runs and across machines (asserted in CI by hash). Multi-threaded segmented render is allowed only where a graph is provably stateless per segment, and is separately tested for equality.
4. **Golden-reference harness.** Per-module reference renders + hashes, compared with a documented relative tolerance; an intentional one-sample change must fail CI with a readable diff (max abs error, first differing sample). This is the permanent quality floor.
5. **Replay.** Re-executing a journal reproduces the session bit-exactly. Recovery after an arbitrary process kill restores the last good state in under 10 s.
6. **Numerics policy.** Internal DSP `f32` with `f64` accumulation at sums, filters, meters and analysis; optional global `f64` mode for offline/scientific renders; denormals-are-zero enabled on the audio path; no fast-math flags that break reproducibility; FFT sizes and window functions are part of the module's declared identity (changing them changes the golden render, deliberately).
7. **Provenance.** A project records every module version and asset hash it needs. Missing or incompatible modules produce a **repair dialog** (stub / substitute / cancel) — never a crash, never a silent wrong sound.

## Consequences

* Live sets become re-renderable masters, and generative work becomes archivable rather than ephemeral.
* Every nondeterministic influence must be *declared* (wall-clock reads, thread scheduling, hash-map iteration order, unseeded RNG, `f32` reduction order). CI includes a determinism canary that fails on unexplained divergence — this is a real cost and is paid deliberately.
* Journal size must be bounded by design: no stream recording by default; per-stream opt-in with a size estimate shown before enabling.
* Golden files need a regeneration command that is explicit, reviewed and logged (never automatic).

## Alternatives considered

* **Best-effort reproducibility** (save the patch, hope for the best). Rejected: loses the artistic thesis and makes live debugging impossible.
* **Full event-sourcing (journal is the only truth, no project snapshots).** Elegant but slow to load and hostile to text diffing; rejected in favour of journal + snapshot container (`.sparq`) with a text-diffable graph representation.
* **`f64` everywhere by default.** Rejected for real-time cost; retained as an offline mode.

## Review trigger

When multi-threaded render is implemented (Phase 1), and when stream recording lands (Phase 5) — both threaten determinism and need their own test evidence.
