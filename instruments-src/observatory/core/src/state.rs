//! Deterministic state save/restore (the journal and the project file hash these bytes).
//!
//! The contract's rule (guest.wit `save-state`): "two calls with unchanged state must produce
//! identical bytes." For the Observatory the state that matters — the state a project save, a scene
//! recall or an undo must carry — is the wall's CONFIGURATION: the 26 params (cell assignments,
//! layout, toggles, sliders). That is what this blob persists.
//!
//! # What is deliberately NOT in the blob: the ticker scroll phase
//!
//! The scroll phase is display-instance animation, not configuration. Two reasons it stays out:
//! * `save-state` is called on the AUDIO instance (instrument-host §4), which never runs `draw` —
//!   so its phase is always the initial 0.0. Serialising it would persist a value the instance does
//!   not own, and applying that 0.0 to the display instance on every sync would reset the scroll.
//! * A display is never replayed (ADR-007 / display.wit): the scroll rides `frame-context.time-sec`
//!   and reaches neither the audio path nor the journal, so it is not part of the deterministic
//!   state the journal hashes. It lives on [`crate::wall::Wall`] as ephemeral per-instance state.
//!
//! The encoding is fixed and dependency-free: a 4-byte magic, a 1-byte schema version, and the 26
//! params as little-endian f32 bit patterns in manifest order. No maps, no timestamps, no floats
//! whose text form could vary — `to_bits` of an equal value is an equal pattern, so the byte stream
//! is a pure function of the params. A blob that does not parse (wrong magic, wrong version, wrong
//! length) is refused IN WORDS via [`StateError`], never applied half-way.

use crate::params::{Params, PARAM_COUNT};

/// The blob magic (`"OBS1"`), so a mis-routed state blob is recognised and refused rather than
/// misread as Observatory state.
pub const MAGIC: [u8; 4] = *b"OBS1";
/// The state schema version (the manifest's `state.schema_version = 1`). A blob at another version
/// is refused — the host runs the migration chain BEFORE `configure`, so the guest only ever sees
/// its current version (guest.wit).
pub const VERSION: u8 = 1;
/// The blob length: magic (4) + version (1) + params (26 × 4).
pub const BLOB_LEN: usize = 4 + 1 + PARAM_COUNT * 4;

/// The Observatory's persisted state — the wall configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// The wall configuration.
    pub params: Params,
}

/// Why a state blob could not be applied — always in words (the contract's honest-refusal rule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    /// The blob is not [`BLOB_LEN`] bytes.
    BadLength(usize),
    /// The magic is not [`MAGIC`] — this is not an Observatory state blob.
    BadMagic,
    /// The version is not [`VERSION`] — the host should have migrated it first.
    BadVersion(u8),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadLength(n) => write!(
                f,
                "state blob is {n} byte(s), expected {BLOB_LEN} (magic + version + {PARAM_COUNT} \
                 params) — this is not a dat/observatory/state v{VERSION} blob"
            ),
            Self::BadMagic => write!(
                f,
                "state blob magic is not \"OBS1\" — this blob belongs to a different module; the \
                 host routes state by schema_id, so this is a mis-delivery, not a corrupt save"
            ),
            Self::BadVersion(v) => write!(
                f,
                "state blob version {v} != {VERSION} — the host runs the migration chain before \
                 configure, so a version mismatch means a missing migration, not a guest problem"
            ),
        }
    }
}

impl std::error::Error for StateError {}

impl State {
    /// Serialises to the deterministic blob. Two calls with unchanged params produce identical bytes.
    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BLOB_LEN);
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        for x in self.params.to_values() {
            out.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        debug_assert_eq!(out.len(), BLOB_LEN, "the blob length is fixed");
        out
    }

    /// Applies a state blob, refusing a malformed one in words.
    ///
    /// # Errors
    /// A [`StateError`] naming the defect when the blob is not a well-formed v[`VERSION`]
    /// Observatory state.
    pub fn restore(blob: &[u8]) -> Result<Self, StateError> {
        if blob.len() != BLOB_LEN {
            return Err(StateError::BadLength(blob.len()));
        }
        if blob[0..4] != MAGIC {
            return Err(StateError::BadMagic);
        }
        if blob[4] != VERSION {
            return Err(StateError::BadVersion(blob[4]));
        }
        let mut values = [0.0f32; PARAM_COUNT];
        for (i, slot) in values.iter_mut().enumerate() {
            let off = 5 + i * 4;
            let bits = u32::from_le_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]]);
            *slot = f32::from_bits(bits);
        }
        Ok(Self { params: Params::from_values(&values) })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn save_restore_round_trips() {
        let mut s = State::default();
        s.params.cells[2] = 7;
        s.params.layout = crate::layout::Layout::Grid3x3;
        s.params.ticker_speed = 2.0;
        let blob = s.save();
        let t = State::restore(&blob).unwrap();
        assert_eq!(s, t);
    }

    #[test]
    fn save_is_deterministic_byte_for_byte() {
        // The contract's own rule: two calls with unchanged state produce identical bytes.
        let s = State::default();
        assert_eq!(s.save(), s.save());
        let mut t = State::default();
        t.params.selected_cell = 4;
        t.params.history_s = 600.0;
        assert_eq!(t.save(), t.save(), "unchanged state ⇒ identical bytes");
        assert_ne!(s.save(), t.save(), "different state ⇒ different bytes");
    }

    #[test]
    fn the_blob_length_is_fixed() {
        assert_eq!(State::default().save().len(), BLOB_LEN);
        assert_eq!(BLOB_LEN, 4 + 1 + PARAM_COUNT * 4);
    }

    #[test]
    fn a_wrong_length_is_refused_in_words() {
        let e = State::restore(&[0, 1, 2]).unwrap_err();
        assert!(matches!(e, StateError::BadLength(3)));
        assert!(e.to_string().contains("3 byte"), "{e}");
    }

    #[test]
    fn a_wrong_magic_is_refused_in_words() {
        let mut blob = State::default().save();
        blob[0] = b'X';
        let e = State::restore(&blob).unwrap_err();
        assert_eq!(e, StateError::BadMagic);
        assert!(e.to_string().contains("OBS1"), "{e}");
    }

    #[test]
    fn a_wrong_version_is_refused_in_words() {
        let mut blob = State::default().save();
        blob[4] = 99;
        let e = State::restore(&blob).unwrap_err();
        assert_eq!(e, StateError::BadVersion(99));
        assert!(e.to_string().contains("migration"), "{e}");
    }

    #[test]
    fn negatives_clamp_so_no_negative_zero_reaches_the_blob_via_decode() {
        // from_values clamps float params into their (positive) domains, so a -0.0 input cannot
        // produce a -0.0 in the blob. And a directly-set -0.0 still serialises deterministically —
        // the contract's rule is "unchanged state ⇒ identical bytes", which holds per bit-pattern.
        let via = Params::from_values(&[-0.0; PARAM_COUNT]);
        assert!(
            via.ticker_speed >= crate::params::TICKER_SPEED_MIN,
            "negatives clamp to the domain min"
        );
        assert!(via.intensity >= crate::params::INTENSITY_MIN);
        let mut a = State::default();
        a.params.ticker_speed = -0.0;
        let b = a; // the same value
        assert_eq!(a.save(), b.save(), "the same value serialises identically");
    }
}
