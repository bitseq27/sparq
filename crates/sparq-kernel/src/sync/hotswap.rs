//! The cross-thread hot-swap slot: ADR-009 decision 3's literal pointer swap, with epoch
//! retirement.
//!
//! This is the primitive the live rig hands a finished patch from the control thread to the
//! audio thread through. The contract, in the ADR's words: *the control thread builds a
//! complete new graph, then swaps a pointer at a block boundary; the audio thread retires the
//! old graph after a grace period. The audio thread never observes a half-built graph.*
//!
//! # Shape
//!
//! [`HotSwap::split`] produces two halves that live on two threads and never share a lock:
//!
//! * [`HotSwapControl`] (control thread): [`stage`](HotSwapControl::stage) a boxed payload,
//!   [`reclaim`](HotSwapControl::reclaim) retired payloads and drop them OFF the audio thread,
//!   read [`stats`](HotSwapControl::stats).
//! * [`HotSwapAudio`] (audio thread): [`boundary`](HotSwapAudio::boundary) once per block —
//!   the swap point — then use [`live_mut`](HotSwapAudio::live_mut) to render.
//!
//! # The protocol, and why it is safe
//!
//! One control thread, one audio thread, no locks, no allocation after construction, no
//! blocking, and the audio thread never drops a payload (dropping is the control thread's job —
//! a retired executor's teardown has no place inside a device callback).
//!
//! * **Staging** is a single `AtomicPtr::swap` on `staged`. Both sides only ever exchange the
//!   pointer through `swap`, so exactly one of them holds any given box: a stage that lands on
//!   an occupied slot *supersedes* the waiting payload and the controller drops it immediately
//!   (counted in [`SwapStats::superseded`] — never silently).
//! * **The boundary** (audio): load `staged`; if a payload waits AND a retirement slot is free,
//!   take it (`swap(null)`), run the caller's inherit hook (transport-clock inheritance — see
//!   `sparq-audio`'s engine), replace `live`, and publish the outgoing box into the retirement
//!   slot. Then store `epoch + 1` with Release. If no slot is free the swap is **deferred**:
//!   the payload stays staged for the next boundary and [`SwapStats::deferred`] counts the
//!   event. The audio thread's fallback is always "keep rendering the live patch" — it never
//!   waits, never drops, never allocates.
//! * **Epoch retirement** is the grace period, made mechanical instead of argued. The audio
//!   thread tags a retirement with the epoch it is leaving and stores the new epoch AFTER the
//!   publication, with Release. The controller loads the epoch with Acquire and may only take
//!   a retirement whose tag is strictly below it — so a box becomes reclaimable only once the
//!   audio thread has demonstrably passed the boundary that let it go. That boundary IS the
//!   reader's quiescent state: after the `mem::replace`, the old payload is unreachable from
//!   the audio side by construction, and the epoch gate makes "unreachable" an observable fact
//!   the controller waits for rather than a claim the code hopes is true.
//! * **Retirement slots** are a fixed array of [`RETIRE_SLOTS`] atomic pointer/tag pairs. Only
//!   the audio thread fills a slot and only the controller empties one, so a slot observed
//!   empty stays empty until the audio thread claims it — which is what makes the boundary's
//!   capacity precheck exact without any locking.
//!
//! # Why `unsafe` lives here
//!
//! The payload crosses threads as a raw pointer inside atomics — the literal pointer swap the
//! ADR names. Safety rests on four invariants, each restated at its `unsafe` site:
//!
//! 1. **Exclusive ownership travels with the pointer.** Every non-null pointer in `staged` or a
//!    retirement slot is the `Box::into_raw` end of exactly one box, and ownership passes only
//!    through atomic `swap`s that hand the old value to exactly one winner.
//! 2. **Single producer, single consumer per location.** `staged` is filled only by the
//!    controller and emptied only by the audio thread; each retirement slot is filled only by
//!    the audio thread and emptied only by the controller.
//! 3. **Publication ordering.** Payload writes before the swap happen-before the Release store
//!    that publishes them; Acquire loads on the other side synchronise with it, so the receiver
//!    always sees a fully built payload (the control thread's "build completely, then swap" —
//!    the audio thread never observes a half-built graph).
//! 4. **The grace period is enforced, not assumed.** A retirement is only converted back into a
//!    `Box` after the epoch gate proves the audio thread passed the boundary that published it.
//!
//! `T: Send` is required for the halves to travel to their threads; the payload itself is only
//! ever reachable from one thread at a time (invariant 1), which is exactly what `Send`
//! promises is sound.
//!
//! Tested by the in-file suite (handoff order, the epoch gate across two real threads,
//! supersede/defer counters, teardown drop hygiene, a two-thread exchange stress) and by Miri
//! in CI (ADR-000). The 10 000-mutation-while-playing acceptance runs one level up, over real
//! executors: `sparq-audio/tests/cross_thread.rs`.

