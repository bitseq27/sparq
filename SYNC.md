# Sync manifest — WO-006 increment 1.4 (defect #89: the exclusive period the driver ACCEPTED and could not SUSTAIN — plus the compact log digest the operator asked for)

**Current bundle: `sync-wo006-inc14.zip` (15 entries, listed below).** Applies on top of
**`sync-wo008-inc6.zip` — which is APPLIED**: test004 attempt 2's own build line proved the whole
chain landed (2026-09-28: `sync_check: OK - 149 files match sync wo008-inc6, src 92f/1986012B`).
Extract at the repo root `Q:\morphosis\code\sparq`, overwriting. No drift-repair copy step.

**The stamp changes: `src 92f/2001521B`** (`SYNC-STAMP.txt` regenerated for a **151-file**
covered set — two new files join it: `tools/log_digest.py` and `scripts/digest.bat`; the `src`
fingerprint's file count is unchanged at 92, its byte count moved because `hal/period.rs` and
`hal/wasapi.rs` grew. `Cargo.lock` stays SOFT and is NOT in the zip; SATURN keeps its own.)

## What this is

ONE increment from the 2026-09-28 fifth session, driven by **test004 attempt 2** — the most
informative device run yet. Attempt 2 **opened WASAPI exclusive for the first time**
(`i24-in-32 (converting)` @ 96 kHz on the UMC 204HD — #77's format ladder and #79's period
asking both did their job; caps, shared unplug → `Removed`, recovery, the 2 h shared soak and
all three conformance suites PASSED) — and then the stream **stalled**: the open landed at
**288 fr (3.00 ms)**, the driver's reported minimum / alignment granularity, and the driver
could not sustain what it had accepted. The numbers, which all agree (arithmetic in
`docs/hal/windows-notes.md` §4d): 1718 wakes in ~10.07 s splitting into ~1559 at ~3.0 ms plus
~159 at ~33.2 ms — one ~33 ms stall every ~62.5 ms — half throughput (7732 blocks ≈ 49.1k fr/s
against a 96 kHz negotiation), drift −491 656 ppm, no clean tone, rc 1, with **0 budget overruns
and 0 FIFO starvations** (the pump kept every promise it could see). The control experiment ran
in the same session: the shared 10 ms engine on the same endpoint soaked **2 h, 719 895 wakes,
0 xruns, max jitter 12.04 ms**. **Defect #89: the third lying-`min` face — `Initialize` ACCEPTS
the period and the engine cannot SUSTAIN it**, which no open-path probe can see. The attempt-1
log copy in the repo is truncated, so which door produced the 3 ms ask is not pinnable from the
evidence; the fix therefore closes all three doors, each as pure data in `hal/period.rs` where
Linux unit-tests it (the null backend's discipline applied to a negotiation, as in inc 1.3):

1. **Default-first for coarse engines.** When the driver's reported minimum sits ABOVE the sparq
   block period — a driver saying it needs an engine coarser than the block — its minimum is not
   a promise: the ladder now asks the **default period first** (the number its engine actually
   runs — the shared 10 ms engine is the same-session proof this machine sustains it), the
   min-clamped ask demoted to fallback. Sub-block engines keep the honest low-latency ask first
   (#79's shape unchanged — a liar there refuses at `Initialize`, which the ladder survives);
   four of inc 1.3's seven unit pins pass untouched, and the fifth — which pinned the UMC's own
   shape the WRONG way round — is replaced in place by
   `the_defect_89_device_asks_its_default_before_its_min` (`[10 ms, 3 ms]`).
2. **The alignment two-step rounds UP.** On `BUFFER_SIZE_NOT_ALIGNED`, `GetBufferSize` reports
   the driver's granularity; inc 1.3 re-asked the granularity itself (the documented recipe's
   literal reading — and a way a 10 ms ask could silently become the 3 ms open). Now
   `align_up_frames` rounds the ORIGINAL ask up to whole units: 960 fr ask × 288 fr granularity
   → **1152 fr = 12.000 ms** — aligned AND default-class. Sub-granularity asks still land on one
   unit (inc 1.2's two-step behaviour, pinned).
3. **The silent-shrink guard.** A driver that ACCEPTS an ask and then allocates seriously less
   (< ¾, `allocation_seriously_shrunk`) is reporting its granularity through the allocation:
   one rounded-up re-ask on a fresh client; if that refuses, the shrunken open is KEPT and the
   open line prints the mismatch — `device period 288 fr (3.00 ms, ask 960 fr)` — rather than
   dressing it as a clean negotiation (`Negotiated.ask_frames`, exclusive only; the shared
   engine picking its own buffer is documented behaviour, not a mismatch).

The pump needed **no change** (Period ≠ block; the FIFO absorbs any period the ladder lands on).
Expected attempt-3 open line: **`device period 960 fr (10.00 ms)`**, or **`1152 fr (12.00 ms)`**
if the driver enforces its 288 fr alignment. **A 288 fr (3.00 ms) period must never open
silently again.**

**Defect #90 rode along — the acceptance script's own honesty:** attempt 2's summary printed
"exclusive still refused — the [04] probe table is the diagnosis" when exclusive had OPENED and
run rough (`EXCL_OK` keyed off rc alone; no probe table was in the log, correctly), and the
banner still announced "increment 1.2" two increments later. `test004.bat` now captures
`EXCL_OPENED` (findstr on the [04] output), distinguishes refused from opened-but-not-clean in
the summary, names #89's runtime signature in its hints, and names both legal periods (960 fr /
1152 fr) plus the 288-fr signature in its [04] expectation text.

**And the operator's ask, shipped in the same bundle: the compact log copies.** The device logs
had grown to the point where a 2 h soak's 240 identical status lines drowned the dozen lines
that decide the run (attempt 2's `test004.log`: 67 569 B). **`tools/log_digest.py`** writes
`NAME-digest.log` beside any log: a COPY, not a rewrite — every line byte-identical except
inside runs of exactly three classes: `[t+ Ns]` periodic status runs (first 2 + last 2 kept, the
middle replaced by ONE count line carrying the blocks/xrun ranges), cargo build chatter, and
runs of 4+ identical lines. The header states exactly what was collapsed; `--self-test` (16
checks) proves every class fires and every verdict line survives; re-digesting a digest is
refused (idempotence guard). Attempt 2's log digests to **30 994 B (−54 %)** with every
diagnostic line intact. `test004.bat`, `test006.bat` and `gates.bat` make their digests at the
end of a run (and delete stale ones first, so the send-back file is always from THIS run);
`scripts\digest.bat` sweeps every `test*.log` and `logs\*.log` on demand. **The digest is the
send-back artefact; the full log stays on the device.** Without Python on PATH the scripts say
so and ask for the full log — no run ever depends on the digest.

## Measured in sandbox

**757 tests** (was 752; +6 `hal/period.rs` unit gates, 1 replaced in place — ALL RUNNING ON
LINUX, including the exact attempt-2 device shape pinned twice) · 0 failed, 1 ignored · fmt
clean · clippy clean in every runnable cell: default workspace, `bootstrap-audio`, `ui`, native
`ui-window`/gles (`-j 1`, dev debuginfo off), and all six MSVC cells (module-api, music,
kernel+hal-wasapi, app+hal-wasapi, kernel default, app default — the two `windows`-crate OOM
cells remain documented-unrunnable) · release golden tests **2/2, hashes unchanged**
(`ba577186c988db21`) — the offline path never sees a device period, and `hal/period.rs` feeds
only the `cfg(windows)` exclusive open, so every golden is unchanged **by construction** ·
5 python gates clean · `log_digest --self-test` **16/16** · `log_check --self-test` **25/25**
(BASELINE moved with the seal — 757 / `src 92f/2001521B`, #83's discipline) · `sync_check
--write` → `--quiet` clean → `--self-test` 11/11 failable · `selftest --golden` and `ui --audit`
NOT re-run this session: the release-app build was cut short by the sandbox's memory ceiling and
the diff touches neither surface (windows-gated HAL open + tools + docs); SATURN's `gates.bat`
runs both and its log_check comparison will say so if that was wrong.

## Files in this zip (15)

Covered by the stamp — 8:

```
crates/sparq-kernel/src/hal/period.rs   (the #89 ladder: default-first for coarse engines; align_up_frames / frames_to_100ns / period_100ns_to_frames / allocation_seriously_shrunk — all pure, all pinned; 12 unit tests)
crates/sparq-kernel/src/hal/wasapi.rs   (open_exclusive: the round-UP two-step, the silent-shrink retry, Negotiated.ask_frames, the honest open line; comments carry #89's story)
scripts/digest.bat                      (NEW — sweep every test*.log and logs\*.log into digests)
scripts/gates.bat                       (digest hook after the log is saved; cannot change the gates verdict)
scripts/test004.bat                     (#90: EXCL_OPENED, refused vs opened-but-not-clean summary, current banner/hints/expectations; digest hook replaces the send-back line)
scripts/test006.bat                     (digest hook for test006.log + logs\ui.log)
tools/log_check.py                      (BASELINE moved: 757 tests / src 92f/2001521B; fixture stamp)
tools/log_digest.py                     (NEW — the compactor; --self-test 16 checks; idempotence guard)
```

Documents — 7 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt
WO006-INC14-RUN-SHEET.md   docs/hal/windows-notes.md
```

Heading = list = contents = **15** (8 + 7), checked against the zip's namelist below.

## What SATURN runs

The standing order stacks, with baselines moved (`log_check.py` in the bundle already expects
them). Apply this bundle, then:

1. `scripts\test004.bat` — the WO-006 exclusive acceptance, **attempt 3** (still the 🔴
   blocker; pass shape and both legal open lines in `WO006-INC14-RUN-SHEET.md`). Same prep
   discipline as attempt 2.
2. `scripts\test006.bat` — the WO-013 window session with steps F–I (unchanged by this bundle).
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 757 passed / 0 failed /
   1 ignored**, stamp `src 92f/2001521B`, `sparq modules --strict` lists **17**, selftest
   **PASS (9 gates)**, `ui --audit` **PASS (25 smokes)**.
4. Evidence, **with the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free — the
   two WO-009 device boxes (a LISTENED tempo sweep and the 30-min drift run).
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero xruns).

**Wanted back — the DIGESTS, not the full logs: `test004-digest.log`, `test006-digest.log`
(with the F–I answers), `gates-digest.log`, `logs\ui-digest.log`.** Each script makes its own at
the end of the run; `scripts\digest.bat` catches anything older. The full logs stay on the
device — if a diagnosis ever needs a collapsed line, the run sheet says which full file to send
instead.

## Superseded bundles

Applied chain (each on top of the previous, newest first): `sync-wo008-inc6.zip` (17 — APPLIED,
proven by attempt 2's build line 2026-09-28), `sync-wo014-inc6.zip` (13 — APPLIED, same
evidence), `sync-wo013-inc5.zip` (20 — APPLIED, user-confirmed 2026-09-27),
`sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22), `sync-wo014-inc4.zip` (12),
`sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19), `sync-wo008-inc4.zip` (38),
`sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15), `sync-wo006-inc13.zip` (10),
`sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
LATER.md
PHASE0-WORKORDERS.md
SYNC-STAMP.txt
SYNC.md
WO006-INC14-RUN-SHEET.md
crates/sparq-kernel/src/hal/period.rs
crates/sparq-kernel/src/hal/wasapi.rs
docs/hal/windows-notes.md
scripts/digest.bat
scripts/gates.bat
scripts/test004.bat
scripts/test006.bat
tools/log_check.py
tools/log_digest.py
```
