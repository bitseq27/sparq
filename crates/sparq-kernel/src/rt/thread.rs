//! Real-time thread discipline (WO-006 task 4, plan §5.2): MMCSS, priority, ideal processor,
//! working-set lock and pre-touch.
//!
//! Every rule in plan §5.2 ("no allocation, no locks, no syscalls, no page faults on the audio
//! thread") has two halves: the code must obey it, and the *operating system* must not sabotage it.
//! This module is the second half on Windows, the primary platform (ADR-004):
//!
//! * **MMCSS "Pro Audio"** — tells the scheduler this thread carries a hard deadline; Windows then
//!   gives it a boosted, throttle-exempt scheduling class. Without it, a background process can
//!   preempt the audio thread for tens of milliseconds.
//! * **`TIME_CRITICAL` priority** — MMCSS manages the boost, but the base priority still decides
//!   what happens when MMCSS is not honoured (some power plans and remote sessions weaken it —
//!   exactly why [`RtReport`] reports each step separately instead of one boolean).
//! * **Ideal processor** — pins the thread's preferred core so the scheduler stops migrating it
//!   (migration flushes caches at the worst possible moment). The *ideal* processor is a
//!   preference, not an affinity mask: Windows may still move the thread under load, which is the
//!   correct trade for a laptop stage rig.
//! * **Working-set lock + pre-touch** — pages that are not resident cause a page fault *inside the
//!   callback*, which is a syscall with unbounded latency. Pre-touching commits them before
//!   `start`, and raising the process working-set minimum stops Windows from trimming them later.
//!
//! # Failure is reported, never ignored (allowlist entry 3)
//!
//! Every step returns whether it succeeded. [`RtReport::summary`] is printed at stream start and
//! stored in the diagnostics readout, so a machine where MMCSS silently does nothing (observed
//! under some power settings — WO-006 risk register) is *visible* rather than mysterious.
//!
//! # Non-Windows platforms — and Windows builds without `hal-wasapi`
//!
//! Without the feature (or the platform), everything here is an honest no-op that reports
//! `false`: Linux CI must not believe it applied Windows scheduling policy, and a default-features
//! Windows build has no `windows-sys` dependency to apply it with (defect #36). PipeWire/JACK backends (Phase 1) will add the POSIX equivalents
//! (`SCHED_FIFO`, `mlockall`) behind the same report shape.
//!
//! # Why `unsafe` lives here
//!
//! Allowlisted: docs/unsafe-allowlist.md entry 3. The calls are plain Win32 FFI with no aliasing
//! and no lifetimes to get wrong; the discipline is that every handle obtained is reverted in
//! `Drop`, and every failure is recorded in the report. Tested by `rt_report_is_honest_off_windows`
//! (here) and surfaced at runtime by the stream diagnostics; the hardware acceptance (MMCSS
//! honoured under load) is part of the WO-006 2-hour soak on the stage machine.

// Allowlisted: docs/unsafe-allowlist.md entry 3 (`sparq-kernel/src/rt/thread.rs`).
// SAFETY summary for every block below: each is a single Win32 call with by-value handles or
// pointers to locals that outlive the call; nothing is aliased, stored or dereferenced later,
// except the MMCSS task handle, which is owned by `RtThreadGuard` and reverted exactly once in
// `Drop`. The `cfg(windows)` split means non-Windows builds contain no unsafe at all.
#![allow(unsafe_code)]

/// Compile-time UTF-16 for the MMCSS task names (no allocator, no macro dependency):
/// `windows-sys` takes `PCWSTR`, which is a `*const u16` into a null-terminated wide string.
#[cfg(all(windows, feature = "hal-wasapi"))]
const fn ascii_wide(s: &str) -> [u16; 16] {
    let mut out = [0u16; 16];
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 15 {
        out[i] = b[i] as u16;
        i += 1;
    }
    out
}

/// MMCSS task name for professional audio (the documented name; boosts priority and exempts the
/// thread from the multimedia scheduler's throttling).
#[cfg(all(windows, feature = "hal-wasapi"))]
const PRO_AUDIO_W: [u16; 16] = ascii_wide("Pro Audio");