// Allowlisted: docs/unsafe-allowlist.md entry 6 (`sparq-kernel/src/graph/hotswap.rs`). Ships in
// `sync/` because the primitive is payload-generic — the kernel cannot name the executor type
// (sparq-audio sits above it), so the swap belongs with the rings, not with the graph module.
// The module docs above state the four invariants; each `unsafe` block restates the one it
// relies on. Tested by this file's suite and by Miri in CI (ADR-000).
#![allow(unsafe_code)]

use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Arc;

/// Retirement slots. A boundary swap needs one free slot; when all are occupied (the controller
/// has not reclaimed for [`RETIRE_SLOTS`] consecutive swaps) further swaps are deferred and
/// counted, never forced. Eight gives a control thread that polls once per UI frame plenty of
/// slack at audio block rates while keeping the scan trivial.
pub const RETIRE_SLOTS: usize = 8;

/// The tag a retirement carries while it is being published: `tag < epoch` is the reclaim
/// gate, and `u64::MAX` is never below any epoch, so a half-published slot cannot be taken.
const UNCLAIMABLE: u64 = u64::MAX;

/// One retirement slot: a published payload pointer plus the epoch that must be passed before
/// the controller may take it.
struct RetireSlot<T> {
    /// Non-null while a retired payload waits. Filled by the audio thread only, emptied by the
    /// controller only (invariant 2).
    ptr: AtomicPtr<T>,
    /// The epoch the payload was retired at, or [`UNCLAIMABLE`] mid-publication.
    tag: AtomicU64,
    /// The shared state logically owns the boxes the slots hold (dropcheck).
    marker: PhantomData<T>,
}

/// The state both halves share. Every field is an atomic; the boxes behind them are owned by
/// whoever holds the pointer (invariant 1), and [`Shared::drop`] reclaims whatever is still in
/// flight when the last handle goes.
struct Shared<T> {
    /// The staging slot: control fills, audio empties (invariant 2).
    staged: AtomicPtr<T>,
    /// The retirement array: audio fills, control empties (invariant 2).
    retire: [RetireSlot<T>; RETIRE_SLOTS],
    /// Boundaries passed. Written by the audio thread only, with Release; read by the
    /// controller with Acquire as the grace-period clock (invariant 4).
    epoch: AtomicU64,
    swaps: AtomicU64,
    superseded: AtomicU64,
    deferred: AtomicU64,
    retirements: AtomicU64,
}

impl<T> Shared<T> {
    fn new() -> Self {
        Self {
            staged: AtomicPtr::new(ptr::null_mut()),
            retire: std::array::from_fn(|_| RetireSlot {
                ptr: AtomicPtr::new(ptr::null_mut()),
                tag: AtomicU64::new(0),
                marker: PhantomData,
            }),
            epoch: AtomicU64::new(0),
            swaps: AtomicU64::new(0),
            superseded: AtomicU64::new(0),
            deferred: AtomicU64::new(0),
            retirements: AtomicU64::new(0),
        }
    }

    /// The index of a free retirement slot, if one exists. Exact without locking: only the
    /// audio thread fills slots, so a slot seen free here is still free when this (audio)
    /// thread publishes into it moments later.
    fn free_retire_slot(&self) -> Option<usize> {
        self.retire.iter().position(|s| s.ptr.load(Ordering::Acquire).is_null())
    }

