//! A bounded single-producer single-consumer lock-free ring.
//!
//! This is the transport for control messages (parameter changes, transport commands) from the
//! control/UI thread to the audio thread. It never blocks and never allocates after construction:
//! `push` fails loudly with a counted drop rather than waiting (plan §4.3, §5.2).
//!
//! # Why `unsafe` lives here
//!
//! The consumer reads a slot that the producer wrote without taking a lock. Safety rests on three
//! invariants, upheld by the SPSC discipline and the `Acquire`/`Release` ordering below:
//!
//! 1. **One producer, one consumer.** `head` is written only by the consumer, `tail` only by the
//!    producer. Each index is therefore owned by exactly one thread.
//! 2. **No simultaneous access to a slot.** The producer may only write index `tail` when
//!    `tail != head + capacity`, i.e. the consumer has finished with it. The consumer may only read
//!    index `head` when `head != tail`, i.e. the producer has finished writing it.
//! 3. **Publication ordering.** The producer's element write happens-before its `Release` store of
//!    `tail`; the consumer's `Acquire` load of `tail` synchronises-with it and therefore sees the
//!    element. Symmetrically, the consumer's `Release` store of `head` publishes the freed slot.
//!
//! Storage is a plain `Vec`, so allocation happens only in [`SpscRing::new`] and the `Drop`
//! implementation is the default. `T: Copy` is required on the data paths: a copy-out leaves no
//! moved-from value behind, which is what makes invariant 2 hold without per-slot state.
//!
//! `head`/`tail` are **monotonic** counters, masked to a slot index only at the point of access.
//! The buffer length is therefore a power of two (rounded up from the requested capacity) so the
//! mask is exact. This matters: with wrapped indices, `tail - head` is ambiguous between "empty"
//! and "full", and a ring that occasionally believes it is full when it is empty silently drops
//! messages. That bug existed in the first draft of this file and was caught by
//! `spsc_stress_no_tearing` — keep that test.
//!
//! Tested by `spsc_stress_no_tearing` (two threads, 200 000 messages, ordering and checksum
//! verified) and by Miri in CI (ADR-000).

// Allowlisted: docs/unsafe-allowlist.md entry 5 (`sparq-kernel/src/sync/rings.rs`).
// The module-level docs above state the three invariants; each `unsafe` block restates the one it
// relies on. Tested by `spsc_stress_no_tearing` and by Miri in CI (ADR-000).
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// A bounded SPSC ring of `T`.
///
/// `capacity` is the number of *usable* slots; one extra slot is allocated internally so that
/// "full" and "empty" are distinguishable without a separate counter.
pub struct SpscRing<T: Copy> {
    buf: Box<[T]>,
    /// Monotonic read counter, masked on access. Written by the consumer only.
    head: AtomicUsize,
    /// Monotonic write counter, masked on access. Written by the producer only.
    tail: AtomicUsize,
    /// Slot mask: `slots - 1`, where `slots` is a power of two.
    mask: usize,
    usable: usize,
    drops: AtomicU64,
}

impl<T: Copy + Default> SpscRing<T> {
    /// Allocate a ring with `capacity` usable slots (rounded up to at least 1).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let usable = capacity.max(1);
        // Round the slot count up to a power of two so masking is exact. No sentinel slot is
        // needed: with monotonic counters, `tail - head == usable` is unambiguously "full".
        let slots = (usable + 1).next_power_of_two();
        Self {
            buf: vec![T::default(); slots].into_boxed_slice(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            mask: slots - 1,
            usable,
            drops: AtomicU64::new(0),
        }
    }
}

impl<T: Copy> SpscRing<T> {
    /// Usable capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.usable
    }

    /// Number of `push` calls that were **refused** because the ring was full.
    ///
    /// Semantics matter here (plan §4.3 requires counted, never silent, failure — it does not
    /// dictate who owns the message):
    ///
    /// * `push` returns `false` and this counter increments. **The caller still holds the value.**
    /// * A caller that retries therefore inflates this counter without losing anything. Backpressure
    ///   is not loss.
    /// * A caller that discards the value has dropped a message — which is exactly the
    ///   `drop-and-count` policy the plan specifies for UI telemetry.
    ///
    /// So this counter answers "how often did the consumer fall behind?", not "how many messages
    /// were lost?". Diagnostics must label it accordingly; a `drop-oldest` policy would need a
    /// second counter, and no Phase 0 ring uses one.
    #[must_use]
    pub fn refusals(&self) -> u64 {
        self.drops.load(Ordering::Relaxed)
    }

    /// Alias for [`SpscRing::refusals`]; read its docs before asserting anything about this number.
    #[must_use]
    pub fn drops(&self) -> u64 {
        self.refusals()
    }

    /// Number of queued items. Safe from either side; may be immediately stale.
    #[must_use]
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    /// Whether the ring appears empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the ring appears full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() == self.usable
    }

    /// Producer side. Returns `false` and counts a drop when full — it never blocks.
    pub fn push(&self, value: T) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let mut head = self.head.load(Ordering::Acquire);
        // Not collapsed on purpose: the inner check runs only after re-reading `head`, and
        // collapsing it would re-test the stale value.
        #[allow(clippy::collapsible_if)]
        if tail.wrapping_sub(head) == self.usable {
            // Re-read once: the consumer may have just freed a slot.
            head = self.head.load(Ordering::Acquire);
            if tail.wrapping_sub(head) == self.usable {
                self.drops.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }

        // SAFETY: invariant 2 — the consumer cannot read slot `tail & mask` until we publish it
        // with the Release store below, so this write is unobserved. The mask keeps the index
        // inside `self.buf`, whose length is `mask + 1`.
        unsafe {
            let slot = self.buf.as_ptr().add(tail & self.mask) as *mut T;
            slot.write(value);
        }
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Consumer side. Returns `None` when empty — it never blocks.
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        let mut tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            // Re-read once: the producer may have just published.
            tail = self.tail.load(Ordering::Acquire);
            if head == tail {
                return None;
            }
        }
        // SAFETY: invariant 2/3 — `head != tail` means the producer wrote slot `head & mask`
        // before its Release store of `tail`, which our Acquire load synchronises with, so the
        // value is visible. `T: Copy` means the read is a bitwise copy and leaves the slot intact
        // for the producer to overwrite on its next lap. The mask keeps the index in bounds.
        let value = unsafe { self.buf.as_ptr().add(head & self.mask).read() };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// Drain up to `out.len()` items into a caller-provided buffer. Returns how many were read.
    ///
    /// Allocation-free by construction: the caller owns the destination.
    pub fn drain(&self, out: &mut [T]) -> usize {
        let mut n = 0;
        while n < out.len() {
            match self.pop() {
                Some(v) => {
                    out[n] = v;
                    n += 1;
                },
                None => break,
            }
        }
        n
    }
}

