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

/// Candidate device periods for exclusive `Initialize`, in 100-nanosecond units (WASAPI's
/// `REFERENCE_TIME`), in the order they should be tried. Never empty, deduplicated.
///
/// `hw_default_100ns` / `hw_min_100ns` are what the endpoint said (`GetDevicePeriod` on the live
/// client — legal before `Initialize`); `block_frames` at `rate` is what sparq would like. The
/// order:
///
/// 1. **The block period, clamped up to the driver's minimum** — the honest low-latency ask. On
///    an endpoint whose minimum is at or below the block period (most internal cards, and any
///    driver with a sub-millisecond engine), this equals the old single ask, so the fix costs
///    those devices nothing.
/// 2. **The driver's default period** — the number the endpoint's own engine already runs at.
///    For a driver that reports a minimum it then refuses (the documented lying-`min` pattern),
///    this is the rung that opens. On #79's device min = default = 10 ms, so dedup lands there
///    directly, in ONE attempt: the ladder's first rung is already the driver's own number.
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
    let def = hw_default_100ns.filter(|v| *v > 0);

    let first = min.map_or(block, |m| block.max(m));
    let mut out: Vec<i64> = Vec::with_capacity(2);
    out.push(first);
    if let Some(d) = def {
        let not_below_min = !min.is_some_and(|m| d < m);
        if not_below_min && !out.contains(&d) {
            out.push(d);
        }
    }
    out
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
    fn a_minimum_above_the_block_clamps_the_ask_up() {
        // 64 frames at 96 kHz (666 µs) against a 3 ms minimum: rung 1 is the minimum, rung 2 the
        // (different) default.
        let ladder = exclusive_period_ladder(64, 96_000, Some(10 * MS), Some(3 * MS));
        assert_eq!(ladder, vec![3 * MS, 10 * MS]);
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
}