    /// How many retirement slots hold a payload right now.
    fn pending_retirements(&self) -> usize {
        self.retire.iter().filter(|s| !s.ptr.load(Ordering::Acquire).is_null()).count()
    }
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // Both halves are gone (this is the last `Arc`), so every pointer below is unreachable
        // from any thread and this drop is the exclusive owner.
        // SAFETY: invariant 1 — each non-null pointer is the `Box::into_raw` end of exactly one
        // box that no `swap` has handed to anyone else; converting each back exactly once drops
        // each payload exactly once. `&mut self` in `Drop` rules out concurrent access.
        unsafe {
            let staged = self.staged.swap(ptr::null_mut(), Ordering::Relaxed);
            if !staged.is_null() {
                drop(Box::from_raw(staged));
            }
            for slot in &self.retire {
                let p = slot.ptr.swap(ptr::null_mut(), Ordering::Relaxed);
                if !p.is_null() {
                    drop(Box::from_raw(p));
                }
            }
        }
    }
}

// SAFETY: `Shared<T>` is the synchronisation mechanism (the same claim `SpscRing` makes): every
// field is an atomic, and the swap protocol above guarantees a payload is reachable from exactly
// one thread at a time. `T: Send` is precisely the condition for that hand-over to be sound;
// `&Shared` is safe to hold on both threads because all access goes through the atomics.
unsafe impl<T: Send> Send for Shared<T> {}
unsafe impl<T: Send> Sync for Shared<T> {}

/// What the controller can observe without touching the audio thread. Every field is a
/// measurement, not a promise — a snapshot that may be stale the moment it is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapStats {
    /// Boundaries the audio thread has passed (the grace-period clock).
    pub epoch: u64,
    /// Boundary swaps performed (a staged payload went live).
    pub swaps: u64,
    /// Staged payloads superseded by a later stage before going live — each one was dropped by
    /// the controller at supersede time, counted, never silent.
    pub superseded: u64,
    /// Boundaries at which a staged payload could NOT swap because no retirement slot was free.
    /// The payload stays staged; the live patch keeps rendering. Backpressure, counted.
    pub deferred: u64,
    /// Retirements published by the audio thread (each swap publishes exactly one).
    pub retirements: u64,
    /// Whether a payload is waiting in the staging slot right now.
    pub staged: bool,
    /// Retirement slots holding a payload right now (reclaimed or not yet reclaimable).
    pub pending_retirements: usize,
}

/// The factory: [`HotSwap::split`] is the only way to create a slot, and it returns the two
/// halves — one per thread — the audio one already holding the initial payload.
pub struct HotSwap;

impl HotSwap {
    /// Create a slot around the initial live payload. The audio half owns it immediately; the
    /// control half starts with an empty staging slot at epoch 0.
    #[must_use]
    pub fn split<T>(initial: T) -> (HotSwapControl<T>, HotSwapAudio<T>) {
        let shared = Arc::new(Shared::new());
        (
            HotSwapControl { shared: Arc::clone(&shared) },
            HotSwapAudio { shared, live: Box::new(initial), epoch: 0 },
        )
    }
}

/// The control-thread half: stage complete payloads, reclaim retired ones, observe.
///
/// All methods take `&self` and are lock-free; the discipline is one control thread (the
/// `swap`-based protocol keeps ownership exact even if a caller violates that, but the
/// [`SwapStats`] counters then describe a conversation between several controllers).
pub struct HotSwapControl<T> {
    shared: Arc<Shared<T>>,
}

