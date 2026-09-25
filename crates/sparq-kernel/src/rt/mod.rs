//! Real-time status types shared by the kernel and the layers above it, plus the thread-level
//! RT discipline (MMCSS, priority, working-set) that the HAL pump threads apply (WO-006).

pub mod thread;

/// What happened to a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockStatus {
    /// Processed inside its time budget.
    Ok,
    /// The producer did not keep up: frames were dropped (an underrun).
    Underrun,
    /// The consumer did not keep up: a control message was dropped.
    ControlDrop,
    /// The device reported an error.
    DeviceError,
}

impl BlockStatus {
    /// Whether this status represents a failure that must be counted and surfaced.
    #[must_use]
    pub const fn is_failure(self) -> bool {
        !matches!(self, Self::Ok)
    }
}

/// Counters that must never fail silently (plan §4.3: every ring and every failure path has a
/// counter exposed in the diagnostics panel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Blocks processed.
    pub blocks: u64,
    /// Underruns: the producer missed a deadline.
    pub underruns: u64,
    /// Control messages dropped because a ring was full (`drop-oldest` / `drop-and-count` policy).
    pub control_drops: u64,
    /// Allocations observed on the audio path. Must remain zero (plan §5.2 rule 1).
    pub audio_allocations: u64,
    /// Device errors.
    pub device_errors: u64,
}

impl Counters {
    /// A zeroed set.
    #[must_use]
    pub const fn zero() -> Self {
        Self { blocks: 0, underruns: 0, control_drops: 0, audio_allocations: 0, device_errors: 0 }
    }

    /// Whether every discipline counter is clean.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.audio_allocations == 0 && self.device_errors == 0
    }

    /// One-line readout for the diagnostics overlay.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "blocks {} · underrun {} · ctrl-drop {} · alloc {} · dev-err {}",
            self.blocks,
            self.underruns,
            self.control_drops,
            self.audio_allocations,
            self.device_errors
        )
    }
}
