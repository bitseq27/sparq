# WO-006 increment 1.3 — SATURN run sheet (the `test004` re-run)

**Sync bundle:** `sync-wo006-inc13.zip` (10 entries) → extract at the repo root
`Q:\morphosis\code\sparq`, overwriting. **Prerequisite:** `sync-wo013-inc3.zip` must be applied
first if it has not landed yet (this bundle stacks on it; `scripts\synccheck.bat` names any file
that did not land, per defect #78's remedy). **The build stamp changes**: `build.bat`'s guard
forces exactly one rebuild — defect #49's remedy working, not a failure.

Companion to the build-log entry in `PHASE0-WORKORDERS.md` (search **increment 1.3**) and
`docs/hal/windows-notes.md` §4c.

---

## 0. What this increment is — and why test004 failed on 2026-09-24

Your `test004` run split in two, and both halves were informative:

* **[03] caps PASSED** — the endpoints now list `exclusive 96000`. Defect #77's format fix (the
  four-rung ladder: f32 → i24-in-32 → i32 → i16) is verified on the device that raised it.
* **[04] exclusive play FAILED** on every rung with `HRESULT 0x88890020` =
  `AUDCLNT_E_INVALID_DEVICE_PERIOD`. **Defect #79:** the open asked the driver for sparq's
  *block* (64 frames = **666.7 µs** at 96 kHz) as the exclusive *device period* — the UMC 204HD's
  engine runs at **10 ms** (960 frames), the number the caps line had been printing as
  `hw period 10.000 ms` all along. The format probes could not catch it (`IsFormatSupported` takes
  no period); only `Initialize` tells the truth.

**The fix:** the open now asks `GetDevicePeriod` first and tries a ladder of *periods* (block
period clamped to the driver minimum → driver default), one fresh `IAudioClient` per candidate,
with the documented alignment two-step per candidate. On the UMC 204HD min = default = 10 ms, so
the ladder is a **single candidate: the driver's own number**. Nothing else moved: the pump's FIFO
already decouples device period from sparq block — the 2 h *shared* soak of 09-24 ran exactly this
ratio (10 ms period, 64-fr blocks, 10 798 597 blocks, 0 xruns), so the exclusive path inherits a
transport shape that is already proven at duration on this machine.

The decision arithmetic is **pure data in `sparq-kernel::hal::period`, unit-tested on Linux** —
including a test with your exact device shape (64 fr @ 96 kHz, default = min = 10 ms ⇒ one 10 ms
candidate). The WASAPI side is compile-verified by the MSVC cross-lint cells, as every WASAPI
increment before it; **the device run below is what makes it verified**.

## 1. The run

```bat
scripts\test004.bat
```

Unchanged contract: run it, answer the prompts, send back **one file** — `test004.log` from the
repo root. What PASS looks like now:

* **[03] caps** — as before: `exclusive 96000` on the Behringer endpoints.
* **[04] exclusive tone** — the open line must now name **both** truths:
  `… i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr` — and the
  10 s 220 Hz tone must be **audible**. (If the period reads differently, the log line is still
  the truth — `GetBufferSize` after `Initialize` — send it back either way.)
* **[05] unplug mid-run in exclusive** — clean `Removed` state, dev-err 1, 0 xruns, recovery tone
  after re-plug (the shared-mode criterion passed on 09-24; this is its exclusive twin).
* **[07] the acceptance itself** — the **2 h zero-xrun soak at 96 kHz/64 exclusive**. When it
  finishes clean: **WO-006 acceptance closes** and ADR-008's exit condition fires — the cpal
  bootstrap gets deleted and the HAL becomes `play`'s default (that is the next increment, not
  this run).

If **[04] still refuses**, the error now carries a **device-period probe table** (each candidate
with its HRESULT) *plus* the endpoint's reported default/minimum — that table is the diagnosis;
send the log back either way. A second `INVALID_DEVICE_PERIOD` against a 10 ms ask would mean the
driver wants a number it is not reporting, and the table will say so.

## 2. Send back

* `test004.log` (repo root) — the one file.
* If the soak ran: its section is inside the same log (the script tees everything).
* Optional but wanted, unchanged asks: the dispatch-bench numbers for ADR-009
  (`cd tools\dispatch-bench && cargo run --release` → the `C. VERDICT` block and both
  `D. PER-CALL COST` lines), and `logs\ui.log` + the real-finger/DPI items from the
  WO-013 inc-3 run sheet (`test006.bat`) whenever a window session is open anyway.

## 3. Known limits, stated before you find them

* The exclusive latency pipeline is now **block + device period** (≈ 0.67 ms + 10 ms on this
  interface) — the honest floor of a 10 ms engine. Lower-latency exclusive on the UMC 204HD is a
  driver question, not a sparq one; the log's latency report reads the true period, never an
  invented number.
* Friendly names still fall back to `<endpoint N>` (`E_ACCESSDENIED` on the property store, #75) —
  cosmetic; the registry path reads names fine and increment 2 carries the STA retry.
* `IAudioClock` drift stays `SUSPECT`-labelled (#76 decoded it: buffer-step advances per event
  tick); throughput accounting uses delivered frames, which stayed truthful in every run.
