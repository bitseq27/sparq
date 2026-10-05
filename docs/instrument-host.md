# The instrument host — WO-018 design record

**Status:** the zero-dependency half is BUILT and gated (this tree); the runtime half is DESIGNED
here and awaits a build environment (§8 records why, and the retry recipe). **Companions:**
`MODULE-BUILD-GUIDE.md` (the author-facing text this implements) · `docs/adr/010-instrument-layer.md`
(the decision) · `docs/api/instrument-wit/README.md` (the frozen ABI, incl. the two debts this
file closes) · `WO018-STATE.md` / `ROUND7-HANDOFF.md` (the round records).

**Provenance.** The first WO-018 session (2026-10-05) wrote the zero-dependency half blind — the
host's thread table exhausted mid-round before any of it compiled (§8). The code was lost with
that sandbox before it reached origin; this tree's copy was REBUILT from `ROUND7-HANDOFF.md` +
`WO018-STATE.md` on a clean clone, on the round-4 precedent (`READ-ME-FIRST.txt`: round 4 was
rebuilt from its handoff the same way), and then compiled, tested and gated for real. Judgement
calls recorded in `ROUND7-HANDOFF.md` §1 were kept as written — each is reversible while no
instrument exists.

---

## 1. The split, and why it is the honest one

WO-018's ticket spans two halves with different dependency footprints:

