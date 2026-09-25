# P1 fixes — run sheet (2026-09-23)

**Sync bundle:** `sync-p1-fixes.zip` (7 entries) → extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. It applies on top of `sync-wo007-complete.zip` and touches **tests and scripts only** —
the stamp stays `src 68f/1070593B`, so the binary you already built remains valid and log_check's
baseline is unchanged.

What P1 proved (recorded in full in `PHASE0-WORKORDERS.md` §2.1, defects #66–#71): the increment
**builds and links on real MSVC**, the golden render is **bit-identical** (`ba577186c988db21`),
selftest is **8/8** on the device at **1059× realtime**, and `sparq-module-api` is verified on
Windows. The two test failures were sandbox-side test bugs — a parallel-audit race that only a
machine with your core count could expose (#66, reproduced in the sandbox at `--test-threads=8`),
and a hardcoded `/src/` separator (#67). `build.bat` had two bugs of its own (#68 four-vs-five-root
stamp guard, #69 the cmd parser death). All four are fixed below; #69 stays *open* until its probe
names the culprit statement.

## Steps

1. **`scripts\build.bat`**
   Expected: `stamp guard: sources 68f/1070593B  -  binary 68f/1070593B`, then the version block,
   **no** rebuild loop (that was #68 — the guard was comparing a four-root count against a
   five-root stamp and would have condemned a perfect binary).
   **If `. was unexpected at this time.` appears again:** run `scripts\probe-stamp.bat` and send
   `logs\probe.log` — that is #69's root-cause hunt; the last echoed statement before the error
   names the killer. Everything else still works meanwhile (the hardened capture ships in this zip).

2. **`scripts\gates.bat`**
   Expected: every stage `[ ok ]`, `GATES` all-pass, **388 passed / 0 failed / 1 ignored**. Two new
   stages run after the release build (#70): `version stamp` and `selftest + golden` — they put the
   stamp, the golden hash, the rt-discipline allocation count, the reopen-leak balance and the
   realtime figure into `gates.log`, so log_check stops reporting them MISSING on Windows.

3. **`python tools\log_check.py --dir logs`**
   Expected: `stamp OK · golden_hash OK · tests 388/0 · selftest PASS · ui_audit PASS ·
   allocations 0 · reopen_leak 0 · realtime_x ~1059 (SOFT, 3.4× the sandbox — expected, it is a
   real machine)`. The only remaining MISSING rows should be the five P2/P3 fields
   (`soak_xruns`, `dispatch_*` ×4).

4. **Then continue the original run sheet:** P2 (`cd tools\dispatch-bench && cargo run --release`,
   send the `C. VERDICT` block + both `D. PER-CALL COST` lines), start the P3 soak and let it run
   unattended, P4 after it, P5's one-liner.

## Send back

- the log_check table (and raw `gates.log` sections for any `[FAIL]`);
- `logs\build.log` — or `logs\probe.log` if step 1 died again;
- `echo %NUMBER_OF_PROCESSORS%` — one number, for #66's record (the race needed a machine with
  enough cores to run all eight tests at once; yours is the first that could).

## Caution (#71)

**Do not clone the GitHub repo onto this machine — keep syncing by zip.** The published repo
carries a stray byte-identical duplicate, `crates\sparq-module-api\src\decode (2).rs` (a Windows
copy artefact); it is harmless to the compiler but poisons the build stamp to `69f/1097801B`,
which log_check would read as a permanently stale binary. Your tree is provably clean — your exe
stamps `68f/1070593B`. If any other machine cloned the repo, delete that one file before building.
