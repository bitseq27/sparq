# ADR-004 — Primary platform: Windows (decision D-4)

**Status:** accepted · **Date:** 2026-09-20 · **Related:** ADR-001, ADR-009, `docs/hardware/stage-baseline.md`

## Context

Where live sound actually happens decides the primary platform: venue laptops and rental rigs are overwhelmingly Windows, multichannel interfaces ship their best drivers for Windows (ASIO), and the touch device that will be played on stage is a Windows tablet/convertible. Linux remains attractive for kiosk/install mode and a future appliance; macOS remains a secondary port. One platform must be primary, because "equal everywhere" means unoptimised everywhere.

Hardware decisions D-11 (stage touch device) and D-12 (interface channel count) are resolved in **WO-001**, and this ADR is amended with the specifics.

## Decision

**Windows 11 is the primary platform.** Linux and macOS are first-class ports kept green in CI from Phase 1, but Windows gets: the reference hardware, the latency targets, the stage-mode preset, and the tie-break in any platform-specific trade-off.

Consequences accepted up front:

1. **Audio path:** WASAPI exclusive (event-driven) first, **ASIO** as a peer backend (required for many multichannel interfaces), WASAPI shared as fallback and for the non-performance monitoring path, null/virtual device for CI and offline render. One HAL trait over all of them (ADR-009).
2. **Real-time discipline (plan §5.2):** MMCSS `Pro Audio` at critical priority with ideal-processor assignment; working-set lock + pre-touch; high-resolution waitable timers instead of raising the global timer resolution; no waits at all on the audio thread; process priority class raised in Perform mode.
3. **DPC/ISR latency is the named xrun risk.** A measurement routine plus a documented mitigation set ships with the app: high-performance power plan, core parking off, NIC power saving/EEE off, Wi-Fi power management off, USB selective suspend off, GPU driver version pinned and hardware-accelerated scheduling tested both ways, Defender exclusions for project/asset/module directories, notifications and focus-stealing suppressed.
4. **Stage mode is a scriptable, *verifying* preset.** At startup in Perform mode sparq applies what it can and reports pass/fail per item — not tribal knowledge.
5. **Device aggregator designed in from day 1.** Windows has no aggregate-device concept, so reaching 16/32/64 outputs from several interfaces requires our own aggregator (clock-master selection, drift-compensating resampling PLL, per-device latency alignment). The HAL trait must be able to express it now; the implementation lands in Phase 7.
6. **Per-monitor DPI awareness** from the first window (WO-012): fractional scaling at 100/125/150/200 % and mixed-DPI multi-monitor must map touch coordinates correctly.
7. **No Windows-isms in the kernel.** Paths, threading, timing and device concepts stay behind platform modules so the Linux appliance path (D-9) remains a packaging exercise.

## Consequences

* Latency and reliability claims are made on the machine that will be on stage — the point of the whole decision.
* ASIO FFI adds C++ interop work and an `unsafe` allowlist entry (ADR-001).
* Some Linux-only conveniences (JACK graph introspection, PipeWire session policy) are unavailable; we implement equivalents (rig presets, matrix routing) in-app.
* Install/kiosk mode on Windows needs a locked-down configuration story (assigned access, no auto-reboot, no update interruptions) — noted for Phase 8.

## Alternatives considered

* **Linux primary.** Better real-time kernel story, JACK/PipeWire, ideal for kiosk/appliance. Rejected as *primary* because the stage hardware, touch devices and venue machines are Windows; retained as the appliance target.
* **macOS primary.** Excellent CoreAudio and touch hardware, but a closed device ecosystem for multichannel interfaces and no venue presence. Retained as a secondary port.
* **Tablet-first client/server split.** Attractive ergonomically; rejected because it doubles the system (transport, sync, failure modes) before the instrument exists. Revisit only if the thin client (D-10) proves people want to play from a separate surface.

## Review trigger

WO-001 findings (if the chosen interface's ASIO/WASAPI behaviour invalidates the latency target), or if D-9 (appliance) is promoted before v1.0.
