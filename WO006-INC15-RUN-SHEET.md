# WO-006 increment 1.5 — SATURN run sheet (the `test004` re-run, fourth attempt)

**Sync bundle:** `sync-wo006-inc15.zip` (15 entries) → extract at the repo root
`Q:\morphosis\code\sparq`, overwriting. **Prerequisite:** the chain through
`sync-wo006-inc14.zip` must be applied — your 2026-09-28 evening build line already proved it is
(`sync_check: OK - 151 files match sync wo006-inc14`). **The build stamp changes**
(`src 92f/2015804B` — the exact value is in the bundle's `SYNC-STAMP.txt`): `build.bat`'s guard
forces exactly one rebuild — defect #49's remedy working, not a failure.

Companion to the build-log entry in `PHASE0-WORKORDERS.md` (search **increment 1.5**) and
`docs/hal/windows-notes.md` §4e.

---

## 0. What attempt 3 proved — and what this increment changes

Your 2026-09-28 evening run confirmed the increment-1.4 fix **and falsified its remaining
assumption**, which is the most useful kind of failure:

* **The period ladder landed exactly where designed.** The open line read
  `i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr` — the driver's
  default period, asked first, granted, no shrink note. Defect #89's open-path fix is
  device-verified; nothing about it needs revisiting.
* **The stall stayed — and it does not care about the period.** Same signature as attempt 2 at a
  different period: ~16 stalls per second (~158 late wakes / 10 s), each swallowing ~3 cadences
  (~40 ms), half throughput (7756 blocks ≈ 49.4k fr/s against a 96 kHz negotiation), no clean
  tone. The arithmetic (`windows-notes.md` §4e): **delivered frames = wakes × buffer** held to
  within one block on BOTH attempts (517 × 960 and 1718 × 288). In event-driven exclusive the
  API forbids banking more than one period (`hnsPeriodicity` must equal `hnsBufferDuration`), so
  every swallowed cadence is audio the device can never receive. The shared engine runs the
  opposite shape on the same endpoint — 2112 fr buffer over a ~960 fr cadence — and soaked 2 h
  clean. **Defect #92**, open, device-side.
* **The 96 kHz was never YOUR request — it was a probe bug.** `play` asked its standing 48 kHz;
  the HAL "adjusted" it up because caps listed `exclusive 96000` — and that lone rate was an
  artefact: the exclusive probe only asked at rates the SHARED (engine) probe had accepted, and
  the engine accepted nothing but its 96 kHz mix. The driver — a "UMC 204HD **192k**" — was never
  asked whether it does 48 kHz exclusive. **Defect #91**, fixed: the exclusive sweep is its own
  probe now, at every standard rate, against the driver.

**The fixes in this bundle:**

1. **The exclusive rate envelope is its own probe** (`capabilities`): full standard-rate sweep ×
   the four format rungs, asked of the driver — never sieved through the engine again.
2. **Adjustment consults the envelope the open negotiates in** (`exclusive_rate_for`): play/soak
   on `wasapi-exclusive` adjust within the exclusive list (nearest rate, ties to the lower).
   Your 48 kHz ask now survives to `Initialize`.
3. **Defect #76 shipped: drift = delivered frames vs wall.** The pump counts frames the device
   ACCEPTED (every `ReleaseBuffer`, preroll included); the `IAudioClock` binding is deleted — its
   `GetPosition` advanced in buffer-sized steps per event tick (+1.2 M ppm on a soaked-clean
   shared stream; §4b). A healthy run now reads within a few hundred ppm of zero in BOTH share
   modes, and attempt 3's shape reads −49 % as a measurement. The old "implausible clock
   (virtual/RDP endpoint)" excuse text is gone. Suspect drift (> ±1 %, after a 2 s trust window)
   now fails `play`/`soak` verdicts in its own right.

## 1. The run

```bat
scripts\test004.bat
```

Same prep discipline as attempt 3 (UMC default + powered, both exclusive boxes ticked, every
other audio app closed, High-performance plan, monitors down). What PASS looks like now:

* **[03] caps** — the Behringer endpoints list **multiple** exclusive rates — expect
  `exclusive 44100,48000,88200,96000,176400,192000` (or the subset the driver truly accepts;
  any subset containing 48000 unblocks [04]). A still-lone `exclusive 96000` means the driver
  refuses 48 kHz exclusive at probe time — a finding; send the caps block back.
* **[04] exclusive tone** — **no `adjusted` line** (the 48 kHz ask survives), and the open line
  reads `… 48000 Hz · 2 ch · i24-in-32 (converting) · device period 480 fr (10.00 ms) · sparq
  block 64 fr` (or the driver's own default if it reports a different one at 48 kHz). Because
  nothing was adjusted, the `NEGOTIATED …` proof line does NOT print — its absence is the good
  case. Latency reads ~10.67 ms. The status lines' drift sits within a few hundred ppm of 0,
  late wakes 0, and the verdict is `play: PASS (0 xruns, 0 allocations, clean stop)` with the
  tone clean for the full 10 s.
* **[05]/[06] unplug + recovery** — run in exclusive when [04] passed: `Removed` state, clean
  stop, no crash; recovery tone audible.
* **[07] the acceptance soak** — the prompt defaults to **48000**, the rate [04] just proved;
  `96000` is offered as the stretch the WO text named. See §2 for the acceptance wording.

## 2. The acceptance wording, and why the rate prompt exists

WO-006's original box says "zero xruns in a 2-hour soak at **96 kHz** / 64 samples on the stage
device". That wording predates two discoveries: (a) the lone-96k exclusive envelope was a probe
bug, not the driver's truth (#91); (b) the exclusive event path stalls at 96 kHz on this driver
at every period tried so far (#92), while the same hardware runs 96 kHz mix flawlessly through
the shared engine. The amended acceptance this bundle proposes — **ratified or struck by the next
session against your attempt-4 evidence, not by this run sheet**: *the 2 h zero-xrun exclusive
soak at the rate the device proves, which on the UMC 204HD is expected to be 48 kHz; the 96 kHz
stretch run stays welcome as quirk-table evidence either way.* The prompt exists so ONE device
session can serve both readings: run the default first (it is the acceptance candidate); if it
passes and the machine is free, re-run [07] at 96000 and let the digest say whether #92 is
rate-coupled.

## 3. Reading a failure (the digest is self-diagnosing now)

* **`adjusted rate 48000 -> 96000 … for exclusive operation`** — the driver refused 48 kHz
  exclusive in the probe. [03]'s caps block is the evidence; send it back.
* **Open line shows 48 kHz but the triple returns** (late wakes on status lines, drift near
  −50 %, `play: NOT CLEAN`): the #92 stall is **rate-independent**. The drift line is now
  delivered-vs-wall truth, so −50 % means the device consumed half the negotiated rate — not a
  clock artefact. Send the digest back; the next lever is push mode (windows-notes §5), its own
  increment.
* **Drift suspect but cadence quiet** (no late wakes, drift > ±1 % after 2 s): the device
  consumes at a steady wrong rate — the rate lie in its pure form. The verdict line says
  `throughput did not match the negotiated rate`; send it back, the caps block with it.
* **`play: NOT CLEAN — xruns 0, … state Stopped` with a suspect drift clause** — same as above;
  the clause only prints when the measurement fired.

## 4. After test004

The standing order continues (`SYNC.md` §"What SATURN runs"): `test006` (window session, steps
F–I), `gates.bat` — **expected 761 passed / 0 failed / 1 ignored**, stamp per the bundle's
`SYNC-STAMP.txt`, **17** modules, selftest **PASS (9 gates)**, `ui --audit` **PASS (25
smokes)** — and the pinned evidence renders (`mod-demo` 1 s `1621e1f65b1b64e1`, `drum-demo` 2 s
`f2303f13aa0cf299`, `demo` 2.8 s `53de3b1f3f40e3c9` — unchanged: inc 1.5 touches no render
path). Digests back, full logs stay on the device.
