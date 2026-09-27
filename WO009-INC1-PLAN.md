# WO-009 increment 1 — clocks + transport v0: the plan

**Date:** 2026-09-26 · **Track:** sandbox (CHECKLIST task 3) · **Seal target:** `sync wo009-inc1`
→ `../sync-wo009-inc1.zip` · **Artefacts (the WO's):** `sparq-music::transport`, ADR-006 addendum

## The design, as already pre-committed by the tree

* ADR-006: three clocks, `t_sample` master, ONE `ClockBroker` owning `t_sample ↔ t_musical ↔
  t_wall` as a *piecewise-linear map with smoothed derivative*; sample accuracy is rule 7
  (1 000 random ticks, ±0 samples).
* `sparq-kernel::clock`'s own header: "the piecewise-linear tempo map … is WO-009 and will
  replace `Clock::tick_at_sample` with a segment lookup **while keeping this type's public
  surface**" — so the map lives in the kernel `Clock` (Copy → Clone is the one surface change;
  no consumer copies a Clock by value — verified: patch.rs locals, hal/null.rs field, block.rs
  takes `&Clock`).
* The executor's contract-v1 door (`push_host_event`) is where the transport publishes; the
  executor's declared limit "the musical clock is static (`tick` stays 0): three clocks are
  WO-009" is what this increment retires.
* The drum-demo golden `f2303f13aa0cf299` (app-side 120 BPM schedule) is the cross-check: the
  TRANSPORT-driven render of the same patch must hash identically — two independent mechanisms,
  one bit pattern.

## What ships

