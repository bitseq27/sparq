# WO-006 increment 1.4 — SATURN run sheet (the `test004` re-run, third attempt)

**Sync bundle:** `sync-wo006-inc14.zip` (15 entries) → extract at the repo root
`Q:\morphosis\code\sparq`, overwriting. **Prerequisite:** the chain through
`sync-wo008-inc6.zip` must be applied — your 2026-09-28 build line already proved it is
(`sync_check: OK - 149 files match sync wo008-inc6`). **The build stamp changes**
(`src 92f/2001521B`): `build.bat`'s guard forces exactly one rebuild — defect #49's remedy
working, not a failure.

Companion to the build-log entry in `PHASE0-WORKORDERS.md` (search **increment 1.4**) and
`docs/hal/windows-notes.md` §4d.

---

## 0. What attempt 2 proved — and what this increment changes

Your 2026-09-28 run was the most informative failure yet. Three things happened:

* **Exclusive OPENED for the first time** — `i24-in-32 (converting)` at 96 kHz on the UMC 204HD.
  The format ladder (#77) and the period asking (#79) both did their job. The caps checkpoint,
  the shared unplug → `Removed`, the re-plug recovery, the **2 h shared soak (0 xruns over
  719 895 wakes)** and all three conformance suites passed.
* **The open landed at 288 fr (3.00 ms)** — the driver's reported minimum / alignment
  granularity — and the driver could not *sustain* what it had *accepted*: **one ~33 ms stall
  every ~62.5 ms** (159 late wakes in 10 s), half throughput (≈ 49.1k frames/s against a 96 kHz
  negotiation), drift −49 %, and no clean tone. **Defect #89.** The pump kept every promise it
  could see (0 budget overruns, 0 FIFO starvations) — the device simply stopped asking for data
  on its own 3 ms cadence, while the same machine's shared 10 ms engine ran 2 h clean.
* **The script's own summary then misdiagnosed it** — "exclusive still refused" when exclusive
  had opened and run rough (**defect #90**, fixed: the summary now tells the two apart).

**The fix (all three doors the 3 ms ask could have come through, closed):**

1. **Default-first ladder.** When a driver reports a minimum coarser than the sparq block, its
   minimum is not a promise — the open now asks the driver's **default period first** (the
   number its engine actually runs), the minimum only as a fallback.
2. **The alignment two-step rounds UP.** If `Initialize` answers `BUFFER_SIZE_NOT_ALIGNED`, the
   re-ask is your original period rounded up to the driver's granularity (10 ms ask, 288 fr
   granularity → **1152 fr = 12 ms**), never the granularity itself.
3. **The silent-shrink guard.** If a driver accepts an ask and then allocates seriously less
   (< ¾ of it), that allocation is treated as the granularity: one rounded-up re-ask, and if
   that fails the open line prints the mismatch (`(3.00 ms, ask 960 fr)`) instead of dressing it
   as a clean negotiation.

## 1. The run

```bat
scripts\test004.bat
```

Same prep discipline as attempt 2 (UMC default + powered, both exclusive boxes ticked, every
other audio app closed, High-performance plan, monitors down). What PASS looks like now:

* **[03] caps** — unchanged: `exclusive 96000` on the Behringer endpoints.
* **[04] exclusive tone** — the open line must read
  `… i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr`
  **or** `… device period 1152 fr (12.00 ms) …` if the driver enforces its 288 fr alignment at
  `Initialize`. Both are the driver's own engine class. The 10 s tone must be **clean** and
  `play: PASS (0 xruns …)`. **A 288 fr (3.00 ms) period is the #89 signature — if you ever see
  one again, the log now says which door it came through; send it back.**
* **[05]/[06]** — with [04] clean, these now run **in exclusive**: unplug mid-run → clean
  `Removed`, dev-err 1, no crash; recovery tone after re-plug.
* **[07] the acceptance itself** — the **2 h zero-xrun soak at 96 kHz/64 EXCLUSIVE**. When it
  finishes clean: **WO-006 acceptance closes** and ADR-008's exit fires (cpal bootstrap
  deleted, HAL becomes `play`'s default — the next increment, not this run).

If [04] fails again, the log carries the diagnosis by construction: the period probe table
(every candidate with its HRESULT), the opened-period line with the ask when they differ, and
the late-wake/jitter/drift triple that decoded attempt 2. Send it back either way.

## 2. Send back — now the COMPACT copy

The script ends by making **`test004-digest.log`** beside the full log (it also deletes a stale
one first, so the file you send is always from this run). **Send that one file.** It is a copy,
not a rewrite: every verdict, banner, open line, probe table and operator answer is
byte-identical; exactly three classes of repetition collapse — the `[t+ Ns]` periodic status
runs (first 2 + last 2 of each run kept, the middle replaced by ONE count line carrying the
blocks/xrun ranges), the cargo build chatter, and runs of identical lines. Attempt 2's log
digested to **54 % smaller** with every diagnostic line intact. The header states exactly what
was collapsed. The full `test004.log` stays on disk — if a diagnosis ever needs a line the
digest collapsed, that is one `type` away, and the run sheet asks for the full file instead.
(`scripts\digest.bat` digests every `test*.log` and `logs\*.log` on demand; `test006.bat` and
`gates.bat` make their own digests at the end of a run.)

Optional but wanted, unchanged asks: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST`
lines (ADR-009), `logs\ui.log` + the WO-013 items (`test006.bat`, whose send-back is now
`test006-digest.log` + `logs\ui-digest.log` when they were made), and the WO-009 device boxes
(LISTENED tempo sweep, 30-min drift run) when the HAL is free.

## 3. Known limits, stated before you find them

* If the open lands at 1152 fr, the exclusive latency pipeline reads ≈ 0.67 ms + 12 ms — the
  honest floor of a driver that enforces 3 ms alignment and cannot sustain 3 ms periods. Lower
  latency on this interface is a driver question, not a sparq one; the log reads the true period
  from `GetBufferSize`, never an invented number.
* Friendly names still fall back to `<endpoint N>` (`E_ACCESSDENIED`, #75) — cosmetic;
  increment 2 carries the STA retry.
* `IAudioClock` drift stays `SUSPECT`-labelled on this endpoint (#76 decoded it: buffer-step
  advances per event tick — the +1 199 624 ppm in your shared soak is that signature to four
  digits). Throughput accounting uses delivered frames and stayed truthful in every run — it is
  what caught #89.
* The digest tool needs Python 3.11+ on PATH (`python` or `py`). Without it the script says so
  and asks for the full log instead — the run itself never depends on the digest.
