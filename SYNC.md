# Sync manifest — WO-008 increment 7 (host-side `required`-unconnected enforcement — the declared baseline moved ON PURPOSE and recorded)

**Current bundle: `sync-wo008-inc7.zip` (17 entries, listed below).** Applies on top of
**`sync-wo006-inc15.zip` — which is APPLIED** (operator-reported, 2026-09-29). Extract at the
repo root `Q:\morphosis\code\sparq`, overwriting. No drift-repair copy step. This bundle
changes **no device contract** — test004 attempt 4 runs exactly as `WO006-INC15-RUN-SHEET.md`
says, before or after applying it — but it MOVES the gates' numbers, so apply it before the
next `gates.bat` run.

**The stamp changes: `src 92f/2021628B`** (`SYNC-STAMP.txt` regenerated for the same
**151-file** covered set — no new covered files; the `src` fingerprint's file count is
unchanged at 92, its byte count moved because `executor.rs`, `determinism.rs`, `bridge.rs` and
`headless.rs` grew. `Cargo.lock` stays SOFT and is NOT in the zip; SATURN keeps its own.
`modules/**` is untouched — no manifest moved; the vocabulary was already declared, only its
enforcement ships.)

## What this is

ONE increment from the 2026-09-29 sixth session (its second build, after `sync-wo006-inc15`):
the sandbox-track item the checklist named "the next candidate in this list" — **host-side
`required`-unconnected enforcement** — small, well-specified, declared in LATER.md §WO-008
since increment 3, and deliberately its own increment because it is the one change that MOVES
the stress baseline. Plan of record: `WO008-INC7-PLAN.md` (eight decisions, recorded before the
code).

* **The rule:** `Executor::build` refuses a patch in which any node has a `required` input port
  with zero incoming edges — a new `ExecError::RequiredUnconnected { node, module, port }`
  rendered as ONE sentence naming both remedies (connect a source, or remove the node; a
  genuinely optional port is a manifest `required = false` — a module change, not a patch
  change). Placed after the edge-rules pass (an ILLEGAL wire is a more specific defect than a
  MISSING one) and before buffer allocation; graph order × manifest order, first violation
  wins — deterministic like every refusal in the gauntlet. Scope is the compiled contract's own
  words (`Port::required` = "whether an unconnected INPUT is an error"): outputs stay free (a
  sinkless `out/main`, an undrawn `rms.level` wire remain legal), ≥1 carried edge satisfies a
  port, and cv fan-in was already refused upstream — the check can never legitimise a wiring
  the matrix refuses. No bypass/mute exemption (build enforces the manifest; runtime states do
  not excuse structural ones — declared in the plan).
* **The two declared dependents moved with it.** (1) The determinism world's unwired spare gain
  is now WIRED — from the chain gain's output, not the sine, on purpose: the spare is the
  world's initial master, so retune mutations reach the rendered samples and the script's hash
  stays sensitive to parameter edits (pin moved: 4 nodes, 3 edges). (2) The canvas demo patch's
  fourth node became a bare **`out/main`** — the one shipped module DESIGNED to sit bare
  (optional input): the smokes keep their free stereo input to drag onto, `resolve_master`'s
  documented handover rule ignores an unwired out/main (the master stays the chain gain), and
  **`canvas-render.wav` does not move**: 1 920 046 bytes — the device baseline's exact size —
  sha256 `d7ad294ea0b5e6e960f8dbc7f231dcbd84edec327e6352e80dc2f6ed786ec0b7` recorded for
  future A/Bs (bit-identical by construction: same master, ×1.0 path). The wire-level gates
  gained an honest COLD subject (the bare out/main renders `Silenced` and reads 0.0).
* **The unconnected-signal semantics survive at their declared home — OPTIONAL ports:** the
  `an_unconnected_input_is_silenced_and_reported` test migrated in place (its probe declares
  `required = false`), the test probes' audio inputs all declare optionality (the suite run was
  the census: exactly 8 tests touched the enforcement, all migrated or reworked in place), and
  the new gate `a_required_input_with_no_wire_is_refused_in_words` proves the refusal sentence
  AND its compliant twin. `cross_thread.rs`'s status pin TIGHTENED to `Ok` only — a `Silenced`
  meter would now mean the enforcement leaked.
