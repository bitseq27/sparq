//! DSP nodes for the sparq audio path.
//!
//! Organisation: one file per node family, each carrying its own unit and property tests. Every
//! node follows the same contract from plan §5.2 and `docs/api/module-api-v1.md`:
//!
//! * `new(...)` — cheap construction, no rate-dependent state;
//! * `prepare(sample_rate, ...)` — **all** allocation and coefficient computation happens here;
//! * `process(&mut self, ...)` — no allocation, no locking, no syscall, no wall-clock read,
//!   deterministic for a given state and input.
//!
//! These are not yet modules in the WO-007 sense (no manifest, no ports, no discovery). They are
//! the DSP that those modules will wrap, written now so it can be measured and golden-tested before
//! a module system exists to hide behind.

pub mod bitcrush;
pub mod core;
pub mod delay;
pub mod env;
pub mod fft;
pub mod filter;
pub mod noise;
pub mod osc;

pub use bitcrush::BitCrush;
pub use delay::DelayLine;
pub use env::{AdEnv, AdsrEnv, EnvCurve, EnvSegment};
pub use filter::{SvfFilter, SvfMode};
pub use noise::{Dust, Noise, NoiseColour};
pub use osc::{PolyBlepOsc, Sync, WaveShape};

pub use self::core::{db_to_linear, linear_to_db, Gain, SineOsc};

/// Note number (MIDI-ish, 69 = A440) to frequency in Hz. Supports fractional notes, which is what
/// microtonal and gliding parts need (plan §10.4).
#[must_use]
pub fn note_to_hz(note: f64) -> f64 {
    440.0 * 2.0_f64.powf((note - 69.0) / 12.0)
}

/// Frequency in Hz to note number.
#[must_use]
pub fn hz_to_note(hz: f64) -> f64 {
    if hz <= 0.0 {
        return f64::NEG_INFINITY;
    }
    69.0 + 12.0 * (hz / 440.0).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_hz_roundtrip() {
        for note in [0.0f64, 21.5, 60.0, 69.0, 81.25, 127.0] {
            let hz = note_to_hz(note);
            assert!((hz_to_note(hz) - note).abs() < 1e-9, "note {note} -> {hz} Hz");
        }
        assert!((note_to_hz(69.0) - 440.0).abs() < 1e-12);
        assert!((note_to_hz(60.0) - 261.6255653).abs() < 1e-6);
        assert!((note_to_hz(57.0) - 220.0).abs() < 1e-9);
    }
}
