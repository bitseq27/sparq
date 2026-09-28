//! Exclusive-mode period negotiation (WO-006 increment 1.3, defect #79) — the decision as pure
//! data.
//!
//! Why this file exists outside `wasapi.rs`: the WASAPI module is `cfg(windows)`-gated, and this
//! project is developed on a Linux sandbox. Defect #79's lesson (test004, 2026-09-24, SATURN):
//! exclusive `IAudioClient::Initialize` was asked for sparq's **block** period — 64 frames =
//! 666 µs at 96 kHz — and the Behringer UMC 204HD refused it with
//! `AUDCLNT_E_INVALID_DEVICE_PERIOD` (0x88890020), because its engine period is **10 ms**. The
//! caps line had been printing that 10 ms all along (`GetDevicePeriod`'s default); the open path
//! just never asked it. The block↔period decoupling that makes accepting the driver's period
//! free already exists in the pump's FIFO (*Period ≠ block*, `docs/hal/windows-notes.md`): the
//! callback always sees exactly B frames, the device always gets exactly what it asked for. What
//! was missing was the asking.
//!
//! So the asking lives here, ungated and unit-tested on the development machine, and the Windows
//! code only *executes* the list this file returns against real COM objects — the null backend's
//! discipline applied to a negotiation: whatever can be decided without the device is tested
//! without the device.
//!
//! Defect #89's lesson (test004 attempt 2, 2026-09-28, SATURN): the inc-1.3 ladder — the
//! block-clamped driver minimum FIRST — opened the UMC 204HD at its reported minimum (288 fr =
//! 3.00 ms @ 96 kHz), and the driver *accepted* it, and then could not *sustain* it: buffer
//! events arrived every ~5.9 ms on average instead of every 3 ms (159 of them ~33 ms late — a
//! stall every ~62.5 ms), throughput halved (7732 blocks in 10 s ≈ 49.1k fr/s delivered against
//! a 96 kHz negotiation), the device clock read −49 % against the wall, and the operator heard no
//! clean tone — while the SAME machine ran the shared 10 ms engine for 2 h with zero xruns in the
//! same session. An accepted `Initialize` is therefore not a promise the period will run; the
//! number the driver's engine actually runs at is its DEFAULT period. So when a driver says it
//! needs an engine coarser than the sparq block, its default is asked first and its minimum only
//! as a fallback — and when a driver reports its alignment granularity (through the
//! `BUFFER_SIZE_NOT_ALIGNED` handshake, or silently through a shrunken allocation), the ask is
//! rounded UP to that granularity, never replaced by it ([`align_up_frames`],
//! [`allocation_seriously_shrunk`]).

