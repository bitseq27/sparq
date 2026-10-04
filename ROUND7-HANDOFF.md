# ROUND7-HANDOFF.md — fifteenth session handoff: WO-018 first increment, written but NOT compiled (2026-10-05)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's ⚠ IN-FLIGHT head and
`WO018-STATE.md`, which is the mechanical resume card). This round is unlike the six before it:
**the work is complete as text and unaudited by any compiler.** The sandbox host lost its thread
budget mid-round (§3); everything below states exactly what is proven, what is written-but-
unproven, and the ordered recipe that turns the second into the first. Do not trust a single
line of the new code until that recipe has run — and do not delete this caveat when it does.

---

## 0. READ THIS FIRST — state of the tree, delivery, provenance

**What this tree is:** the round-6 sealed state (fresh clone of the re-publicised origin at
`6f842d7` + five real commits `1ae05a3…15492fe`, WO-017 CLOSED, contract v1.1 FROZEN, all gates
measured green: 909/0/1-ignored) **plus the uncommitted WO-018 zero-dependency half** written
in this session. The full inventory of the uncommitted work is `WO018-STATE.md`'s table — it is
the authoritative list; this handoff does not duplicate it.

**Provenance / durability:**
* Round 6's history is sealed TWICE: in the local `.git` (commits through `15492fe`) and in
  `/home/user/handoff/sparq-handoff-2026-10-05.bundle` (+ the overlay zip + sha256sums, all
  verified at pack time). If the snapshot layer loses `.git` objects again (ROUND5 §3.1's
  defect class), the bundle restores everything through round 6 — and the WORKING TREE holds
  all of round 7's files regardless, because round 7 made **no commits** (nothing to lose).
* Round 7 delivery artefacts: **none yet, by design.** The overlay zip / bundle for this round
  get built AFTER the resume recipe runs green — packing an uncompiled tree would ship the one
  thing six rounds of discipline forbade.
* `WO018-STATE.md`, this file, and CHECKLIST's ⚠ paragraph are the only round-7 files written
  with certainty-of-content (no compiler needed for prose).

**Operator rulings this round:** one, via the question tool, at the blockage: end the round with
a full handoff rather than keep probing an exhausted host. The round's SCOPE came from the
operator's `go wo018` + the ticket itself; no contract rulings were needed or made (the two
freeze debts WO-018 owns were closed as design+data — ceilings table, two-instance ordering —
neither of which touches the frozen WIT; the hash pins prove it: no `docs/api/instrument-wit/`
file changed this round).

---

## 1. What this round attempted, and the split it made

