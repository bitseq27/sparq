//! Device abstraction.
//!
//! Phase 0 ships the **null/virtual device** only. It is not a stub: it is the CI workhorse and the
//! thing that makes every layer above the HAL testable on a machine with no audio hardware
//! (ADR-009). The real backends (WASAPI exclusive, ASIO, WASAPI shared, PipeWire/JACK, CoreAudio)
//! arrive in WO-006 behind the same `StreamConfig`.

pub mod null;

pub use null::{NullDevice, RenderConfig, RenderReport};

/// The device configuration every backend must be able to express.
///
/// Written now, in Phase 0, against Phase 7 requirements — multichannel, per-channel latency and
/// device aggregation (ADR-004) — so that the trait never has to be broken later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Frames per block.
    pub block_frames: usize,
    /// Input channel count (0 = output only).
    pub inputs: usize,
    /// Output channel count. Phase 7 target: up to 64.
    pub outputs: usize,
    /// Whether the backend may claim the device exclusively.
    pub exclusive: bool,
}

impl StreamConfig {
    /// Stereo at 48 kHz with 64-frame blocks — the WO-005 default.
    #[must_use]
    pub const fn stereo_48k() -> Self {
        Self { sample_rate: 48_000, block_frames: 64, inputs: 0, outputs: 2, exclusive: false }
    }

    /// Seconds of audio per block.
    #[must_use]
    pub fn block_seconds(&self) -> f64 {
        self.block_frames as f64 / f64::from(self.sample_rate)
    }

    /// Bytes of interleaved `f32` per block.
    #[must_use]
    pub const fn block_bytes(&self) -> usize {
        self.block_frames * self.outputs * std::mem::size_of::<f32>()
    }
}
