//! The backend conformance suite: one set of behavioural promises every HAL backend must keep.
//!
//! WO-006 task 1 says the trait is written and *reviewed* before backends are implemented; this
//! module is the review made executable. A backend that passes conformance can be swapped for any
//! other without the layers above noticing — which is also exactly what the Phase 7 aggregator
//! will demand of its child streams.
//!
//! The suite is deliberately split:
//!
//! * [`conformance_offline`] — no wall-clock, no threads, no hardware required. Runs in CI for
//!   every compiled backend, and against null everywhere.
//! * [`conformance_live`] — paced operation; needs a real (or simulated) interrupt source. Runs
//!   against null in CI (the paced pump) and against real devices on the stage machine
//!   (`sparq devices --conformance`, WO-006 evidence list).
//!
//! Every check returns a `(name, passed, detail)` triple instead of panicking, so a failing
//! backend produces a *report*, not a stack trace — the same philosophy as `sparq selftest`.
//!
//! # Two rules this suite lives by
//!
//! 1. **Ask the device, don't assume it.** Every config the suite opens with is derived from the
//!    backend's own probed [`Capabilities`](crate::hal::Capabilities) — a suite that demands
//!    48 kHz/stereo from a 44.1 kHz endpoint measures the suite's assumptions, not the backend
//!    (defect #39: the first SATURN run failed exactly this way).
//! 2. **SKIP ≠ PASS ≠ FAIL, and each has one meaning.** Infrastructure missing (no counting
//!    allocator) is a FAIL — a gate that cannot fail is not a gate. The *device* legitimately
//!    lacking a capability (no exclusive mode on a remote endpoint) is a SKIP with a reason: the
//!    promise under test becomes the honesty of the refusal, and that much IS checked. Nothing
//!    ever skips silently.

// The live-conformance harness must sleep and read the clock: it is *observing* a paced stream
// from the outside (exactly like `device::null`'s harness). The audio path it measures — the
// callback — still sees neither clock nor thread primitives.
#![allow(clippy::disallowed_methods)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::device::StreamConfig;
use crate::hal::{
    BackendKind, Capabilities, Fault, FrameCtx, HalBackend, HalError, OpenOptions, ProcessFn,
};
use crate::rt::BlockStatus;

/// One conformance check result.
#[derive(Clone, Debug)]
pub struct Check {
    /// What was checked.
    pub name: &'static str,
    /// Did it pass?
    pub passed: bool,
    /// The measured evidence (numbers, not adjectives).
    pub detail: String,
}

/// A full conformance report.
pub type Report = Vec<Check>;

impl Check {
    fn pass(name: &'static str, detail: String) -> Self {
        Self { name, passed: true, detail }
    }

    fn fail(name: &'static str, detail: String) -> Self {
        Self { name, passed: false, detail }
    }

    /// A device-dependent skip: the promise could not be exercised because the *device*
    /// legitimately lacks the capability. Reported as passed-with-SKIP-detail (rule 2 above) —
    /// the honesty of the limitation was itself verified before skipping.
    fn skip(name: &'static str, detail: String) -> Self {
        Self { name, passed: true, detail: format!("SKIP (device-dependent): {detail}") }
    }
}

/// Render a report as the gate-table format the project already uses.
#[must_use]
pub fn format_report(backend: BackendKind, report: &[Check]) -> String {
    let mut s = format!("conformance · backend {backend}\n");
    let mut failed = 0;
    for c in report {
        s.push_str(&format!(
            "  [{}] {} — {}\n",
            if c.passed { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        ));
        failed += usize::from(!c.passed);
    }
    s.push_str(&format!(
        "conformance {backend}: {} ({} check(s), {failed} failed)\n",
        if failed == 0 { "PASS" } else { "FAIL" },
        report.len()
    ));
    s
}

/// A callback that counts invocations and writes a deterministic ramp — the suite's workhorse.
///
/// The ramp is scaled to ±0.001 (about -60 dBFS): conformance may run against real speakers
/// (`sparq devices --conformance` on the stage machine), and a test signal has no business being
/// loud. Determinism checks compare like against like, so the scale changes nothing.
fn counting_ramp(counter: Arc<AtomicU64>) -> ProcessFn {
    Box::new(move |f: &mut FrameCtx<'_>| {
        counter.fetch_add(1, Ordering::Relaxed);
        for (i, s) in f.out.iter_mut().enumerate() {
            *s = (((f.block.sample_offset as usize + i) % 997) as f32 / 997.0 - 0.5) * 0.002;
        }
        BlockStatus::Ok
    })
}