WO-018's ticket spans a zero-dependency half (package model, validator's static stages,
discovery, ceilings, CLI) and a wasmtime half (loader, fuel, hot reload, runtime stages). The
round opened by probing the wasmtime build FIRST, because it was the pacing risk — and the probe
is what surfaced the host's thread exhaustion (§3). With compilation blocked outright, the round
pivoted to the only honest work left: writing the zero-dependency half completely (it is the
half the ticket's acceptance criterion 2 demands be green with the feature OFF anyway), plus the
design notes that make the runtime half mechanical, plus every record a blocked round owes.

**Judgement calls made without the operator (flagged for review — each is reversible while no
instrument exists):**
1. **The `gpu_class` ceiling numbers** (`ceilings.rs`): none/light/medium/heavy/very_heavy →
   {0,0,0} / {65 536, 4 096, 16 384} / {262 144, 16 384, 65 536} / {1 048 576, 65 536, 262 144} /
   {4 194 304, 262 144, 1 048 576}. Anchored so `light` = exactly one 256×256 heightfield (the
   fm-terrain case) and one 128×128 heat-cell grid; ~4× steps; numbers only move UP outside a
   recorded re-baselining (spellings are the frozen part, drift-pinned).
2. **Launch-vs-strict package profiles** (`package.rs` header): the cap + manifest + component
   are fatal in BOTH; missing README/example are words-at-launch, failures-at-hand-in;
   `preview.svg` is NEVER a required input (it is stage 6's output — requiring it would make
   first validation impossible).
3. **The static capability-honour list** (`validate.rs`): T2 may not declare `network`/`device`/
   `process_spawn`/`gpu`/`fs_write` on (E-CROSS-FIELD, the fix names the door that CAN serve:
   T3 plane, host rendering, state blobs); `fs_read` (asset scopes) and the numeric knobs are
   honourable. This makes guide §6's sentence mechanical.
4. **`HOST_MODULE_API = 1`** lives in `sparq-host-wasm::validate` for now (its doc says it
   moves to `sparq-module-api` when load-time negotiation lands with the runtime).
5. **`scene` display ⇒ `gpu_class` above `none`** as a static cross-field rule (the zero row
   cannot be honoured; refuse at validation, not at first draw).
6. **CI shape**: feature-on run-cell on Linux only + MSVC crosscheck clippy (the HAL's own
   split), and `sparq mod list --strict` as a release-binary smoke step.
7. **Five new CodeKind variants were added NOW** (catalogue 25→30) although only
   E-PACKAGE-FILECOUNT is wired: the house precedent is pre-registered codes (E-TOUCH-CLASS-
   TOO-SMALL / E-WIDGET-KIND-UNKNOWN shipped before widget validation existed), and the
   spellings are the frozen part (manifest-schema §7).

---

## 2. Verification matrix — the honest one

| gate | round-6 seal (start of round) | round 7 (this round) |
|---|---|---|
| `cargo test --workspace` | **909/0/1-ignored** (measured) | **NOT RUN — blocked** (§3). Zero measurements exist for the new code. |
| fmt / clippy | CLEAN | NOT RUN — the blind-written code has never even been `cargo fmt`'d; expect reformatting, not just fixes |
| python gates ×5 | CLEAN | NOT RUN (they would likely pass — no python changed except none — but "likely" is not the standard) |
| `sync_check` | FAIL BY DESIGN (11 files) | NOT RE-MEASURED; it will list ~10 more moved files + the new ones; device re-stamps as usual |
| WIT hash pins | pinned at freeze | UNTOUCHED — `docs/api/instrument-wit/wit/*` did not change this round, so `wit_snapshot.rs` still passes by construction (the pins are of files nobody edited) |
| wasmtime probe | — | FAILED on environment, not on code: the dep tree fetched and began compiling; killed by OOM under `-j 2`, then by the thread ceiling. Diagnosis + retry recipe: `docs/instrument-host.md` §8 |
| ui --audit | carry-forward (round 5/6) | unchanged; this round's only UI-adjacent surface is the CLI (no painter, no geometry) |

Round-4 device obligations stand untouched (test006 A–U, test004 attempt 4, defect #95, the
dropdown picker) — and round 6 added none (its matrix was fully measured).

---

## 3. The incident, in full — host thread-table exhaustion

**Timeline.** The round opened clean: baseline probes fine, wasmtime 49.0.2 fetched, the dep
tree began compiling under `-j 2` with full debuginfo. `cranelift-assembler-x64` was
**SIGKILLed (OOM)** — MemTotal is 1.06 GB. Retried per the standard playbook (`-j 1`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`, separate `CARGO_TARGET_DIR`) and hit a NEW
failure class: `zmij`'s build-script link died with zig's LLD reporting
`unable to spawn LLD …: SystemResources` / `thread constructor failed: Resource temporarily
unavailable`, and `rustc` began ICEing on trivial crates (`fnv`, `semver`) with empty query
stacks and `__clone` in the backtrace — the signature of `pthread_create` → EAGAIN, not compiler
bugs (proven: the IDENTICAL rustc invocations succeed standalone in quiet windows).

**Diagnosis.** `/proc/loadavg` reads `1/1129` against `kernel.threads-max = 1129` — the thread
table is FULL. Enumerating our own PID namespace (pure-bash walk of `/proc`, since even `ps`
could not fork) found ~20 threads total (pid 1 = the arena's node supervisor, 13 threads):
**the saturation lives outside our namespace** — shared host or sibling workload. From inside,
it is unfixable: no leak of ours to kill, no limit of ours to raise. Spawn success became a
lottery (~1 in 5 tool calls; `echo`-class calls occasionally slipped through, every cargo
attempt died, and bash itself exits 254 when its fork retries exhaust — so in-shell retry loops
do not survive either).

**Mitigations that remain useful (all recorded in `docs/instrument-host.md` §8):** `-j 1`,
debug=0, incremental off, separate target dir, the `zigcc-direct` shim (execs the zig binary
without the python wrapper — one less process+80 MB per link), `-Clink-arg=-Wl,--threads=1`,
`CARGO_PROFILE_*_CODEGEN_UNITS=1`. None were sufficient against a full host thread table; they
are the right settings for a TIGHT one.

**Standing rule added:** when `/bin/true` fails, STOP probing in a loop (each failed spawn costs
the host too) — write the state down (file tools do not fork) and hand the decision to the
operator. A fresh sandbox gets a fresh thread budget; that is the actual remedy.

---

## 4. The resume recipe (ordered; `WO018-STATE.md` carries the command block)

1. **Probe the environment first:** `/bin/true`, then `cat /proc/loadavg` — if the second field
   of `N/M` is near `M` again, stop and re-hand-off; do not fight it.
2. **Environment:** if `.cache` survived, the toolchain is ready (rustc 1.99 + zigcc-direct +
   cargo registry incl. the fetched wasmtime tree). If not, rebuild per ROUND5 §7 + ROUND6 §7
   deltas (~25 min), and create the `zigcc-direct` shim (ROUND6 §7.3 explains why).
3. **`cargo fmt --all`** — the blind-written code's first formatting pass; review the diff.
4. **`cargo check --workspace --all-targets -j 1`** — the cheapest compile signal; fix against
   `WO018-STATE.md`'s risk list (borrow shapes in validate.rs, `ALL` array length in error.rs,
   binding modes in decode.rs, the `run` shadow in instruments.rs).
5. **`cargo test -p sparq-module-api -p sparq-host-wasm`**, then **`cargo test --workspace`** —
   expected: 909 + the new tests (manifest drift-pin +1, decode gpu +1, ceilings +2, package +5,
   package_gate +5, instruments parse +2 ≈ **927** — measure, don't trust this arithmetic), 0
   failed, 1 ignored.
6. **`cargo clippy --workspace --all-targets -- -D warnings`** + the five python gates +
   `module_docs --check` (24/24 — no manifest changed).
7. **Commit in the four logical increments** (the state file's table groups them): contract-crate
   additions (codes + gpu_class + pins) · the host-wasm crate + its tests · the CLI wiring ·
   docs/CI/layout. Bundle after each (ROUND5 §3.1's standing rule).
8. **Only then attempt the wasmtime half** (`docs/instrument-host.md` §8's recipe + §2–§4's
   design + the test plan). If it builds: runtime.rs increment, the Rust mirror of smoke.mjs,
   the noop-wasm fixture (rebuild from the SDK, sha-pin it), fuel→watchdog through the real
   executor, launch-loading wiring, then re-tick WO-018's acceptance criteria that close.
9. **Pack the round** (overlay zip + bundle + sha256sums, round-6 pattern), fold
   `WO018-STATE.md` into the build-log entry, update CHECKLIST's head (drop the ⚠ paragraph),
   and write the device-apply steps (re-stamp + gates.bat; the digest moves: new test count,
   new src fp — both UNMEASURED today).

**If the next environment also blocks:** the zero-dep half still ships once compiled anywhere
(device included — `scripts\gates.bat` after applying the tree is the same proof); the design
notes make the runtime half a device-side or later-sandbox increment. The round is not wasted;
it is deferred verification, recorded as such.

---

## 5. Git artefacts

```
6f842d7  clone head (round-5 tree, device-applied, origin public again)
1ae05a3  WO-017 close (start): operator rulings applied to the contract text        ┐ round 6 —
e3bdcc8  WO-017 close (native): classification.layer + ui.displays …               │ SEALED, in the
03676e8  WO-017 close (token bundle + snapshot gates) …                            │ bundle + zip of
7c1a6f3  WO-017 close (SDK at freeze) …                                            │ 2026-10-05
15492fe  WO-017 close (docs): the freeze record                                    ┘
<none>   round 7: ZERO commits — the uncommitted tree IS the round (WO018-STATE.md)
```

Author `sparq-agent <agent@localhost>` (placeholder — re-author at will). `/home/user/handoff/`:
`sparq-handoff-2026-10-05.bundle` (through `15492fe`), `sparq-update-2026-10-05.zip` (round-6
tree — NOTE: it does NOT contain round-7 files), `sha256sums.txt` (of both, verified at pack).