impl<T> HotSwapControl<T> {
    /// Stage a finished payload for the next boundary. Returns `true` when this stage
    /// **superseded** a payload that was still waiting: the superseded box is dropped here,
    /// on the control thread, immediately, and counted. The timeline never skips — what goes
    /// live next is always exactly one boundary away.
    pub fn stage(&self, next: Box<T>) -> bool {
        let incoming = Box::into_raw(next);
        let previous = self.shared.staged.swap(incoming, Ordering::AcqRel);
        if previous.is_null() {
            return false;
        }
        // SAFETY: invariant 1 — `previous` is the `Box::into_raw` end of an earlier `stage`
        // call, and this `swap` is the single point where its ownership returns to the control
        // side (the audio thread only ever receives it through this same atomic). It was never
        // dropped and never went live, so re-boxing drops exactly this payload once.
        unsafe { drop(Box::from_raw(previous)) };
        self.shared.superseded.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Whether a payload is waiting in the staging slot. May be stale the moment it is read.
    #[must_use]
    pub fn is_staged(&self) -> bool {
        !self.shared.staged.load(Ordering::Acquire).is_null()
    }

    /// Take the next retirement whose grace period has passed, for the caller to drop (off the
    /// audio thread — that is the point). Returns `None` when nothing is reclaimable: either no
    /// retirement is waiting, or the audio thread has not yet passed the boundary that would
    /// make the waiting one safe to touch. Call in a loop to drain.
    #[must_use]
    pub fn reclaim(&self) -> Option<Box<T>> {
        let epoch = self.shared.epoch.load(Ordering::Acquire);
        for slot in &self.shared.retire {
            // Pointer first, tag second — the order matters (invariants 3/4): an Acquire load
            // that sees a published pointer also sees everything the audio thread stored
            // before its Release store of that pointer, including the `UNCLAIMABLE` tag it
            // writes first. Reading the tag first could pair a NEW pointer with a PREVIOUS
            // occupant's finalised tag and take the box one boundary early.
            let p = slot.ptr.load(Ordering::Acquire);
            if p.is_null() {
                continue;
            }
            if slot.tag.load(Ordering::Acquire) >= epoch {
                continue; // mid-publication, or the grace period has not passed
            }
            let taken = slot.ptr.swap(ptr::null_mut(), Ordering::AcqRel);
            if taken.is_null() {
                continue; // unreachable under the single-controller discipline; defensive
            }
            // SAFETY: invariant 4 — the tag is strictly below the epoch this call Acquired, and
            // the audio thread stored that epoch with Release only AFTER replacing its live
            // pointer and publishing this box, so the audio thread has passed the boundary that
            // let the payload go and can no longer reach it. Invariant 1: the `swap` hands this
            // pointer to exactly one winner (us), so re-boxing is the sole ownership of a box
            // that was never dropped.
            return Some(unsafe { Box::from_raw(taken) });
        }
        None
    }

    /// A snapshot of every counter. Cheap enough to poll per UI frame.
    #[must_use]
    pub fn stats(&self) -> SwapStats {
        SwapStats {
            epoch: self.shared.epoch.load(Ordering::Acquire),
            swaps: self.shared.swaps.load(Ordering::Relaxed),
            superseded: self.shared.superseded.load(Ordering::Relaxed),
            deferred: self.shared.deferred.load(Ordering::Relaxed),
            retirements: self.shared.retirements.load(Ordering::Relaxed),
            staged: self.is_staged(),
            pending_retirements: self.shared.pending_retirements(),
        }
    }
}

/// The audio-thread half: cross the boundary once per block, render the live payload between
/// boundaries.
///
/// Not `Sync` unless `T` is: this half belongs to exactly one thread (move it there with
/// [`std::thread::spawn`]), and its methods that mutate take `&mut self`, so the compiler
/// enforces the audio-side half of the protocol.
pub struct HotSwapAudio<T> {
    shared: Arc<Shared<T>>,
    /// The payload that renders between boundaries. Owned outright — no atomic, no option:
    /// there is always a live payload.
    live: Box<T>,
    /// Local mirror of the shared epoch. The audio thread is its only writer, so the mirror is
    /// exact and saves an atomic read per boundary.
    epoch: u64,
}

impl<T> HotSwapAudio<T> {
    /// Cross one block boundary: apply a staged payload if one is waiting (and a retirement
    /// slot is free), then advance the epoch — the reader's quiescent-state marker that makes
    /// the just-retired payload reclaimable. Returns whether a swap happened.
    ///
    /// `inherit` runs at the swap point, on the audio thread, with the incoming payload (live
    /// from the next sample on) and the outgoing one (which rendered the previous block): the
    /// place where a successor adopts the stream's clock. It must be real-time safe — no
    /// allocation, no locks — like everything called between boundaries.
    ///
    /// This method never blocks, never allocates and never drops a payload.
    pub fn boundary<F: FnOnce(&mut T, &T)>(&mut self, inherit: F) -> bool {
        let epoch = self.epoch;
        let mut swapped = false;
        if !self.shared.staged.load(Ordering::Acquire).is_null() {
            // Capacity is checked BEFORE the take: a free slot stays free (only this thread
            // fills slots), so the publication below cannot fail, and the audio thread is
            // never left holding a payload it may not drop.
            if let Some(slot) = self.shared.free_retire_slot() {
                let incoming = self.shared.staged.swap(ptr::null_mut(), Ordering::AcqRel);
                if !incoming.is_null() {
                    // SAFETY: invariant 1/3 — `incoming` is the `Box::into_raw` end of the
                    // controller's `stage`, and this swap is the single point where ownership
                    // passes to the audio side. The controller's AcqRel swap published the
                    // fully built payload; our AcqRel swap synchronises with it, so the box
                    // and everything it owns is visible and exclusive to us.
                    let mut incoming = unsafe { Box::from_raw(incoming) };
                    inherit(incoming.as_mut(), self.live.as_ref());
                    let outgoing = std::mem::replace(&mut self.live, incoming);
                    self.publish_retirement(slot, outgoing, epoch);
                    swapped = true;
                    self.shared.swaps.fetch_add(1, Ordering::Relaxed);
                }
            }
            if !swapped {
                // A payload is waiting but no retirement slot is free: keep rendering the live
                // patch and try again next boundary. Counted, never silent, never blocking.
                self.shared.deferred.fetch_add(1, Ordering::Relaxed);
            }
        }
        // The boundary is passed: publish the new epoch AFTER the retirement (if any) so that
        // a controller observing `epoch > tag` knows the audio thread has let the payload go.
        self.epoch = epoch + 1;
        self.shared.epoch.store(epoch + 1, Ordering::Release);
        swapped
    }

