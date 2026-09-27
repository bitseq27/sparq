# Sync manifest — WO-014 increment 6 (`util/mixer` 0.2.0 — the cv merge side)

**Current bundle: `sync-wo014-inc6.zip` (13 entries, listed below).** Applies on top of
**`sync-wo013-inc5.zip`** — which the user confirmed APPLIED on 2026-09-27, so this bundle
applies directly. Extract at the repo root `Q:\morphosis\code\sparq`, overwriting. No
drift-repair copy step: the tree was sealed from a `sync_check --quiet`-clean state.

**The stamp changes: `src 92f/1977302B`** (`SYNC-STAMP.txt` regenerated for the same **149-file**
covered set — no new `src` files; the new gate file lives under `tests/`, which the stamp does not
cover but `gates.bat` runs. `Cargo.lock` stays SOFT and is NOT in the zip; SATURN keeps its own).

## What this is

ONE increment from the 2026-09-27 third session: the checklist's sandbox-track item 8, first
entry — **`util/mixer`'s cv merge side**, retiring the "not built yet" half of the executor's cv
fan-in refusal. The build-log entry in `PHASE0-WORKORDERS.md` (search **WO-014 increment 6**)
carries the full story; `WO014-INC6-PLAN.md` is the plan of record (nine decisions, recorded
before the code). Headline:

1. **The module** — v0.2.0 grows four block-rate unipolar cv inputs (`cv-0..cv-3`,
   `cv_reduce = "mean"` DECLARED per G3: the receiver declares, the host performs, nothing
   re-decides in code), per-input gains `cvm0..cvm3` (params **20..23, appended LAST** — every
   v0.1.0 snapshot index and every audio golden keeps its bits; short snapshots read 0.0 past
   their end, so the existing 20-value tests are unchanged BY CONSTRUCTION), and one `cv-out`
   summing in f64, clamped to the declared unipolar range on publish (the module's own documented
   rule — like the lfo's "mapped into 0..1" — never a silent host transformation). **Identity
   default**: `cvm0 = 1`, rest 0 — an untouched cv side is a bit-exact wire from `cv-0`, the
   audio side's transparency philosophy at cv scale. `BlockStatus` stays the AUDIO contract:
   all-unconnected audio ⇒ zeros + `Silenced` while the cv side merges and publishes (a cv-only
   mixer is a legitimate patch citizen — pinned). A wrong-rate presentation is `Failed` (the
   env/ad precedent). Unipolar because every shipped modulation source and consumer is unipolar;
   the bipolar side waits for `util/range` (Phase 1) rather than refusing the ecosystem today.
2. **The refusal, re-worded not removed** — cv fan-in still refuses (no implicit summing, ever;
   the compat-matrix cell, the drift gate and `Adapter::Merge` all keep their meaning), but the
   sentence now carries the WORKING remedy: wire each source to its own `cv-N` input and feed the
   destination from `cv-out`. `tests/contract_v1.rs`'s "names the merge module" pin passes on the
   new text; `mixer_cv.rs` pins the port names too.
3. **`Adapter::Merge.first_phase` stays Phase 1 ON PURPOSE** — the MODULE exists at Phase 0, but
   the compiled connect functions never return `Merge` (the fan-in cell is prose) and the canvas's
   one-tap insert mechanism fits INLINE pair adapters, not a three-wire merge re-patch. The
   auto-insert offer waits; recorded as decision 8 of the plan, not an oversight.
4. **A gate caught a real fragility mid-increment** — the WO-013 inc-5 inspector-scroll smoke
   failed when the mixer's param count moved 20 → 24 (its fixed drag distance had been tuned to
   the old `max_scroll`). The smoke now asserts the CONTRACT — the panel overflowed before, and
   after the drag some previously-hidden row is visible AND touchable where drawn — not the
   arithmetic of one module's panel. This is the gates doing their job, recorded honestly.

