# P1c — run sheet (2026-09-23, `sync-p1c.zip`)

**#69 is closed.** Your probe run named the killer on its first section: inside the BUILD-FAILED
hint block, `echo ... Cargo.toml (the per-target wgpu features live there).` — in a parenthesised
block the first unescaped `)` ends the block (the same-line `(` does not balance it), and the
leftover `.` is the token cmd rejected. The block is parsed on **every** run whether or not the
build failed, so `build.bat` died after every successful build since WO-012 added that line —
which is why P1a had a healthy exe but no stamp in `build.log`. The parens are now escaped, and
`check_text_io.py` grew **rule 4** so the class can never ship again (proven failable against the
verbatim original; the probe keeps it as section S0a, where a nonzero exit is the *expected*
result — a reproduction that was fixed proves nothing). S2 also cleared the version-capture loop
that was the prime suspect — the record now says so.

**Bundle:** `sync-p1c.zip` (5 entries: `scripts/build.bat`, `scripts/probe-stamp.bat`,
`tools/check_text_io.py`, `PHASE0-WORKORDERS.md`, `P1C-RUN-SHEET.md`) → extract at the repo root,
overwriting. No Rust, no tests, no stamp change.

## Steps — order matters

```bat
:: 1. the build, which now runs to completion: guard line
::    "stamp guard: sources 68f/1070593B  -  binary 68f/1070593B", then the version block,
::    then "built: ... size: ... next: ...". One forced rebuild is possible and is the #49
::    remedy working (your exe may still carry the 69f stamp from the P1b build; the guard
::    deletes, rebuilds and re-verifies until sources == binary).
scripts\build.bat

:: 2. the gates - REQUIRED after build.bat, not optional: your current gates.log still carries
::    the P1a "version stamp" line reading src 69f/1097801B, and log_check lets gates.log win
::    over build.log for the stamp field. A fresh gates run replaces it (and re-runs the text
::    I/O gate with its new rule 4). Expect every stage [ ok ], "all gates passed",
::    388 passed / 0 failed / 1 ignored.
scripts\gates.bat

:: 3. expect: RESULT: no hard failures - stamp OK, tests 388/0, golden OK, selftest PASS,
::    ui_audit PASS, allocations 0, reopen-leak 0, realtime ~1000x SOFT; MISSING only the
::    five P2/P3 fields (soak + dispatch).
python tools\log_check.py --dir logs
```

## Then: P2 and P3, per the original run sheet

* **P2** — `cd tools\dispatch-bench && cargo run --release`; send back the `C. VERDICT` block and
  both `D. PER-CALL COST` lines (ADR-009 executor decision 7's device numbers).
* **P3** — start the soak first and let it run unattended: `scripts\soak.bat 120 wasapi-exclusive`,
  then `scripts\hal.bat`, `scripts\devices.bat`, and the three human-and-cable sheets (latency
  table, unplug test, UMC204HD verdict). **Nothing else runs while the soak is recording.**
* **P4/P5** afterwards, as written.

## Still wanted for the record

1. `echo %NUMBER_OF_PROCESSORS%` — one number, for #66 (the audit race needed a machine with
   enough cores to run all eight tests at once; yours is the first that could).
2. How `decode (2).rs` reached this machine — a `git clone`/`pull` of the GitHub repo (which
   still contains the stray; delete it in the web UI so the next clone does not reimport it),
   an extractor answering "keep both files", or a manual copy. S4's clean `68f/1070593B` proves
   the tree itself is right again; the answer is for the #71 record and for prevention.
3. `logs\build.log` + `logs\gates.log` (or just the log_check table if both are green — the
   stamp line in build.log is the one worth keeping on record).