/// The config the suite should ask a device for: its own defaults where our preferences are not
/// supported (rule 1). Stereo where the device does stereo, its default rate where it refuses 48k.
#[must_use]
fn suite_config(caps: &Capabilities, kind: BackendKind) -> StreamConfig {
    let rate = if caps.supports_rate(48_000) { 48_000 } else { caps.default_rate.max(1) };
    let outputs = usize::from(caps.default_channels_out)
        .clamp(usize::from(caps.channels_out.0.max(1)), usize::from(caps.channels_out.1.max(1)));
    StreamConfig {
        sample_rate: rate,
        block_frames: 64,
        inputs: 0,
        outputs,
        exclusive: kind == BackendKind::WasapiExclusive,
    }
}

/// Offline conformance: enumeration, capabilities, manual stepping, faults, lifecycle, latency.
///
/// `paced_fallback`: device backends have no manual mode; pass `true` and the suite will use a
/// short paced run for the checks that need blocks to flow (clearly labelled in the details).
#[must_use]
pub fn conformance_offline(backend: &dyn HalBackend, paced_fallback: bool) -> Report {
    let mut r = Report::new();

    // ---- 1. enumeration -------------------------------------------------------------
    let devices = match backend.enumerate() {
        Ok(d) => d,
        Err(e) => {
            r.push(Check::fail("enumerate", format!("error: {e}")));
            return r;
        },
    };
    if devices.is_empty() {
        r.push(Check::fail(
            "enumerate",
            String::from("backend reports zero devices (a device-less runner must use `null`)"),
        ));
        return r;
    }
    let default_first = devices.iter().any(|d| d.is_default_output);
    r.push(Check::pass(
        "enumerate",
        format!(
            "{} device(s); default marked: {}; first is default: {}",
            devices.len(),
            devices.iter().filter(|d| d.is_default_output).count(),
            devices[0].is_default_output
        ),
    ));
    if !default_first {
        // Not fatal (a capture-only machine has no default *output*), but it must be visible.
        r.push(Check::fail(
            "default device is enumerated first",
            String::from("no default output among devices"),
        ));
    }

    let dev = &devices[0].id;

    // ---- 2. capabilities ------------------------------------------------------------
    let caps = match backend.capabilities(dev) {
        Ok(c) => {
            let ok = !c.rates.is_empty()
                && c.channels_out.1 >= c.channels_out.0
                && c.channels_out.1 >= 1;
            r.push((if ok { Check::pass } else { Check::fail })(
                "capabilities are probed, not guessed",
                c.summary().lines().next().unwrap_or("").to_string(),
            ));
            if c.exclusive {
                r.push(Check::pass(
                    "exclusive rates verified by probe",
                    format!("{:?}", c.exclusive_rates),
                ));
            }
            c
        },
        Err(e) => {
            r.push(Check::fail("capabilities", format!("error: {e}")));
            return r;
        },
    };

    let cfg = suite_config(&caps, backend.kind());

    // ---- 2b. the honest-refusal promise (exclusive backend, exclusive-less endpoint) ---
    if backend.kind() == BackendKind::WasapiExclusive && !caps.exclusive {
        // The endpoint has no verified exclusive rates (the RDP `Remote Audio` case). The
        // promise under test flips: open MUST fail with Format, named and explained — never
        // silently reroute to shared mode, never crash, never pretend.
        let probe = backend.open(
            dev,
            &cfg,
            &OpenOptions::default(),
            counting_ramp(Arc::new(AtomicU64::new(0))),
        );
        match probe {
            Err(HalError::Format(m)) => r.push(Check::pass(
                "exclusive-less endpoint is refused honestly (never silently rerouted)",
                format!("Format: {}", truncate(&m, 140)),
            )),
            Err(e) => r.push(Check::fail(
                "exclusive-less endpoint is refused honestly (never silently rerouted)",
                format!("refused with the wrong error class: {e}"),
            )),
            Ok(_) => r.push(Check::fail(
                "exclusive-less endpoint is refused honestly (never silently rerouted)",
                String::from(
                    "open SUCCEEDED despite no verified exclusive rates — caps or open is lying",
                ),
            )),
        }
        r.push(Check::skip(
            "start → blocks flow → stop",
            String::from(
                "endpoint offers no exclusive mode here; the null backend proves the lifecycle in full",
            ),
        ));
        r.push(Check::skip(
            "latency report matches the negotiated config",
            String::from("no stream to report on"),
        ));
        return r;
    }

    // ---- 3. open/start/pump/stop lifecycle -------------------------------------------
    let counter = Arc::new(AtomicU64::new(0));
    let opts = if paced_fallback {
        OpenOptions { paced: true, ..OpenOptions::default() }
    } else {
        OpenOptions::manual_with_capture(256)
    };
    let mut stream = match backend.open(dev, &cfg, &opts, counting_ramp(Arc::clone(&counter))) {
        Ok(s) => s,
        Err(e) => {
            r.push(Check::fail("open", format!("error: {}", e.with_hint())));
            return r;
        },
    };
    r.push(Check::pass(
        "open allocates buffers up front",
        format!(
            "actual {} Hz / {} fr / {} ch vs requested {} / {} / {}",
            stream.actual_config().sample_rate,
            stream.actual_config().block_frames,
            stream.actual_config().outputs,
            cfg.sample_rate,
            cfg.block_frames,
            cfg.outputs
        ),
    ));

    let pumped = if paced_fallback {
        match stream.start() {
            Ok(()) => {
                std::thread::sleep(Duration::from_millis(120));
                let _ = stream.stop();
                Ok(stream.diag().blocks)
            },
            Err(e) => Err(e),
        }
    } else {
        stream.start().and_then(|()| stream.pump(16))
    };
    match pumped {
        Ok(n) if n >= 8 => {
            let after = stream.stop();
            r.push(Check::pass(
                "start → blocks flow → stop",
                format!(
                    "{n} block(s), callback invocations {}; stop: {:?}",
                    counter.load(Ordering::Relaxed),
                    after.map(|()| "ok")
                ),
            ));
        },
        Ok(n) => r.push(Check::fail(
            "start → blocks flow → stop",
            format!("only {n} blocks — the pump is not pumping"),
        )),
        Err(e) => {
            r.push(Check::fail("start → blocks flow → stop", format!("error: {}", e.with_hint())))
        },
    }

    // ---- 4. latency report is consistent with the negotiated config ------------------
    let lat = stream.latency_report();
    let ac = stream.actual_config();
    let consistent = lat.sample_rate == ac.sample_rate
        && lat.block_frames == ac.block_frames as u32
        && lat.output_pipeline_frames >= lat.block_frames;
    r.push((if consistent { Check::pass } else { Check::fail })(
        "latency report matches the negotiated config",
        lat.summary(),
    ));

    // ---- 5. error report is readable from every state --------------------------------
    let rep = stream.error_report();
    r.push(Check::pass(
        "error report readable after stop",
        format!(
            "state {:?}, device_errors {}, rt [{}]",
            rep.state,
            rep.device_errors,
            rep.rt.summary()
        ),
    ));

    // ---- 6. diagnostics saw the blocks -----------------------------------------------
    let d = stream.diag();
    let ok = d.blocks > 0 && d.allocations == 0;
    r.push((if ok { Check::pass } else { Check::fail })(
        "diagnostics counted the run; callback allocated nothing",
        d.summary(),
    ));

    // ---- 7. fault injection (null; device backends refuse honestly) --------------------
    if !paced_fallback {
        let reopen = backend.open(
            dev,
            &cfg,
            &OpenOptions::manual_with_capture(0),
            counting_ramp(Arc::new(AtomicU64::new(0))),
        );
        match reopen {
            Ok(mut s2) => {
                let _ = s2.start();
                let injected = s2.inject_fault(Fault::XrunNext);
                let _ = s2.pump(2);
                let xruns = s2.diag().xruns;
                let _ = s2.stop();
                match injected {
                    Ok(()) => r.push((if xruns == 1 { Check::pass } else { Check::fail })(
                        "injected xrun is counted exactly once",
                        format!("xruns {xruns}"),
                    )),
                    Err(HalError::Unsupported(_)) => r.push(Check::pass(
                        "fault injection refused honestly (real hardware: unplug test is manual per WO-006)",
                        String::from("Unsupported"),
                    )),
                    Err(e) => {
                        r.push(Check::fail("fault injection", format!("unexpected error: {e}")))
                    },
                }
            },
            Err(e) => r.push(Check::fail("reopen after stop", format!("error: {e}"))),
        }
    }

    r
}