* **The baseline moved ON PURPOSE and is recorded** (LATER.md's condition for this increment):
  **stress `7bb06379bd6845e5` — 10 001 blocks · 2 383 swaps · 7 617 refused · 0 audio-path
  allocations · 2 nodes / 0 edges final · max path latency 94 — debug AND release
  bit-identical** (was `b42068ec7b206789` · 7 203 swaps · 2 797 refused). The mix flipped
  because add/disconnect mutations that bare a required input are now build refusals: the
  leave-no-trace refusal path is exercised 2.7× harder, the swap machinery still cycles 2 383
  times under churn (acceptance floor: > 1 000), and every invariant assertion is
  rate-independent of the baseline by design. The cross-thread ledger moved with it (21 509
  blocks · 2 299 staged = swaps = reclaimed · 7 701 refused · 0 allocs · 0 errors). Historical
  entries keep the old hash as history; the live citations moved (CHECKLIST, LATER.md,
  `cross_thread.rs`'s doc comment).
* **Docs moved in the same session (#88's lesson):** `module-api-v1.md`'s "still declared open"
  list loses the item (replaced by the enforcement record), the `manifest-fields.toml` note and
  the `manifest-schema.md` row grow the host-enforcement clause (notes, not vocabulary — no
  table-first move needed; the paths pin is untouched), the executor header's v0 bullet is
  replaced. Parked in LATER.md beside the watchdog→UI hairline: **a canvas badge for the
  bare-required node** (the refusal reaches the shell log at RENDER time; drawing the warning
  at DROP time is a WO-013-side painter increment).
* **Defect #93 logged + remedied in passing (environment/delivery class):** the sandbox
  workspace drops any directory named `out` at message boundaries — `modules/out/main/
  sparqmod.toml` (stamp-covered) vanished mid-increment; upstream GitHub 404'd (repo
  rebuilt/private — #86's class). The operator supplied the file; restored byte-exact,
  hash-verified against the seal (`725bf07a…`, 2 691 B) before use; a durable recovery copy
  lives at the workspace root's `sparq-recovery/`, and the CHECKLIST environment notes carry
  the session-start discipline. The device tree was never affected.

## Measured in sandbox

**762 tests** (was 761; +1 the refusal gate; the silenced-report test and the reference-chain
pin migrated in place) · 0 failed, 1 ignored · fmt clean · clippy clean in every runnable cell:
default workspace, `bootstrap-audio`, `ui`, native `ui-window`/gles (`-j 1`, dev debuginfo off),
and all six MSVC cross-lint cells (the two `windows`-crate OOM cells remain
documented-unrunnable) · release goldens **bit-identical**: the three pinned exec renders at
their durations (`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`), the wo005 +
phase-b manifests, every module golden — enforcement only ADDS refusals, no render path moved ·
`selftest --golden` **PASS (9 gates)** · `ui --audit` **PASS (25 smokes)** with the new demo
node · `modules --strict` **17/17** · `canvas-render.wav` 1 920 046 B (sha256 above) · 5 python
gates clean · `log_digest --self-test` **16/16** · `log_check --self-test` **25/25** (BASELINE
moved with the seal — 762 / `src 92f/2021628B`, #83's discipline) · `sync_check --write` →
`--quiet` clean → `--self-test` 11/11 failable.

## Files in this zip (17)

Covered by the stamp — 5:

```
crates/sparq-app/src/bridge.rs          (the demo patch's fourth node: bare out/main, the master-handover module waiting to be wired; the wire-level gates' cold subject)
crates/sparq-app/src/ui/headless.rs     (smokes 6/7 comments: the free stereo input is out/main.in now — an optional port, the only legal bare one)
crates/sparq-audio/src/determinism.rs   (the world WIRES its spare from the chain gain; the reference-chain pin moves to 3 edges; the master-cone rationale in the doc)
crates/sparq-audio/src/executor.rs      (the enforcement: ExecError::RequiredUnconnected + its sentence, the gauntlet step, the header bullet replaced)
tools/log_check.py                      (BASELINE moved: 762 tests / src 92f/2021628B; fixture stamp moved with it)
```

Test sources — 2 (crates' `tests/` dirs are deliberately outside the stamp's covered set; they
ship because the device's `gates.bat` runs them):

```
crates/sparq-audio/tests/cross_thread.rs (the doc's hash citation moved; the status pin tightened to Ok-only)
crates/sparq-audio/tests/executor.rs     (probes declare optional inputs; the silenced-report test migrated in place; the new refusal gate + compliant twin)
```

Documents — 10 (excluded from the stamp on purpose):

```
CHECKLIST.md   EXTRACT-AT-REPO-ROOT.txt   LATER.md   PHASE0-WORKORDERS.md   SYNC.md
SYNC-STAMP.txt   WO008-INC7-PLAN.md   docs/api/manifest-fields.toml   docs/api/manifest-schema.md
docs/api/module-api-v1.md
```

Heading = list = contents = **17** (5 + 2 + 10), checked against the zip's namelist below.

## What SATURN runs

The standing order stacks, with baselines moved (`log_check.py` in the bundle already expects
them). Apply this bundle (on top of the applied inc15), then:

1. `scripts\test004.bat` — the WO-006 exclusive acceptance, **attempt 4** (still the 🔴
   blocker; unchanged by this bundle — pass shape, the PROPOSED acceptance-rate amendment and
   the failure-reading guide are in `WO006-INC15-RUN-SHEET.md`). Same prep discipline.
2. `scripts\test006.bat` — the window session, steps F–I standing. NEW reading with inc7: the
   demo patch shows a **Main Out** node instead of the spare Gain (bare, legal, waiting to be
   wired — wire it and the MASTER badge moves to it, the handover rule's demo); [05]'s
   `canvas-render.wav` baseline hash must be UNCHANGED (1 920 046 B, sha256
   `d7ad294ea0b5e6e960f8dbc7f231dcbd84edec327e6352e80dc2f6ed786ec0b7`), and [07]'s slider edit
   must still move it.
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 762 passed / 0 failed /
   1 ignored**, stamp `src 92f/2021628B`, `sparq modules --strict` lists **17**, selftest
   **PASS (9 gates)**, `ui --audit` **PASS (25 smokes)** — and the mutation-stress evidence line
   inside the test output reads **`7bb06379bd6845e5` · 2 383 swaps · 7 617 refused**: the moved
   baseline, recorded on purpose, NOT a regression.
4. Evidence, **with the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`) — all three re-verified
   bit-identical in the sandbox — and, when the HAL is free, the two WO-009 device boxes: a
   LISTENED tempo sweep and the 30-min drift run (on inc15's delivered-vs-wall metric).
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero
   xruns) — note for the loaded soak's rig: every processor node in it must have its required
   inputs wired now, or the build refuses in words (that is the feature).

**Wanted back — the DIGESTS, not the full logs: `test004-digest.log`, `test006-digest.log`
(with the F–I answers), `gates-digest.log`, `logs\ui-digest.log`.** Each script makes its own at
the end of the run; `scripts\digest.bat` catches anything older. The full logs stay on the
device — if a diagnosis ever needs a collapsed line, the run sheet says which full file to send
instead.

## Superseded bundles

Applied chain (each on top of the previous, newest first): `sync-wo006-inc15.zip` (15 —
APPLIED, operator-reported 2026-09-29), `sync-wo006-inc14.zip` (15 — APPLIED, proven by
attempt 3's build line 2026-09-28 evening), `sync-wo008-inc6.zip` (17 — APPLIED, proven by
attempt 2's build line), `sync-wo014-inc6.zip` (13 — APPLIED, same evidence),
`sync-wo013-inc5.zip` (20 — APPLIED, user-confirmed 2026-09-27),
`sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22), `sync-wo014-inc4.zip` (12),
`sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19), `sync-wo008-inc4.zip` (38),
`sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15), `sync-wo006-inc13.zip` (10),
`sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
EXTRACT-AT-REPO-ROOT.txt
LATER.md
PHASE0-WORKORDERS.md
SYNC-STAMP.txt
SYNC.md
WO008-INC7-PLAN.md
crates/sparq-app/src/bridge.rs
crates/sparq-app/src/ui/headless.rs
crates/sparq-audio/src/determinism.rs
crates/sparq-audio/src/executor.rs
crates/sparq-audio/tests/cross_thread.rs
crates/sparq-audio/tests/executor.rs
docs/api/manifest-fields.toml
docs/api/manifest-schema.md
docs/api/module-api-v1.md
tools/log_check.py
```
