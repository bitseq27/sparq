# HAL · Windows notes (WO-006)

Implementation notes, driver quirks, and the running log of what real hardware actually did.
Everything in the "measured" sections is evidence, not aspiration: an unmeasured claim lives in
the plan, not here.

**Status:** increment 1 — null + WASAPI exclusive/shared (event-driven) — **proven on hardware**
(SATURN physical, 2026-09-24: shared mode 0 xruns / 0 allocations across every run, the unplug
acceptance criterion passed, reopen-leak 0; see §4). **Increment 1.2 (2026-09-24, sandbox-built,
hardware-pending via test004):** the exclusive ladder gained integer rungs — f32 → i24-in-32 →
i32 → i16, all extensible — fixing defect #77 (USB DAC drivers refuse f32 exclusive; the
Behringer UMC 204HD 192k reported `exclusive none` while being exclusive-capable). The pump's
i16 conversion generalised to `DevFmt {F32, I16, I32}` (`pop_into_i32`, saturating, one
arithmetic serving 32-bit and left-justified 24-in-32); caps probe all four rungs per rate; a
refusal now carries the full four-rung probe table. MSVC cross-lint clean both cells, 418 Linux
tests green, fmt/unsafe/text-io/token gates clean — the same sandbox-verified-then-device-proven
path increment 1 walked. Increment 2: ASIO, full-duplex capture, round-trip measurement,
friendly-name registry fallback (#75), drift-without-GetPosition (#76).

---

## 1. What the HAL does on Windows

| Piece | Where | Note |
|---|---|---|
| Trait, capabilities, errors | `sparq-kernel/src/hal/mod.rs` | ADR-009's sketch, with `actual_config()` added so negotiated ≠ requested is always visible |
| Diagnostics | `sparq-kernel/src/hal/diag.rs` | xruns, log2-bucket histogram (p50/p99/max), wake jitter, device-clock drift vs wall, allocation counts — all relaxed atomics, snapshot from any thread |
| Null backend | `sparq-kernel/src/hal/null.rs` | paced (hybrid sleep+spin pump) and manual (deterministic stepping) modes; fault injection: unplug, xrun, pump stall |
| Conformance suite | `sparq-kernel/src/hal/conformance.rs` | the executable form of the trait review; every backend runs it (`sparq devices --conformance`) |
| WASAPI | `sparq-kernel/src/hal/wasapi.rs` | exclusive + shared, event-driven, allowlist entry 1 |
| RT discipline | `sparq-kernel/src/rt/thread.rs` | MMCSS Pro Audio, TIME_CRITICAL, ideal processor, working-set raise + pre-touch; allowlist entry 3 |

### WASAPI specifics worth knowing before reading the code

* **COM threading.** All interfaces are created on the control thread inside a persistent
  per-thread **MTA** init that is deliberately never released (interfaces must outlive function
  calls). The pump thread initialises its own MTA; ownership of the interfaces *moves* to the
  pump and returns on `join`, so exactly one thread ever holds them. If a future GUI host
  (WO-012) initialises STA on the same thread, `open` refuses with an explanation rather than
  misbehaving — the shell must use MTA on the audio-control thread.
* **`windows-sys`, not `windows`.** The high-level `windows` crate needs >1 GB of rustc memory to
  compile; CI and the dev sandbox have 1 GB. `windows-sys` ships free functions and POD types
  within budget but **no COM interfaces**, so the seven vtables in `wasapi.rs` are declared by
  hand against the SDK header slot order (ABI-frozen since Vista; each slot is commented with its
  header method). This is allowlist entry 1's "raw interface calls", made literal.
* **No `AvSetMmMaxThreadCharacteristicsW`.** The Win32 metadata binding for it is wrong in
  windows-sys 0.61 (`(PCWSTR, PCWSTR, *mut u32) -> HANDLE` vs the documented
  `(HANDLE, LPCWSTR) -> BOOL`) — calling it would pass garbage. The critical boost comes from
  `SetThreadPriority(TIME_CRITICAL)` after MMCSS registration instead (the miniaudio/cpal
  pattern). Revisit when the metadata is fixed; `RtReport` will show it.
* **Period ≠ block.** The device period P (exclusive: negotiated, alignment two-step possible;
  shared: the engine's, typically 10 ms) and the sparq block B are decoupled by a pre-allocated
  FIFO in the pump. The callback always sees exactly B frames; the device always gets exactly
  what it asked for. FIFO starvation at write time = counted xrun + silence.
* **Shared-mode formats.** Probe ladder: f32@requested-rate → f32@mix-rate → mix-as-is. A 16-bit
  mix (the common Windows default — the Phase A lesson) is converted in the pump (clamp to
  ±32767, symmetric). Anything neither f32 nor i16 is refused at `open` with the format tag in
  the message.
* **Exclusive alignment.** `AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED` triggers the documented two-step:
  `GetBufferSize` on the failed client → recompute the period → **fresh** `Activate` → re-
  `Initialize`. The first client cannot be reused (documented WASAPI behaviour). Since #89 the
  recompute **rounds the ask UP** to the reported granularity instead of adopting it: the
  granularity is the driver's smallest unit, not a period its engine can sustain (§4d). And a
  driver that ACCEPTS an ask yet allocates seriously less (< ¾) through `GetBufferSize` gets the
  same treatment once — the allocation is its granularity, told silently.
* **Failure semantics.** `AUDCLNT_E_DEVICE_INVALIDATED` anywhere → `StreamState::Removed`, pump
  exits, `stop()` still joins cleanly, `start()` refuses with guidance. Wait timeouts (4×period,
  ≥250 ms) count as late wakes and keep the loop alive; `WAIT_FAILED` fails the stream. Every
  HRESULT is classified once (`classify`) into the recovery strategy it belongs to.
* **Preroll.** The FIFO is primed to P+B before `Start`, and one full period is written to the
  device pre-Start where the stack accepts it. A refused preroll is logged and skipped (some
  shared stacks refuse; the first event then writes a full period — with a counted xrun if it
  starves).

## 2. Running it

```
scripts\hal.bat                     build + devices + caps + conformance + 2×10 s play + 2 min soak
sparq devices --caps                what every backend/device verifiably supports
sparq devices --conformance         the behavioural promises, measured (-60 dB signal)
sparq play --backend wasapi-exclusive --seconds 10
sparq play --backend wasapi-shared  --seconds 10
sparq soak --backend wasapi-exclusive --minutes 2 --rate 96000 --block 64
scripts\soak.bat 120 wasapi-exclusive     THE acceptance run (2 h, stage machine, idle)
```

`--backend wasapi` (bare) still routes to the cpal bootstrap: the proven path is never rerouted
by an unproven one. Once the acceptance soak passes, the HAL becomes the default and the
bootstrap is deleted (ADR-008's exit condition).

Read the diagnostics, in order of what they tell you:

* `rt setup NO-mmcss …` → the boost didn't apply: power plan, remote session, or policy. xruns
  under load are then expected and mean nothing about sparq.
* `xruns (late wakes N, budget overruns M …)` → late wakes = the pump missed the device's
  interrupt cadence (system/DPC noise); overruns = the *callback* exceeded its budget (our DSP).
  Different diseases, different cures — this split is the whole point of counting them apart.
* `drift ±ppm` → device clock vs wall clock. Real interfaces sit in the ±10..100 ppm range;
  large values on a *shared* stream can also mean the engine resampled.

## 3. Known-environment quirks (carried from Phase A + this increment)

| Environment | Behaviour | Consequence |
|---|---|---|
| RDP session ("Remote Audio") | single 44.1 kHz stereo endpoint; USB interfaces invisible; bursty 9–70 ms delivery | real-time claims are meaningless over RDP — verify DSP by offline render + golden hash, verify latency/xruns only at the physical machine (WINDOWS.md) |
| Windows shared mode | default mix format often 16-bit; buffer durations < ~3 ms get rejected or silently rounded | HAL asks ≥3 ms shared and reports `actual_config`; i16 mixes are converted, not refused |
| Exclusive mode | can be blocked per-device ("Allow applications to take exclusive control") or held by another app | classified as `Busy` with the settings path in the hint |
| Linux CI sandbox | paced null pump shows 1–2 late wakes per ~10 s (shared 2-core runner, sleep overshoots >2 ms) | expected: the *simulation* is at the mercy of the Linux scheduler; the WASAPI pump is event-driven and doesn't sleep. CI conformance runs with an xrun tolerance and says so |

## 4. Driver quirk log (fill in per interface tested — WO-006 task 7)

Template: one row per (machine, interface, backend, config) actually run.

| Date | Machine | Interface | Backend | Config | Result | Notes |
|---|---|---|---|---|---|---|
| 2026-09-21 | SATURN (Win10 19045, Zen 4) **over RDP** | RDP `Remote Audio`-class endpoints (6 render endpoints, no UMC204HD — RDP hides USB audio, as in Phase A) | wasapi-shared | default endpoint: mix **44100 Hz / 4 ch / f32 (extensible)**, `GetBufferSize` 970 fr (22 ms) | **Every plain-`WAVEFORMATEX` f32 probe refused** with `AUDCLNT_E_UNSUPPORTED_FORMAT (0x88890008)`; the **extensible mix verbatim was accepted**. Stream then ran 10 s: 6945 blocks, **0 allocations**, callback p50 2.0 µs / p99 8.2 µs / max 16.6 µs, `rt setup mmcss priority workset` | defects #38/#46 fixed: both format shapes are now probed at every ladder level (rungs 1–7); `Initialize` remains the arbiter, with the full probe table logged on refusal |
| 2026-09-21 | SATURN over RDP | same | wasapi-shared | — | **Event cadence ≈ 10 ms** (1005 wakes in 10 s, jitter avg 9.99 ms) while `GetBufferSize` claims 970 fr ≈ 22 ms | the engine delivers ~441-frame chunks regardless of buffer size — the Phase A "use the delivered period" lesson. Wake-jitter stats report the delivered truth; the latency estimate keeps using `GetBufferSize` and stays labelled `est` |
| 2026-09-21 | SATURN over RDP | same | wasapi-shared | — | **`IAudioClock` reads ~2.2× real time**: drift computed to +1 200 367 ppm while actual throughput (blocks × frames ÷ wall) was within ~1 % of 44.1 kHz | defect #45: beyond ±1 % (`DRIFT_SUSPECT_PPM`) the readout says `SUSPECT … implausible clock`. On such endpoints trust throughput and jitter, never drift |
| 2026-09-21 | SATURN over RDP | same | wasapi-exclusive | 44100/4 (caps-derived) | refused: `exclusive none` in caps; open returns an honest `Format` error quoting `AUDCLNT_E_UNSUPPORTED_FORMAT`; conformance verifies the refusal is never a silent reroute | expected: RDP endpoints do not grant exclusive mode. Correct behaviour, correctly classified — the physical machine decides |
| 2026-09-21 | SATURN over RDP | same | (all) | — | friendly names denied: `IPropertyStore::GetValue(PKEY_Device_FriendlyName)` → **`E_ACCESSDENIED (0x80070005)`**, vt 0 — while `GetId`, `Activate`, format probes and streaming all work | defect #40 explained: the RDP virtual device's property store denies reads. Cosmetic (ids resolve; `<endpoint N>` labels shown). Increment-2 backlog: retry under STA, try `PKEY_Device_Desc` |
| 2026-09-24 | SATURN **physical, operator present** (Win10 19045.6216, Zen 4 Family 25, 32 cores) — first non-RDP session (`test001.log`) | **UNRESOLVED**: 8 anonymous render endpoints, friendly-name read denied on ALL (0x80070005); no capture enumerated (increment 2); default `<endpoint 5>` `{d8db3e5b…}` | wasapi-shared | default: mix **96000 Hz / 2 ch / f32**, `GetBufferSize` 2112 fr (22 ms), event cadence avg 9.99 ms; play asked 48 k, adjusted to the listed 96 k | **FIRST SOUND EVER THROUGH THE HAL** — 10 s tone audible to the operator (WO-005 "hear it" achieved): play **PASS**, 15112 blocks, **0 xruns, 0 allocations**, callback p50/p99 2.0 µs / max 7.5 µs, jitter min 700 ns / avg 9.99 / max 12.55 ms over 1004 wakes, clean stop, `rt setup mmcss priority NO-ideal-proc workset` (core_applied = mmcss+priority per rt/thread.rs; ideal-proc is a printed refinement). Conformance: null 9/9 + shared 7/7, reopen-leak 0 outstanding | throughput math says the stream is real 96 kHz (15112 × 64 ÷ 96000 = 10.07 s vs ≈10.0 s wall); **drift reads +1 182 084 ppm → `SUSPECT` label correct** (#45 signature, ~2.18× real). On a *physical* session this can no longer be pinned on "RDP endpoint" alone: either the default is still a virtual-class endpoint, or the drift readout is wrong everywhere — test002's PnP/registry table discriminates |
| 2026-09-24 | SATURN physical | same 8 endpoints | wasapi-exclusive | f32 @ 96000 / 2 ch on the default | **REFUSED everywhere, honestly**: caps show `exclusive none` + a single rate on ALL 8 endpoints; open → `AUDCLNT_E_UNSUPPORTED_FORMAT (0x88890008)`, classified `Format`, no silent reroute; conformance verified the refusal | exclusive-dependent acceptance (2 h soak 96k/64, unplug, recovery) is **BLOCKED until the target endpoint is identified and offers exclusive**. Two live theories: **(A)** the listed endpoints are Remote-Audio-class/virtual (name denial + #45 clock + 10 ms-cadence/22 ms-buffer shape) and the physical interface is not enumerated or not default; **(B)** `<endpoint 5>` *is* the reinstalled interface with the exclusive-control checkbox reset to OFF by the driver reinstall — a real device with exclusive disabled presents exactly this shape (single mix rate, `exclusive none`), which would also reclassify #40 from "RDP quirk" to a general sparq COM property-store bug. `test002` captures Windows-side ground truth (PnP AudioEndpoint names+states, `Win32_SoundDevice`, `qwinsta`, and the MMDevices registry `FriendlyName` — which bypasses the denied COM path and maps GUID-for-GUID onto sparq's endpoint list) and adds `--device N` targeting |
| 2026-09-24 | SATURN physical, `test002.log`, no remote session (`qwinsta`: console active, rdp-tcp Listen only), High-performance scheme confirmed | **RESOLVED: the endpoints ARE the real hardware.** PnP + MMDevices-registry names: `{d8db3e5b…}` = **OUT 1-2 (BEHRINGER UMC 204HD 192k)** and is the default; `{1e9f6bf9…}` = OUT 1-4 (4 ch); `{593c4844…}` = OUT 3-4; IN 1-2 present (capture, increment 2); BenQ EL2870U ×3 (NVIDIA), Steam Streaming pair. `Win32_SoundDevice`: BEHRINGER UMC 204HD 192k status=OK | wasapi-shared | OUT 1-4 via `--device 1`: requested 96k/4ch plain-shape refused → **negotiated rung 2 extensible f32 96000 Hz / 4 ch** (the #38/#46 two-shapes fix, working as designed on real hardware); period 2112 fr (22 ms), block 64 | play 10 s **PASS** (15112 blocks, 0 xruns, 0 allocs, p99 2.0 µs, tone AUDIBLE); **UNPLUG TEST PASSED THE ACCEPTANCE CRITERION**: at ~8 s into a 30 s run the stream logged `entered Removed: the endpoint was invalidated … re-enumerate and open again`, final state `Removed`, dev-err 1, **0 xruns, 0 allocs, clean stop, no crash/hang**; re-plug recovery play **PASS** (15127 blocks, 0 xruns, tone audible); 5-min shared soak @96k/64 **PASS**: 450 307 blocks, **0 xruns / 0 late / 0 allocs**, p50 1.0 µs p99 2.0 µs max 108 µs (3 outliers >65 µs), jitter avg 10.00 ms; reopen-leak 0 outstanding on hardware | **Multichannel criterion resolved honestly**: UMC204HD maxes at 4 out — caps report `4..4` correctly, 4 ch f32 @96k negotiated and audible, and the ≥8-out path stays proven by the null backend (64 ch). test002's UTF-8 soak tee verified the #74 harness fix |
| 2026-09-24 | SATURN physical (same session) | OUT 1-2 / OUT 1-4 (UMC 204HD) | wasapi-exclusive | f32 @ 96000, 2 ch and 4 ch, operator confirmed BOTH exclusive-control checkboxes ticked | **REFUSED by the driver**: `AUDCLNT_E_UNSUPPORTED_FORMAT (0x88890008)` — not `Busy`, not policy: the driver does not accept IEEE-float in exclusive. wasapi.rs header says it plainly: *"Exclusive: f32 extensible … no conversion on the hot path"* — the ladder has no integer rungs | **Defect #77 — the acceptance blocker**: USB DAC drivers (Behringer/Thesycon class) speak integer (24-in-32) in exclusive; sparq only asks for f32, so a fully exclusive-capable interface reports `exclusive none`. Increment 1.2: integer rungs (i32 / 24-in-32 / i16, both format shapes) + pump conversion generalising the existing shared-mode i16 path, then the 2 h exclusive acceptance soak via test004 |

## 4b. Findings from the first physical session (2026-09-24, test001/test002)

* **Defect #40 is reclassified (call it #75).** Friendly-name `E_ACCESSDENIED (0x80070005)` fires on
  the *real* Behringer endpoints with **no remote session connected** — it is not an RDP quirk. It
  is either sparq's COM property-store usage or a machine policy. The registry path
  (`HKLM\...\MMDevices\Audio\Render\<guid>\Properties`, value
  `{a45c254e-df1c-4efd-8020-67d146a850e0},2`) returns every name correctly, so a registry fallback
  is a legitimate fix alongside the STA retry.
* **The `SUSPECT` drift signature is decoded (#76).** On BOTH endpoints tested — the RDP session of
  2026-09-21 (970 fr buffer, ~10 ms cadence, 44.1 kHz → measured +1 200 367 ppm) and the Behringer
  (2112 fr, ~10 ms, 96 kHz → measured +1 199 108 ppm, asymptoting over a 5-min soak) — the drift
  equals `(buffer_frames ÷ event_period) ÷ nominal_rate − 1` to four digits (+1.2 M ppm both
  times). The `IAudioClock` position on these event-driven shared streams advances in
  buffer-sized steps per event tick, not per sample. The #45 attribution ("virtual/RDP endpoint")
  is falsified: this is systematic, and the `SUSPECT` guard did exactly its job — throughput
  (blocks × frames ÷ wall) stayed truthful in every run. Increment 2: derive drift from delivered
  frames vs QPC instead of `GetPosition`, or probe the position semantics per event.
* **`NO-ideal-proc` on a 32-core Zen 4** while mmcss+priority+workset apply: `core_applied()` holds
  (rt/thread.rs documents ideal-proc as a refinement), and zero xruns across every hardware run so
  far says the boost is doing its job. Recorded so it cannot be overlooked; investigate with the
  MMCSS max-characteristics retry.

## 4c. test004, first exclusive acceptance attempt (2026-09-24 evening, increment 1.2 build)

* **Caps checkpoint PASSED — #77's fix works on hardware.** With the four-rung format ladder, the
  Behringer endpoints list `exclusive 96000` (and the 4-ch endpoint its rates) instead of
  `exclusive none`. The probe side of the format question is closed on the device that raised it.
* **Defect #79 — the period, not the format.** Exclusive playback then failed on EVERY rung with
  one code: `Initialize (exclusive): HRESULT 0x88890020` = `AUDCLNT_E_INVALID_DEVICE_PERIOD`.
  The arithmetic, in the style of #76: the open asked for the sparq **block** as the exclusive
  **device period** — 64 fr ÷ 96 000 Hz = **666.7 µs** — while the endpoint's own `GetDevicePeriod`
  reports default = minimum = **10 ms** (960 fr), the number the caps line had been printing as
  `hw period 10.000 ms` since increment 1.1. A 15× under-run of the engine period; the driver
  refuses, and because `IsFormatSupported` takes no period, no format probe can ever see this —
  only `Initialize` tells the truth (the file's own comment, proved again).
* **Why the fix costs nothing structurally:** the pump's FIFO already decouples *device period*
  from *sparq block* ("Period ≠ block", §WASAPI specifics) — the 2 h shared-mode soak of the same
  day ran a 10 ms period over 64-frame blocks with 0 xruns (10 798 597 blocks). Only the exclusive
  open's *asking* was missing. Increment 1.3 adds the asking: `hal/period.rs` computes the
  candidate ladder as pure data (block period clamped to the driver minimum, then the driver
  default, deduplicated — on the UMC 204HD that collapses to a single 10 ms candidate, the
  driver's own number), each candidate gets a fresh `IAudioClient` (a failed `Initialize` consumes
  it) plus the documented alignment two-step, non-period HRESULTs abort early, and total refusal
  carries the full period probe table. Expected open line on the re-run:
  `device period 960 fr (10.00 ms) · sparq block 64 fr`.
* **Also re-confirmed on the real endpoints:** friendly-name `E_ACCESSDENIED` (0x80070005, vt 0)
  fires with no remote session (#75's signature), endpoints fall back to `<endpoint N>` labels,
  registry names read fine — the fallback path is doing its job until increment 2's STA retry.

## 4d. test004 attempt 2 — the exclusive stream OPENED and then stalled (2026-09-28, increment 1.3 build)

* **The period ladder worked — and the period it landed on is the defect.** Attempt 2 opened
  exclusive for the first time ever: `i24-in-32 (converting) · device period 288 fr (3.00 ms) ·
  sparq block 64 fr` at 96 kHz on the UMC 204HD (`{d8db3e5b…}`). #77's format side and #79's
  asking side both did their job. But the open line's period — **3 ms, not the 10 ms the checklist
  expected** — is where the run died: 10 s tone, **159 late wakes, 7732 blocks (≈ 49.1k fr/s
  delivered against a 96 kHz negotiation — half throughput), jitter min 600 ns / avg 5.84 ms /
  max 33.24 ms, drift −491 656 ppm, operator heard no clean tone, rc 1**.
* **The arithmetic, in the style of #76.** The jitter triple splits the run exactly: 1718 wakes in
  ~10.07 s, avg 5.84 ms, max 33.24 ms ⇒ ~1559 wakes at ~3.0 ms (the device DID run at 96 kHz
  between stalls — 1559 × 3 ms ≈ 4.68 s) plus ~159 wakes at ~33.2 ms (≈ 5.30 s — the other half of
  the wall time). 159 stalls ÷ 10 s ≈ **one ~33 ms stall every ~62.5 ms**; blocks 7732 × 64 =
  494 848 fr ≈ 96 000 × (1559 × 3 ms) — every number agrees: the stream ran at full rate roughly
  half the time and starved the other half. The xrun line confirms it (159 late wakes, **0**
  budget overruns, **0** FIFO-starvation writes: the pump kept every promise it could see; the
  device simply stopped asking for data on its own 3 ms cadence). The short exclusive conformance
  run in [08] shows the same signature (jitter max 33.01 ms, 2 late wakes in 123 blocks).
* **The control experiment ran in the same session:** the shared 10 ms engine on the same endpoint
  — same machine, same RDP session, same day — soaked **2 h, 719 895 wakes, 0 xruns, max jitter
  12.04 ms**. So this is not a machine-wide DPC storm and not the pump: it is the 3 ms exclusive
  period the driver *accepted* and cannot *sustain*.
* **Defect #89 — a third lying-`min` face.** #79's driver refused an under-minimum ask at
  `Initialize` (honest refusal, recoverable). The documented lying-min pattern refuses its OWN
  minimum (recoverable the same way). The UMC 204HD does worse: it **accepts** a period — whether
  read from `GetDevicePeriod`'s minimum or from the alignment two-step's `GetBufferSize`
  granularity (288 fr = 3 ms; the attempt-1 log copy is truncated, so which door produced the
  3 ms ask cannot be pinned from the evidence, and the fix therefore closes all of them) — and
  then misbehaves at RUNTIME, where no open-path check can see it. **An accepted `Initialize` is
  not a promise the period will run.** The number a driver's engine actually runs at is its
  DEFAULT period; that is now rung 1 whenever the reported minimum sits above the sparq block
  (the class this device belongs to), with the min-clamped ask demoted to fallback. Sub-block
  engines keep the honest low-latency ask first — a lying driver THERE refuses at `Initialize`,
  which the ladder survives (#79's own shape, unchanged).
* **Increment 1.4 closes the three doors:** (1) the ladder reorder above (`hal/period.rs`,
  pinned by `the_defect_89_device_asks_its_default_before_its_min`); (2) the alignment two-step
  now rounds the ask UP to the reported granularity (10 ms ask, 288 fr granularity → 1152 fr =
  12.000 ms) instead of adopting the granularity — inc 1.3's literal reading of the recipe is
  exactly how a 10 ms ask could have become the 3 ms open; (3) a post-open truth check: an
  allocation seriously smaller (< ¾) than the accepted ask is treated as an undeclared
  granularity, re-asked once rounded up, and if that fails the mismatch is printed on the open
  line (`device period 288 fr (3.00 ms, ask 960 fr)`) rather than dressed as a clean negotiation.
  Expected attempt-3 open line: `device period 960 fr (10.00 ms)` — or `1152 fr (12.00 ms)` if
  the driver enforces its 288 fr alignment. Both are the driver's own engine class; the 2 h
  shared soak says the machine sustains it.
* **What stayed green in attempt 2** (recorded so the next run reads against it): caps checkpoint
  ✓ (`exclusive 96000` listed), shared unplug → `Removed` + dev-err 1 + clean stop ✓, re-plug
  recovery ✓ (tone heard), 2 h shared soak ✓ (10 798 477 blocks, 0 xruns, 0 allocs, p99 ≤ 4.1 µs),
  post-soak conformance all three backends ✓ (9 + 8 + 7 checks, reopen-leak 0). The friendly-name
  `E_ACCESSDENIED` (#75) and the `SUSPECT` drift labels (#45/#76: the shared clock's +1 199 624 ppm
  is exactly the 2112-fr-buffer-per-10-ms-tick signature #76 decoded) behaved as documented.

## 5. Increment 2 backlog (declared, not hidden)

* **Increment 1.2 (pulls ahead of ASIO — it blocks the acceptance soak): integer exclusive
  rungs (#77).** **SHIPPED 2026-09-24; probe side verified on hardware by test004's caps
  checkpoint; the open side surfaced #79, fixed by increment 1.3 the next day (`hal/period.rs`).
  Acceptance is the test004 re-run.** The exclusive ladder probes f32 only; the Behringer UMC 204HD 192k driver refuses
  f32 exclusive (`AUDCLNT_E_UNSUPPORTED_FORMAT`) and USB DAC drivers generally speak integer in
  exclusive. Add i32 / 24-in-32 / i16 rungs (both `WAVEFORMATEX` and `WAVEFORMATEXTENSIBLE`
  shapes), generalise the pump's existing shared-mode i16 conversion to a `DeviceSampleFormat`
  enum, report per-format exclusive rates in caps, and keep the refusal honest when even integers
  fail. Hardware acceptance then runs via test004: exclusive tone → exclusive unplug → the 2 h
  96 kHz/64 exclusive soak.
* **Friendly-name fix (#75)**: registry fallback and/or STA retry — the COM denial now has a
  proven non-COM read path (test002 [02e]).
* **Drift without `GetPosition` (#76)**: frames-delivered vs QPC, with the arithmetic from §4b as
  the regression fixture (both recorded sessions must come out ≈ 0 ppm under the new derivation).
* **ASIO backend** (`hal/asio.rs`, allowlist entry 2 reserved): registry enumeration, COM
  instantiation of driver CLSIDs, `IASIO` vtable, double-buffered `createBuffers`, driver-thread
  callback discipline. Sequenced after WASAPI *on hardware* so there is a proven reference to
  diff against.
* **Full duplex**: capture endpoint pairing (render+capture on one physical device), input
  latency in `LatencyReport`, input buffers in `FrameCtx` (already in the trait; null already
  exercises the path — `multichannel_32_out_8_in_opens_and_processes`).
* **Round-trip measurement utility**: loopback impulse → `measured_roundtrip_frames` stops being
  `None`; acceptance criterion "matches the WO-001 table within ±1 block" becomes checkable.
* **MMCSS max-characteristics**: re-add when the windows-sys metadata binding is fixed (§1).
* **Friendly-name retry**: STA apartment and/or `PKEY_Device_Desc` fallback for endpoints whose
  property store denies `PKEY_Device_FriendlyName` (observed: RDP, `E_ACCESSDENIED`).
* **Stage-mode preset application at startup** (WO-006 scope, needs WO-001 measurements first).
