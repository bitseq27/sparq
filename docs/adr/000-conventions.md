# ADR-000 — Engineering conventions

**Status:** accepted · **Date:** 2026-09-20 · **Work order:** WO-000

## Context

A solo, multi-year project dies from inconsistency and forgotten reasoning long before it dies from hard problems. Conventions must be enforced by the build, not by memory, because the person reviewing the code is the same person who wrote it.

## Decision

**Layout.** Workspace per plan §20: `crates/` (kernel, module-api, audio, music, sample, assets, ui, visual, perform, io, host-wasm, bridge, app), `modules/` (first-party T1, one directory per category), `adapters/`, `design/`, `docs/`, `reference/` (golden renders, LFS), `tools/`, `examples/`, `field-kit/`. Empty crates are debt — create a crate when it has contents.

**Language policy.** Rust, toolchain pinned in `rust-toolchain.toml`. `#![forbid(unsafe_code)]` at the crate root everywhere. Exceptions live in an explicit allowlist file (`docs/unsafe-allowlist.md`): audio HAL, lock-free rings, FFI, SIMD intrinsics. Each `unsafe` block carries a `SAFETY:` comment naming the invariant and who upholds it. `cargo deny` for licences and advisories.

**Lint/format gate.** `rustfmt` (nightly options where useful) + `clippy -D warnings` in CI. Custom lints/assertions for real-time discipline: no `Box`/`Vec`/`String`/`format!` in audio-path modules (enforced by the counting allocator in debug + a `cargo geiger`-style grep gate in CI).

**CI matrix.** Windows (primary, with a runner that has a real audio device), Linux, macOS. Debug + release. Nightly jobs: sanitizers (ASan/TSan), Miri for the allowlisted unsafe, fuzzing of parsers (WAV, OSC, MIDI, manifests, journals). Every DSP change runs the golden-render diff; every UI change runs the screenshot diff and the ≥44 px layout audit.

**Versioning.** Engine: semver. Module API: its own version, frozen per minor, machine-checked by an API-surface snapshot test that fails CI on an unreviewed signature change. Project/journal/manifest formats: explicit version header + migration chain; never write an unversioned container.

**Docs.** Every module's docs are generated from its manifest (no hand-written duplicates). Every significant choice gets an ADR. `LATER.md` is the parking lot; a WO is not done while a tempting idea remains unbudgeted in someone's head.

**Definition of done.** The seven-point rule in `PHASE0-WORKORDERS.md` §6, including "it was used at least once to make sound."

## Consequences

* Adding a module requires zero CI or engine edits (glob discovery) — enforced by a throwaway-module test.
* Slow CI is a real risk (three OSes × debug/release × golden renders). Mitigation: shard jobs, cache aggressively, run golden diffs only for crates under `sparq-audio`/`modules/`.
* The `unsafe` allowlist will be small and auditable; anything new needs a review note even though the reviewer is you.

## Alternatives considered

* **Convention-by-discipline (no gates).** Rejected: fails within a month on a solo project.
* **Monorepo with a single crate.** Rejected: the module boundary must be a compile-time boundary to prove the contract (ADR-002).
* **C++ style guide equivalent.** N/A after ADR-001.

## Review trigger

If CI exceeds ~20 minutes on a trivial change, or if the `unsafe` allowlist grows past ~15 entries.
