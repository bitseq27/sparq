//! The instrument host's runtime half (WO-018 §2–§4, built by WO-020 INC5) — the wasmtime loader,
//! the four import doors, the two-instance ordering, fuel/epoch against the block watchdog, and
//! the hand-in gate's runtime stages.
//!
//! Everything here rides the `instrument-host` feature (wasmtime, exact-pinned at the workspace
//! level). The design of record is `docs/instrument-host.md`; this module is that document as
//! code, and where the two could drift the document wins and the code is the defect.
//!
//! # The shape
//!
//! * [`bindings`] — the frozen `sparq:instrument@1.1.0` world, expanded by wasmtime's own
//!   `bindgen!` from the hash-pinned WIT directory (`docs/api/instrument-wit/wit`). The expansion
//!   is machine-derived from the same bytes `wit_snapshot` pins, so the host cannot drift from
//!   the contract without failing a test that already exists.
//! * [`doors`] — the four imports (`tokens`, `host`, `assets`, `sources`) as one store-data type
//!   ([`doors::InstrumentHost`]) implementing the generated `Host` traits. Capability by absence:
//!   the linker defines these four interfaces and NOTHING else — no WASI, no ambient authority
//!   (instrument-wit decision E). A component importing anything else fails to instantiate, and
//!   that failure IS the capability honour proof (stage 3).
//! * [`convert`] — the native↔WIT mappings (data records, params, resources, the clock) and the
//!   D13 display-list interchange writer: a WIT `surface` becomes the SAME JSON bytes the
//!   observatory harness's `ir_json` writes, which is what lets stage 4 hash the wasm path's
//!   render against the harness's pinned golden — one interchange, two producers, byte-equal.
//! * [`load`] — the engine (fuel + epoch + component model + wasmtime's own compile cache), the
//!   package load (compile → identity cross-check → prepare-before-anything → the two-instance
//!   pair), the memory limiter from `[capabilities]`, and [`load::WasmInstrument`] — the
//!   `sparq_module_api::Module` face the executor adopts (fuel trap → `BlockStatus::Overrun`,
//!   the §3 mapping onto the EXISTING watchdog vocabulary, never a second mechanism).
//! * [`stages`] — the runtime stages of `sparq mod validate` (3 smoke, 4 golden render, 5's
//!   measurement half, 6 visual conformance + `preview.svg`), each producing the contract crate's
//!   `ValidationReport` so the CLI's voice does not fork.
//!
//! # What v1 honestly does not do (words, not holes)
//!
//! * **Ported instruments refuse at load.** The audio-path conversion (native `AudioCtx` ↔ WIT
//!   `block-input`) ships when a ported instrument exists to prove it against; v1's only
//!   instrument is port-less (plan D5) and the refusal names that. The port-less `process` call
//!   is real: the exact negotiated empty shapes, checked both ways.
//! * **`assets.open` answers `not-found` for declared hashes.** The content-addressed asset store
//!   is Phase 3; until then a declared asset is honestly absent (the WIT's own `not-found` case),
//!   never a silent empty read.
//! * **The `port` source binding serves nothing** (in words): the analysis-ring door for wasm
//!   instruments arrives with the live display binding work. Stream bindings — the Observatory's
//!   whole data path — are fully served.
//!
//! # Threads
//!
//! The audio instance lives on the audio thread (its `Store` is `Send`, never `Sync`; one owner).
//! The display instance lives on the UI thread. They share nothing mutable; coherence is the
//! §4 ordering (audio applies state first, the display follows at its next frame). Nothing in
//! this module blocks on a lock on the audio path — the sources door is UI-thread only and says
//! so when called from `process` (an RT violation, captured in words).

use std::path::{Path, PathBuf};

pub mod convert;
pub mod doors;
pub mod load;
pub mod stages;

/// The frozen world, expanded from the hash-pinned WIT by wasmtime's own generator. `AssetRep`
/// is the host representation of the `assets.asset` resource (the `with:` mapping); it is
/// declared here so the expansion resolves it at the invocation site.
pub mod bindings {
    /// The host's asset representation: content-addressed bytes (v1: never constructed — the
    /// asset store is Phase 3, `assets.open` answers `not-found` in words; the type exists so
    /// the door's shape is complete and the day the store lands is a body change, not a
    /// contract change).
    #[derive(Debug)]
    pub struct AssetRep {
        /// The asset's bytes.
        pub bytes: std::sync::Arc<Vec<u8>>,
    }

