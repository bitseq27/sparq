# P1b — run sheet (2026-09-23, `sync-p1b.zip`)

Your last run was **green on everything the tree controls**: 0 failed tests (both P1 defects fixed
and proven on the device), selftest PASS, golden `ba577186c988db21`, allocations 0, reopen-leak 0,
1063× realtime. The two remaining FAILs are not the build:

* **`stamp src 69f/1097801B`** — defect #71 landed on this machine: the stray byte-identical
  duplicate `crates\sparq-module-api\src\decode (2).rs` is in the tree (68 files + 1 = 69;
  1 070 593 B + 27 208 B = 1 097 801 B — the arithmetic matches to the byte). The exe is **not
  stale**; it honestly stamped a tree that gained one file it should not have. Deleting the stray
  restores `68f/1070593B`.
* **`tests_passed 390 != 388`** — defect #72, a checker artifact: a green gates.log contains the
  two golden tests twice (debug inside `cargo test --workspace`, release in the `golden reference`
  stage). `tools/log_check.py` in this bundle scopes the sum to the `---- tests ----` stage, so
  your existing gates.log will read 388 without re-running anything.

**Bundle:** `sync-p1b.zip` (3 entries: `tools/log_check.py`, `PHASE0-WORKORDERS.md`,
`P1B-RUN-SHEET.md`) → extract at the repo root, overwriting. Tests, scripts and engine untouched —
no rebuild of the suite needed.

## Steps

```bat
:: 1. delete the stray and prove there are no other copy artefacts (must print nothing)
del "crates\sparq-module-api\src\decode (2).rs"
dir /s /b "crates\* (2)*"

:: 2. rebuild so the exe re-stamps from the clean tree (~1 min; guard should print
::    "stamp guard: sources 68f/1070593B  -  binary 68f/1070593B" and NOT loop)
scripts\build.bat

:: 3. re-check against the existing logs - expect: stamp OK, tests 388/0, everything
::    else as before; MISSING only the five P2/P3 fields (soak + dispatch)
python tools\log_check.py --dir logs
```

No gates re-run is required (the scoped parser reads your existing `gates.log` as 388); run
`scripts\gates.bat` again anyway if you want a fresh log on record — it is ~2 minutes.

## Still wanted back (from P1a, plus one new question)

1. **Did `build.bat` complete this time?** i.e. does `logs\build.log` end with the stamp-guard
   line and the version block, or did `. was unexpected at this time.` recur? One command answers:
   `findstr /n /c:"stamp guard" /c:"was unexpected" /c:"sparq 0.0.0" logs\build.log`
   If it recurred: run `scripts\probe-stamp.bat` and send `logs\probe.log` — that is #69's
   root-cause hunt and it stays open until the probe names the statement.
2. `echo %NUMBER_OF_PROCESSORS%` — one number, for #66's record.
3. **How did `decode (2).rs` get onto this machine?** (a `git clone`/`pull` of the GitHub repo —
   which still contains the stray — an extractor answering "keep both files", or a manual copy?)
   Needed for the #71 record and to keep it from recurring. If the repo was cloned here, the
   delete in step 1 already restored parity with the zip chain, and step 2's stamp proves it.

## Then: green light for P2 and P3

Once step 3 shows `RESULT: no hard failures`, continue the original run sheet: **P2**
(`cd tools\dispatch-bench && cargo run --release` — send the `C. VERDICT` block and both
`D. PER-CALL COST` lines), then **start the P3 soak and let it run unattended** (`scripts\soak.bat
120 wasapi-exclusive`), P4 after it — not during — and P5's one-liner.

Housekeeping, off-machine: the GitHub repo still carries `decode (2).rs`; delete it in the web UI
(or push from a cleaned tree) so the next clone does not reimport it. The workspace clone on the
sandbox side already has the deletion staged.
