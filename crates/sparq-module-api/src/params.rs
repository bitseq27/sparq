//! Parameter delivery: an immutable snapshot, swapped at the block boundary.
//!
//! module-api §2 and §9: `configure` runs on the control thread, `process` on the audio thread, and
//! the only thing that crosses is a double-buffered snapshot. Two properties are load-bearing and
//! both are tested below:
//!
//! * **the swap is atomic at the block boundary** — the audio thread reads one [`ParamSet`] for the
//!   whole block and never a half-written one;
//! * **a change made during a block is deferred to the next one** — provably, not by convention.
//!
//! The snapshot is `Copy` and fixed-size so it can travel through the kernel's lock-free
//! [`SpscRing`] without the audio thread ever allocating. That is why [`MAX_PARAMS`] is a hard v0
//! limit rather than a default: a `Vec` here would be an allocation on the audio path, and the whole
//! point is that there isn't one.

use std::fmt;
use std::sync::Arc;

use sparq_kernel::sync::SpscRing;

/// The most parameters a module may declare in v0.
///
/// A snapshot is [`MAX_PARAMS`] `f32` values plus a version, so it stays `Copy` and fits in one ring
/// slot. A manifest declaring more is rejected at validation (`E-CROSS-FIELD:params-cap`) rather
/// than silently truncated — a parameter that cannot be delivered must not pretend to exist.
pub const MAX_PARAMS: usize = 32;

/// How deep the control→audio ring is. Deeper than one so a burst of UI writes coalesces into the
/// newest value instead of being refused; shallow enough that a stale value cannot survive long.
pub const DEFAULT_BUS_CAPACITY: usize = 8;

/// An immutable parameter snapshot for one block.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ParamSet {
    version: u64,
    values: [f32; MAX_PARAMS],
}

impl ParamSet {
    /// The snapshot a module starts with: version 0, every parameter 0.
    #[must_use]
    pub const fn zeroed() -> Self {
        Self { version: 0, values: [0.0; MAX_PARAMS] }
    }

    /// A snapshot from a slice of initial values.
    ///
    /// Returns `None` if `values` is longer than [`MAX_PARAMS`], which validation reports as
    /// `E-CROSS-FIELD:params-cap` rather than truncating.
    #[must_use]
    pub fn new(version: u64, values: &[f32]) -> Option<Self> {
        if values.len() > MAX_PARAMS {
            return None;
        }
        let mut set = Self { version, values: [0.0; MAX_PARAMS] };
        set.values[..values.len()].copy_from_slice(values);
        Some(set)
    }

    /// The snapshot's version. Two snapshots with the same version are the same values.
    #[must_use]
    pub const fn version(self) -> u64 {
        self.version
    }

    /// Parameter `index`, or `None` past the declared count.
    #[must_use]
    pub fn get(self, index: usize) -> Option<f32> {
        self.values.get(index).copied()
    }

    /// Parameter `index` for use inside `process`: 0.0 past the end.
    ///
    /// Returning a value rather than an `Option` is deliberate — a per-sample branch on presence is
    /// a cost the audio path should not pay, and 0.0 is the neutral value for every modulation
    /// target in the closed unit vocabulary.
    #[must_use]
    pub fn at(self, index: usize) -> f32 {
        self.values.get(index).copied().unwrap_or(0.0)
    }

    /// The whole snapshot as a slice, for modules that want to iterate.
    #[must_use]
    pub const fn values(&self) -> &[f32; MAX_PARAMS] {
        &self.values
    }
}

/// The control-thread end of parameter delivery. Cheap to clone; every clone publishes to the same
/// audio-thread slot.
#[derive(Clone)]
pub struct ParamBus {
    ring: Arc<SpscRing<ParamSet>>,
}

// `SpscRing` is not `Debug` (it is a lock-free structure whose interesting state is counters), so
// the bus reports what actually matters: how much is in flight and how much was refused.
impl fmt::Debug for ParamBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParamBus")
            .field("pending", &self.ring.len())
            .field("refusals", &self.ring.refusals())
            .field("drops", &self.ring.drops())
            .finish()
    }
}

impl ParamBus {
    /// A bus and its matching audio-thread slot, joined by a ring of `capacity` snapshots.
    #[must_use]
    pub fn new(capacity: usize) -> (Self, ParamSlot) {
        let ring = Arc::new(SpscRing::new(capacity.max(1)));
        (Self { ring: Arc::clone(&ring) }, ParamSlot { ring, current: ParamSet::zeroed() })
    }

    /// A bus with [`DEFAULT_BUS_CAPACITY`].
    #[must_use]
    pub fn paired() -> (Self, ParamSlot) {
        Self::new(DEFAULT_BUS_CAPACITY)
    }

    /// Publishes a snapshot. It becomes visible to the audio thread at the **next** block boundary.
    ///
    /// `false` means the ring was full and the write was refused — counted by the ring, never
    /// silent, because an invisible failure is how a parameter appears to stop working.
    #[must_use]
    pub fn publish(&self, set: ParamSet) -> bool {
        self.ring.push(set)
    }

