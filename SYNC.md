# Sync manifest — WO-008 increment 5: the cross-thread hot-swap primitive (ADR-009 d3 + d8)

**Current bundle: `sync-wo008-inc5.zip` (22 entries, listed below).** Applies on top of
**`sync-wo014-inc4.zip`** — which is itself still waiting: apply **wo014-inc4 first, then this
one**, in that order. The user confirmed 2026-09-26 that everything through wo009-inc1 is
APPLIED on SATURN (wo013-inc3 → wo006-inc13 → wo008-inc3 → wo014-inc2 → wo008-inc4 →
wo014-inc3 → wo009-inc1). Extract at the repo root `Q:\morphosis\code\sparq`, overwriting —
but run the two `copy` commands in the drift-repair section FIRST.

**The stamp changes: `src 90f/1842678B`** (`SYNC-STAMP.txt` regenerated for the whole
**144-file** covered set — one new `src` file, the hot-swap primitive; and the covered set is
six-rooted again: defect #86's repair puts `sparq-music` back into `sync_check.py`'s
`CRATE_DIRS`/`FP_ROOTS`, which is why the count reads 144 where the drifted tool would have
said 140).

## What this is

**WO-008 increment 5**, sandbox-built and sandbox-green on 2026-09-27 — the CHECKLIST's "only
remaining increment with a proofs burden this heavy": the kernel hot-swap primitive, the
cross-thread engine, and decision 8's first ring publication. The build-log entry in
`PHASE0-WORKORDERS.md` (search **increment 5 — the cross-thread hot-swap primitive**) carries
the full story. Headline:

1. **`sparq-kernel::sync::hotswap`** (allowlist entry 6, shipped in `sync/` with the auditor's
   alias): ADR-009 d3's LITERAL pointer swap. Control stages complete boxed patches into one
   atomic slot; the audio thread's once-per-block `boundary` takes it with a single `swap`,
   runs the inherit hook at the swap point, and publishes the outgoing patch into one of eight
   **epoch-tagged retirement slots**. The grace period is mechanical: a retirement is
   reclaimable only once the controller's Acquire-loaded epoch exceeds its tag — i.e. only
   after the audio thread demonstrably passed the boundary that let it go — and the drop
   happens control-side, so module deactivation never runs inside a device callback. Slot
   exhaustion DEFERS the swap and counts it; the audio thread never blocks, never allocates,
   never drops a payload.
2. **The contract grows one bound: `Module: Send`** — built control-side, rendered audio-side,
   retired control-side; the handover moves the executor, so its modules must be movable.
   Additive (every shipped implementor is data-only and satisfies it automatically), pinned in
   `api_snapshot.rs`. `Sync` deliberately NOT required.
3. **The cross-thread engine** (`sparq-audio::engine`): `SharedEngine` (control: stage,
   `set_params`/`set_musical_position`, reclaim, `read_meters`, stats) + `AudioEngine` (audio:
   boundary swap with `inherit_runtime` → `EngineCmd` ring → render → per-node `MeterUpdate`
   publication). Both rings bounded, refuse-and-count. The single-owner `Engine` is untouched —
   the determinism harness keeps its vehicle and the stress hash `b42068ec7b206789` its meaning
   (verified unchanged in debug AND release; the `World::candidate_mutation` extraction the
   cross-thread stress reuses preserved the RNG draw order exactly).
4. **The proofs:** 9 kernel tests (epoch gate white-boxed, deferral without drops, teardown
   hygiene both orders, a paced 5 000-payload two-thread exchange) + 7 audio tests + **the
   acceptance: 10 000 seeded mutations staged from a control thread while the audio thread
   plays** — 7 400 swaps · ~43 000 blocks · 0 failed · 0 audio-thread allocations · every
   retirement reclaimed and dropped exactly once. No golden hash on purpose: which block a
   mutation lands on is a scheduling fact. **selftest gate 9** (64 paced swaps across two
   threads) is the device-side cell, so SATURN measures the primitive on its own atomics. CI's
   Miri widened `sync::rings` → `sync::` (the suite scales itself down under `cfg!(miri)`; the
   1 GB sandbox OOM-kills the nightly sysroot build, so Miri stays CI-only — recorded).

**Measured in sandbox:** **682 tests** (was 665; +17) · fmt clean · clippy clean in the
default, `bootstrap-audio`, `ui`, native `ui-window`/gles and MSVC×6 cells · ALL pre-existing
goldens unchanged and re-verified (`ba577186c988db21`, `0f5c3e86c7f117a9`, `53de3b1f3f40e3c9`,
`3f325d4f99ca2a01`, batch-2's six + chain `9170415cd3852736`, `1621e1f65b1b64e1`,
`1d1c84bd37c99b91`, `f2303f13aa0cf299`, `914d9063ce9d8a0f`) · stress hash `b42068ec7b206789`
unchanged · `selftest --golden` **9/9** · `ui --audit` PASS 19 smokes · `modules --strict`
**14/14** · 5 Python gates + `sync_check --self-test` 11/11 · stamp `--write` then `--quiet`
clean.