    /// A boundary with nothing to inherit — payload swaps that carry no runtime state.
    pub fn boundary_plain(&mut self) -> bool {
        self.boundary(|_incoming, _outgoing| {})
    }

    /// The payload that renders the current block.
    #[must_use]
    pub fn live(&self) -> &T {
        self.live.as_ref()
    }

    /// The payload that renders the current block, mutably (the render call itself).
    #[must_use]
    pub fn live_mut(&mut self) -> &mut T {
        self.live.as_mut()
    }

    /// Boundaries this half has passed — the local epoch mirror.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Stop: hand the live payload back to the caller (to drop wherever teardown belongs —
    /// NOT inside a device callback). Retirement slots and the staging slot are left for the
    /// controller: drain [`HotSwapControl::reclaim`] after the stream stops, and whatever is
    /// still staged or unretired is freed exactly once when the last handle drops.
    pub fn shutdown(self) -> T {
        // Destructure instead of Drop: the live box moves out, the `Arc` decrements, and
        // nothing else in this half owns anything.
        let Self { shared: _, live, epoch: _ } = self;
        *live
    }

    /// Publish a retired payload into the slot this boundary reserved. The store order is the
    /// protocol: tag `UNCLAIMABLE` (so no stale tag can authorise a take), then the pointer
    /// with Release (publishing the box and everything before it), then the final tag — after
    /// which the boundary's epoch store is what opens the reclaim gate.
    fn publish_retirement(&self, slot: usize, outgoing: Box<T>, tag: u64) {
        let target = &self.shared.retire[slot];
        let raw = Box::into_raw(outgoing);
        target.tag.store(UNCLAIMABLE, Ordering::Relaxed);
        // SAFETY: invariant 1/2 — `raw` is the sole ownership of the outgoing box, this slot
        // was observed free by this same (audio) thread and only this thread fills slots, so
        // the store overwrites a null and races with nothing. The controller may only convert
        // it back after the epoch gate (invariant 4), which this boundary has not reached yet.
        target.ptr.store(raw, Ordering::Release);
        target.tag.store(tag, Ordering::Release);
        self.shared.retirements.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::sync::atomic::AtomicBool;

    /// Miri runs this suite; the big exchange scales down under it so CI stays usable.
    const fn stress_n() -> u64 {
        if cfg!(miri) {
            200
        } else {
            5_000
        }
    }

    /// A payload that counts its own drops, so "exactly once, and where" is measured rather
    /// than assumed. `counter` is written by the controller before staging (a tear would show)
    /// and rewritten by the inherit hook (the chain would break).
    struct Counted {
        id: u64,
        counter: u64,
        drops: Arc<AtomicU64>,
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn payload(id: u64, drops: &Arc<AtomicU64>) -> Box<Counted> {
        Box::new(Counted { id, counter: id * 10, drops: Arc::clone(drops) })
    }

    #[test]
    fn a_staged_payload_goes_live_at_exactly_the_next_boundary() {
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(0, &drops));
        assert!(!control.is_staged());

        control.stage(payload(1, &drops));
        assert!(control.is_staged());
        // Before the boundary the old payload is still live and nothing is reclaimable.
        assert_eq!(audio.live().id, 0);
        assert!(control.reclaim().is_none());

        assert!(audio.boundary_plain(), "the boundary applies the staged payload");
        assert_eq!(audio.live().id, 1);
        assert!(!control.is_staged());

        // The boundary passed, so the outgoing payload is past its grace period: reclaimable,
        // exactly once.
        let retired = control.reclaim().expect("the retirement passed its grace period");
        assert_eq!(retired.id, 0);
        drop(retired);
        assert_eq!(drops.load(Ordering::Relaxed), 1, "only the retired payload was dropped");
        assert!(control.reclaim().is_none());

        // A boundary with nothing staged changes nothing.
        assert!(!audio.boundary_plain());
        assert_eq!(audio.live().id, 1);
        drop(audio.shutdown());
        assert_eq!(drops.load(Ordering::Relaxed), 2, "shutdown handed the live payload back");
    }

    #[test]
    fn the_inherit_hook_sees_both_payloads_at_the_swap_point() {
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(7, &drops));
        control.stage(payload(8, &drops));
        let mut saw: Option<(u64, u64)> = None;
        audio.boundary(|incoming, outgoing| {
            // The successor adopts the stream's clock — modelled here by extending the chain.
            incoming.counter = outgoing.counter + 1;
            saw = Some((incoming.id, outgoing.id));
        });
        assert_eq!(saw, Some((8, 7)), "the hook ran with the incoming and outgoing payloads");
        assert_eq!(audio.live().counter, 71, "the hook's inheritance took effect before render");
        drop(control.reclaim().unwrap());
        drop(audio.shutdown());
    }

