//! HAL diagnostics: the numbers behind every latency and reliability claim (WO-006 task 5).
//!
//! Design constraints, in the order they bite:
//!
//! 1. **The writer is the audio pump.** Recording must not allocate, lock, block or syscall.
//!    Everything here is relaxed atomics — one `fetch_add` per event, one `fetch_max` per block.
//! 2. **The reader is anyone, anytime** (CLI status line, future overlay, soak report), from
//!    another thread, while the pump runs. Snapshots may therefore be *slightly* inconsistent
//!    across fields (no seqlock ceremony for a diagnostics readout); every field is individually
//!    atomic, so no field is ever torn. That trade is documented, deliberate, and the same one
//!    the bootstrap telemetry made in Phase A.
//! 3. **Percentiles without storing samples.** A 2-hour soak at 96 kHz/64 is 10.8 M blocks; we
//!    will not keep 10.8 M durations. The histogram is 32 power-of-two buckets over nanoseconds
//!    (bucket *b* covers `[2^b, 2^{b+1})` ns, i.e. 1 ns … 4.3 s), so p50/p99/max are *upper
//!    bounds* good to a factor of two — precise enough to answer "is the callback inside budget?"
//!    which is the only question a histogram is for. Exact min/max/mean of callback *jitter* is
//!    kept separately, because jitter is the number that distinguishes a healthy event-driven
//!    stream from a polling disaster, and it needs no buckets.
//!
//! `DiagSnapshot::summary` is the one-line console readout; `lines` is the multi-line report the
//! soak and `--report-every` paths print.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::Duration;

use crate::rt::thread::RtReport;
use crate::rt::BlockStatus;

/// Number of log2 histogram buckets (ns scale: bucket b = `[2^b, 2^{b+1})` ns).
pub const HIST_BUCKETS: usize = 32;

/// Lock-free diagnostics recorder. Owned (via `Arc`) by a stream; written by its pump thread.
pub struct DiagRecorder {
    rate: AtomicU32,
    blocks: AtomicU64,
    xruns: AtomicU64,
    overruns: AtomicU64,
    late_wakes: AtomicU64,
    device_errors: AtomicU64,
    allocations: AtomicU64,
    hist: [AtomicU64; HIST_BUCKETS],
    worst_ns: AtomicU64,
    jitter_min_ns: AtomicU64,
    jitter_max_ns: AtomicU64,
    jitter_sum_ns: AtomicU64,
    jitter_count: AtomicU64,
    /// Consumption truth: total frames the device has ACCEPTED (every backend counts its own
    /// releases — defect #76: WASAPI's `IAudioClock::GetPosition` advances in buffer-sized
    /// steps per event tick and was removed as a drift source, not repaired).
    device_frames: AtomicU64,
    /// Wall time (ns since stream start) at which `device_frames` was read.
    wall_ns: AtomicU64,
    /// Bit-packed [`RtReport`] (bit0 mmcss, bit1 priority, bit2 ideal-proc, bit3 workset).
    rt_flags: AtomicU8,
}

impl DiagRecorder {
    /// A recorder for a stream at `rate` Hz. Allocations happen here (control thread), never later.
    #[must_use]
    pub fn new(rate: u32) -> Self {
        Self {
            rate: AtomicU32::new(rate),
            blocks: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            late_wakes: AtomicU64::new(0),
            device_errors: AtomicU64::new(0),
            allocations: AtomicU64::new(0),
            hist: std::array::from_fn(|_| AtomicU64::new(0)),
            worst_ns: AtomicU64::new(0),
            jitter_min_ns: AtomicU64::new(u64::MAX),
            jitter_max_ns: AtomicU64::new(0),
            jitter_sum_ns: AtomicU64::new(0),
            jitter_count: AtomicU64::new(0),
            device_frames: AtomicU64::new(0),
            wall_ns: AtomicU64::new(0),
            rt_flags: AtomicU8::new(0),
        }
    }