    wasmtime::component::bindgen!({
        path: "../../docs/api/instrument-wit/wit",
        world: "instrument",
        with: {
            "sparq:instrument/assets.asset": AssetRep,
        },
    });
}

/// Which phase the guest is in — the RT discipline's evidence (host.wit rule 3: `log` during
/// `process`/`draw` is a violation the validator names; sources.wit: the door is UI-thread only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Control-thread work: prepare/configure/message/save-state. Doors are open.
    Control,
    /// Inside `process` (audio thread). Host calls are captured as RT violations.
    Process,
    /// Inside `draw` (UI thread). `sources` is open; `host.log` is a violation.
    Draw,
}

/// The deterministic clocks, as pure data — the host-side twin of `bindings`' `time-info`,
/// aligned with what WO-009 shipped (host.wit's own note): `sparq_kernel::block::BlockContext`
/// carries `t_sample`/`tick`/`ppqn`; tempo and bar/beat arrive from the musical clock when the
/// executor exposes it (until then the loader fills the declared defaults and the fields say so
/// in words — a lie in a clock field is a replay defect, so v1 states the gap instead).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockReading {
    /// First sample of the current block.
    pub t_sample: u64,
    /// Transport playing.
    pub playing: bool,
    /// Musical position in ticks (transport domain).
    pub tick: u64,
    /// Ticks per quarter note.
    pub ppqn: u32,
    /// Effective tempo mid-ramp.
    pub tempo_bpm: f64,
    /// Target tempo.
    pub target_bpm: f64,
    /// Derived bar (1-based).
    pub bar: u64,
    /// Derived beat (1-based).
    pub beat: u64,
    /// Derived tick-in-beat (0-based).
    pub tick_in_beat: u64,
}

impl Default for ClockReading {
    fn default() -> Self {
        Self {
            t_sample: 0,
            playing: false,
            tick: 0,
            ppqn: 960,
            tempo_bpm: 120.0,
            target_bpm: 120.0,
            bar: 1,
            beat: 1,
            tick_in_beat: 0,
        }
    }
}

impl ClockReading {
    /// Fills what a `BlockContext` carries; the musical-clock fields keep their defaults until
    /// the executor publishes them (the gap is declared in the type's docs, not hidden).
    #[must_use]
    pub fn from_block(block: &sparq_kernel::block::BlockContext) -> Self {
        Self {
            t_sample: block.sample_offset,
            tick: block.tick,
            ppqn: block.ppqn,
            ..Self::default()
        }
    }

    /// The WIT record the `host.now` door returns.
    #[must_use]
    pub fn to_time_info(&self) -> bindings::sparq::instrument::host::TimeInfo {
        bindings::sparq::instrument::host::TimeInfo {
            t_sample: self.t_sample,
            playing: self.playing,
            tick: self.tick,
            ppqn: self.ppqn,
            tempo_bpm: self.tempo_bpm,
            target_bpm: self.target_bpm,
            bar: self.bar,
            beat: self.beat,
            tick_in_beat: self.tick_in_beat,
        }
    }
}

/// The host API version in force — the contract's own package version (`sparq:instrument@1.1.0`),
/// which is what `identity.host_api` negotiates against at load.
#[must_use]
pub const fn host_api_version() -> bindings::sparq::instrument::types::Semver {
    bindings::sparq::instrument::types::Semver { major: 1, minor: 1, patch: 0 }
}

/// The default wasmtime cache directory: `SPARQ_WASM_CACHE` if set, else the user cache dir +
/// `sparq/wasm-cache` (the at-rest store's sibling; env door first, like every house cache).
#[must_use]
pub fn cache_dir() -> PathBuf {
    if let Ok(d) = std::env::var("SPARQ_WASM_CACHE") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    user_cache_dir().join("sparq").join("wasm-cache")
}

/// The user cache dir, resolved from environment only (the at-rest store's rule, mirrored — one
/// shared helper is the better ending and is INC5's note in LATER.md; until then the two agree
/// by test, not by luck).
#[must_use]
pub fn user_cache_dir() -> PathBuf {
    if cfg!(windows) {
        if let Ok(d) = std::env::var("LOCALAPPDATA") {
            if !d.trim().is_empty() {
                return PathBuf::from(d);
            }
        }
    }
    if let Ok(d) = std::env::var("XDG_CACHE_HOME") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".cache")
}

/// Reads a file to bytes, in words on failure (the control-thread door; `read_to_string`-class
/// per the `clippy.toml` fs ban — `File` + `read_to_end`, never the banned conveniences).
///
/// # Errors
/// A sentence naming the path and the OS error.
pub fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(buf)
}