    #[test]
    fn the_epoch_gate_refuses_a_retirement_the_audio_thread_has_not_passed_yet() {
        // White-box: the gate is `tag < epoch`, and this test sets the fields directly to walk
        // its three states — a two-thread test cannot deterministically catch the window
        // between the publication and the epoch store, so the window is tested where it lives.
        let drops = Arc::new(AtomicU64::new(0));
        let (control, audio) = HotSwap::split(*payload(0, &drops));
        let shared = &control.shared;

        // Publish a retirement "as if" at epoch 5, with the audio clock still at 5: refused.
        shared.retire[0].tag.store(UNCLAIMABLE, Ordering::Relaxed);
        shared.retire[0].ptr.store(Box::into_raw(payload(42, &drops)), Ordering::Release);
        shared.retire[0].tag.store(5, Ordering::Release);
        shared.epoch.store(5, Ordering::Release);
        assert!(
            control.reclaim().is_none(),
            "tag 5 is not below epoch 5: the grace period is open"
        );

        // Mid-publication (UNCLAIMABLE tag) refuses at any epoch.
        shared.retire[0].tag.store(UNCLAIMABLE, Ordering::Relaxed);
        shared.epoch.store(500, Ordering::Release);
        assert!(control.reclaim().is_none(), "a half-published slot must never be taken");

        // The audio thread passes the boundary: epoch 6 > tag 5 — now reclaimable.
        shared.retire[0].tag.store(5, Ordering::Release);
        shared.epoch.store(6, Ordering::Release);
        let taken = control.reclaim().expect("the grace period passed");
        assert_eq!(taken.id, 42);
        drop(taken);
        drop(audio.shutdown());
        assert_eq!(drops.load(Ordering::Relaxed), 2, "both payloads dropped exactly once");
    }

    #[test]
    fn a_superseded_stage_is_dropped_by_the_controller_and_counted() {
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(0, &drops));
        assert!(!control.stage(payload(1, &drops)), "an empty slot supersedes nothing");
        assert!(control.stage(payload(2, &drops)), "the second stage supersedes the first");
        assert_eq!(
            drops.load(Ordering::Relaxed),
            1,
            "the superseded payload was dropped immediately, control-side"
        );
        let stats = control.stats();
        assert_eq!(stats.superseded, 1);
        assert_eq!(stats.swaps, 0);