/// What the RT setup actually achieved on this thread/process.
///
/// One field per mechanism, because "did real-time setup work?" has a different answer per
/// mechanism on real machines (WO-006 risk: "MMCSS not being honoured under some power settings").
/// A single boolean would hide exactly the case this exists to surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RtReport {
    /// MMCSS registration ("Pro Audio") succeeded.
    pub mmcss: bool,
    /// Base thread priority set to `TIME_CRITICAL` — this is the critical boost: the documented
    /// `AvSetMmMaxThreadCharacteristicsW` ceiling call is deliberately NOT used, because the
    /// Win32 metadata binding for it is wrong in windows-sys 0.61 (`(PCWSTR, PCWSTR, *mut u32)
    /// -> HANDLE` instead of the documented `(HANDLE, LPCWSTR) -> BOOL`); calling it would pass
    /// garbage. `TIME_CRITICAL` after MMCSS registration is what miniaudio and cpal rely on.
    /// Tracked in docs/hal/windows-notes.md; revisit when the metadata is fixed.
    pub priority: bool,
    /// An ideal processor was accepted by the scheduler.
    pub ideal_processor: bool,
    /// The process working-set minimum was raised (callers separately pre-touch their buffers).
    pub working_set: bool,
}

impl RtReport {
    /// A report claiming nothing was applied — the honest default off Windows.
    #[must_use]
    pub const fn none() -> Self {
        Self { mmcss: false, priority: false, ideal_processor: false, working_set: false }
    }

    /// The two mechanisms that matter most for surviving scheduler contention.
    ///
    /// `ideal_processor` and `working_set` are refinements; a run with these two is considered
    /// "RT setup applied" and anything missing is printed so it cannot be overlooked.
    #[must_use]
    pub const fn core_applied(&self) -> bool {
        self.mmcss && self.priority
    }

    /// One-line readout, e.g. `mmcss+critical · priority · ideal-proc · workset` — every field is
    /// always shown, with `NO-` prefixes for failures, because a silent success is
    /// indistinguishable from a silent failure in a log.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}mmcss {}priority {}ideal-proc {}workset",
            if self.mmcss { "" } else { "NO-" },
            if self.priority { "" } else { "NO-" },
            if self.ideal_processor { "" } else { "NO-" },
            if self.working_set { "" } else { "NO-" },
        )
    }
}

/// RAII guard: applies RT discipline to the *calling* thread and reverts MMCSS on drop.
///
/// Construct this as the first thing a pump/callback thread does. The guard is deliberately not
/// `Send`: MMCSS registration belongs to the thread that made it, and reverting from another
/// thread is a bug the type system should refuse.
pub struct RtThreadGuard {
    report: RtReport,
    /// MMCSS task handle, kept for `AvRevertMmThreadCharacteristics` in `Drop`.
    /// Stored as `isize` (not `HANDLE`) so the struct shape — and therefore the diagnostics that
    /// embed the report — is identical on every platform; `0` means "not registered".
    #[cfg(all(windows, feature = "hal-wasapi"))]
    mmcss_task: isize,
}

impl RtThreadGuard {
    /// Apply the full RT discipline to the current thread.
    ///
    /// `ideal_processor`: preferred core index (`Some(n)`), or `None` to let the scheduler choose.
    /// `working_set_bytes`: how much extra working-set minimum to request for the *process*; pass
    /// the total size of the stream's pre-allocated buffers (0 disables the call).
    #[must_use]
    pub fn apply(ideal_processor: Option<usize>, working_set_bytes: usize) -> Self {
        // Gated on the FEATURE, not just the OS: `windows-sys` is an optional, target-specific
        // dependency that only exists under `hal-wasapi`. A default Windows build gets the honest
        // no-op report below — defect #36 was exactly this gate being `cfg(windows)` alone, which
        // made `gates.bat`'s default-features clippy fail to compile on the primary platform.
        #[cfg(all(windows, feature = "hal-wasapi"))]
        {
            use windows_sys::Win32::System::Threading::{
                AvSetMmThreadCharacteristicsW, GetCurrentProcess, GetCurrentThread,
                GetProcessWorkingSetSize, SetProcessWorkingSetSize, SetThreadIdealProcessor,
                SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
            };

            let mut report = RtReport::none();
            let mut task_index: u32 = 0;

            // SAFETY: `PRO_AUDIO_W` is a compile-time, null-terminated, 'static UTF-16 literal
            // (the exact task name defined by MMCSS); `task_index` is a local whose address stays
            // valid for the duration of the call. MMCSS returns NULL on failure — checked below.
            // The returned handle is owned by this guard and reverted in `Drop`.
            let task =
                unsafe { AvSetMmThreadCharacteristicsW(PRO_AUDIO_W.as_ptr(), &mut task_index) };
            report.mmcss = !task.is_null();
            let mmcss_task = if report.mmcss { task as isize } else { 0 };

            // NOTE: no AvSetMmMaxThreadCharacteristicsW here — see the `priority` field docs:
            // the metadata binding for it is wrong in windows-sys 0.61 (it would pass garbage),
            // and TIME_CRITICAL below provides the critical boost (the miniaudio/cpal pattern).

            // SAFETY: `GetCurrentThread` returns a pseudo-handle valid without closing;
            // `SetThreadPriority` takes it by value. BOOL is nonzero on success.
            report.priority =
                (unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) })
                    != 0;