    /// How many writes were refused for a full ring since creation.
    #[must_use]
    pub fn refusals(&self) -> u64 {
        self.ring.refusals()
    }

    /// How many snapshots were dropped since creation.
    #[must_use]
    pub fn drops(&self) -> u64 {
        self.ring.drops()
    }

    /// Snapshots waiting to be picked up at the next boundary.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.ring.len()
    }
}

/// The audio-thread end. Drained exactly once per block, in `begin_block`.
pub struct ParamSlot {
    ring: Arc<SpscRing<ParamSet>>,
    current: ParamSet,
}

impl fmt::Debug for ParamSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParamSlot")
            .field("current_version", &self.current.version())
            .field("pending", &self.ring.len())
            .finish()
    }
}

impl ParamSlot {
    /// Called once per block, before `process`: takes the newest published snapshot.
    ///
    /// Returns whether the snapshot changed. Allocation-free — it pops from the ring and keeps the
    /// last value, so a burst of control writes coalesces instead of queueing.
    pub fn begin_block(&mut self) -> bool {
        let before = self.current;
        while let Some(set) = self.ring.pop() {
            self.current = set;
        }
        self.current != before
    }

    /// The snapshot `process` will see for the whole of this block.
    #[must_use]
    pub fn current(&self) -> ParamSet {
        self.current
    }

    /// Snapshots still waiting, i.e. writes that arrived during this block and are therefore
    /// deferred to the next one.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.ring.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn a_snapshot_is_copy_and_small_enough_for_a_ring_slot() {
        assert_eq!(std::mem::size_of::<ParamSet>(), 8 + 4 * MAX_PARAMS);
        let a = ParamSet::new(1, &[0.5]).unwrap();
        let b = a;
        assert_eq!(a, b, "Copy, so the audio thread never shares a heap buffer");
    }

    #[test]
    fn more_params_than_the_cap_is_refused_not_truncated() {
        let too_many = vec![0.0; MAX_PARAMS + 1];
        assert!(ParamSet::new(1, &too_many).is_none());
        assert!(ParamSet::new(1, &[0.0; MAX_PARAMS]).is_some());
    }

    #[test]
    fn at_is_neutral_past_the_end() {
        let set = ParamSet::new(1, &[0.25]).unwrap();
        assert_eq!(set.at(0), 0.25);
        assert_eq!(set.at(1), 0.0);
        assert_eq!(set.at(MAX_PARAMS + 100), 0.0);
        assert_eq!(set.get(1), Some(0.0));
    }

    #[test]
    fn a_change_during_a_block_is_deferred_to_the_next_one() {
        // The acceptance criterion, as a test: "a param change during a block is provably deferred
        // to the next block."
        let (bus, mut slot) = ParamBus::paired();
        assert!(bus.publish(ParamSet::new(1, &[0.1]).unwrap()));
        assert!(slot.begin_block(), "the first snapshot must land");
        assert_eq!(slot.current().version(), 1);

        // Block 2 begins, then the control thread writes *during* it.
        assert!(!slot.begin_block(), "nothing was published between the two boundaries");
        let mid_block = slot.current();
        assert!(bus.publish(ParamSet::new(2, &[0.9]).unwrap()));
        assert_eq!(slot.pending(), 1, "the write is queued, not applied");
        assert_eq!(slot.current(), mid_block, "block 2 still sees version 1");
        assert_eq!(slot.current().at(0), 0.1);

        // Block 3: now it lands.
        assert!(slot.begin_block(), "begin_block reports that the snapshot changed");
        assert_eq!(slot.current().version(), 2);
        assert_eq!(slot.current().at(0), 0.9);
        assert_eq!(slot.pending(), 0);
    }

    #[test]
    fn a_burst_of_writes_coalesces_to_the_newest() {
        let (bus, mut slot) = ParamBus::paired();
        for v in 1..=5 {
            assert!(bus.publish(ParamSet::new(v, &[v as f32 * 0.001]).unwrap()));
        }
        assert!(slot.begin_block());
        assert_eq!(slot.current().version(), 5, "newest wins; the rest are dropped, not replayed");
        assert_eq!(bus.pending(), 0);
    }

    #[test]
    fn begin_block_reports_no_change_when_nothing_was_written() {
        let (_bus, mut slot) = ParamBus::paired();
        assert!(!slot.begin_block());
        assert_eq!(slot.current(), ParamSet::zeroed());
    }

    #[test]
    fn a_full_ring_refuses_loudly() {
        let (bus, _slot) = ParamBus::new(2);
        assert!(bus.publish(ParamSet::new(1, &[0.0]).unwrap()));
        assert!(bus.publish(ParamSet::new(2, &[0.0]).unwrap()));
        // Capacity is rounded up to a power of two internally, so keep writing until it refuses.
        let mut refused = false;
        for v in 3..64 {
            if !bus.publish(ParamSet::new(v, &[0.0]).unwrap()) {
                refused = true;
                break;
            }
        }
        assert!(refused, "a bounded ring must eventually refuse");
        assert!(bus.refusals() > 0, "and the refusal must be counted, never silent");
    }
}