        assert!(audio.boundary_plain());
        assert_eq!(audio.live().id, 2, "the LAST staged payload is the one that goes live");
        drop(control.reclaim().unwrap());
        drop(audio.shutdown());
        assert_eq!(
            drops.load(Ordering::Relaxed),
            3,
            "initial + superseded + final: each exactly once"
        );
    }

    #[test]
    fn a_retirement_crosses_to_the_controller_only_after_the_audio_thread_let_it_go() {
        // The grace period across two real threads: nothing is reclaimable while the audio
        // thread has not reached its boundary; after it has (and the thread joined, so every
        // store it made is visible), the retired payload arrives intact and drops control-side.
        let drops = Arc::new(AtomicU64::new(0));
        let (control, audio) = HotSwap::split(*payload(0, &drops));
        let ready = Arc::new(AtomicBool::new(false));

        let audio_thread = {
            let ready = Arc::clone(&ready);
            std::thread::spawn(move || {
                let mut audio = audio;
                while !ready.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
                let swapped = audio.boundary_plain();
                let live_id = audio.live().id;
                drop(audio.shutdown());
                (swapped, live_id)
            })
        };

        control.stage(payload(1, &drops));
        for _ in 0..1_000 {
            assert!(control.reclaim().is_none(), "reclaim before the boundary must refuse");
            std::hint::spin_loop();
        }
        ready.store(true, Ordering::Release);
        let (swapped, live_id) = audio_thread.join().unwrap();
        assert!(swapped);
        assert_eq!(live_id, 1);

        let retired = control.reclaim().expect("the audio thread passed the boundary");
        assert_eq!(retired.id, 0, "the payload crossed threads intact");
        drop(retired); // dropped HERE, on the controller's thread — off the audio thread
        assert!(control.reclaim().is_none());
        drop(control);
        assert_eq!(drops.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn retirement_capacity_defers_swaps_and_drops_nothing() {
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(0, &drops));

        // Fill every retirement slot: RETIRE_SLOTS swaps with no reclaiming.
        for id in 1..=RETIRE_SLOTS as u64 {
            control.stage(payload(id, &drops));
            assert!(audio.boundary_plain(), "slot {id} is free, the swap must happen");
        }
        assert_eq!(control.stats().pending_retirements, RETIRE_SLOTS);

        // The next swap has nowhere to retire into: it must DEFER — the payload stays staged,
        // the live patch keeps rendering, the event is counted.
        control.stage(payload(100, &drops));
        assert!(!audio.boundary_plain(), "no retirement slot, no swap — deferred, not forced");
        assert_eq!(audio.live().id, RETIRE_SLOTS as u64, "the live patch is untouched");
        assert!(control.is_staged(), "the deferred payload is still waiting");
        assert_eq!(control.stats().deferred, 1);

        // One reclaim frees one slot; the next boundary completes the deferred swap.
        let first = control.reclaim().expect("the oldest retirement is past its grace period");
        assert_eq!(first.id, 0);
        drop(first);
        assert!(audio.boundary_plain());
        assert_eq!(audio.live().id, 100);

        // Teardown: drain, shut down, drop the controller. Every payload drops exactly once.
        let mut reclaimed = 1;
        while let Some(r) = control.reclaim() {
            reclaimed += 1;
            drop(r);
        }
        drop(audio.shutdown());
        drop(control);
        assert_eq!(
            reclaimed,
            RETIRE_SLOTS + 1,
            "the eight fills plus the deferred swap's own retirement"
        );
        assert_eq!(
            drops.load(Ordering::Relaxed),
            (RETIRE_SLOTS + 2) as u64,
            "initial + RETIRE_SLOTS retired + the deferred survivor: no leak, no double drop"
        );
    }

    #[test]
    fn teardown_frees_in_flight_payloads_exactly_once() {
        // A staged-but-never-swapped payload and an unreclaimed retirement must not leak when
        // the halves die in either order.
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(0, &drops));
        control.stage(payload(1, &drops));
        assert!(audio.boundary_plain()); // retires 0, lives 1
        control.stage(payload(2, &drops)); // staged, never swapped
        drop(audio); // audio half dropped WITHOUT shutdown: the live payload drops here
        assert_eq!(drops.load(Ordering::Relaxed), 1, "the live payload dropped with its half");
        drop(control); // last handle: Shared::drop frees the staged AND the retired payload
        assert_eq!(drops.load(Ordering::Relaxed), 3, "nothing in flight leaked");
    }