            report.ideal_processor = match ideal_processor {
                // SAFETY: pseudo-handle by value; `u32` processor index by value. A returned
                // 0xFFFFFFFF means failure; the scheduler rejects indices beyond the group
                // count, which the report then shows honestly.
                Some(n) => {
                    (unsafe { SetThreadIdealProcessor(GetCurrentThread(), n as u32) }) != u32::MAX
                },
                None => false,
            };

            if working_set_bytes > 0 {
                report.working_set = unsafe {
                    // SAFETY: `GetCurrentProcess` returns a pseudo-handle needing no release. The
                    // two out-pointers address locals that live until the end of this block. The
                    // second call passes the values read by the first plus our delta; both are
                    // plain by-value `usize` arguments. BOOL is nonzero on success.
                    let proc = GetCurrentProcess();
                    let (mut min, mut max) = (0usize, 0usize);
                    if GetProcessWorkingSetSize(proc, &mut min, &mut max) != 0 {
                        let add = working_set_bytes;
                        SetProcessWorkingSetSize(proc, min + add, max.max(min + add)) != 0
                    } else {
                        false
                    }
                };
            }

            // The MMCSS registration handle lives in the guard; `Drop` reverts it exactly once,
            // on the thread that registered it, even if the steps above failed.
            Self { report, mmcss_task }
        }
        #[cfg(not(all(windows, feature = "hal-wasapi")))]
        {
            let _ = (ideal_processor, working_set_bytes);
            Self { report: RtReport::none() }
        }
    }

    /// What was actually applied. Print this; never assume it.
    #[must_use]
    pub fn report(&self) -> RtReport {
        self.report
    }
}

impl Drop for RtThreadGuard {
    fn drop(&mut self) {
        #[cfg(all(windows, feature = "hal-wasapi"))]
        if self.mmcss_task != 0 {
            use windows_sys::Win32::System::Threading::AvRevertMmThreadCharacteristics;
            // SAFETY: `mmcss_task` is the non-null registration handle obtained in `apply` on
            // this thread; `Drop` runs exactly once per guard, so the handle is reverted exactly
            // once. The field is zeroed afterwards so a hypothetical second drop is a no-op.
            unsafe {
                let _ = AvRevertMmThreadCharacteristics(self.mmcss_task as *mut core::ffi::c_void);
            }
            self.mmcss_task = 0;
        }
    }
}

/// Pre-touch every page of a buffer so the first *real* access cannot page-fault.
///
/// Safe and portable: `black_box` on both the read and the write defeats the optimiser's
/// dead-store elimination, which a plain `buf[i] = buf[i]` loop would suffer. Call at `open`,
/// before `start`.
pub fn pre_touch(buf: &mut [u8]) {
    const PAGE: usize = 4096;
    let mut i = 0;
    while i < buf.len() {
        buf[i] = std::hint::black_box(std::hint::black_box(buf[i]));
        i += PAGE;
    }
    if let Some(last) = buf.last_mut() {
        *last = std::hint::black_box(*last);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::print_stdout)]

    use super::*;

    /// Off Windows every mechanism must report `false` — a CI runner that believes it applied
    /// MMCSS is worse than one that knows it did not. (On Windows this test runs the real calls;
    /// either outcome is acceptable, but the report must be *internally consistent*.)
    #[test]
    fn rt_report_is_honest_off_windows() {
        let guard = RtThreadGuard::apply(Some(0), 0);
        let r = guard.report();
        if cfg!(windows) {
            // On a real Windows machine the core mechanisms should succeed; if the runner forbids
            // them, the summary still shows exactly what is missing.
            println!("rt report on windows: {}", r.summary());
        } else {
            assert_eq!(r, RtReport::none(), "non-Windows must apply nothing");
            assert!(!r.core_applied());
        }
        // The summary never hides a failure: every failed field carries its NO- prefix.
        let s = r.summary();
        assert!(s.contains("mmcss"));
        assert_eq!(s.contains("NO-mmcss"), !r.mmcss);
    }

    #[test]
    fn pre_touch_writes_every_page_and_preserves_content() {
        let mut buf = vec![7u8; 4096 * 3 + 11];
        pre_touch(&mut buf);
        assert!(buf.iter().all(|&b| b == 7), "pre-touch must not alter content");
        let mut empty: [u8; 0] = [];
        pre_touch(&mut empty); // must not panic on the empty edge
    }
}
