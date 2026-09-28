# Sync manifest — WO-008 increment 6 (`cv_interp = "spline"` — the host performs the last word of G4)

**Current bundle: `sync-wo008-inc6.zip` (17 entries, listed below).** Applies on top of
**`sync-wo014-inc6.zip`** — which is still WAITING — which applies on top of
`sync-wo013-inc5.zip` (APPLIED, user-confirmed 2026-09-27). **Apply the two waiting bundles in
that order**, extract at the repo root `Q:\morphosis\code\sparq`, overwriting. No drift-repair
copy step: the tree was sealed from a `sync_check --quiet`-clean state (the pristine clone passed
at session start, defect #86's forensics step not needed).

**The stamp changes: `src 92f/1986012B`** (`SYNC-STAMP.txt` regenerated for the same **149-file**
covered set — no new `src` files; the new gate file lives under `tests/`, which the stamp does not
cover but `gates.bat` runs. `Cargo.lock` stays SOFT and is NOT in the zip; SATURN keeps its own).

## What this is

ONE increment from the 2026-09-27 fourth session: the checklist's sandbox-track item 8, second
entry — **`cv_interp = "spline"`**, refused at build until now ("faking it with a hold is exactly
the silent transformation the refusal exists to prevent" — LATER.md). The build-log entry in
`PHASE0-WORKORDERS.md` (search **WO-008 increment 6**) carries the full story;
`WO008-INC6-PLAN.md` is the plan of record (eight decisions, recorded before the code). Headline:

1. **The curve, in one compiled copy** (`CvInterp::Spline`'s doc in `port.rs`): the parabola
   through the last three block values — `v(t) = prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2`,
   `t = i/frames` — equivalently a cubic Hermite with the causal central-difference start tangent
   `(cur−prev2)/2` and the second-order backward end tangent `(3cur−4prev+prev2)/2`. Exact for
   any signal quadratic in block index; the exact line for collinear knots (a ramp renders
   BIT-IDENTICALLY to the same wire declaring `linear`); constant in, constant out; and
   `linear`'s arrival contract kept — frame 0 IS the previous knot exactly, `cur` is reached at
   the next block boundary, so the declaration changes the shape of the ride, never its timing.
   The non-causal splines (true Catmull-Rom, the natural cubic) were REJECTED on the record: both
   buy smoothness with a block of latency; a monotone PCHIP variant too (its limiter bends
   parabolas, losing exactness). f64 evaluation, one rounding per frame (the mixer's cv-sum rule).
2. **Range discipline, in words:** the parabola carries momentum and can locally exceed the knot
   span — all interpolating splines do. The overshoot is HOST-made, so the spline branch clamps to
   the wire's declared range (`CvRange::clamp_f64`, new). `hold`/`linear` deliberately do NOT
   clamp: they cannot exceed their knots, and an out-of-range frame there can only be a SOURCE
   bug, which must stay visible rather than be silently rounded away.
3. **The refusal retired; the philosophy kept.** The build refusal existed because the vocabulary
   promised what the host did not perform; every other cv refusal (range, fan-in, delay-kind)
   stands untouched. `contract_v1`'s refusal gate is replaced IN PLACE by a behaviour gate, and
   the matrix cell / §17 / the schema row / the author guide now carry the same sentence as the
   compiled copy — pinned by the widened G4 drift gate (the "answered twice, in two places"
   discipline).
4. **Nothing else moved, by the shape of the diff:** the hold/linear branches are byte-identical,
   no shipped module declares `spline` (grep-verified), no demo patch contains a block→audio cv
   edge, and the stress/determinism generators never synthesise an interp — which is why every
   pre-existing golden, the stress hash and the three pinned exec renders are unchanged by
   construction, not by luck. `CvPlan` grew `prev2` + `range` (both history slots start at 0.0 —
   `linear`'s own zero-history convention); the audio path still allocates nothing.
5. **Defect #88 (docs-only, fixed in passing):** the author guide's §11 still called `util/mixer`
   "Phase 1, not yet built" — stale since WO-014 inc 6. The bullet had to be rewritten for
   `spline` anyway; both halves now tell the truth.

**Measured in sandbox:** **752 tests** (was 742; +8 `tests/cv_spline.rs`, +2 `port.rs` units,
contract_v1's gate replaced 1:1) · fmt clean · clippy clean in default / bootstrap-audio / ui /
native ui-window-gles / all six runnable MSVC cells (MSVC×ui-window and MSVC×bootstrap-audio
remain the documented `windows`-crate OOM class) · ALL pre-existing goldens re-verified unchanged
(`ba577186c988db21`, drum-demo `914d9063ce9d8a0f`, the three pinned exec renders at their
`--seconds`: `1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`) · stress hash
`b42068ec7b206789` unchanged with identical counters (10 001 blocks · 7 203 swaps · 2 797
refused) in debug AND release · `selftest --golden` **9/9** · `ui --audit` PASS **25 smokes**
(the canvas never saw the refusal — interp lives in the executor, not `connect_cv`) ·
`modules --strict` **17/17** · `module_docs --check` (17) · 5 python gates + `sync_check
--self-test` + `log_check --self-test` 25/25 · stamp `--write` then `--quiet` clean.

## Files in this zip (17)

Covered by the stamp — 3:

```
crates/sparq-module-api/src/port.rs         (CvInterp::Spline's definition; expand widened — the parabola + the clamp; CvRange::clamp_f64; 2 unit pins)
crates/sparq-audio/src/executor.rs          (the spline refusal retired; CvPlan grew prev2 + range; the wire pass shifts the two-slot history; docs corrected)
tools/log_check.py                          (BASELINE moved: 752 tests / src 92f/1986012B; fixture stamp)
```

Tests — 4 (not stamp-covered; `gates.bat` runs them and the log's test count depends on them):

```
crates/sparq-audio/tests/cv_spline.rs       (NEW — the 8 increment-6 gates)
crates/sparq-audio/tests/contract_v1.rs     (the refusal gate REPLACED in place by the behaviour gate)
crates/sparq-module-api/tests/api_snapshot.rs   (the expand pin moved ON PURPOSE, reason at the pin; clamp_f64 pinned)
crates/sparq-module-api/tests/compat_matrix.rs  (the G4 drift pin widened: the cell must carry the curve's definition)
```

Contract/API documents — 4 (excluded from the stamp on purpose; the drift gate ties the first to the code):

```
docs/api/compat-matrix.toml                 (the G4 cell's condition gains the definition sentence)
docs/api/module-api-v1.md                   (§17: the vocabulary is performed in full, with the gates named)
docs/api/manifest-schema.md                 (the cv_interp row gains the definition pointer)
docs/module-author-guide-v0.md              (both refusal mentions rewritten; the stale mixer clause fixed — defect #88)
```

Documents — 6 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt   WO008-INC6-PLAN.md
```

Heading = list = contents = **17** (3 + 4 + 4 + 6), checked against the zip's namelist below.

## What SATURN runs

No new device script — the standing order stacks, with baselines moved (`log_check.py` in the
bundle already expects them). **Apply `sync-wo014-inc6.zip` first if it has not landed yet**, then
this bundle:

1. `scripts\test004.bat` — the WO-006 exclusive acceptance (**still the 🔴 blocker**; pass shape
   in `WO006-INC13-RUN-SHEET.md`). Unchanged by this bundle.
2. `scripts\test006.bat` — the WO-013 window session with steps F–I (unchanged by this bundle; no
   spline wire appears in the demo patch, so A–E and the [07] hash claim behave exactly as
   before).
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 752 passed / 0 failed / 1
   ignored**, stamp `src 92f/1986012B`, `sparq modules --strict` lists **17**, selftest
   **PASS (9 gates)**, `ui --audit` **PASS (25 smokes)**.
4. Evidence, **with the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free — the
   two WO-009 device boxes (a LISTENED tempo sweep and the 30-min drift run).
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero xruns).

**Wanted back: `test004.log`, `test006.log` (with the F–I answers), `gates.log`, `logs\ui.log`.**

## Superseded bundles

Prerequisite, still WAITING: `sync-wo014-inc6.zip` (13). Then the applied chain:
`sync-wo013-inc5.zip` (20 — APPLIED, user-confirmed 2026-09-27),
`sync-wo014-inc5+wo013-inc4.zip` (29 — APPLIED),
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
WO008-INC6-PLAN.md
crates/sparq-audio/src/executor.rs
crates/sparq-audio/tests/contract_v1.rs
crates/sparq-audio/tests/cv_spline.rs
crates/sparq-module-api/src/port.rs
crates/sparq-module-api/tests/api_snapshot.rs
crates/sparq-module-api/tests/compat_matrix.rs
docs/api/compat-matrix.toml
docs/api/manifest-schema.md
docs/api/module-api-v1.md
docs/module-author-guide-v0.md
tools/log_check.py
```