    /// Bucket index for a duration: `floor(log2(ns))`, clamped to `0..HIST_BUCKETS`.
    #[must_use]
    pub fn bucket_of(d: Duration) -> usize {
        let ns = d.as_nanos().min(u128::from(u64::MAX)) as u64;
        if ns == 0 {
            0
        } else {
            (63 - ns.leading_zeros()) as usize % HIST_BUCKETS
        }
    }

    /// Upper bound (ns) of a bucket — the conservative percentile value.
    #[must_use]
    pub const fn bucket_upper_ns(b: usize) -> u64 {
        if b + 1 >= 64 {
            u64::MAX
        } else {
            1u64 << (b + 1)
        }
    }

    /// Record one processed callback: its duration, what it returned, the budget it was up
    /// against, and how many allocations the audit caught inside it.
    pub fn record_callback(
        &self,
        dur: Duration,
        status: BlockStatus,
        budget: Option<Duration>,
        allocs: u64,
    ) {
        self.blocks.fetch_add(1, Ordering::Relaxed);
        self.hist[Self::bucket_of(dur)].fetch_add(1, Ordering::Relaxed);
        let ns = dur.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.worst_ns.fetch_max(ns, Ordering::Relaxed);
        if allocs > 0 {
            self.allocations.fetch_add(allocs, Ordering::Relaxed);
        }
        match status {
            BlockStatus::Ok => {},
            BlockStatus::Underrun | BlockStatus::ControlDrop => {
                self.xruns.fetch_add(1, Ordering::Relaxed);
            },
            BlockStatus::DeviceError => {
                self.device_errors.fetch_add(1, Ordering::Relaxed);
            },
        }
        if let Some(b) = budget {
            if dur > b {
                // A budget overrun is recorded separately from an xrun: overrunning the *callback*
                // budget only becomes audible when the device pipeline runs dry. Conflating them
                // would hide exactly the headroom question the soak exists to answer.
                self.overruns.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record the wake-to-wake interval of the pump against the expected device period.
    pub fn record_wake(&self, gap: Duration, expected: Duration) {
        let g = gap.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.jitter_min_ns.fetch_min(g, Ordering::Relaxed);
        self.jitter_max_ns.fetch_max(g, Ordering::Relaxed);
        self.jitter_sum_ns.fetch_add(g, Ordering::Relaxed);
        self.jitter_count.fetch_add(1, Ordering::Relaxed);
        // A wake later than 1.5 expected periods means the device consumed data we had not
        // produced yet (or the scheduler ate our deadline): either way, count it — silently.
        if !expected.is_zero() && gap > expected + expected / 2 {
            self.late_wakes.fetch_add(1, Ordering::Relaxed);
            self.xruns.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a consumption reading: cumulative frames the device has accepted, and the wall
    /// time (since stream start) at which that count stood. The pair becomes `drift_ppm`:
    /// throughput vs the negotiated rate — the number that says whether the device is really
    /// consuming what `Initialize` accepted (defect #76's re-derivation).
    pub fn record_device_clock(&self, device_frames: u64, wall: Duration) {
        self.device_frames.store(device_frames, Ordering::Relaxed);
        self.wall_ns.store(wall.as_nanos().min(u128::from(u64::MAX)) as u64, Ordering::Relaxed);
    }

    /// Record a device-level error (HRESULT on Windows, simulated removal on null).
    pub fn record_device_error(&self) {
        self.device_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an injected/simulated xrun (null backend fault injection; WASAPI padding anomalies).
    pub fn record_xrun(&self) {
        self.xruns.fetch_add(1, Ordering::Relaxed);
    }

    /// Publish the RT discipline the pump thread actually achieved.
    pub fn set_rt_report(&self, r: RtReport) {
        let mut bits = 0u8;
        if r.mmcss {
            bits |= 1;
        }
        if r.priority {
            bits |= 2;
        }
        if r.ideal_processor {
            bits |= 4;
        }
        if r.working_set {
            bits |= 8;
        }
        self.rt_flags.store(bits, Ordering::Relaxed);
    }

    /// Decode a packed [`RtReport`].
    #[must_use]
    pub fn rt_report_from_flags(bits: u8) -> RtReport {
        RtReport {
            mmcss: bits & 1 != 0,
            priority: bits & 2 != 0,
            ideal_processor: bits & 4 != 0,
            working_set: bits & 8 != 0,
        }
    }

    /// The RT report the pump published.
    #[must_use]
    pub fn rt_report(&self) -> RtReport {
        Self::rt_report_from_flags(self.rt_flags.load(Ordering::Relaxed))
    }

    /// Take a snapshot. Cheap, lock-free, callable from any thread at any time.
    #[must_use]
    pub fn snapshot(&self) -> DiagSnapshot {
        let counts: [u64; HIST_BUCKETS] =
            std::array::from_fn(|i| self.hist[i].load(Ordering::Relaxed));
        let n: u64 = counts.iter().sum();
        let pct = |q: f64| -> Option<u64> {
            if n == 0 {
                return None;
            }
            let rank = ((n as f64) * q).ceil().max(1.0) as u64;
            let mut seen = 0u64;
            for (b, &c) in counts.iter().enumerate() {
                seen += c;
                if seen >= rank {
                    return Some(Self::bucket_upper_ns(b));
                }
            }
            Some(Self::bucket_upper_ns(HIST_BUCKETS - 1))
        };

        let jc = self.jitter_count.load(Ordering::Relaxed);
        let jitter_min = self.jitter_min_ns.load(Ordering::Relaxed);
        let rate = self.rate.load(Ordering::Relaxed);
        let device_frames = self.device_frames.load(Ordering::Relaxed);
        let wall_ns = self.wall_ns.load(Ordering::Relaxed);
        // Drift: accepted frames vs wall-time expectation (defect #76's derivation — the pump
        // counts what the device took, never what a device clock claims). Two atomic loads ⇒
        // the pair may be one read apart; at 96 kHz that is ~10 µs of staleness — noise against
        // a ppm figure measured over seconds. Documented, deliberate.
        let drift_ppm = if wall_ns > 0 && rate > 0 {
            let expected = wall_ns as f64 * f64::from(rate) / 1e9;
            Some((device_frames as f64 - expected) / expected.max(1.0) * 1e6)
        } else {
            None
        };
        // Plausibility bound: real clocks sit inside ±100 ppm; beyond 1 % the stream's
        // throughput does not match its negotiated rate — the device is consuming what
        // `Initialize` accepted at some OTHER rate (test004 attempt 3: half throughput against
        // a 96 kHz exclusive open, windows-notes.md §4e), or sustained starvation kept feeding
        // it silence it still accepted (the xrun/late-wake counts say which; the two stories
        // must never share one label again — the old GetPosition derivation read +1.2 M ppm on
        // a soaked-clean SHARED stream, §4b). The verdict waits for the trust window: frames
        // are counted in whole device periods, so on short runs a big ratio is quantisation,
        // not evidence (DRIFT_TRUST_NS carries the arithmetic).
        let drift_suspect =
            drift_ppm.is_some_and(|d| d.abs() > DRIFT_SUSPECT_PPM) && wall_ns >= DRIFT_TRUST_NS;

        DiagSnapshot {
            rate,
            blocks: self.blocks.load(Ordering::Relaxed),
            xruns: self.xruns.load(Ordering::Relaxed),
            overruns: self.overruns.load(Ordering::Relaxed),
            late_wakes: self.late_wakes.load(Ordering::Relaxed),
            device_errors: self.device_errors.load(Ordering::Relaxed),
            allocations: self.allocations.load(Ordering::Relaxed),
            p50_ns: pct(0.50),
            p99_ns: pct(0.99),
            max_ns: (n > 0).then(|| self.worst_ns.load(Ordering::Relaxed)),
            jitter_min_ns: if jc > 0 { Some(jitter_min) } else { None },
            jitter_avg_ns: self.jitter_sum_ns.load(Ordering::Relaxed).checked_div(jc),
            jitter_max_ns: if jc > 0 {
                Some(self.jitter_max_ns.load(Ordering::Relaxed))
            } else {
                None
            },
            jitter_count: jc,
            drift_ppm,
            drift_suspect,
            rt: Self::rt_report_from_flags(self.rt_flags.load(Ordering::Relaxed)),
            hist: counts,
        }
    }
}

/// Above this magnitude, "drift" is not a clock error but a throughput that does not match the
/// negotiated rate (1 %). Real crystal oscillators sit within ±100 ppm; a resampling engine
/// mismatch might reach a few hundred. Beyond 1 % something structural is true — the device
/// clocks at a different rate than it accepted, or the stream spent its life starving.
pub const DRIFT_SUSPECT_PPM: f64 = 10_000.0;

/// Wall time a stream must have run before `drift_suspect` may fire (2 s). The numerator counts
/// whole device periods: at 96 kHz a 10 ms period is 960 frames, so the quantisation of ONE
/// period over a 2 s window is ±5 000 ppm — half the suspect threshold. Below the floor a large
/// ratio is arithmetic, not a verdict; the number still prints, the label does not. Short runs
/// (the conformance fallback, a 1 s play) are exactly what this protects.
pub const DRIFT_TRUST_NS: u64 = 2_000_000_000;

/// An immutable diagnostics readout.
#[derive(Clone, Debug)]
pub struct DiagSnapshot {
    /// Stream sample rate.
    pub rate: u32,
    /// Callbacks processed.
    pub blocks: u64,
    /// Device-level xruns (late wakes + injected + driver-reported).
    pub xruns: u64,
    /// Callback budget overruns (cause; audible only when the pipeline runs dry).
    pub overruns: u64,
    /// Wakes later than 1.5× the expected period.
    pub late_wakes: u64,
    /// Device errors.
    pub device_errors: u64,
    /// Allocations caught inside callbacks by the audit.
    pub allocations: u64,
    /// Median callback duration, bucket upper bound (ns). `None` before the first block.
    pub p50_ns: Option<u64>,
    /// 99th-percentile callback duration, bucket upper bound (ns).
    pub p99_ns: Option<u64>,
    /// Worst callback duration, exact (ns).
    pub max_ns: Option<u64>,
    /// Exact min wake-to-wake interval (ns), `None` before the second block.
    pub jitter_min_ns: Option<u64>,
    /// Mean wake-to-wake interval (ns).
    pub jitter_avg_ns: Option<u64>,
    /// Exact max wake-to-wake interval (ns).
    pub jitter_max_ns: Option<u64>,
    /// Wake intervals observed.
    pub jitter_count: u64,
    /// Accepted-frames throughput vs wall clock, in ppm (`None` until the backend reports its
    /// first consumption reading). ≈ 0 ppm is a stream running at its negotiated rate.
    pub drift_ppm: Option<f64>,
    /// `drift_ppm` exceeded [`DRIFT_SUSPECT_PPM`] past the [`DRIFT_TRUST_NS`] window: the
    /// device is not consuming at the negotiated rate (or the stream starved sustainably —
    /// the xrun counters discriminate). Displayed AND acted on: the play/soak verdicts treat
    /// it as not-clean, because a stream at half its rate is not a passing stream.
    pub drift_suspect: bool,
    /// RT discipline actually applied on the pump thread.
    pub rt: RtReport,
    /// Raw histogram (bucket b = `[2^b, 2^{b+1})` ns).
    pub hist: [u64; HIST_BUCKETS],
}

impl DiagSnapshot {
    /// Is every discipline counter clean? (The soak's fail-fast condition.)
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.xruns == 0 && self.device_errors == 0 && self.allocations == 0 && self.late_wakes == 0
    }

    /// One-line readout for status lines.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "blocks {} · xrun {} · overrun {} · late {} · dev-err {} · alloc {} · p99 {} · jitter {} · drift {}",
            self.blocks,
            self.xruns,
            self.overruns,
            self.late_wakes,
            self.device_errors,
            self.allocations,
            self.p99_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.jitter_max_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.drift_text(),
        )
    }

    /// Multi-line report for soak logs and `sparq devices`-style readouts.
    #[must_use]
    pub fn lines(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("  blocks       {}\n", self.blocks));
        s.push_str(&format!(
            "  xruns        {} (late wakes {}, budget overruns {}, device errors {})\n",
            self.xruns, self.late_wakes, self.overruns, self.device_errors
        ));
        s.push_str(&format!(
            "  callback     p50 {} · p99 {} · max {}   (histogram buckets are powers of two: values are upper bounds)\n",
            self.p50_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.p99_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.max_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
        ));
        s.push_str(&format!(
            "  jitter       min {} · avg {} · max {} over {} wakes\n",
            self.jitter_min_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.jitter_avg_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.jitter_max_ns.map(fmt_ns).unwrap_or_else(|| String::from("—")),
            self.jitter_count,
        ));
        s.push_str(&format!("  drift        {}\n", self.drift_text_long()));
        s.push_str(&format!("  allocations  {} on the audio path\n", self.allocations));
        s.push_str(&format!("  rt setup     {}\n", self.rt.summary()));
        s
    }

    /// Short drift readout for one-line summaries, carrying the suspect label.
    #[must_use]
    pub fn drift_text(&self) -> String {
        match self.drift_ppm {
            Some(d) if self.drift_suspect => format!("{d:+.0} ppm SUSPECT"),
            Some(d) => format!("{d:+.1} ppm"),
            None => String::from("not reported"),
        }
    }

    /// Long drift readout for report blocks. A suspect reading says what it now MEANS: the
    /// number is throughput truth (defect #76's derivation), so the verdict is actionable.
    #[must_use]
    pub fn drift_text_long(&self) -> String {
        match self.drift_ppm {
            Some(d) if self.drift_suspect => format!(
                "{d:+.2} ppm accepted frames vs wall — SUSPECT: beyond ±{DRIFT_SUSPECT_PPM:.0} ppm \
                 the throughput does not match the negotiated rate — the device is consuming \
                 what Initialize accepted at some other rate (the exclusive half-throughput \
                 signature, windows-notes.md §4e), or sustained starvation fed it silence — \
                 the late-wake and xrun counts say which"
            ),
            Some(d) => format!("{d:+.2} ppm accepted frames vs wall"),
            None => String::from("not reported by backend"),
        }
    }

    /// A compact ASCII histogram of the non-empty buckets — the console/overlay readout
    /// (WO-006: "diagnostics are visible without a debugger").
    #[must_use]
    pub fn histogram_text(&self) -> String {
        let max = self.hist.iter().copied().max().unwrap_or(0).max(1);
        let mut s = String::new();
        for (b, &c) in self.hist.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let bar = "█".repeat(((c * 40) / max).max(1) as usize);
            s.push_str(&format!(
                "  {:>9} {:>9} | {bar} {}\n",
                fmt_ns(1u64 << b),
                fmt_ns(1u64 << (b + 1)),
                c
            ));
        }
        if s.is_empty() {
            s.push_str("  (no blocks recorded yet)\n");
        }
        s
    }
}

/// Format nanoseconds for humans: ns / µs / ms with one decimal.
#[must_use]
pub fn fmt_ns(ns: u64) -> String {
    if ns < 1_000 {
        format!("{ns} ns")
    } else if ns < 1_000_000 {
        format!("{:.1} µs", ns as f64 / 1e3)
    } else if ns < 1_000_000_000 {
        format!("{:.2} ms", ns as f64 / 1e6)
    } else {
        format!("{:.3} s", ns as f64 / 1e9)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn bucket_of_is_log2_and_clamped() {
        assert_eq!(DiagRecorder::bucket_of(Duration::from_nanos(0)), 0);
        assert_eq!(DiagRecorder::bucket_of(Duration::from_nanos(1)), 0);
        assert_eq!(DiagRecorder::bucket_of(Duration::from_nanos(2)), 1);
        assert_eq!(DiagRecorder::bucket_of(Duration::from_nanos(1023)), 9);
        assert_eq!(DiagRecorder::bucket_of(Duration::from_nanos(1024)), 10);
        assert_eq!(DiagRecorder::bucket_of(Duration::from_secs(10)), 33 % HIST_BUCKETS);
        // upper bounds are powers of two and cover the bucket
        assert_eq!(DiagRecorder::bucket_upper_ns(10), 2048);
    }

    #[test]
    fn percentiles_are_conservative_upper_bounds() {
        let r = DiagRecorder::new(48_000);
        // 90 callbacks at ~100 µs (bucket 16: 65.5..131 µs) and 10 at ~10 ms (bucket 23)
        for _ in 0..90 {
            r.record_callback(Duration::from_nanos(100_000), BlockStatus::Ok, None, 0);
        }
        for _ in 0..10 {
            r.record_callback(Duration::from_nanos(10_000_000), BlockStatus::Ok, None, 0);
        }
        let s = r.snapshot();
        assert_eq!(s.blocks, 100);
        // p50 lands in the 100 µs bucket: 2^17 = 131072 ns upper bound
        assert_eq!(s.p50_ns, Some(1 << 17));
        // p99 lands in the 10 ms bucket: 2^24 upper bound
        assert_eq!(s.p99_ns, Some(1 << 24));
        // max is exact
        assert_eq!(s.max_ns, Some(10_000_000));
        assert!(s.is_clean());
    }

    #[test]
    fn budget_overruns_count_separately_from_xruns() {
        let r = DiagRecorder::new(96_000);
        let budget = Some(Duration::from_micros(666));
        r.record_callback(Duration::from_micros(700), BlockStatus::Ok, budget, 0);
        let s = r.snapshot();
        assert_eq!(s.overruns, 1, "over budget");
        assert_eq!(s.xruns, 0, "...but an overrun alone is not an xrun");
        r.record_callback(Duration::from_micros(100), BlockStatus::Underrun, budget, 0);
        assert_eq!(r.snapshot().xruns, 1, "the callback itself reported the underrun");
    }

    #[test]
    fn late_wake_counts_as_xrun() {
        let r = DiagRecorder::new(48_000);
        let period = Duration::from_micros(1333);
        r.record_wake(period + period / 4, period); // 1.25× — tight but fine
        assert_eq!(r.snapshot().late_wakes, 0);
        r.record_wake(period * 2, period); // 2× — the device ran dry
        let s = r.snapshot();
        assert_eq!(s.late_wakes, 1);
        assert_eq!(s.xruns, 1);
        assert!(!s.is_clean());
    }

    #[test]
    fn jitter_min_max_are_exact_and_avg_is_right() {
        let r = DiagRecorder::new(48_000);
        for g in [1000u64, 1500, 1200, 5000] {
            r.record_wake(Duration::from_nanos(g), Duration::ZERO);
        }
        let s = r.snapshot();
        assert_eq!(s.jitter_min_ns, Some(1000));
        assert_eq!(s.jitter_max_ns, Some(5000));
        assert_eq!(s.jitter_avg_ns, Some((1000 + 1500 + 1200 + 5000) / 4));
        assert_eq!(s.jitter_count, 4);
    }

    #[test]
    fn drift_ppm_sign_and_scale() {
        let r = DiagRecorder::new(48_000);
        // device ran 48 frames fast over 10 s → 48/480000 = +100 ppm
        r.record_device_clock(480_048, Duration::from_secs(10));
        let d = r.snapshot().drift_ppm.unwrap();
        assert!((d - 100.0).abs() < 0.01, "expected +100 ppm, got {d}");
        // and slow
        r.record_device_clock(479_952, Duration::from_secs(10));
        let d = r.snapshot().drift_ppm.unwrap();
        assert!((d + 100.0).abs() < 0.01, "expected -100 ppm, got {d}");
        // before any reading: honest None, not 0.0
        assert!(DiagRecorder::new(48_000).snapshot().drift_ppm.is_none());
    }

    #[test]
    fn implausible_throughput_is_labelled_suspect_not_hidden() {
        let r = DiagRecorder::new(44_100);
        // A stream accepting frames at twice its negotiated rate, past the trust window.
        r.record_device_clock(176_400, Duration::from_secs(2));
        let s = r.snapshot();
        let d = s.drift_ppm.unwrap();
        assert!(d > 800_000.0, "expected a huge drift, got {d}");
        assert!(s.drift_suspect, "beyond ±1 % past the trust window must be flagged suspect");
        assert!(s.drift_text().contains("SUSPECT"));
        // The label now says what the number MEANS (defect #76's re-derivation): throughput
        // against the negotiated rate — never the old "implausible clock" excuse again.
        assert!(s.drift_text_long().contains("accepted frames vs wall"));
        assert!(s.drift_text_long().contains("does not match the negotiated rate"));
        assert!(!s.drift_text_long().contains("implausible clock"));
        // The raw number stays visible — labelling is not hiding.
        assert!(s.lines().contains("SUSPECT"));
        // A real crystal's worth of drift is NOT suspect.
        let r2 = DiagRecorder::new(48_000);
        r2.record_device_clock(4_800_005, Duration::from_secs(100)); // +10.4 ppm
        let s2 = r2.snapshot();
        assert!(!s2.drift_suspect);
        assert!(!s2.drift_text().contains("SUSPECT"));
    }

    #[test]
    fn the_suspect_verdict_waits_for_the_trust_window() {
        // Frames are counted in whole device periods, so a short run's ratio is quantisation,
        // not a verdict: one 960-frame period at 96 kHz inside a 0.5 s window reads ±20 000
        // ppm — twice the threshold — and must NOT label a healthy startup a rate lie.
        let r = DiagRecorder::new(96_000);
        r.record_device_clock(48_000 - 960, Duration::from_millis(500)); // −20 000 ppm at 0.5 s
        let s = r.snapshot();
        assert!(s.drift_ppm.unwrap().abs() > DRIFT_SUSPECT_PPM, "the raw ratio is large");
        assert!(!s.drift_suspect, "...but 0.5 s of wall is below the trust floor");
        assert!(!s.drift_text().contains("SUSPECT"));
        // One period of quantisation at the floor itself (960 fr / 2 s = −5 000 ppm) stays
        // inside the threshold — the floor and the threshold are matched arithmetic. A REAL
        // lie past the floor is a verdict: half throughput at 2 s reads −500 000 ppm.
        let r2 = DiagRecorder::new(96_000);
        r2.record_device_clock(192_000 - 960, Duration::from_secs(2));
        assert!(!r2.snapshot().drift_suspect, "−5 000 ppm is quantisation at the floor");
        let r3 = DiagRecorder::new(96_000);
        r3.record_device_clock(96_000, Duration::from_secs(2));
        assert!(r3.snapshot().drift_suspect, "half throughput past the floor is a verdict");
    }

    #[test]
    fn the_attempt_3_exclusive_shape_reads_as_half_throughput() {
        // test004 attempt 3 (2026-09-28, SATURN, windows-notes.md §4e) replayed under the new
        // derivation: a 96 kHz exclusive stream that accepted 496 384 frames over ~10.05 s —
        // the device consumed at half the rate Initialize said yes to. The OLD GetPosition
        // derivation happened to read −49 % here too (position advanced in buffer steps per
        // event tick, 517 × 960), but it read +1.2 M ppm on the SAME session's soaked-clean
        // shared stream — one label, two opposite stories. The delivered derivation gives both
        // runs their true verdicts (this one and the next test).
        let r = DiagRecorder::new(96_000);
        r.record_device_clock(496_384, Duration::from_secs_f64(10.05));
        let s = r.snapshot();
        let d = s.drift_ppm.unwrap();
        // −485 506 ppm at a 10.05 s wall; the log's own −491 161 was measured against its
        // final wall (~10.17 s). The verdict lives in the band, not in its third digit.
        assert!((-500_000.0..-480_000.0).contains(&d), "expected ≈ −49 %, got {d}");
        assert!(s.drift_suspect);
        assert!(s.drift_text_long().contains("SUSPECT"));
    }

    #[test]
    fn the_defect_76_shared_session_reads_zero_under_the_new_derivation() {
        // The §4b regression fixture, both recorded sessions: the Behringer shared stream
        // (2026-09-24: 15 112 blocks × 64 fr delivered over ≈10.07 s at 96 kHz — a stream the
        // 2 h soak proved healthy) read +1 182 084 ppm under GetPosition and must read ≈0 ppm
        // under delivered-vs-wall. The RDP session of 2026-09-21 (6945 × 64 fr over ≈10.08 s
        // at 44.1 kHz, "throughput within ~1 % of 44.1 kHz") must land inside the threshold.
        let r = DiagRecorder::new(96_000);
        r.record_device_clock(15_112 * 64, Duration::from_secs_f64(10.07));
        let s = r.snapshot();
        assert!(s.drift_ppm.unwrap().abs() < 1_000.0, "healthy shared: {:?}", s.drift_ppm);
        assert!(!s.drift_suspect);
        let r2 = DiagRecorder::new(44_100);
        r2.record_device_clock(6_945 * 64, Duration::from_secs_f64(10.08));
        let s2 = r2.snapshot();
        assert!(s2.drift_ppm.unwrap().abs() < DRIFT_SUSPECT_PPM, "RDP session: {:?}", s2.drift_ppm);
        assert!(!s2.drift_suspect);
    }

    #[test]
    fn allocations_are_accumulated_and_shown() {
        let r = DiagRecorder::new(48_000);
        r.record_callback(Duration::from_micros(10), BlockStatus::Ok, None, 2);
        r.record_callback(Duration::from_micros(10), BlockStatus::Ok, None, 1);
        let s = r.snapshot();
        assert_eq!(s.allocations, 3);
        assert!(!s.is_clean(), "an allocation on the audio path is never clean");
        assert!(s.summary().contains("alloc 3"));
    }

    #[test]
    fn rt_flags_roundtrip() {
        let r = DiagRecorder::new(48_000);
        let rep =
            RtReport { mmcss: true, priority: true, ideal_processor: false, working_set: true };
        r.set_rt_report(rep);
        assert_eq!(r.rt_report(), rep);
        assert_eq!(r.snapshot().rt, rep);
    }

    #[test]
    fn fmt_ns_covers_the_scales() {
        assert_eq!(fmt_ns(999), "999 ns");
        assert_eq!(fmt_ns(1500), "1.5 µs");
        assert_eq!(fmt_ns(2_500_000), "2.50 ms");
        assert_eq!(fmt_ns(1_500_000_000), "1.500 s");
    }

    #[test]
    fn empty_recorder_snapshots_to_honest_nones() {
        let s = DiagRecorder::new(48_000).snapshot();
        assert_eq!(s.blocks, 0);
        assert!(s.p50_ns.is_none() && s.p99_ns.is_none() && s.max_ns.is_none());
        assert!(s.jitter_min_ns.is_none());
        assert!(s.is_clean()); // vacuously clean, and the block count says why
        assert!(s.histogram_text().contains("no blocks"));
        assert!(s.lines().contains("not reported"));
    }
}
