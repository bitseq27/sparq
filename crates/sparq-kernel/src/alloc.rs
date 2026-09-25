//! Real-time discipline: making "no allocation on the audio path" measurable instead of aspirational.
//!
//! Plan §5.2 rule 1 forbids allocation, locking, syscalls and page faults on the audio thread.
//! A rule that cannot be measured will be broken. This module provides the measurement:
//! a counting [`GlobalAlloc`] wrapper plus thread-local guards that define *when* counting is
//! active, so the counter never penalises setup code.
//!
//! Usage in a test binary (a `#[global_allocator]` can only be declared once per binary):
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: sparq_kernel::alloc::CountingAllocator<std::alloc::System> =
//!     sparq_kernel::alloc::CountingAllocator::new(std::alloc::System);
//! ```
//!
//! Then, around the code under test:
//!
//! ```
//! let before = sparq_kernel::alloc::allocation_count();
//! // ... call the audio path ...
//! assert_eq!(sparq_kernel::alloc::allocation_count(), before, "audio path allocated");
//! ```
//!
//! [`GlobalAlloc`]: std::alloc::GlobalAlloc

// Allowlisted: docs/unsafe-allowlist.md entry 4 (`sparq-kernel/src/rt/alloc.rs`, shipped here as
// `alloc.rs`). The `unsafe` in this module is limited to forwarding `GlobalAlloc` methods to the
// wrapped allocator.
// SAFETY: every `unsafe` fn/block below forwards its arguments unchanged to `A`'s implementation,
// which upholds the `GlobalAlloc` contract; we only observe the call. Tested by
// `crates/sparq-audio/tests/rt_discipline.rs`, which installs this allocator for a whole test binary.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static DEALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);
/// When set, counting applies on EVERY thread, not just ones holding a guard. Needed by the
/// leak-cycle gate: a spawn closure allocated on the armed control thread is *freed on the pump
/// thread*, and thread-local arming counts one side of that pair — producing phantom "leaks"
/// (defect #44: 22 per cycle on the WASAPI reopen gate, all of them freed memory).
static PROC_WIDE: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Per-thread switch. Counting is recorded only when this *and* [`COUNTING_ENABLED`] are set,
    /// so one thread can audit its own audio path while another thread loads a patch.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    /// Depth counter so nested guards behave correctly.
    static GUARD_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// A [`GlobalAlloc`] wrapper that counts allocations made while counting is armed.
///
/// It adds two relaxed atomic checks per allocation. That is acceptable in debug/CI (where the
/// audit runs) and is the same allocator the release build uses, so there is no behavioural
/// divergence between "the build we test" and "the build we play".
pub struct CountingAllocator<A> {
    inner: A,
}

impl<A> CountingAllocator<A> {
    /// Wrap an existing allocator.
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }
}

impl<A: GlobalAlloc> CountingAllocator<A> {
    #[inline]
    // The conditions are deliberately not collapsed: the global flag is a cheap relaxed load that
    // short-circuits everything when auditing is off — the common case in a release build.
    #[allow(clippy::collapsible_if)]
    fn bump(bytes: usize, counter: &AtomicU64) {
        if COUNTING_ENABLED.load(Ordering::Relaxed) {
            if PROC_WIDE.load(Ordering::Relaxed) || COUNTING.with(Cell::get) {
                counter.fetch_add(1, Ordering::Relaxed);
                if bytes > 0 {
                    // BYTES stays thread-local by design: it answers "what did THIS thread's
                    // audio path allocate", which is per-callback information.
                    BYTES.with(|b| b.set(b.get() + bytes as u64));
                }
            }
        }
    }
}

