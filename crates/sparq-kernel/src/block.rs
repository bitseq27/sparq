//! The block-processing contract and the three-clock model (ADR-006, ADR-009).
//!
//! Everything that touches audio receives a [`BlockContext`] and nothing else: no wall clock, no
//! device handle, no allocator. That is how plan §5.2's "no syscalls on the audio thread" is
//! enforced by types rather than by discipline.

/// Identifies a block of audio. Sample offsets within the block are `0..frames`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockId(pub u64);

impl BlockId {
    /// The first block.
    pub const FIRST: Self = Self(0);

    /// The next block.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Immutable for the duration of a block: parameters, timing and format cannot change underneath
/// a module mid-block (plan §5.2 rule 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockContext {
    /// Which block this is since the stream started.
    pub block: BlockId,
    /// Absolute sample index of the first frame of this block (`t_sample`, ADR-006).
    pub sample_offset: u64,
    /// Frames in this block.
    pub frames: usize,
    /// Device sample rate in Hz.
    pub sample_rate: u32,
    /// Output channel count.
    pub channels: usize,
    /// Musical position at the first frame, in ticks (960 PPQN default).
    pub tick: u64,
    /// Ticks per quarter note.
    pub ppqn: u32,
}

impl BlockContext {
    /// A context for offline/deterministic work starting at block zero.
    #[must_use]
    pub const fn offline(sample_rate: u32, frames: usize, channels: usize) -> Self {
        Self {
            block: BlockId::FIRST,
            sample_offset: 0,
            frames,
            sample_rate,
            channels,
            tick: 0,
            ppqn: 960,
        }
    }

    /// Duration of this block in seconds.
    #[must_use]
    pub fn block_seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate)
    }

    /// Absolute sample index of the first frame of the next block.
    #[must_use]
    pub fn next_sample_offset(&self) -> u64 {
        self.sample_offset + self.frames as u64
    }

    /// The same context, advanced by one block. `tick` advances from the tempo in `clock`.
    #[must_use]
    pub fn advance(&self, clock: &crate::clock::Clock) -> Self {
        Self {
            block: self.block.next(),
            sample_offset: self.next_sample_offset(),
            frames: self.frames,
            sample_rate: self.sample_rate,
            channels: self.channels,
            tick: clock.tick_at_sample(self.next_sample_offset()),
            ppqn: self.ppqn,
        }
    }

    /// Sample-accurate musical position of a frame offset within this block.
    ///
    /// This is the primitive every sequencer will use (WO-009 acceptance: a trigger scheduled at
    /// tick T fires at exactly the corresponding sample index).
    #[must_use]
    pub fn tick_at(&self, frame: usize, clock: &crate::clock::Clock) -> u64 {
        clock.tick_at_sample(self.sample_offset + frame as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    #[test]
    fn block_seconds_is_exact_for_power_of_two_rates() {
        let ctx = BlockContext::offline(48_000, 64, 2);
        assert!((ctx.block_seconds() - 64.0 / 48_000.0).abs() < 1e-12);
    }

    #[test]
    fn advance_moves_sample_offset_by_frames() {
        let ctx = BlockContext::offline(96_000, 64, 2);
        let clock = Clock::new(96_000, 120.0, 960);
        assert_eq!(ctx.advance(&clock).sample_offset, 64);
        assert_eq!(ctx.advance(&clock).advance(&clock).sample_offset, 128);
    }
}