* **Zero-dependency half (built):** the five-file package model + cap, `instruments/` discovery,
  the `gpu_class` ceiling table (freeze debt #1), the validator's STATIC stages, the CLI doors
  (`sparq mod validate` / `sparq mod list`), and the five pre-registered error codes. Acceptance
  criterion 2 demands exactly this half be green with the feature OFF — the default
  `cargo test --workspace` proves the door without proving the sandbox.
* **Runtime half (designed, §2–§4):** the wasmtime loader, fuel/epoch wired to the existing block
  watchdog, hot reload at block boundaries, and the gate stages that need a live guest (smoke,
  golden render, budget measurement, visual conformance). Rides the `instrument-host` feature;
  default-off keeps the zero-dependency core build, the CI default cells and every golden
  untouched (ADR-010's consequence clause).

The static gate REFUSES the runtime stages in words (`validate::Verdict::Refused`, the PARTIAL
verdict, `sparq mod validate` exiting non-zero on PARTIAL) rather than passing them silently: a
partial gate that reads as a full one is the invisible failure the house forbids.

## 2. Loader architecture (runtime half)

One `wasmtime::Engine` for the host, configured once: cranelift, **fuel enabled**, **epoch
interruption enabled**, component model on, and **no WASI** — capability by absence
(instrument-wit decision E: what is not in the world does not exist for a guest).

Per package, at discovery:

1. **Compile + cache.** The component is compiled to the engine's serialisation format and cached
   content-addressed by the wasm file's sha256 (the ticket's "content-addressed cache directory is
   enough" — the full asset store is Phase 3). A package whose hash is unchanged across launches
   loads without recompiling; hot reload (§4) is the only in-session compile.
2. **Instantiate TWICE** (decision D): an audio instance (control + audio threads) and a display
   instance (UI thread). The `Linker` provides exactly the four imported interfaces of the
   `sparq:instrument` world — `tokens`, `host`, `assets`, `sources` — and nothing else. The
   audio instance's `sources` door answers "no snapshot" in words until a display binding exists;
   the display instance's `host.now` reads the same musical clock, never a wall clock.
3. **Cross-check identity.** `guest.id()` must equal the manifest's `identity.id` — a lying id is
   refused in words at load (the ticket's breakage; the static half cannot see it, stage 3 can).
4. **`prepare` before anything else**, with the resolved `Resources` (per-port channel COUNTS,
   the token bundle, `fuel-per-block` = the manifest's `max_fuel`). All guest allocation happens
   here; a `prepare` that traps or exceeds its epoch deadline refuses the LOAD, in words — a
   package that cannot prepare never enters the graph.
5. **Hand to the registry.** The loader pairs the validated manifest with a `Factory` that owns
   the two stores — the registry stays tier-blind (`sparq-module-api::discovery`'s own rule:
   discovery produces manifests, not modules; pairing is the loader's job).

Fuel and memory caps come from `[capabilities]` (`max_fuel`, `max_memory_mb`); the store's memory
limiter enforces the latter at the wasmtime level, so a guest that under-declares gets a trap the
watchdog turns into words — not a host OOM.

## 3. Fuel / epoch ↔ the existing watchdog

The executor already owns the failure vocabulary (WO-008 task 7): a block that overruns is
auto-bypassed with words in the UI, and the set continues. The sandbox maps onto it instead of
inventing a second mechanism:

| guest misbehaviour | mechanism | host-side consequence |
|---|---|---|
| `process` burns more than `max_fuel` | wasmtime fuel trap | mapped to `BlockStatus::Overrun` → the existing auto-bypass + words; repeated bypasses surface in the validator's report (guide §6) |
| `process` loops without consuming fuel (impossible in wasm, cheap insurance) | epoch tick from the watchdog's own deadline | same Overrun path |
| `prepare` hangs | epoch deadline on the control thread | LOAD refused in words — the package never enters the graph |
| `draw` hangs | epoch deadline on the UI thread | frames dropped and counted; N consecutive → the DISPLAY instance is bypassed (the panel shows words); **the audio instance is untouched** — decision D's isolation, the reason there are two instances |
| memory growth past `max_memory_mb` | store memory limiter | trap → Overrun path |

Fuel determinism (the ticket's risk note): fuel consumption is a function of code path and input,
so it is journal-replay-safe like everything else (ADR-007). The golden-render stage records the
per-block fuel counts in the package's report and the nightly matrix diffs them — a fuel number
that moves without a wasm hash change is a determinism defect, not a statistic.

## 4. Two-instance ordering — freeze debt #2, CLOSED as spec

The debt (instrument-wit README §5): "exact ordering of `configure` application across
audio/display instances at the boundary swap". The spec, in the boundary-swap engine's existing
slots — no new synchronisation primitive, because the ring and the swap slot are already the
proven doors (WO-008 increment 5, Miri-checked):

**Steady state (block boundary N):**

1. The parameter/state writer publishes to the audio instance's swap slot (existing path — param
   edits cross the command ring live; structural changes re-stage at the boundary).
2. The audio instance applies `configure(bytes)` at boundary N, BEFORE `process(N)`. It is the
   only authoritative applier: `save-state` is called on the audio instance and on nothing else.
3. The audio instance's post-configure state digest (the bytes it applied, versioned — not a
   re-serialisation) rides the existing analysis-ring publication to the UI thread.
4. The display instance applies the SAME bytes at its next frame boundary AFTER that publication —
   never before. The invariant: **a frame is never drawn from state the audio instance has not
   already applied.** Visual-ahead-of-audio is the race decision D exists to kill; audio waiting
   on visual is the xrun the same decision forbids. The display instance therefore lags by at
   most one frame, drops frames rather than block, and a wedged display instance can never xrun
   the audio (it is bypassed per §3).

**Hot reload (changed package, block boundary):**

1. Detection: the launch-discovery scan re-runs at the user's explicit reload (Perform mode never
   re-scans mid-set — a surprise recompile is not a stage behaviour).
2. The new component is compiled (§2.1) and instantiated as a SCRATCH pair beside the live pair.
3. `prepare` on the scratch audio instance; then, at boundary N: live audio instance
   `save-state()` → scratch `configure(bytes)` → the executor's adoption rule applies (WO-012
   round 4: an unchanged node keeps its state across a hotswap — adding or reloading a module
   while playing does not rebuild the world).
4. The scratch pair becomes live at boundary N+1; the old pair is retired after the last frame
   that could reference it completes (the display instance retires last — it is the lagging one).
5. State continuity is `configure`'s contract (guide §6.7): same schema_id/version → carried;
   changed schema → the migration chain runs, and a chain gap is the existing
   `E-MIGRATION-CHAIN-GAP` refusal, in words, with the old pair left live. The reload either
   succeeds whole or changes nothing.

## 5. `gpu_class` ceilings — freeze debt #1, CLOSED as data

The numbers live in `crates/sparq-host-wasm/src/ceilings.rs` (one copy, in code, with the
anchors and the movement rule in its header): `light` = exactly one 256×256 heightfield — the
fm-terrain case the guide walks — and one 128×128 heat-cell grid; every step multiplies every
column by 4; `none` is the zero row, and because the zero row cannot be honoured by a `scene`
display, the validator refuses `scene` + `gpu_class = "none"` statically (at validation, not at
first draw). The SPELLINGS are the frozen part (`manifest::GPU_CLASSES`, drift-pinned against
`docs/api/manifest-fields.toml`, value-pinned in `tests/api_snapshot.rs`); the numbers are host
policy and move UP only, with a recorded re-baselining. Over budget at runtime the renderer
degrades LOD, and at Full-LOD refusal the display shows words, not garbage (guide §5).

## 6. Code-wiring matrix — which file enforces which rule

| rule (source) | enforced in |
|---|---|
| five-file cap, roles, profiles (ADR-010 d5; guide §3) | `sparq-host-wasm::package` (`E-PACKAGE-FILECOUNT`, the launch-vs-strict split, `preview.svg` never required) |
| discovery, sorted, one-broken-never-hides-others (module-api §11) | `package::discover` + `loose_files`; CLI voice in `sparq-app::instruments` |
| schema + placement + host_api pin + capability honour + data-port refusal + scene⇒gpu (guide §6/§7 stage 1; v1.1 status rulings) | `sparq-host-wasm::validate::schema_check` (on top of `sparq-module-api::decode`) |
| `gpu_class` domain (manifest-schema §8) | `sparq-module-api::decode::check_gpu_class` (the displays precedent — NOT modelled into `ResourceDecl`) |
| ceiling numbers (guide §5 budget) | `sparq-host-wasm::ceilings` |
| declared budgets sane (guide §4/§7 stage 5, static half) | `validate::budgets_stage` |
| signature presence/shape + badge words (plan §15; schema §13) | `validate::signature_stage` (crypto verification: runtime keyring, §2) |
| code spellings (manifest-schema §7) | `sparq-module-api::error` — catalogue 30: 25 catalogue + 5 derived; the five v1.1 codes pre-registered, `E-PACKAGE-FILECOUNT` wired now, the other four wired by the runtime stages that own them |
| runtime stages 3/4/6 + measurement (guide §7) | REFUSED IN WORDS until §2–§4 land (`Verdict::Refused`, PARTIAL) |
| feature-on/feature-off CI cells (ticket acceptance 2) | `.github/workflows/ci.yml` — default cells untouched; the Linux feature-on cell + the MSVC crosscheck clippy (the HAL's own split) + `sparq mod list --strict` as the release-binary smoke |

## 7. The runtime increment's test plan

1. **The Rust mirror of `smoke.mjs`.** The JS harness's 11 checks (full lifecycle over the frozen
   WIT, refusals in words, exact negotiated shapes) become a feature-gated integration test in
   this crate — same assertions, no node in the rust CI cells.
2. **The `noop-wasm` fixture, rebuilt and sha-pinned.** Built from `tools/sparq-module-guest`
   per instrument-wit README §7's corrected recipe (`cargo build --target
   wasm32-unknown-unknown` → `wasm-tools component new` → checked in); the pin moves only with a
   recorded rebuild, like every golden.
3. **Fuel → watchdog, through the real executor.** A guest that burns `max_fuel + 1` is
   auto-bypassed mid-playback with the existing words-in-the-UI behaviour and the set continues
   (ticket acceptance 3); the fuel count lands in the golden report (§3's determinism claim).
4. **Hot reload at a block boundary**, state carried through `configure` per §4 (ticket
   acceptance 4); an unchanged package reloads to unchanged golden renders, bit-exact.
5. **Launch loading.** A synthetic five-file package dropped in `instruments/` loads at next
   launch with zero config edits (ticket acceptance 1's first half); the deliberately broken ones
   are the `package_gate` breakage list, driven through the real launch path instead of the
   library door.
6. **Validate end-to-end on the template skeleton** — stages 3/4/6 running for real, under the
   ticket's 60 s bar (acceptance 5), on the dev machine.

## 8. Build-blockage record + retry recipe (the 2026-10-05 incident)

**What happened.** The first WO-018 session probed the wasmtime build FIRST, because it was the
pacing risk. wasmtime 49.0.2's dependency tree fetched and began compiling under `-j 2` with full
debuginfo; `cranelift-assembler-x64` was SIGKILLed (OOM — MemTotal 1.06 GB). Retried per the
standard playbook (`-j 1`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`, separate
`CARGO_TARGET_DIR`) and hit a NEW failure class: the zig-cc link died with `unable to spawn LLD
…: SystemResources` / `thread constructor failed`, and `rustc` began ICEing on trivial crates
(`fnv`, `semver`) with empty query stacks and `__clone` in the backtrace — the signature of
`pthread_create` → EAGAIN, not compiler bugs (proven: the IDENTICAL invocations succeed
standalone in quiet windows). `/proc/loadavg` read `1/1129` against `kernel.threads-max = 1129`:
the thread table was FULL, and enumerating our own PID namespace found ~20 threads — the
saturation lived OUTSIDE the container (shared host or sibling workload). From inside: unfixable.

**Mitigations (the right settings for a TIGHT host; none sufficient for a FULL one):** `-j 1` ·
`CARGO_PROFILE_DEV_DEBUG=0` · `CARGO_INCREMENTAL=0` · separate `CARGO_TARGET_DIR` · the
`zigcc-direct` shim (exec the zig binary without the python wrapper — one less process and ~80 MB
per link; ROUND6 §7.3) · `-Clink-arg=-Wl,--threads=1` · `CARGO_PROFILE_*_CODEGEN_UNITS=1`.

**Standing rule (added by the incident):** when `/bin/true` fails, STOP probing in a loop — each
failed spawn costs the host too. Write the state down (file tools do not fork) and hand the
decision to the operator. A fresh sandbox gets a fresh thread budget; that is the actual remedy.

**Retry recipe for the runtime half**, in a healthy environment (`/bin/true` spawns,
`/proc/loadavg`'s second field far from `threads-max`, ≥ 2 GB free):

```sh
# 1. environment per ROUND5-HANDOFF §7 + ROUND6 §7 (rust standalone, zigcc-direct, wasm std faces)
# 2. feature-on compile, cheapest signal first, memory-guarded:
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=/tmp/sparq-wasmtime-target     # keep the bulk off the snapshot path
cargo check -p sparq-host-wasm --features instrument-host -j 1
# 3. then the §7 test plan, in its order. If cranelift OOMs even at -j 1 / debug=0, the host is
#    the limit, not the code: record it here and defer to the device (scripts\gates.bat runs the
#    same proofs; the zero-dep half already ships either way).
```

**Measured, 2026-10-05 (the rebuild sandbox — this IS step 2's outcome, recorded per the
recipe's own rule):** the retry recipe ran on a healthy host (loadavg `1/537` of 1129 threads —
no exhaustion this time) with every mitigation above. wasmtime 49.0.2's dependency tree
**fetched, resolved and checked green** — ~60 crates including `cranelift-assembler-x64`, the
round-7 victim, which passed this time on the debug=0 setting — until **`cranelift-codegen` was
SIGKILLed (OOM) at 4 m 21 s**: its ISLE macro expansion needs more than the 1.06 GB MemTotal, at
`cargo check` — the cheapest possible signal. `swapon` on a swapfile is refused in the container
(no CAP_SYS_ADMIN), so the ceiling is unfixable from inside. **Conclusion: the blockage is the
sandbox's memory, categorically — not the toolchain, not the pin, not this tree's code.** The
pin's metadata is proven valid (resolution + lock succeeded; the dependency tree compiled up to
the kill). The recipe's "≥ 2 GB free" is no longer a guess — cranelift-codegen's check alone
exceeds 1.06 GB, so 2 GB free is a floor with a measurement behind it.

**Unmeasured-risk note (recorded, narrowed by the probe above):** the feature-ON CI cells added
with this increment have still never run to completion anywhere — but the Linux cell's risk is
now bounded: the dependency tree checks green up to the host's memory ceiling, so on a
normal-size runner the cell is a compile-time question, not a correctness one. The MSVC cell's
risk is unchanged and specific: wasmtime's build scripts under cross-clippy from Linux — if they
refuse, the documented fallback is feature-OFF MSVC clippy (which IS measured clean on this tree:
module-api, host-wasm, music, kernel, audio, ui, app — all default-feature MSVC cells green in
the rebuild sandbox) plus the feature-on Linux cell, and the narrowing gets recorded here before
it is applied.