/// Candidate device periods for exclusive `Initialize`, in 100-nanosecond units (WASAPI's
/// `REFERENCE_TIME`), in the order they should be tried. Never empty, deduplicated.
///
/// `hw_default_100ns` / `hw_min_100ns` are what the endpoint said (`GetDevicePeriod` on the live
/// client — legal before `Initialize`); `block_frames` at `rate` is what sparq would like. The
/// order depends on which side of the block period the driver's reported minimum sits — the two
/// device shapes have each taught one half of the rule (#79, #89):
///
/// * **`min` ≤ block period — a sub-block engine** (most internal cards, any driver with a
///   sub-millisecond engine): the honest low-latency ask goes first, byte-identical to inc 1.2's
///   single ask, so this class of device pays nothing. A driver that lies here refuses at
///   `Initialize` *honestly* (#79's original signature — `INVALID_DEVICE_PERIOD`), which the
///   ladder survives: the driver's default period is rung 2.
/// * **`min` > block period — the driver needs an engine coarser than sparq's block**: its
///   reported minimum is NOT a promise. Test004 attempt 2 (2026-09-28) proved the third lying-min
///   pattern on the UMC 204HD: the driver ACCEPTED its own 3 ms minimum at `Initialize` and then
///   could not sustain it — ~33 ms stalls every ~62.5 ms, half throughput, −49 % clock-vs-wall,
///   no clean tone — while the same driver's 10 ms default engine soak-ran 2 h clean in shared
///   mode. A refusal is survivable; an accepted-but-unsustainable period is not detectable at
///   open at all. So for this class the **driver's default period goes first** — the number its
///   engine actually runs at — and the min-clamped ask survives only as the fallback for a driver
///   that refuses its own default.
///
/// The raw block period is deliberately NOT re-tried after a clamp: a driver whose minimum is
/// above it has already said no, and burning an `Activate` + `Initialize` round-trip on an
/// endpoint that answered is exactly the cost the reordered attempt ladder (#38/#46) removed.
///
/// A default *below* a reported minimum is contradictory driver information; it is skipped
/// rather than tried, because trying it would re-ask for something the endpoint just refused.
#[must_use]
pub fn exclusive_period_ladder(
    block_frames: u32,
    rate: u32,
    hw_default_100ns: Option<i64>,
    hw_min_100ns: Option<i64>,
) -> Vec<i64> {
    let rate = i64::from(rate.max(1));
    // The same integer arithmetic the old single ask used: truncate to 100 ns units. A period
    // one 100 ns unit short of the block is still ≥ the block in frames after the driver's own
    // rounding, and the alignment two-step downstream reports the truth via GetBufferSize.
    let block = (i64::from(block_frames) * 10_000_000 / rate).max(1);
    let min = hw_min_100ns.filter(|v| *v > 0);
    // A default below the reported minimum is contradictory driver info: skip it, never re-ask
    // something the endpoint's own numbers already rule out.
    let def = hw_default_100ns.filter(|v| *v > 0).filter(|d| !min.is_some_and(|m| *d < m));

    let ask = min.map_or(block, |m| block.max(m));
    let mut out: Vec<i64> = Vec::with_capacity(2);
    if min.is_some_and(|m| m > block) {
        // Coarser-than-block engine: default first (the engine's own cadence — #89), the
        // min-clamped ask only as the fallback for a driver that refuses its own default.
        if let Some(d) = def {
            out.push(d);
        }
        if !out.contains(&ask) {
            out.push(ask);
        }
    } else {
        // Sub-block engine (or no driver information): the honest low-latency ask first — the
        // inc-1.2 shape this class of device has always survived — the default as the catcher.
        out.push(ask);
        if let Some(d) = def {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// Round a requested period (in frames) UP to a whole number of the driver's alignment-granularity
/// units. The documented `AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED` two-step reads `GetBufferSize` on the
/// failed client for the driver's granularity; the re-ask must keep the original ask's engine
/// intent, so it rounds up to it (10 ms asked, 288 fr granularity → 1152 fr = 12 ms) instead of
/// adopting the granularity itself (→ 288 fr = 3 ms, the #89 trap: aligned, accepted — and
/// unsustainable). An ask already at or below one unit lands on that unit, which IS the documented
/// recipe's behaviour for sub-period asks (inc 1.2's 64 fr block → 288 fr, unchanged). A zero
/// granularity (a driver that reported nothing usable) leaves the ask alone.
#[must_use]
pub fn align_up_frames(ask_frames: u32, granularity_frames: u32) -> u32 {
    if granularity_frames == 0 {
        return ask_frames;
    }
    let g = u64::from(granularity_frames);
    let units = (u64::from(ask_frames).div_ceil(g)).max(1);
    u32::try_from(units * g).unwrap_or(u32::MAX)
}

/// Frames at `rate` for a period in 100 ns units — the same round-half-up integer arithmetic the
/// alignment two-step has always used, so an ask and its allocation are compared with identical
/// rounding. Saturates rather than wrapping on absurd inputs.
#[must_use]
pub fn frames_to_100ns(frames: u32, rate: u32) -> i64 {
    let rate = i64::from(rate.max(1));
    let units = (i64::from(frames) * 10_000_000 + rate / 2) / rate;
    units.max(1)
}

/// The frame count a 100 ns period corresponds to at `rate`, truncated, at least 1 — the frame
/// view of an ask, for comparing against what `GetBufferSize` actually allocated.
#[must_use]
pub fn period_100ns_to_frames(p_100ns: i64, rate: u32) -> u32 {
    let rate = i64::from(rate.max(1));
    let frames = (p_100ns.max(1) * rate) / 10_000_000;
    u32::try_from(frames.max(1)).unwrap_or(u32::MAX)
}

/// True when a driver ALLOCATED a seriously smaller period than the one it just ACCEPTED — under
/// three quarters of the ask. Some stacks report their alignment granularity silently this way
/// instead of through the `BUFFER_SIZE_NOT_ALIGNED` handshake (#89's third face); the ask should
/// then be rounded up to the allocation and re-asked once. The three-quarter tolerance keeps this
/// off the honest cases: a driver rounding an ask to its own alignment (up OR slightly down) is
/// not lying, and firing the retry there would burn an `Activate` round-trip for nothing.
#[must_use]
pub fn allocation_seriously_shrunk(ask_frames: u32, allocated_frames: u32) -> bool {
    u64::from(allocated_frames) * 4 < u64::from(ask_frames).max(1) * 3
}

/// A period in 100 ns units, in human words for logs and probe tables: `"666 µs"`, `"10.000 ms"`.
/// One formatter so the caps line, the open line and the failure table all spell periods the
/// same way.
#[must_use]
pub fn period_text(p_100ns: i64) -> String {
    let us = p_100ns as f64 / 10.0;
    if us >= 1000.0 {
        format!("{:.3} ms", us / 1000.0)
    } else {
        format!("{us:.0} µs")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// 1 ms in 100 ns units.
    const MS: i64 = 10_000;

    #[test]
    fn the_defect_79_device_gets_its_own_period_on_the_first_and_only_try() {
        // Behringer UMC 204HD (test004, 2026-09-24): a 64-frame block at 96 kHz asks for 666 µs;
        // the driver's GetDevicePeriod says default = min = 10 ms. The clamp lands ON the
        // driver's number, the default dedups away, and the ladder is a single 10 ms candidate —
        // the exact ask that device can answer.
        let ladder = exclusive_period_ladder(64, 96_000, Some(10 * MS), Some(10 * MS));
        assert_eq!(ladder, vec![10 * MS]);
    }

    #[test]
    fn a_driver_whose_minimum_fits_the_block_keeps_the_low_latency_ask() {
        // 64 frames at 48 kHz = 1333 µs; min 1 ms ≤ ask, so rung 1 is unchanged from inc 1.2's
        // behaviour, and the 3 ms default rides along as the fallback.
        let ladder = exclusive_period_ladder(64, 48_000, Some(3 * MS), Some(MS));
        assert_eq!(ladder, vec![64 * 10_000_000 / 48_000, 3 * MS]);
        assert_eq!(ladder[0], 13_333, "1333.3 µs truncated, the old arithmetic exactly");
    }

    #[test]
    fn no_driver_information_degrades_to_exactly_the_old_behaviour() {
        // GetDevicePeriod failed (None, None) or reported zeros: ask for the block period, as
        // inc 1.2 did — the fix never makes the uninformed case worse.
        assert_eq!(exclusive_period_ladder(64, 96_000, None, None), vec![6666]);
        assert_eq!(exclusive_period_ladder(64, 96_000, Some(0), Some(0)), vec![6666]);
    }

    #[test]
    fn a_default_below_the_reported_minimum_is_skipped_not_retried() {
        // Contradictory driver info (min 5 ms, default 2 ms): the clamped ask is tried, and the
        // default — which the endpoint's own minimum already rules out — is not wasted on it.
        let ladder = exclusive_period_ladder(64, 48_000, Some(2 * MS), Some(5 * MS));
        assert_eq!(ladder, vec![5 * MS]);
    }

    #[test]
    fn the_defect_89_device_asks_its_default_before_its_min() {
        // Behringer UMC 204HD, test004 attempt 2 (2026-09-28): GetDevicePeriod reports
        // default 10 ms / min 3 ms. Inc 1.3 asked the min-clamped ask FIRST — 3 ms — the driver
        // ACCEPTED it and then stalled ~33 ms every ~62.5 ms (159 late wakes / 10 s, half
        // throughput, drift −49 %, no clean tone). The default is the number its engine actually
        // runs (the same engine cadence soak-ran 2 h clean in shared mode), so it is rung 1 now;
        // the min-clamped ask survives as the fallback for a driver that refuses its own default.
        let ladder = exclusive_period_ladder(64, 96_000, Some(10 * MS), Some(3 * MS));
        assert_eq!(ladder, vec![10 * MS, 3 * MS]);
    }

    #[test]
    fn a_coarser_than_block_minimum_always_puts_the_default_first() {
        // The #89 rule generalised beyond the UMC's exact numbers: whenever the driver says it
        // needs an engine coarser than the sparq block, the ladder leads with the default.
        // 64 frames at 48 kHz = 1333 µs < min 5 ms ⇒ [default 10 ms, ask 5 ms].
        let ladder = exclusive_period_ladder(64, 48_000, Some(10 * MS), Some(5 * MS));
        assert_eq!(ladder, vec![10 * MS, 5 * MS]);
    }

    #[test]
    fn degenerate_inputs_never_yield_an_empty_or_nonpositive_ladder() {
        let ladder = exclusive_period_ladder(0, 0, Some(0), Some(-1));
        assert_eq!(ladder, vec![1], "rate 0 and block 0 clamp to the smallest legal period");
        assert!(exclusive_period_ladder(0, 48_000, None, None).iter().all(|p| *p > 0));
    }

    #[test]
    fn period_text_says_microseconds_below_a_millisecond_and_milliseconds_above() {
        assert_eq!(period_text(6666), "667 µs");
        assert_eq!(period_text(10 * MS), "10.000 ms");
        assert_eq!(period_text(MS), "1.000 ms");
        assert_eq!(period_text(9999), "1000 µs"); // just under a ms reads in µs (999.9 → 1000)
    }

    #[test]
    fn the_alignment_two_step_rounds_the_ask_up_never_down() {
        // #89's second face: the UMC answered a 10 ms ask with BUFFER_SIZE_NOT_ALIGNED and
        // reported granularity 288 fr (3 ms). Inc 1.3 followed the documented recipe literally —
        // re-ask the granularity itself — and landed on the period the driver cannot sustain.
        // The re-ask now keeps the ask's engine intent: 960 fr → ceil(960/288)×288 = 1152 fr
        // = 12.000 ms at 96 kHz, aligned AND sustainable-class.
        let aligned = align_up_frames(period_100ns_to_frames(10 * MS, 96_000), 288);
        assert_eq!(aligned, 1152);
        assert_eq!(frames_to_100ns(aligned, 96_000), 12 * MS);
        assert_eq!(period_text(frames_to_100ns(aligned, 96_000)), "12.000 ms");
    }

    #[test]
    fn align_up_keeps_the_documented_recipe_for_sub_granularity_asks() {
        // Inc 1.2's shape, unchanged: a 64-frame ask (666 µs @ 96 kHz) against a 288-fr
        // granularity lands on exactly one unit — the number the documented two-step exists for.
        assert_eq!(align_up_frames(64, 288), 288);
        // Exact multiples and already-aligned asks are fixed points; rounding is UP.
        assert_eq!(align_up_frames(288, 288), 288);
        assert_eq!(align_up_frames(1152, 288), 1152);
        assert_eq!(align_up_frames(1153, 288), 1440);
        // Degenerate drivers: granularity 0 leaves the ask alone; ask 0 lands on one unit.
        assert_eq!(align_up_frames(960, 0), 960);
        assert_eq!(align_up_frames(0, 288), 288);
    }

    #[test]
    fn frame_and_period_conversions_roundtrip_on_the_device_shapes() {
        // The two conversions use the HAL's standing integer arithmetic (round-half-up to
        // 100 ns; truncate back to frames) — pinned on the shapes test004 actually meets.
        assert_eq!(frames_to_100ns(960, 96_000), 10 * MS);
        assert_eq!(frames_to_100ns(288, 96_000), 3 * MS);
        assert_eq!(period_100ns_to_frames(10 * MS, 96_000), 960);
        assert_eq!(period_100ns_to_frames(3 * MS, 96_000), 288);
        assert_eq!(frames_to_100ns(64, 96_000), 6667, "round-half-up, not the ladder's truncate");
        assert_eq!(period_100ns_to_frames(0, 96_000), 1, "never a zero-frame ask");
        assert_eq!(frames_to_100ns(0, 96_000), 1, "never a zero period");
    }

    #[test]
    fn a_seriously_shrunk_allocation_is_the_third_lying_granularity_face() {
        // #89's third face: Initialize ACCEPTS the ask and GetBufferSize then reports something
        // much smaller — the granularity, without the NOT_ALIGNED handshake. 288 against 960 is
        // the observed shape and must fire the round-up-and-re-ask-once path.
        assert!(allocation_seriously_shrunk(960, 288));
        // Honest drivers stay out of the retry path: exact allocations, round-ups to alignment,
        // and small round-downs are all inside the three-quarter tolerance.
        assert!(!allocation_seriously_shrunk(960, 960));
        assert!(!allocation_seriously_shrunk(960, 1152));
        assert!(!allocation_seriously_shrunk(960, 720), "exactly 3/4 is not serious");
        assert!(allocation_seriously_shrunk(960, 719), "one frame under 3/4 is");
        assert!(allocation_seriously_shrunk(960, 0), "a zero allocation is maximally shrunk");
    }
}