## Drift repair (defect #86) — read BEFORE extracting

The GitHub history rebuild ("Fresh start") lost bytes against the sealed tree: the pristine
clone fails `sync_check --quiet` on `crates\sparq-module-api\src\lib.rs` (230B of doc prose
smaller than sealed) and on `tools\sync_check.py` (lost sparq-music from its six-root
definition). This bundle carries repaired/declared copies of BOTH, so the device tree and the
new stamp agree byte-for-byte after extraction. The device's sealed copies are the only record
of what the rebuild lost — save them first:

```
copy crates\sparq-module-api\src\lib.rs logs\lib.rs.sealed
copy tools\sync_check.py logs\sync_check.py.sealed
```

…and send the two files back with the evidence logs. All 682 gates pass against the 3449B
lib.rs (the API surface is pinned by `api_snapshot.rs`), so the lost bytes are prose; the
returned diff will say exactly what they were.

## Files in this zip (22)

Covered by the stamp — 12:

```
crates/sparq-kernel/src/sync/hotswap.rs   (NEW — the primitive; allowlist entry 6)
crates/sparq-kernel/src/sync/mod.rs       (module + re-exports)
crates/sparq-kernel/src/lib.rs            (scope paragraph)
crates/sparq-module-api/src/module.rs     (Module: Send — the contract change, documented)
crates/sparq-module-api/src/lib.rs        (DRIFT REPAIR #86 — content otherwise unchanged)
crates/sparq-audio/src/engine.rs          (SharedEngine/AudioEngine/LivePatch/MeterUpdate/EngineCmd)
crates/sparq-audio/src/executor.rs        (status codec visibility only)
crates/sparq-audio/src/determinism.rs     (World pub + candidate_mutation extracted, hash-preserving)
crates/sparq-app/src/selftest.rs          (gate 9 — the two-thread hot-swap cell)
tools/sync_check.py                       (DRIFT REPAIR #86 — the six-root definition restored)
tools/unsafe_audit.py                     (entry 6's shipping alias)
tools/log_check.py                        (BASELINE moved: 682 tests / src 90f/1842678B)
```

Tests — 2 (not stamp-covered; `gates.bat` runs them and the log's test count depends on them):

```
crates/sparq-audio/tests/cross_thread.rs      (NEW — the 7 gates incl. the 10 000 acceptance)
crates/sparq-module-api/tests/api_snapshot.rs (the Send pin)
```

CI — 1 (not stamp-covered):

```
.github/workflows/ci.yml   (the Miri cell widened to sync:: — ring + hot-swap)
```

Documents — 2 (excluded from the stamp on purpose):

```
docs/unsafe-allowlist.md   docs/adr/009-executor-and-hal.md
```

Documents — 5 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt
```

Heading = list = contents = **22** (12 + 2 + 1 + 2 + 5), checked programmatically against the
zip's namelist at packing time.

## What SATURN runs

No new device script — the standing order stacks, with baselines moved (`log_check.py` in the
bundle already expects them):

0. **The two `copy` commands above, BEFORE extraction** (defect #86 forensics).
1. `scripts\test004.bat` — the WO-006 exclusive acceptance (**still the 🔴 blocker**; pass
   shape in `WO006-INC13-RUN-SHEET.md`).
2. `scripts\test006.bat` — the WO-013 window session (step [07]: slider edit changes the
   canvas-render.wav hash).
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 682 passed / 0 failed / 1
   ignored**, stamp `src 90f/1842678B`, `sparq modules --strict` lists **14**, and the selftest
   line reads **PASS (9 gates)** — gate 9 (`kernel hot-swap: 64 paced swaps across two
   threads`) is this increment's MSVC evidence.
4. Evidence, wanted **with the pinned durations** (defect #85 — the CLI default renders 5 s and
   hashes differently from every golden): `sparq exec --patch mod-demo --seconds 1` (`cv wire`
   line, cutoff > 200 Hz, hash `1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2`
   (transport-driven; beat triggers on the 120-BPM grid, hash `f2303f13aa0cf299`), `sparq exec
   --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free — the two
   WO-009 device boxes: a LISTENED tempo sweep and the 30-min drift run.
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero
   xruns) — which now ALSO discharges the paced-real-time half of allowlist entry 6's stress
   sentence. Plus `logs\lib.rs.sealed` and `logs\sync_check.py.sealed` from step 0.

## Superseded bundles

`sync-wo014-inc4.zip` (12 — WAITING: apply it before this one), then the applied chain:
`sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19), `sync-wo008-inc4.zip` (38),
`sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15), `sync-wo006-inc13.zip` (10),
`sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip` — each applied on top of the previous, in that order.