**Measured in sandbox:** **742 tests** (was 733; +9 — `tests/mixer_cv.rs`) · fmt clean · clippy
clean in default / bootstrap-audio / ui / native ui-window-gles / MSVC×8 cells (MSVC×ui-window and
MSVC×bootstrap-audio remain the documented `windows`-crate OOM class) · ALL pre-existing goldens
re-verified unchanged (`ba577186c988db21`, `0f5c3e86c7f117a9`, drum-demo `914d9063ce9d8a0f` —
which renders THROUGH the mixer — the three pinned exec renders at their `--seconds`, the phase-B
demo values) · stress hash `b42068ec7b206789` unchanged (debug AND release — the render path did
not move) · `selftest --golden` **9/9** · `ui --audit` PASS **25 smokes** (same count; the scroll
smoke hardened) · `modules --strict` **17/17** (the mixer row now reads v0.2.0, 13 ports, 24
params) · docs regenerated (17) + `--check` · 5 python gates + `sync_check --self-test` 11/11 +
`log_check --self-test` 25/25 · stamp `--write` then `--quiet` clean.

## Files in this zip (13)

Covered by the stamp — 5:

```
modules/util/mixer/sparqmod.toml             (0.2.0 — cv-0..3 + cv-out appended after out-3; cvm0..3 appended LAST; header decisions)
crates/sparq-audio/src/modules.rs            (Mixer::process — the cv merge before the audio-side early return; doc rules)
crates/sparq-audio/src/executor.rs           (the fan-in refusal re-worded to the built remedy; crate-header line corrected)
crates/sparq-app/src/ui/headless.rs          (the inc-5 scroll smoke hardened — the reveal-contract, not one module's arithmetic)
tools/log_check.py                           (BASELINE moved: 742 tests / src 92f/1977302B; fixture stamp)
```

Tests — 1 (not stamp-covered; `gates.bat` runs it and the log's test count depends on it):

```
crates/sparq-audio/tests/mixer_cv.rs         (NEW — the 9 increment-6 gates)
```

Generated docs — 1 (not stamp-covered; `module_docs.py --check` gates it):

```
docs/modules/util-mixer.md
```

Documents — 6 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt   WO014-INC6-PLAN.md
```

Heading = list = contents = **13** (5 + 1 + 1 + 6), checked against the zip's namelist below.

## What SATURN runs

No new device script — the standing order stacks, with baselines moved (`log_check.py` in the
bundle already expects them):

1. `scripts\test004.bat` — the WO-006 exclusive acceptance (**still the 🔴 blocker**; pass shape
   in `WO006-INC13-RUN-SHEET.md`). Unchanged by this bundle.
2. `scripts\test006.bat` — the WO-013 window session with steps F–I (unchanged by this bundle;
   the mixer is not part of the demo patch, so A–E and the [07] hash claim behave exactly as
   before).
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 742 passed / 0 failed / 1
   ignored**, stamp `src 92f/1977302B`, `sparq modules --strict` lists **17** (mixer: v0.2.0, 13
   ports, 24 params), selftest **PASS (9 gates)**, `ui --audit` **PASS (25 smokes)**.
4. Evidence, **with the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free — the
   two WO-009 device boxes (a LISTENED tempo sweep and the 30-min drift run).
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero xruns).

**Wanted back: `test004.log`, `test006.log` (with the F–I answers), `gates.log`, `logs\ui.log`.**

## Superseded bundles

`sync-wo013-inc5.zip` (20 — APPLIED, user-confirmed 2026-09-27),
`sync-wo014-inc5+wo013-inc4.zip` (29 — APPLIED), then the applied chain:
`sync-wo008-inc5.zip` (22), `sync-wo014-inc4.zip` (12), `sync-wo009-inc1.zip` (26),
`sync-wo014-inc3.zip` (19), `sync-wo008-inc4.zip` (38), `sync-wo014-inc2.zip` (29),
`sync-wo008-inc3.zip` (15), `sync-wo006-inc13.zip` (10), `sync-wo013-inc3.zip` (19),
`sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`, `sync-p1-fixes.zip`,
`sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`, `sync-wo006-inc11g.zip`
— each applied on top of the previous, in that order.

## Namelist

```
CHECKLIST.md
LATER.md
PHASE0-WORKORDERS.md
SYNC-STAMP.txt
SYNC.md
WO014-INC6-PLAN.md
crates/sparq-app/src/ui/headless.rs
crates/sparq-audio/src/executor.rs
crates/sparq-audio/src/modules.rs
crates/sparq-audio/tests/mixer_cv.rs
docs/modules/util-mixer.md
modules/util/mixer/sparqmod.toml
tools/log_check.py
```