// SAFETY: the ring *is* the synchronisation mechanism. `T: Copy + Send` plus the single-writer
// discipline on each index makes concurrent use from one producer thread and one consumer thread
// sound; no two threads ever touch the same slot at the same time.
unsafe impl<T: Copy + Send> Send for SpscRing<T> {}
unsafe impl<T: Copy + Send> Sync for SpscRing<T> {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::sync::Arc;

    #[test]
    fn push_pop_roundtrip() {
        let r: SpscRing<u32> = SpscRing::new(4);
        assert_eq!(r.capacity(), 4);
        assert!(r.is_empty());
        for v in [1, 2, 3, 4] {
            assert!(r.push(v));
        }
        assert!(r.is_full());
        assert!(!r.push(5), "must refuse when full, never block");
        assert_eq!(r.refusals(), 1, "a refused push must be counted");
        for v in [1, 2, 3, 4] {
            assert_eq!(r.pop(), Some(v));
        }
        assert_eq!(r.pop(), None);
        assert!(r.is_empty());
    }

    #[test]
    fn drain_is_ordered_and_bounded() {
        let r: SpscRing<u32> = SpscRing::new(8);
        for v in 0..6 {
            assert!(r.push(v));
        }
        let mut out = [0u32; 8];
        assert_eq!(r.drain(&mut out), 6);
        assert_eq!(&out[..6], &[0, 1, 2, 3, 4, 5]);
        assert_eq!(r.drain(&mut out), 0);
    }

    #[test]
    fn wraparound_preserves_order() {
        let r: SpscRing<u64> = SpscRing::new(3);
        for round in 0..200u64 {
            for i in 0..3 {
                assert!(r.push(round * 10 + i));
            }
            for i in 0..3 {
                assert_eq!(r.pop(), Some(round * 10 + i));
            }
        }
        assert_eq!(r.refusals(), 0, "the ring was never full, so nothing was refused");
    }

    #[test]
    fn refusals_are_counted_under_backpressure() {
        // Deterministic: fill a small ring with nobody consuming, then keep pushing.
        let r: SpscRing<u32> = SpscRing::new(8);
        for i in 0..8 {
            assert!(r.push(i), "slot {i} should be free");
        }
        assert_eq!(r.len(), 8);
        assert!(r.is_full());
        for _ in 0..100 {
            assert!(!r.push(999), "a full ring must refuse, never block and never overwrite");
        }
        assert_eq!(r.refusals(), 100);
        // The oldest data must be intact: a refusal must not corrupt the ring.
        assert_eq!(r.pop(), Some(0));
        assert_eq!(r.pop(), Some(1));
        assert_eq!(r.refusals(), 100, "popping does not change the refusal count");
    }

    #[test]
    fn spsc_stress_no_tearing() {
        const N: u64 = 200_000;
        let r = Arc::new(SpscRing::<u64>::new(1024));
        let pc = Arc::clone(&r);
        let producer = std::thread::spawn(move || {
            for i in 0..N {
                while !pc.push(i) {
                    std::hint::spin_loop();
                }
            }
        });
        let cc = Arc::clone(&r);
        let consumer = std::thread::spawn(move || {
            let mut sum = 0u64;
            let mut seen = 0u64;
            let mut last: Option<u64> = None;
            while seen < N {
                if let Some(v) = cc.pop() {
                    if let Some(prev) = last {
                        assert_eq!(v, prev + 1, "ordering violated at seen={seen}: {prev} -> {v}");
                    }
                    last = Some(v);
                    sum = sum.wrapping_add(v);
                    seen += 1;
                } else {
                    std::hint::spin_loop();
                }
            }
            (sum, seen)
        });
        producer.join().unwrap();
        let (sum, seen) = consumer.join().unwrap();
        assert_eq!(seen, N);
        assert_eq!(sum, (0..N).sum::<u64>(), "checksum mismatch: a slot was torn or lost");
        // The producer retried on every refusal, so nothing was lost. Whether any refusals
        // happened at all depends on scheduling, so it is not asserted here — see
        // `refusals_are_counted_under_backpressure` for the deterministic version.
    }
}