/// Reopen cycles must not leak (WO-006 acceptance: "switching on stop/start must not leak").
///
/// Needs the counting allocator installed (test binaries and `sparq-app` do). Without it the
/// check FAILS as skipped — infrastructure gaps are failures, never silent passes.
///
/// Counting is armed **process-wide** for the duration of the cycles, because the lifecycle
/// crosses threads: the control thread opens and drops, the pump thread runs. The price is the
/// quiescent-process requirement — unrelated threads allocating during the window are counted
/// too — so this runs in the CLI gates (`selftest`, `devices --conformance`), never inside a
/// parallel test binary. A pump thread leaking *internally* mid-block remains covered by the
/// per-block `allocations` counter, which is armed inside the callback.
///
/// Two defects taught the current shape, and both are worth naming because both produced a
/// confident wrong answer:
///
/// * **#44** — thread-local arming counted an allocation on the control thread but not its free
///   on the pump thread. Fixed by arming process-wide (176 → 152 phantoms).
/// * **#48** — the residual 152 was not a thread boundary at all: `realloc` bumped the allocation
///   counter alone, so every `format!`/`Vec` growth in the diagnostics text left the balance one
///   higher, permanently. Fixed in `alloc.rs` (a realloc now moves neither counter); proven by
///   `outstanding_balance_survives_realloc_growth`.
///
/// The lesson is in the gate's own spirit: a leak counter that has never returned zero on a
/// backend it should trust is measuring itself, not the backend.
#[must_use]
pub fn reopen_cycles_do_not_leak(backend: &dyn HalBackend, cycles: u32) -> Check {
    const NAME: &str = "reopen cycles do not leak";
    if !crate::alloc::audit_is_live() {
        return Check::fail(
            NAME,
            String::from("SKIPPED-AS-FAIL: counting allocator not installed in this binary"),
        );
    }
    let devices = match backend.enumerate() {
        Ok(d) if !d.is_empty() => d,
        _ => return Check::fail(NAME, String::from("no device to cycle")),
    };
    let caps = match backend.capabilities(&devices[0].id) {
        Ok(c) => c,
        Err(e) => return Check::fail(NAME, format!("capabilities: {e}")),
    };
    let cfg = suite_config(&caps, backend.kind());
    let paced = backend.kind() != BackendKind::Null;
    let opts = if paced {
        OpenOptions {
            paced: true,
            capture_frames: 0,
            log_negotiation: false,
            ..OpenOptions::default()
        }
    } else {
        OpenOptions { log_negotiation: false, ..OpenOptions::manual_with_capture(0) }
    };

    // One warm-up cycle: lazy one-time initialisations (thread info, allocator arenas, COM) must
    // not be mistaken for a leak. If the warm-up open is refused by the DEVICE (no exclusive
    // mode on this endpoint, format refusal, device held elsewhere), that is a device-dependent
    // skip with a reason — distinct from the allocator-missing FAIL above.
    {
        let proc: ProcessFn = Box::new(|_: &mut FrameCtx<'_>| BlockStatus::Ok);
        match backend.open(&devices[0].id, &cfg, &opts, proc) {
            Ok(mut s) => {
                let _ = s.start();
                if paced {
                    std::thread::sleep(Duration::from_millis(30));
                } else {
                    let _ = s.pump(2);
                }
                let _ = s.stop();
            },
            Err(e @ (HalError::Format(_) | HalError::Busy(_) | HalError::Unsupported(_))) => {
                return Check::skip(
                    NAME,
                    format!(
                        "open refused on this endpoint ({}); the null backend proves the cycle \
                         in full",
                        truncate(&e.to_string(), 90)
                    ),
                );
            },
            Err(e) => return Check::fail(NAME, format!("open: {e}")),
        }
    }

    // Process-wide arming, not thread-local: the cycles allocate on THIS thread but the paced
    // pump frees parts of the spawn machinery on ITS thread — thread-local counting sees one
    // side of those pairs and reports phantom leaks (defect #44: 22 per cycle on the WASAPI
    // gate, every byte of it freed). Requires a quiescent process, which the CLI gates are.
    let _base = crate::alloc::start_counting_all_threads();
    for _ in 0..cycles {
        let proc: ProcessFn = Box::new(|_: &mut FrameCtx<'_>| BlockStatus::Ok);
        let Ok(mut s) = backend.open(&devices[0].id, &cfg, &opts, proc) else {
            crate::alloc::stop_counting_all_threads();
            return Check::fail(NAME, String::from("open failed mid-cycle"));
        };
        let _ = s.start();
        if paced {
            std::thread::sleep(Duration::from_millis(15));
        } else {
            let _ = s.pump(1);
        }
        let _ = s.stop();
        drop(s);
    }
    let outstanding = crate::alloc::outstanding_allocations();
    crate::alloc::stop_counting_all_threads();

    if outstanding == 0 {
        Check::pass(
            NAME,
            format!("{cycles} open/start/run/stop/drop cycles, 0 outstanding allocations"),
        )
    } else {
        Check::fail(NAME, format!("{outstanding} allocation(s) outlived {cycles} cycles"))
    }
}

/// Live conformance: paced operation with jitter and drift measurement.
///
/// Runs `seconds` of paced audio and asserts the pump produced at least 80 % of the expected
/// blocks with zero allocations. Xrun *tolerance* is a parameter: null on a loaded CI runner may
/// legitimately miss a deadline; the stage-machine acceptance run passes 0 and means it.
#[must_use]
pub fn conformance_live(
    backend: &dyn HalBackend,
    cfg: &StreamConfig,
    seconds: f64,
    xrun_tolerance: u64,
) -> Report {
    let mut r = Report::new();
    let devices = match backend.enumerate() {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => {
            r.push(Check::fail("live: device present", String::from("backend enumerates nothing")));
            return r;
        },
        Err(e) => {
            r.push(Check::fail("live: enumerate", format!("{e}")));
            return r;
        },
    };
    let opts = OpenOptions { paced: true, ..OpenOptions::default() };
    let counter = Arc::new(AtomicU64::new(0));
    let mut stream =
        match backend.open(&devices[0].id, cfg, &opts, counting_ramp(Arc::clone(&counter))) {
            Ok(s) => s,
            Err(e) => {
                r.push(Check::fail("live: open", e.with_hint()));
                return r;
            },
        };
    if let Err(e) = stream.start() {
        r.push(Check::fail("live: start", e.with_hint()));
        return r;
    }
    std::thread::sleep(Duration::from_secs_f64(seconds));
    let _ = stream.stop();

    let d = stream.diag();
    let expected = (seconds * f64::from(cfg.sample_rate) / cfg.block_frames as f64) as u64;
    let blocks_ok = d.blocks >= (expected * 8) / 10;
    r.push((if blocks_ok { Check::pass } else { Check::fail })(
        "live: the pump kept pace",
        format!("{} blocks in {seconds:.1} s (expected ~{expected})", d.blocks),
    ));
    let alloc_ok = d.allocations == 0;
    r.push((if alloc_ok { Check::pass } else { Check::fail })(
        "live: zero allocations on the audio path",
        format!("alloc {}", d.allocations),
    ));
    let xrun_ok = d.xruns <= xrun_tolerance;
    r.push((if xrun_ok { Check::pass } else { Check::fail })(
        "live: xruns within tolerance",
        format!(
            "xruns {} (tolerance {xrun_tolerance}), late wakes {}, overruns {}",
            d.xruns, d.late_wakes, d.overruns
        ),
    ));
    r.push(Check::pass("live: diagnostics", d.summary()));
    r
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::hal::null::NullBackend;

    #[test]
    fn null_backend_passes_full_offline_conformance() {
        let b = NullBackend::new();
        let report = conformance_offline(&b, false);
        let text = format_report(b.kind(), &report);
        assert!(
            report.iter().all(|c| c.passed),
            "null backend must pass every offline check:\n{text}"
        );
        // The suite must contain the checks the acceptance criteria name — a conformance suite
        // that quietly shrinks is how backends rot.
        let names: Vec<&str> = report.iter().map(|c| c.name).collect();
        assert!(names.contains(&"enumerate"));
        assert!(names.contains(&"start → blocks flow → stop"));
        assert!(names.contains(&"latency report matches the negotiated config"));
        assert!(names.contains(&"injected xrun is counted exactly once"));
    }

    #[test]
    fn suite_config_uses_device_defaults_when_48k_stereo_is_not_on_offer() {
        // The defect-#39 regression: a 44.1 kHz/4ch-only endpoint must be asked for what it has.
        let caps = Capabilities {
            rates: vec![crate::hal::RateRange { min: 44_100, max: 44_100 }],
            exclusive_rates: vec![],
            channels_out: (4, 4),
            channels_in: (0, 0),
            default_rate: 44_100,
            default_channels_out: 4,
            exclusive: false,
            event_driven: true,
            full_duplex: false,
            hw_period: None,
            hw_period_min: None,
            f32_native: true,
        };
        let cfg = suite_config(&caps, BackendKind::WasapiShared);
        assert_eq!(cfg.sample_rate, 44_100, "must fall back to the device default rate");
        assert_eq!(
            cfg.outputs, 4,
            "must use the device's channel truth, clamped to what it offers"
        );
        // and where 48k/stereo IS offered, the preference stands
        let caps48 = Capabilities {
            rates: vec![crate::hal::RateRange { min: 8_000, max: 384_000 }],
            channels_out: (1, 64),
            default_rate: 48_000,
            default_channels_out: 2,
            ..caps
        };
        let cfg48 = suite_config(&caps48, BackendKind::Null);
        assert_eq!((cfg48.sample_rate, cfg48.outputs), (48_000, 2));
    }

    #[test]
    fn null_backend_passes_live_conformance() {
        let b = NullBackend::new();
        let cfg = StreamConfig {
            sample_rate: 48_000,
            block_frames: 128,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        };
        // CI runners are shared and spiky: the paced null pump tolerates a few late wakes here.
        // The stage-machine soak (scripts/soak.bat + hal.bat) runs with tolerance 0 and is the
        // acceptance evidence; this test proves the plumbing, not the hardware.
        let report = conformance_live(&b, &cfg, 0.3, 5);
        let text = format_report(b.kind(), &report);
        assert!(report.iter().all(|c| c.passed), "live conformance failed:\n{text}");
    }

    #[test]
    fn conformance_report_names_every_failure() {
        // A report is for reading: failures must carry the measured number, not just "failed".
        let c = Check::fail("demo", String::from("xruns 3 (tolerance 0)"));
        let text = format_report(BackendKind::Null, &[c]);
        assert!(text.contains("FAIL"));
        assert!(text.contains("xruns 3"));
        assert!(text.contains("1 failed"));
    }

    #[test]
    fn skips_are_labelled_and_never_silent() {
        let s = Check::skip("demo", String::from("endpoint offers no exclusive mode"));
        assert!(s.passed, "a skip is not a failure...");
        assert!(s.detail.starts_with("SKIP"), "...and it must SAY it is a skip");
        assert!(s.detail.contains("exclusive"), "with the reason");
    }
}