thread_local! {
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::bump(layout.size(), &ALLOC_COUNT);
        // SAFETY: `layout` is forwarded to `A` unchanged; `A` upholds the `GlobalAlloc` contract
        // and returns either a valid pointer for `layout` or null. We do not dereference it.
        unsafe { self.inner.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::bump(0, &DEALLOC_COUNT);
        // SAFETY: `ptr`/`layout` come from the allocator runtime and are forwarded unchanged, so
        // `A` receives exactly the pointer and layout it produced.
        unsafe { self.inner.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::bump(layout.size(), &ALLOC_COUNT);
        // SAFETY: as `alloc` — pass-through, no dereference, `A` guarantees zeroed memory.
        unsafe { self.inner.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // A realloc replaces one live block with another: the outstanding *count* must not move.
        // Counting it as a bare allocation (the old behaviour) inflated the balance by one per
        // growth, which is defect #48: `outstanding_allocations()` reported 152 phantom "leaks"
        // across 8 WASAPI reopen cycles, produced by `format!`/`Vec` growth in the diagnostics
        // text on the control thread. The fix for #44 (cross-thread arming) was aimed at a thread
        // boundary that was not the problem; this is the actual accounting error.
        //
        // Byte accounting stays one-sided by design: `BYTES` answers "what did this thread's
        // audio path allocate", where counting the bytes moved through a realloc is informative
        // and double-counting costs nothing, because the audio-path gates assert it is ZERO.
        Self::bump(0, &ALLOC_COUNT);
        Self::bump(0, &DEALLOC_COUNT);
        // SAFETY: as `dealloc`/`alloc` — pass-through of a pointer the runtime allocated with
        // `layout`, resized to `new_size`. `A` owns the copy and the free.
        unsafe { self.inner.realloc(ptr, layout, new_size) }
    }
}

/// Total allocations observed while counting was armed, process-wide.
#[must_use]
pub fn allocation_count() -> u64 {
    ALLOC_COUNT.load(Ordering::Relaxed)
}

/// Total deallocations observed while counting was armed, process-wide.
#[must_use]
pub fn deallocation_count() -> u64 {
    DEALLOC_COUNT.load(Ordering::Relaxed)
}

/// Live allocations: `allocation_count() - deallocation_count()`.
///
/// The leak-check primitive for WO-006's "switching backend on stop/start must not leak"
/// acceptance criterion. Caveats, both deliberate:
///
/// * Only meaningful while counting stays armed for the whole measured cycle, on every thread
///   that takes part — use [`start_counting_all_threads`] when the lifecycle crosses threads.
///   Arm once, do all the open/start/stop/drop cycles, then compare.
/// * It counts *blocks*, not bytes: a `realloc` replaces one live block with another and so
///   moves neither counter. Getting that wrong was defect #48 — 152 phantom "leaks" across 8
///   reopen cycles, all of them `format!`/`Vec` growth. Proven by
///   `outstanding_balance_survives_realloc_growth` in `crates/sparq-audio/tests/rt_discipline.rs`.
#[must_use]
pub fn outstanding_allocations() -> u64 {
    allocation_count().saturating_sub(deallocation_count())
}

/// Bytes allocated on the **current thread** while counting was armed.
#[must_use]
pub fn thread_bytes() -> u64 {
    BYTES.with(Cell::get)
}

/// Whether the counting allocator is actually installed as this binary's `#[global_allocator]`.
///
/// This matters more than it looks. If it is not installed, an allocation audit reports **zero** —
/// which is indistinguishable from "the audio path is clean". A gate that silently passes is worse
/// than no gate, so `NullDevice::render` warns when it is asked to audit without a live counter,
/// and `sparq render` refuses to claim `alloc 0` in that case.
///
/// Probe: arm counting, allocate one known box, and see whether the counter moved.
///
/// The result is cached process-wide: whether the counting allocator is installed cannot change at
/// runtime, and the probe itself allocates, so calling it per block would be both wasteful and
/// self-defeating.
#[must_use]
pub fn audit_is_live() -> bool {
    static CACHED: AtomicU8 = AtomicU8::new(0); // 0 = unknown, 1 = live, 2 = not live
    match CACHED.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {},
    }
    let base = start_counting();
    let probe = Box::new([0u64; 8]);
    std::hint::black_box(&probe);
    let moved = allocation_count() > base;
    drop(probe);
    stop_counting();
    CACHED.store(u8::from(moved), Ordering::Relaxed);
    moved
}

/// Reset every counter. Test-only.
pub fn reset() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    DEALLOC_COUNT.store(0, Ordering::Relaxed);
    BYTES.with(|b| b.set(0));
}

/// Arm counting on the current thread. Returns the allocation count at the moment of arming.
#[must_use]
pub fn start_counting() -> u64 {
    COUNTING_ENABLED.store(true, Ordering::Relaxed);
    COUNTING.with(|c| c.set(true));
    let base = allocation_count();
    GUARD_DEPTH.with(|d| d.set(d.get() + 1));
    base
}

/// Arm counting on **every** thread until [`stop_counting_all_threads`].
///
/// For gates that measure lifecycles crossing threads — the reopen-without-leak cycle allocates
/// on the control thread and frees on the pump thread (defect #44). Requires a quiescent process:
/// unrelated threads allocating during the window are counted too, so this belongs in the CLI
/// gates (`selftest`, `devices --conformance`), never inside a parallel test binary.
///
/// Returns the allocation count at the moment of arming, like [`start_counting`].
#[must_use]
pub fn start_counting_all_threads() -> u64 {
    COUNTING_ENABLED.store(true, Ordering::Relaxed);
    PROC_WIDE.store(true, Ordering::Release);
    allocation_count()
}

/// Disarm the process-wide counting started by [`start_counting_all_threads`].
pub fn stop_counting_all_threads() {
    PROC_WIDE.store(false, Ordering::Release);
}

/// Disarm counting on the current thread.
pub fn stop_counting() {
    GUARD_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    let still_guarded = GUARD_DEPTH.with(|d| d.get()) > 0;
    if !still_guarded {
        COUNTING.with(|c| c.set(false));
    }
}

/// RAII guard: counts every allocation made by the current thread until dropped.
///
/// Nesting is safe; only the outermost guard disarms counting.
#[derive(Debug)]
pub struct CountingGuard {
    base: u64,
}

impl CountingGuard {
    /// Arm counting and remember the baseline.
    #[must_use]
    pub fn new() -> Self {
        Self { base: start_counting() }
    }

    /// Allocations since this guard was created.
    #[must_use]
    pub fn allocations(&self) -> u64 {
        allocation_count().saturating_sub(self.base)
    }

    /// Consume the guard, disarming counting, and return the number of allocations observed.
    #[must_use]
    pub fn finish(self) -> u64 {
        let n = self.allocations();
        drop(self);
        n
    }
}

impl Default for CountingGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CountingGuard {
    fn drop(&mut self) {
        stop_counting();
    }
}

/// A convenience wrapper: run `f` with counting armed and report how many allocations it made.
///
/// This is the assertion every audio-path test uses.
pub fn count_allocations<F: FnOnce() -> R, R>(f: F) -> (R, u64) {
    let guard = CountingGuard::new();
    let out = f();
    (out, guard.finish())
}

/// The plain system allocator, re-exported so test binaries need no extra import.
pub type SystemAllocator = System;

// NOTE: there are no in-crate tests for the counting allocator, and that is deliberate. A
// `#[global_allocator]` applies to a whole binary, so a lib crate's own unit-test binary would have
// to hijack its own allocator to exercise this. The audit is therefore proven where it is actually
// used: `crates/sparq-audio/tests/rt_discipline.rs` installs [`CountingAllocator`] as the test
// binary's global allocator and asserts that a full render makes zero allocations.