1. **Kernel `Clock` v2** — piecewise tempo map: segments anchored at `(sample, tick)` with a
   LINEAR BPM RAMP (`ramp_samples`), i.e. the smoothed derivative ADR-006 rule 2 demands —
   position continuous by anchoring, derivative continuous by starting each ramp at the
   effective bpm of the previous segment (mid-ramp changes glide, never step). Within a ramp the
   tick integral is quadratic in closed form; `sample_at_tick` is an **exact integer bisection**
   on the monotone map (nearest-sample rounding consistent with `tick_at_sample`'s round), so
   the ±0-samples acceptance cannot be undermined by float-inversion cleverness. Public surface
   kept: `new/ppqn/bpm/sample_rate/seconds_per_tick/samples_per_beat/tick_at_sample/
   sample_at_tick/bar_beat_tick/reanchored` + new `set_tempo(bpm, at_sample, ramp_samples) ->
   bool`, `effective_bpm_at(sample)`, `tick_at_sample_f64`. bpm ≤ 0 refused (bool), stale
   anchors refused — the map never goes non-monotone.
2. **`sparq-music`** (the planned crate gets its first contents): 
   * `broker::ClockBroker` — owns the `Clock` + the stream sample position + the wall estimator:
     `advance(frames)`, `set_tempo(bpm, ramp_ms)` anchored at the current position,
     `observe_wall(wall_us)` / `wall_at_sample` / `drift_ppm` (windowed-ratio estimator,
     smoothed, re-anchored on `locate`/construction; the 30-minute device measurement is the
     ADR-006 addendum's device-track half), `locate(sample)`.
   * `transport::Transport` — play/stop (stop FREEZES the timeline: offline tape semantics,
     which is what makes "same event twice ⇒ same output" exact), tempo now and as a stream-by-
     design (every change is a map segment), tick playhead with **loop region folding** (the
     clock map stays monotone; the FOLD lives in the transport view — loops fold the playhead and
     the beat/bar triggers, never the map), the tick-scheduled event queue (absolute-timeline
     v0, declared), bar/beat trigger emission (beat: channel 0, bar: channel 1, value 1.0,
     kind Trigger — the drum-demo cross-check pins these fields), and tap tempo (mean of up to
     the last four intervals → `set_tempo`).
   * `BlockEvents` — the bounded (64 = `EVENTS_PER_BLOCK`), allocation-free per-block collector
     `advance_block(frames, &mut BlockEvents)` fills; the driver (offline loop now, HAL pump
     later) forwards it to `Executor::push_host_event`. Overflow counted, never grown.
   * Dependencies: kernel + module-api only. It does NOT depend on sparq-audio; the app wires
     transport → executor (5-line loop), keeping the layering honest.
3. **Executor/engine integration** — `Executor::set_clock(Option<Clock>)` (control-side);
   `render_block`'s manual advance becomes `BlockContext::advance`-equivalent when a clock is
   set (`ctx.tick` follows the map), stays tick-0 when not — **the default path does not move a
   sample**, which the stress hash `b42068ec7b206789` and every golden then prove.
   `inherit_runtime` carries the clock (the timeline belongs to the stream, not the patch — the
   same sentence task 4 wrote for the sample clock).
4. **`sparq exec --patch drum-demo` becomes transport-driven** (120 BPM, beats on): the golden
   MUST stay `f2303f13aa0cf299` — that equality is the acceptance's "transport as event source"
   half, measured. `DrumDemoPatch::is_kick_block` stays (the batch-3 gate pins the schedule
   independently).
5. **Acceptance tests** (the WO's four boxes, mapped):
   * ±0 samples: `tests/wo009.rs` fires 1 000 seeded-random ticks through the queue at constant
     tempo AND across tempo changes, against expectations computed from an INDEPENDENT
     implementation of the segment integral in the test file (not the code under test).
   * No discontinuity in either direction: the continuity test is written FIRST (the WO's risk
     line): a 60→200→70 bpm sweep, one change per block, asserting `tick_at_sample` strictly
     increasing, per-sample deltas bounded by the ramp (second differences only inside ramps),
     `sample_at_tick` monotone, and round-trips within ±1 tick (±0 at segment anchors).
   * Drift: synthetic observations at exactly +1000 ppm → estimator converges to 1000 ± 2;
     wall map monotone; re-anchor on locate. The 30-min-on-device box stays open, declared.
   * Stop/start repeatability: the same script twice → identical event samples AND identical
     executor renders (hash equality), at 48k and 96k.
   * Loop: a 2-bar loop emits identical within-cycle beat offsets for four cycles.
   * Tick flow: an executor probe module renders `ctx.block.tick` — with a clock set, the render
     equals the map; without, zero (the v1 behaviour, pinned).
6. **Docs**: ADR-006 addendum (implemented-status + the device-pending drift box + the Copy→
   Clone surface note), module-api §17's "still v0-declared" list loses the static-clock line,
   CHECKLIST/LATER (lfo + clk-div unblocked → WO-014 inc 4), WORKORDERS row + build-log entry,
   log_check BASELINE moved with the seal (#83's discipline), SYNC.md + zip + namelist check.

## Red lines (must not move)

* Every golden: `ba577186c988db21`, `0f5c3e86c7f117a9`, `53de3b1f3f40e3c9`, `3f325d4f99ca2a01`,
  batch-2 six + chain, `1621e1f65b1b64e1`, `1d1c84bd37c99b91`, **`f2303f13aa0cf299` (now also
  the transport cross-check)**, and the stress hash `b42068ec7b206789` (the determinism world
  never sets a clock — by construction, not by luck).
* Zero allocations in `advance_block` (the pump-safe claim) — measured with the counting
  allocator, warm schedule included.
* No pre-emption of later WOs: no external sync/PLL (Phase 4), no metric-modulation UI, no
  per-track signatures, no count-in, no recording; `mod/lfo`/`mod/clk-div` modules themselves
  are WO-014 inc 4 (this increment ships the clocks they need, proven by tests, not by modules).

## Order of work

A. Kernel `Clock` v2 + its unit tests (continuity FIRST, per the risk line) → workspace compiles
   (patch.rs / null.rs / block.rs unchanged call sites).
B. `sparq-music` crate: broker, transport, BlockEvents + `tests/wo009.rs` acceptance suite.
C. Executor/engine clock integration + the tick-flow test; determinism/stress re-run (hash must
   not move).
D. App: transport-driven drum-demo + the cross-check test (golden equality) + stop/start render
   repeatability.
E. Docs, ADR-006 addendum, gates (fmt · clippy cells · tests · python · release goldens ·
   selftest · audit · strict · exec×3), seal, zip, CHECKLIST/SYNC/WORKORDERS.
