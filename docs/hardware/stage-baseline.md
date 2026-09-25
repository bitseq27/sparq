# WO-001 — stage baseline (fill in on the real hardware)

**Purpose:** every latency, ergonomics and reliability claim in this project must be measured on the machine that will be on stage. This document is the record. It is also the pre-flight reference (plan §12.4) and the input to ADR-004's amendment.

**Machine / date / OS build:** ______________________
**sparq build (commit):** ______________________

---

## 1. Hardware decisions (D-11, D-12)

### Touch device (D-11)

| Candidate | Screen / resolution | Touch points | Refresh | Palm rejection | Glove response | Weight / mount | Verdict |
|---|---|---|---|---|---|---|---|
| | | | | | | | |
| | | | | | | | |

**Chosen device:** ______________________  **Why:** ______________________
**Can it be the only screen?** (no keyboard/mouse needed to play a set) ☐ yes ☐ no — if no, what's the fallback?

### Audio interface (D-12)

| Item | Value |
|---|---|
| Model / firmware / driver version | |
| Connection (USB-C / TB / PCIe) | |
| Channels in / out (physical) | |
| WASAPI exclusive: rates supported | |
| WASAPI exclusive: max channels | |
| ASIO: driver name + version | |
| ASIO: rates / max channels | |
| Shared-mode behaviour (does it hijack?) | |
| Reported hardware latency at 64 / 96 k | |
| Multi-device aggregation feasible? | |

---

## 2. Latency matrix (measure, don't estimate)

Method: loopback impulse from output 1 → input 1, measured inside sparq (`xtask measure-latency`) and cross-checked against the device's reported latency.

| Rate | Block | Backend | Round trip (ms) | Reported HW latency (ms) | Xruns in 10 min | Notes |
|---|---|---|---|---|---|---|
| 44 100 | 64 | WASAPI excl | | | | |
| 48 000 | 64 | WASAPI excl | | | | |
| 48 000 | 128 | WASAPI excl | | | | |
| 96 000 | 32 | WASAPI excl | | | | |
| 96 000 | 64 | WASAPI excl | | | | |
| 96 000 | 64 | ASIO | | | | |
| 96 000 | 128 | ASIO | | | | |
| 48 000 | 64 | WASAPI shared | | | | |

**Target:** < 10 ms round trip at 96 k / 64 (plan Appendix C). **Achieved:** ______ **Gap and plan if missed:** ______________________

## 3. DPC / ISR latency

| Condition | Max DPC (µs) | Max ISR (µs) | Worst offender |
|---|---|---|---|
| Idle, stage mode off | | | |
| Idle, stage mode on | | | |
| GPU load (full-bleed visual running) | | | |
| Network active (sync/streaming) | | | |
| USB hub with sensors attached | | | |

**Budget:** worst case < ~500 µs in stage mode. **Offenders found and mitigations applied:** ______________________

## 4. CPU / memory headroom

| Test | Result |
|---|---|
| Typical Phase 0 patch (17 modules), 96 k / 64, % of one core | |
| 200-module null graph, block time p50 / p99 / max (µs) | |
| Dispatch overhead: 100 null modules, 64-sample block (µs) — target < 20 µs | |
| Memory: arenas reserved at patch load (MB) / budget (MB) | |
| Frame time with full-bleed sonogram + 50-node canvas (ms) | |

## 5. Stage-mode preset (scriptable + verifying)

Each item is applied by sparq at startup in Perform mode and **reported pass/fail** — not assumed.

| # | Item | Applied how | Verified how | Status |
|---|---|---|---|---|
| 1 | High-performance power plan | `powercfg` set active scheme | query active scheme | |
| 2 | Processor boost mode explicit | `powercfg` attribute | query | |
| 3 | Core parking disabled | `powercfg` | query | |
| 4 | Sleep / hibernate / display-off disabled | `powercfg` | query | |
| 5 | NIC power saving + EEE off | driver property / `Set-NetAdapterAdvancedProperty` | query property | |
| 6 | Wi-Fi adapter power management off | device power setting | query | |
| 7 | USB selective suspend off | `powercfg` | query | |
| 8 | GPU driver version pinned + HAGS setting recorded | registry / driver query | compare to recorded good version | |
| 9 | Defender exclusions: project dir, asset store, modules dir | `Add-MpPreference` | query exclusions | |
| 10 | Notifications + focus-stealing suppressed | focus assist / policy | query | |
| 11 | Windows Update paused / no auto-reboot | policy query | query | |
| 12 | Audio device sample rate matches project (no hidden SRC) | device query vs project | compare | |
| 13 | Exclusive-mode access to the performance device | open attempt | open succeeds | |
| 14 | sparq process priority + MMCSS task active | runtime query | diagnostics overlay shows it | |

**Result at startup must be printed as `STAGE MODE verified N/14` in the top bar** (as drawn in both mockups).

## 6. Touch ergonomics log

| Test | Result | Notes |
|---|---|---|
| Wire connection one-handed, port capture 24 px | | |
| Long-press (350 ms) fires reliably without triggering drag | | |
| Two-finger pan/pinch while a hand rests on the screen | | |
| Second-finger fine resolution (×10) during a drag | | |
| Macro pad hit rate, 20 consecutive eyes-up taps | | |
| Glove operation | | |
| Damp fingertips | | |
| 100 / 125 / 150 / 200 % DPI: layout + touch coordinate accuracy | | |
| Mixed-DPI dual monitor (device + projector) | | |
| 3 m legibility of Perform-mode numerals | | |

## 7. Known driver / OS quirks (record everything, it will save you at a venue)

| # | Symptom | Cause | Workaround |
|---|---|---|---|
| 1 | | | |
| 2 | | | |

---

**Sign-off:** ADR-004 amended with the chosen device + interface, and this file committed with real numbers, is the exit condition for WO-001.
