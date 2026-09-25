# `unsafe` allowlist

**Policy (ADR-000):** `#![forbid(unsafe_code)]` at the crate root everywhere. `unsafe` exists only in the
modules listed below, each of which must be named here **before** the code is written, with the invariant
it upholds and the test that proves it. CI fails on any `unsafe` block outside an allowlisted path, and
fails on any allowlisted path that has no corresponding `SAFETY:` comment and test.

Every `unsafe` block carries:

```
// SAFETY: <the invariant> · upheld by <who/what> · tested by <test name>
```

Miri runs nightly over every allowlisted module.

---

## Allowlisted modules

**Phase 0 shipping status:** entries **3**, **4** and **5** exist; **1** ships with WO-006
increment 1 (`hal/wasapi.rs`, Windows-only: `cfg(windows)` + the `hal-wasapi` feature — the auditor
scans it on every platform because the discipline is textual, not conditional). Entry 4 ships at
`sparq-kernel/src/alloc.rs` (the ADR names its final home, `src/rt/alloc.rs`); the auditor knows the
alias. Entries 2 and 6–10 are approved-but-unwritten: an entry here is permission to write the code,
not a claim that it exists (ASIO is WO-006 increment 2).

| # | Path | Why `unsafe` is required | Invariant that must hold | Test that proves it |
|---|---|---|---|---|
| 1 | `sparq-kernel/src/hal/wasapi.rs` | WASAPI is a COM API; event-driven exclusive mode needs raw interface calls | Buffer pointers from the device are valid for exactly `frame_count` frames and only for the duration of the callback; channel count matches the negotiated format | Round-trip impulse test; unplug/hijack recovery test; 2 h zero-xrun soak |
| 2 | `sparq-kernel/src/hal/asio.rs` | ASIO SDK is C++ → FFI shim | Callbacks are invoked on the driver's thread; our buffers outlive the call; no re-entrancy | Same as above, per driver tested |
| 3 | `sparq-kernel/src/rt/thread.rs` | MMCSS (`AvSetMmcThreadCharacteristics`), ideal-processor assignment, working-set lock, process priority | Handles are valid for the thread's lifetime; failures are detected and reported, never ignored | Diagnostics overlay shows MMCSS active; startup self-check |
| 4 | `sparq-kernel/src/rt/alloc.rs` | Counting/pre-reserved arena allocator; `mlock`-class page pinning | No allocation occurs on the audio path; arenas are reserved before `activate` and never grown during it | Allocator counter asserts 0 allocations per block in debug + CI |
| 5 | `sparq-kernel/src/sync/rings.rs` | Lock-free SPSC/MPMC ring buffers | Single producer / single consumer per ring (or documented MPMC); seqlock ordering correct; no torn reads | TSan stress test; 10⁹-iteration soak with checksum verification |
| 6 | `sparq-kernel/src/graph/hotswap.rs` | RCU-style pointer swap of the graph at block boundaries | Old graph is retired only after every audio-thread reader has passed a quiescent state | 10 000-iteration random-mutation-while-playing stress test, zero xruns |
| 7 | `sparq-audio/src/simd/` | Portable SIMD intrinsics where the safe API loses measurable performance | Lane counts match; no UB on unaligned tails (handled explicitly) | Golden-render equality against the scalar reference implementation |
| 8 | `sparq-assets/src/mmap.rs` | Memory-mapped sample data (zero-copy playback) | File is not truncated or modified while mapped; out-of-range reads impossible | Map/unmap soak; truncated-file and corrupt-hash tests |
| 9 | `sparq-bridge/src/shm.rs` | Shared-memory IPC with Tier-3 processes | Ring bounds respected by both ends; a misbehaving or dead peer cannot corrupt host memory | Fuzzed peer; kill -9 mid-transfer recovery test |
| 10 | `sparq-io/src/midi/ump.rs` *(only if parsing needs it)* | Raw UMP byte reinterpretation | Alignment and endianness handled; malformed packets rejected, never reinterpreted | Fuzz corpus of malformed UMP streams |

**Everything else is safe Rust.** In particular: no `unsafe` in any DSP module, in the music/logic plane,
in the UI, in the asset metadata layer, or in any Tier-2 (wasm) host code.

---

## Rules for adding an entry

1. Open a PR that adds the row **before** writing the code, with the invariant and the test named.
2. Prefer a safe design that is 20 % slower over an `unsafe` design that is 20 % faster, unless the
   measurement is on the audio thread and the difference is audible or budget-breaking. Record the
   measurement in the PR.
3. Each entry gets its own module — never sprinkle `unsafe` through a large file.
4. Miri must pass. TSan must pass where concurrency is involved.
5. If an entry's invariant cannot be tested automatically, it does not belong on this list.

## Review trigger

If the list exceeds ~15 entries, the architecture is leaking: re-examine whether a safe abstraction
(a HAL crate, a ring-buffer crate, a battle-tested FFI wrapper) should replace hand-rolled `unsafe`.