    #[test]
    fn stats_count_the_boundaries_even_when_nothing_swaps() {
        let drops = Arc::new(AtomicU64::new(0));
        let (control, mut audio) = HotSwap::split(*payload(0, &drops));
        for _ in 0..10 {
            assert!(!audio.boundary_plain());
        }
        let stats = control.stats();
        assert_eq!(stats.epoch, 10);
        assert_eq!(stats.swaps, 0);
        assert_eq!(stats.retirements, 0);
        assert!(!stats.staged);
        assert_eq!(audio.epoch(), 10, "the audio mirror and the shared clock agree");
        drop(audio.shutdown());
    }

    #[test]
    fn two_threads_exchange_thousands_of_payloads_in_order_without_tearing() {
        // The concurrency stress at kernel scale: paced staging (each payload must be consumed
        // before the next is staged), so the claims are exact — every id goes live in order,
        // every retirement comes back, nothing tears, nothing leaks, and the audio thread
        // allocates nothing after its first boundary. The executor-level 10 000-mutation
        // acceptance lives in sparq-audio's `cross_thread.rs`.
        let n = stress_n();
        let drops = Arc::new(AtomicU64::new(0));
        let (control, audio) = HotSwap::split(*payload(0, &drops));
        let stop = Arc::new(AtomicBool::new(false));
        let failures = Arc::new(AtomicU64::new(0));

        let audio_thread = {
            let stop = Arc::clone(&stop);
            let failures = Arc::clone(&failures);
            std::thread::spawn(move || {
                let mut audio = audio;
                let mut expected_next_live = 1u64;
                let mut boundaries = 0u64;
                let mut swaps = 0u64;
                while !stop.load(Ordering::Acquire) {
                    let swapped = audio.boundary(|incoming, outgoing| {
                        // The tear check: the controller wrote `counter = id * 10` BEFORE
                        // staging; the Release/Acquire pair on `staged` must deliver it intact.
                        if incoming.counter != incoming.id * 10 {
                            failures.fetch_add(1, Ordering::Relaxed);
                        }
                        // The inherit check: the chain advances by exactly one per swap.
                        incoming.counter = outgoing.counter + 1;
                    });
                    if swapped {
                        swaps += 1;
                        let live = audio.live();
                        if live.id != expected_next_live || live.counter != live.id {
                            failures.fetch_add(1, Ordering::Relaxed);
                        }
                        expected_next_live += 1;
                    }
                    boundaries += 1;
                    if boundaries % 32 == 0 {
                        std::thread::yield_now();
                    }
                }
                drop(audio.shutdown());
                (boundaries, swaps)
            })
        };

        let mut reclaimed_ids: Vec<u64> = Vec::new();
        for id in 1..=n {
            control.stage(payload(id, &drops));
            // Wait for consumption: paced staging keeps the order claims exact.
            while control.stats().swaps + control.stats().superseded < id {
                std::hint::spin_loop();
            }
            while let Some(r) = control.reclaim() {
                reclaimed_ids.push(r.id);
                drop(r);
            }
        }
        stop.store(true, Ordering::Release);
        let (boundaries, swaps) = audio_thread.join().unwrap();
        while let Some(r) = control.reclaim() {
            reclaimed_ids.push(r.id);
            drop(r);
        }

        assert_eq!(failures.load(Ordering::Relaxed), 0, "no torn payloads, no out-of-order swaps");
        assert_eq!(swaps, n, "every staged payload went live");
        assert_eq!(reclaimed_ids, (0..n).collect::<Vec<u64>>(), "every retirement came back");
        assert_eq!(control.stats().superseded, 0);
        assert_eq!(control.stats().deferred, 0, "paced reclaim never starved the slots");
        assert_eq!(control.stats().retirements, n);
        assert!(boundaries >= swaps, "the audio thread free-ran between stages");
        drop(control);
        assert_eq!(
            drops.load(Ordering::Relaxed),
            n + 1,
            "the initial payload, every retired one, and the final live one: each exactly once"
        );
    }
}
