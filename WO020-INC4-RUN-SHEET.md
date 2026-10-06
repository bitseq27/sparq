# WO-020 increments 1–4 — SATURN run sheet (The Observatory: stream plane, guest, painters, the wall in the shell)

**Sync bundle:** `sparq-update-2026-10-06-wo020-inc4.zip` (**471 entries**, FULL-TREE — defect #94's
remedy: it overlays any tree at or after `sparq-update-2026-10-05-r8.zip` and does not depend on a
base you might not have) → extract at the repo root `Q:\morphosis\code\sparq`, overwriting.
Excluded from the pack by rule (**your copies win**): `.git\`, `target\`, `Cargo.lock` (cargo
re-resolves on first build), `logs\`, `*.wav`, `handoff\`, and `SYNC-STAMP.txt` — **the stamp does
NOT ride** (the rev-3 pattern): you re-stamp on this machine, step 2 below. The zip's sha256 is in
the delivery message beside this file (check it with `certutil -hashfile <zip> SHA256` before
extracting); it is also recorded in `handoff\sha256sums-wo020.txt`, which rides the repo, not the
zip. Persistent history (sandbox side): `handoff\sparq-wo020-inc1…inc4.bundle` +
`sparq-wo020-delivery.bundle` (`96fe57d`..the delivery commit), each with a `.sha256` sidecar.

Companion records: `WO020-STATE.md` (the full build record + resume recipe), `CHECKLIST.md`'s
WO-020 closeouts, plan of record `WO020-OBSERVATORY-PLAN.md`.

---

## 0. What this pack is

**WO-020 "The Observatory" (`dat/observatory`) through INC4** — the first instrument: a 4 × 4 wall
of live environmental/space-weather cells, port-less (D5), no WIT bytes moved. INC1 (the stream
plane's hermetic half: `sparq-streams`, the 23-stream registry as data, all 23 fixtures recorded at
HTTP 200) and INC2 (ADR-011, the `stream` source binding, `E-STREAM-UNKNOWN`, validator stage 1)
are **already published on main `96fe57d`** — the pack is cumulative and carries them regardless.
What moved vs `96fe57d`: **95 paths (61 A / 34 M), 85 shipped** (the 10 `handoff\` artefacts —
the bundles, the packer, the pack's committed copy and the sums — do not ride) — INC3 (the guest: `instruments-src/observatory/` workspace, the 206 081 B wasm component,
the harness with the pinned golden IR, the 4-file package `instruments/observatory/`) and INC4 (the
host painters: `sparq-ui::{json,displaylist}`, the egui backend, the dropdown picker — round-4's
OWED item 11 discharged, camera FOCUS, D6 sizing 2208×1288, the at-rest wall in the shell,
`sparq instrument render`).

Sandbox evidence at the seal (Linux, rustc 1.99.0, ~1 GB box): root `cargo test --workspace`
**1033 / 0 failed / 1 ignored**; observatory workspace **124 + 6 golden/drift**; clippy clean in
every runnable cell (workspace, `ui`, `ui,streams`, `ui-window`, `bootstrap-audio`); fmt clean both
workspaces; every python gate PASS (`token_gen --check`, `unsafe_audit`, `module_docs --check`,
`check_text_io`, `sync_check --self-test`, `make_coastline --check`, `gen_manifest --check`);
`ui --audit` **PASS**, coverage **97.5 %** (≥ 95 % rule); `mod validate` PARTIAL **by design**
(§2 step 5); goldens untouched (`ba577186c988db21` re-verified at the seal); the component
`wasm-tools validate` CLEAN. **One honest red:** `tools/token_audit.py` fails the §18 mockup SVGs
(14 groups / 298 occurrences, `design/mockups/observatory-*.svg` + `display-sheet.svg`) — it
**predates this pack** (the mockups are on main) and no pack file is flagged; owner is the mockup
gate work, recorded in `WO020-STATE.md`'s house notes.

**Two honest refusals you will see, both by design:**
* `sparq mod validate instruments\observatory\` → **GATE: PARTIAL, exit 1** — stages 3/4/6 are the
  runtime half and refuse **in words** until INC5's wasmtime loader lands ("a partial gate is not
  a hand-in").
* **PLAY with the Observatory on the canvas refuses in words (#58's sentence)** until the same
  INC5 loader registers packages. The shell offers the instrument, the executor declines, nobody
  lies. `reference\observatory\convergence-wo020-inc4.png` shows it.

---

## 1. Apply + re-stamp + the standing gates

1. **Extract** the zip at the repo root, overwriting.
2. **Re-stamp:** `python tools\sync_check.py --write --sync sparq-wo020-inc4-2026-10-06`
   (until then, `build.bat` / `test006`'s `[00b]` name the mismatch in words — EXPECTED, not a
   failure; the stamp in the pack's tree still reads `sparq-wo018-rebuild-2026-10-05 / sent
   unsent` because the sandbox must not move it — MUST-NOT-MOVE, plan §15.1).
3. **`scripts\gates.bat`** — expected: `rustfmt`, `clippy default`, `clippy audio+hal`, `tests`,
   `golden reference`, `release build`, `selftest + golden`, `ui layout audit`, `tokens up to
   date`, `unsafe allowlist`, `module docs generated` all **`[ ok ]`**; `design conformance`
   **[FAIL] on the 14/298 mockup set only** (§0's honest red — pre-existing, not this pack).
   Test counts **move** (sandbox: 1033/0/1 — the digest's new count + new src fingerprint are
   their **first device measurement** of this tree, as at r8). Golden hash must read
   `ba577186c988db21`. Note: the release `ui` cell builds on your box by design — the 1 GB
   sandbox OOMs it (`read-fonts` SIGKILL; recorded box limit, not a code defect).

## 2. The WO-020 device column (first device measurements; send it all back)

All from the repo root unless stated. `sparq.exe` = `target\release\sparq.exe` from step 1.3.

1. **The stream plane (INC1, hermetic):**
   `cargo test -p sparq-streams --features streams` (sandbox: **75 / 0**) and
   `cargo run -q -p sparq-app --features streams -- streams list` → **23 streams**;
   `streams probe` → **refuses in words** without `streams-net` (the device half is INC5).
2. **The guest workspace** — `cd instruments-src\observatory`:
   * `cargo test --workspace` (sandbox: **124 + 6** golden/drift; the golden IR sha is pinned in
     `harness\tests\golden.rs`: `bdf59cdb…974fa` — deterministic replay, it must match on Windows);
   * `cargo clippy --workspace --all-targets -- -D warnings` (clean);
   * `cargo fmt -p observatory-core -p observatory-wasm -p observatory-harness --check` —
     **NEVER `--all` in this workspace**: path-dep auto-membership makes it reformat the frozen
     SDK `tools\sparq-module-guest` (the `exclude` is load-bearing; measured twice);
   * `cargo run -q -p observatory-harness -- --report` → the §5.8 budget block. Sandbox:
     **22 343 verts / 497 instances / 16 264 heat cells** vs `medium` ceilings 262 144 / 16 384 /
     65 536 — **fits**. Deterministic from the checked-in fixtures: the numbers must match
     exactly, not merely fit;
   * `cargo run -q -p observatory-harness -- --out artefacts` → IR JSON + SVG at the three LODs
     (gitignored review artefacts);
   * `python gen_manifest.py --check` and `python ..\..\tools\make_coastline.py --check` → PASS
     (the staleness gates; `tools\data\` caches ride the pack).
3. **The package (INC3):** `sparq mod list` → `dat/observatory v0.1.0 … loadable` (+ the
   `preview.svg` advisory — absent BY DESIGN until stage 6 generates it, INC5);
   `sparq mod validate instruments\observatory\` → stage 1 **PASS**, stage 2 **PASS** (4 files),
   stages 3/4/6 **REFUSED IN WORDS**, stage 5 static half PASS, stage 7 unsigned→badged,
   **GATE: PARTIAL, exit 1 by design**.
4. **The at-rest render (INC4).** The explicit env door — use it, because the two cache-dir
   defaults disagree on Windows today (the harness writes `%USERPROFILE%\.cache\sparq\at-rest\`,
   the app reads `%APPDATA%\sparq\at-rest\`; recorded in `LATER.md` for INC5 to align; the env
   door is the supported review path on every platform):
   ```bat
   set SPARQ_ATREST=%CD%\instruments-src\observatory\artefacts\wall-large.ir.json
   sparq instrument render --dir instruments\observatory --svg-out logs\observatory-render.svg
   ```
   Open the SVG: the §5.4 default wall at full LOD. Compare with
   `reference\observatory\convergence-wo020-inc4.png`.
5. **The shell (INC4, eyes on):** `sparq ui` (with `SPARQ_ATREST` still set) → the wall at rest in
   the Display band; the browser groups it under instruments; tap a STREAM row → the **44 px**
   dropdown picker (16 cells × 24 options, group headers, KEY NEEDED in words for `geo.firms`
   without `SPARQ_FIRMS_KEY`); header double-tap / menu row → **FOCUS**; the §5.2 status label
   reads the ReplayProvider (fixtures). **PLAY refuses in words (#58)** — expected until INC5.
6. **The audit with the wall present:** `sparq ui --audit` (env still set) → **PASS**, coverage
   **97.5 %** at 2560×1600 (≥ 95 % rule), display band == min_size, picker rows 44 px, LOD
   1.0/0.5/0.3 → Full/Simplified/Dot.

## 3. Send back

* `logs\gates.log` (+ the digest block) and the re-stamp verdict line;
* §2's outputs: the streams count, the observatory test totals, the budget block, the validate
  PARTIAL table, the audit summary lines;
* `logs\observatory-render.svg` (or its sha256) and one word on §2.5's visual row (wall, picker,
  FOCUS, the PLAY refusal's sentence);
* anything that says no in words where these steps say PASS — that is a defect, name it and keep
  the log. The standing asks from ROUND8-HANDOFF §7 step 4 are unchanged.

## 4. What comes next (INC5 — device increment)

The wasmtime loader + package registration (PLAY's refusal dies), `BrokerProvider` on
`streams-net` (live feeds behind the same `StreamProvider` trait), stage-6 at-rest publishing to
the cache dir, the full `mod validate` chain (smoke / goldens / budgets / visual / `preview.svg`),
and the live acceptance run-sheet (plan §15.2, `scripts\test007.bat` — ships with INC5). INC6
(sonification, SGP4 tracks, GIBS imagery) stays deferred/gated. Requirement stands: a ≥ 2 GB host
for the wasmtime cells.
