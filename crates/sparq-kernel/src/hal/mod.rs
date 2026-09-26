//! The hardware abstraction layer (WO-006, ADR-009).
//!
//! One trait, several backends, and a diagnostics contract that makes every claim measurable:
//!
//! ```text
//! enumerate() -> [DeviceInfo]
//! capabilities(DeviceId) -> {rates, channels, exclusive, event_driven, hw_latency, duplex}
//! open(DeviceId, StreamConfig, OpenOptions, process) -> Box<dyn AudioStream>
//! AudioStream::{start, stop, pump?, latency_report, error_report, diag, inject_fault?}
//! ```
//!
//! # Rules this module enforces by shape
//!
//! * **The audio callback sees frames and nothing else.** [`ProcessFn`] receives a [`FrameCtx`]:
//!   a [`BlockContext`] (time, format — all `Copy`) plus interleaved buffers. No clock, no device
//!   handle, no allocator, no `&self` of anything. Plan §5.2's "no syscalls on the audio thread"
//!   starts as a type-level fact here.
//! * **Buffers are owned by the stream, allocated at `open`.** The callback borrows them for the
//!   duration of one block. Nothing on the audio path allocates — and when
//!   [`OpenOptions::audit_allocations`] is set, the pump *measures* that instead of trusting it.
//! * **Every failure is counted, reported and recoverable.** [`StreamErrorReport`] distinguishes
//!   a clean stop from a device removal; [`DiagSnapshot`] carries xruns, the block-time histogram
//!   (p50/p99/max), callback jitter and device-clock drift. A stream that cannot say what went
//!   wrong is not allowed to exist (plan §4.3).
//! * **`stop` then `start` (or a new `open`) must not leak.** Proven by
//!   `conformance::reopen_cycles_do_not_leak`, using the counting allocator.
//!
//! # Aggregator review (task 1: "review against Phase 7 needs before implementing")
//!
//! The Phase 7 device aggregator (plan §6.4: several interfaces acting as one 16/32/64-out device)
//! imposes four requirements, all met without breaking changes:
//!
//! 1. **Per-device addressability** — [`DeviceId`] names one physical endpoint (WASAPI endpoint id
//!    where available), so an aggregator can open N streams against N ids through this same trait.
//! 2. **Per-device latency** — [`LatencyReport`] is per-stream and carries the *reported hardware*
//!    period, which is what latency alignment across devices needs.
//! 3. **A clock to master against** — [`DiagSnapshot::drift_ppm`] exposes each stream's device
//!    clock against wall time; the aggregator's resampling PLL consumes exactly this signal.
//! 4. **Independent lifecycles** — [`AudioStream`] is `Send` and start/stop per stream, so one
//!    device can be restarted (unplug/replug) without touching the others.
//!
//! The aggregator itself will be a type that *implements* `HalBackend` by owning several streams —
//! the trait composes rather than grows.
//!
//! # Backends
//!
//! * [`null::NullBackend`] — the CI workhorse and the offline determinism path (ADR-009: "the null
//!   device is load-bearing"). Two modes: paced (a thread simulates the device interrupt) and
//!   manual (the test drives [`AudioStream::pump`], fully deterministic). Simulates removal and
//!   xruns so the *failure* paths are testable without a soldering iron.
//! * `wasapi::WasapiBackend` — Windows only, `hal-wasapi` feature: WASAPI exclusive (event-driven)
//!   and WASAPI shared, with MMCSS, working-set lock, alignment retry and honest capability probes.
//! * ASIO — WO-006 increment 2 (allowlist entry 2 reserved). Until then, requesting it produces a
//!   [`HalError::Unsupported`] that says so and points at the work order, never a silent fallback.
//!
//! # Relationship to `device::NullDevice`
//!
//! [`crate::device::NullDevice::render`] is the one-shot *offline* renderer the golden tests and
//! `sparq render` use; it stays exactly as it is. This module's null backend is the *live stream*
//! shape (open/start/stop) that the real backends share. They are deliberately separate: an
//! offline render must never grow a thread, and a live stream must never assume it can run to
//! completion in one call.

pub mod conformance;
pub mod diag;
pub mod null;
pub mod period;

#[cfg(all(windows, feature = "hal-wasapi"))]
pub mod wasapi;

use std::fmt;
use std::time::Duration;

use crate::block::BlockContext;
use crate::device::StreamConfig;
use crate::rt::thread::RtReport;
use crate::rt::BlockStatus;

use diag::DiagSnapshot;

// --------------------------------------------------------------------------- backends & devices

/// Which implementation drives a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackendKind {
    /// The deterministic virtual device (CI, offline, failure-path testing).
    Null,
    /// WASAPI exclusive mode, event-driven (the stage path, ADR-004).
    WasapiExclusive,
    /// WASAPI shared mode, event-driven (everything else on Windows).
    WasapiShared,
    /// ASIO (WO-006 increment 2).
    Asio,
}

impl BackendKind {
    /// Stable CLI/log name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::WasapiExclusive => "wasapi-exclusive",
            Self::WasapiShared => "wasapi-shared",
            Self::Asio => "asio",
        }
    }

    /// Parse a CLI/log name. Accepts a few aliases; rejects everything else — a misspelled
    /// backend must never silently become a different one (the lesson of defect #29).
    pub fn from_name(name: &str) -> Result<Self, HalError> {
        match name.to_ascii_lowercase().as_str() {
            "null" | "virtual" | "offline" => Ok(Self::Null),
            "wasapi-exclusive" | "wasapi-x" | "exclusive" => Ok(Self::WasapiExclusive),
            "wasapi-shared" | "wasapi-s" | "shared" => Ok(Self::WasapiShared),
            "asio" => Ok(Self::Asio),
            other => Err(HalError::Unsupported(format!(
                "unknown backend `{other}`. Compiled in: {}; the names are: null, \
                 wasapi-exclusive, wasapi-shared, asio",
                compiled_backends().iter().map(|b| b.as_str()).collect::<Vec<_>>().join(", ")
            ))),
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Backends compiled into this binary.
#[must_use]
pub fn compiled_backends() -> Vec<BackendKind> {
    #[allow(unused_mut)] // the pushes below exist only on Windows with the feature
    let mut v = vec![BackendKind::Null];
    #[cfg(all(windows, feature = "hal-wasapi"))]
    {
        v.push(BackendKind::WasapiExclusive);
        v.push(BackendKind::WasapiShared);
    }
    v
}

/// A device as one backend sees it.
///
/// `endpoint` is the backend's *stable* identity (the WASAPI endpoint id) where one exists;
/// `index` is the enumeration position, stable only until the device list changes. Lookups prefer
/// `endpoint` — a stage rig that re-enumerates after a USB hiccup must resolve to the same device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceId {
    /// The backend that produced this id.
    pub backend: BackendKind,
    /// Enumeration index at the time of discovery.
    pub index: u32,
    /// Stable backend-specific identity, when the backend has one.
    pub endpoint: Option<String>,
}

impl DeviceId {
    /// An id with no stable endpoint (null device, tests).
    #[must_use]
    pub const fn by_index(backend: BackendKind, index: u32) -> Self {
        Self { backend, index, endpoint: None }
    }

    /// The default device of a backend (index 0 by convention; every backend puts its default
    /// first — conformance tests enforce it).
    #[must_use]
    pub const fn default_of(backend: BackendKind) -> Self {
        Self::by_index(backend, 0)
    }
}

/// What a device is, for humans and for routing decisions.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    /// Stable id.
    pub id: DeviceId,
    /// Friendly name as the OS reports it.
    pub name: String,
    /// Can render audio.
    pub has_output: bool,
    /// Can capture audio.
    pub has_input: bool,
    /// Is the OS default render endpoint (for this backend).
    pub is_default_output: bool,
    /// Is the OS default capture endpoint (for this backend).
    pub is_default_input: bool,
}

/// A contiguous range of accepted sample rates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateRange {
    /// Lowest rate in Hz.
    pub min: u32,
    /// Highest rate in Hz.
    pub max: u32,
}

impl RateRange {
    /// Does this range accept `rate`?
    #[must_use]
    pub const fn contains(&self, rate: u32) -> bool {
        rate >= self.min && rate <= self.max
    }
}

/// What a device can do. Probed at enumeration/open time, never guessed.
///
/// Every field is a *measured* claim: `exclusive_rates` comes from actual `IsFormatSupported`
/// probes, `hw_period` from the driver's own `GetDevicePeriod`. A capability we cannot probe is
/// `None`/`false`, not optimistic (the Phase A lesson: telemetry that guesses is worse than none).
#[derive(Clone, Debug)]
pub struct Capabilities {
    /// Accepted rates in shared/mix operation (union of probed ranges).
    pub rates: Vec<RateRange>,
    /// Rates verified for exclusive-mode f32 open (discrete probes). Empty = exclusive unsupported.
    pub exclusive_rates: Vec<u32>,
    /// (min, max) output channels.
    pub channels_out: (u16, u16),
    /// (min, max) input channels.
    pub channels_in: (u16, u16),
    /// The device's own default rate (its mix rate).
    pub default_rate: u32,
    /// The device's own default output channel count.
    pub default_channels_out: u16,
    /// Exclusive-mode open is supported at all.
    pub exclusive: bool,
    /// Event-driven operation is supported (WASAPI: yes; polling-only drivers: false).
    pub event_driven: bool,
    /// Simultaneous input+output on one stream is supported by this backend/device.
    pub full_duplex: bool,
    /// Driver-reported default period (the interrupt cadence we will actually get).
    pub hw_period: Option<Duration>,
    /// Driver-reported minimum period (the floor for exclusive latency).
    pub hw_period_min: Option<Duration>,
    /// Native f32 support without conversion (sparq's internal format).
    pub f32_native: bool,
}

impl Capabilities {
    /// Is `rate` acceptable in shared operation?
    #[must_use]
    pub fn supports_rate(&self, rate: u32) -> bool {
        self.rates.iter().any(|r| r.contains(rate))
    }

    /// Is `rate` verified for exclusive operation?
    #[must_use]
    pub fn supports_exclusive_rate(&self, rate: u32) -> bool {
        self.exclusive_rates.contains(&rate)
    }

    /// Multi-line human readout (the `sparq devices --caps` table).
    #[must_use]
    pub fn summary(&self) -> String {
        let rates =
            self.rates
                .iter()
                .map(|r| {
                    if r.min == r.max {
                        r.min.to_string()
                    } else {
                        format!("{}..{}", r.min, r.max)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
        let excl = if self.exclusive_rates.is_empty() {
            String::from("none")
        } else {
            self.exclusive_rates.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        };
        let period = self
            .hw_period
            .map(|p| format!("{:.3} ms", p.as_secs_f64() * 1e3))
            .unwrap_or_else(|| String::from("?"));
        format!(
            "rates {rates} · exclusive {excl} · out {}..{} ch · in {}..{} ch · default {} Hz/{} ch\n\
             event-driven {} · full-duplex {} · f32 native {} · hw period {period}",
            self.channels_out.0,
            self.channels_out.1,
            self.channels_in.0,
            self.channels_in.1,
            self.default_rate,
            self.default_channels_out,
            self.event_driven,
            self.full_duplex,
            self.f32_native,
        )
    }
}

// --------------------------------------------------------------------------- errors

/// Every way the HAL can fail, with the payload a human needs.
///
/// The variants map to *recovery strategies*, which is why they are this shape and not one string:
/// the CLI can retry a different rung of the ladder on [`HalError::Format`], tell the user to
/// close another app on [`HalError::Busy`], and shut down cleanly (never crash) on
/// [`HalError::Removed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HalError {
    /// The device id does not exist (or stopped existing between enumerate and open).
    NotFound(String),
    /// The backend/device cannot express this request at all (e.g. ASIO before increment 2,
    /// duplex on a render-only backend).
    Unsupported(String),
    /// The device is claimed by something else (exclusive-mode hijack is a real stage failure).
    Busy(String),
    /// Format/rate/channel negotiation failed; the payload says what was tried.
    Format(String),
    /// The driver returned an error; payload includes the code where the backend has one.
    Device(String),
    /// The device went away mid-run (unplug). Streams enter this state and `stop` stays callable.
    Removed(String),
    /// OS-level failure (thread spawn, COM init, event handle).
    System(String),
}

impl HalError {
    /// An actionable hint, where one exists — the CLI prints these verbatim.
    #[must_use]
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Busy(_) => Some(
                "another application holds this device (or Windows did not release it). Close other \
                 audio apps; for exclusive mode also check 'Allow applications to take exclusive \
                 control' in the device's Advanced properties.",
            ),
            Self::Format(_) => Some(
                "try the device default: sparq devices --caps shows what it accepts; on Windows \
                 shared mode the mix rate is king — match it or let the ladder fall back.",
            ),
            Self::Removed(_) => Some(
                "the device was unplugged or its driver reset. The stream is stopped but healthy \
                 to dispose; re-enumerate (sparq devices) and open again.",
            ),
            Self::Unsupported(_) => Some(
                "this backend cannot express that request — read the message; if it names a work \
                 order, the capability is scheduled, not broken.",
            ),
            _ => None,
        }
    }

    /// Format with the hint appended, for one-shot user-facing errors.
    #[must_use]
    pub fn with_hint(&self) -> String {
        match self.hint() {
            Some(h) => format!("{self}\n  hint: {h}"),
            None => self.to_string(),
        }
    }
}

impl fmt::Display for HalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(m) => write!(f, "device not found: {m}"),
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
            Self::Busy(m) => write!(f, "device busy: {m}"),
            Self::Format(m) => write!(f, "format negotiation failed: {m}"),
            Self::Device(m) => write!(f, "device error: {m}"),
            Self::Removed(m) => write!(f, "device removed: {m}"),
            Self::System(m) => write!(f, "system error: {m}"),
        }
    }
}

impl std::error::Error for HalError {}

// --------------------------------------------------------------------------- the audio callback

/// What the audio callback receives — frames and time, and nothing else (see module docs).
///
/// `out` is `frames * channels` interleaved f32 and must be fully written. `input` is
/// `frames * inputs` interleaved f32, or empty when the stream is output-only.
pub struct FrameCtx<'a> {
    /// Block identity and timing (`Copy`; advance is the pump's job, never the callback's).
    pub block: BlockContext,
    /// Interleaved output buffer to fill.
    pub out: &'a mut [f32],
    /// Interleaved input buffer to read (empty slice when there are no inputs).
    pub input: &'a [f32],
}

impl FrameCtx<'_> {
    /// Fill `out` with silence — the fallback every callback owes the device.
    pub fn zero_out(&mut self) {
        for s in self.out.iter_mut() {
            *s = 0.0;
        }
    }
}

/// The audio callback. Returns what happened to the block; anything but `Ok` is counted and
/// surfaced (never swallowed). Must not allocate, lock, block or touch the wall clock — the pump
/// *measures* all four when `OpenOptions::audit_allocations` is set.
pub type ProcessFn = Box<dyn FnMut(&mut FrameCtx<'_>) -> BlockStatus + Send + 'static>;

/// A fault the null backend can be told to simulate (WO-006: "device removal produces a clean
/// error state and a recoverable stop, not a crash" — tested, not hoped for).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The device vanishes: the stream enters [`StreamState::Removed`], the pump stops, and
    /// `stop()` still works.
    Unplug,
    /// The next block counts as a device xrun (audio continues).
    XrunNext,
    /// The pump itself stalls for this long before the next block (models a system hiccup; the
    /// stall is on the harness side, like `RenderConfig::stall` — the callback cannot do this
    /// without tripping the budget overrun counter, which is the point).
    PumpStallNext {
        /// Stall duration in milliseconds.
        millis: u64,
    },
}

// --------------------------------------------------------------------------- open options

/// Non-format choices at `open` time. Everything here is decided *before* any buffer exists,
/// because after `start` nothing may allocate (plan §5.2).
#[derive(Clone, Debug)]
pub struct OpenOptions {
    /// Wrap every callback in the counting-allocator guard and publish violations in
    /// [`DiagSnapshot::allocations`]. Debug/CI default: on. Release default: on as well — the
    /// counter is two relaxed atomics and the claim "0 allocations" is only true if something
    /// counts (main.rs installs the counting allocator for the whole binary).
    pub audit_allocations: bool,
    /// Null backend: `true` runs a paced pump thread that simulates the device interrupt; `false`
    /// leaves the stream in manual mode where [`AudioStream::pump`] produces blocks on demand
    /// (deterministic; the CI workhorse). Device backends ignore this — a real device is always
    /// paced by hardware.
    pub paced: bool,
    /// Null backend: keep up to this many output frames for inspection after `stop`
    /// (0 = discard; tests use it to prove the callback ran and to check determinism).
    pub capture_frames: usize,
    /// Ideal processor index for the pump thread (`None` = scheduler's choice). See
    /// [`crate::rt::thread`].
    pub ideal_processor: Option<usize>,
    /// Raise the process working-set minimum by (roughly) the stream's buffer size and pre-touch
    /// the buffers, so the callback cannot page-fault. Windows-only in effect; reported honestly
    /// elsewhere.
    pub working_set_lock: bool,
    /// Whether `open` may print its negotiation log (ladder rungs, quirks). True for the stream
    /// the user asked for; **false for probe opens** — the two-phase open (probe → real) would
    /// otherwise print every line twice, and the conformance reopen cycles print it per cycle,
    /// burying the report in identical noise.
    pub log_negotiation: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            audit_allocations: true,
            paced: true,
            capture_frames: 0,
            ideal_processor: None,
            working_set_lock: true,
            log_negotiation: true,
        }
    }
}

impl OpenOptions {
    /// Deterministic CI/test options: manual stepping, capture on, auditing on.
    #[must_use]
    pub fn manual_with_capture(frames: usize) -> Self {
        Self { paced: false, capture_frames: frames, ..Self::default() }
    }

    /// Probe-open options: silent, cheap, no RT setup — the open exists only to read
    /// `actual_config` before the real stream is built.
    #[must_use]
    pub fn probe() -> Self {
        Self {
            audit_allocations: false,
            paced: false,
            capture_frames: 0,
            ideal_processor: None,
            working_set_lock: false,
            log_negotiation: false,
        }
    }
}

// --------------------------------------------------------------------------- latency & state

/// The latency contract, per stream, as numbers (WO-006: "round-trip latency measured and
/// displayed"). Pipeline figures are what sparq can compute honestly from the negotiated format;
/// `measured_roundtrip_frames` is filled only by an actual measurement utility (the loopback
/// impulse, increment 2 — until then it stays `None` rather than pretending).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatencyReport {
    /// Negotiated sample rate.
    pub sample_rate: u32,
    /// Negotiated block size.
    pub block_frames: u32,
    /// Driver-reported hardware period, if the driver reports one.
    pub hw_period_frames: Option<u32>,
    /// callback-return → sound-at-the-converter estimate: one block of pipeline plus the hardware
    /// period (documented formula, not a measurement).
    pub output_pipeline_frames: u32,
    /// converter → callback-entry estimate for inputs (0 for output-only streams).
    pub input_pipeline_frames: u32,
    /// Filled by the round-trip measurement utility once it exists (WO-006 increment 2).
    pub measured_roundtrip_frames: Option<u32>,
}

impl LatencyReport {
    /// Frames → milliseconds at this report's rate.
    #[must_use]
    pub fn frames_to_ms(&self, frames: u32) -> f64 {
        f64::from(frames) * 1000.0 / f64::from(self.sample_rate.max(1))
    }

    /// Estimated output pipeline latency in ms.
    #[must_use]
    pub fn output_ms(&self) -> f64 {
        self.frames_to_ms(self.output_pipeline_frames)
    }

    /// Estimated round trip (in + out pipelines) in ms — the number performers care about.
    #[must_use]
    pub fn roundtrip_ms(&self) -> f64 {
        self.frames_to_ms(self.input_pipeline_frames + self.output_pipeline_frames)
    }

    /// One-line readout.
    #[must_use]
    pub fn summary(&self) -> String {
        let hw = self
            .hw_period_frames
            .map(|p| format!("{:.2} ms", self.frames_to_ms(p)))
            .unwrap_or_else(|| String::from("?"));
        format!(
            "out {:.2} ms · in {:.2} ms · roundtrip {:.2} ms (est; hw period {hw}, block {:.2} ms)",
            self.output_ms(),
            self.input_ms(),
            self.roundtrip_ms(),
            self.frames_to_ms(self.block_frames),
        )
    }

    /// Estimated input pipeline latency in ms.
    #[must_use]
    pub fn input_ms(&self) -> f64 {
        self.frames_to_ms(self.input_pipeline_frames)
    }
}

/// Lifecycle of a stream. `Removed` and `Failed` are *states*, not panics: the stream object stays
/// valid, `stop` stays callable, and the diagnostics stay readable (WO-006 acceptance).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamState {
    /// Opened, never started.
    Idle,
    /// `start` called; the pump thread is negotiating with the driver (WASAPI creates all COM
    /// objects on its own thread — apartment discipline). Transient: `start` returns once the
    /// stream is `Running`, `Failed` or `Removed`.
    Starting,
    /// Pumping blocks.
    Running,
    /// Stopped by the user; may be started again.
    Stopped,
    /// The device vanished; not restartable, but safely disposable.
    Removed,
    /// The driver failed in a way that this stream cannot recover from; open a new one.
    Failed,
}

/// What a stream says when asked "are you okay?".
#[derive(Clone, Debug)]
pub struct StreamErrorReport {
    /// Current lifecycle state.
    pub state: StreamState,
    /// Human text for the last device-level failure, if any.
    pub last_error: Option<String>,
    /// Count of device-level errors seen (a stream that recovers still reports its history).
    pub device_errors: u64,
    /// RT discipline actually applied to the pump thread (see [`crate::rt::thread`]).
    pub rt: RtReport,
}

impl StreamErrorReport {
    /// Is the stream usable right now?
    #[must_use]
    pub const fn healthy(&self) -> bool {
        !matches!(self.state, StreamState::Removed | StreamState::Failed)
    }
}

/// [`StreamState`] → atomic transport encoding, shared by every pump implementation.
pub(crate) const fn state_to_u8(s: StreamState) -> u8 {
    match s {
        StreamState::Idle => 0,
        StreamState::Running => 1,
        StreamState::Stopped => 2,
        StreamState::Removed => 3,
        StreamState::Failed => 4,
        StreamState::Starting => 5,
    }
}

/// Atomic transport encoding → [`StreamState`]. Unknown values decode to `Failed`: a corrupt
/// state must look broken, never healthy.
pub(crate) fn state_from_u8(v: u8) -> StreamState {
    match v {
        0 => StreamState::Idle,
        1 => StreamState::Running,
        2 => StreamState::Stopped,
        3 => StreamState::Removed,
        5 => StreamState::Starting,
        _ => StreamState::Failed,
    }
}

// --------------------------------------------------------------------------- the traits

/// An opened audio stream. Backends own their pump thread (or manual stepper); the callback was
/// handed over at `open` and runs on the pump.
///
/// Object-safe and `Send` so streams can live in the app's session struct and move to threads.
pub trait AudioStream: Send {
    /// Begin pumping blocks. Idempotent-ish: starting a running stream is an error, not a no-op
    /// (a silent double-start hides lifecycle bugs).
    fn start(&mut self) -> Result<(), HalError>;

    /// Stop pumping and join the pump thread. Callable from every state — including `Removed` —
    /// and always leaves the stream disposable. Blocking here is fine: this is the control thread.
    fn stop(&mut self) -> Result<(), HalError>;

    /// Is the pump running?
    #[must_use]
    fn is_running(&self) -> bool;

    /// The config that was *requested*.
    #[must_use]
    fn config(&self) -> StreamConfig;

    /// The config that was *negotiated*. Device backends may land on the device's mix rate or a
    /// period-aligned block; pretending otherwise is how latency lies get born (Phase A lesson).
    #[must_use]
    fn actual_config(&self) -> StreamConfig;

    /// The latency contract for the negotiated config.
    #[must_use]
    fn latency_report(&self) -> LatencyReport;

    /// Lifecycle + last error + RT discipline report.
    #[must_use]
    fn error_report(&self) -> StreamErrorReport;

    /// Diagnostics snapshot: xruns, block histogram (p50/p99/max), jitter, drift, allocations.
    #[must_use]
    fn diag(&self) -> DiagSnapshot;

    /// Manual mode only (null backend, `OpenOptions::paced == false`): produce exactly `blocks`
    /// blocks through the callback, synchronously. Device backends return
    /// [`HalError::Unsupported`] — hardware paces itself.
    fn pump(&mut self, blocks: u64) -> Result<u64, HalError>;

    /// Ask the backend to simulate a fault (null backend). Device backends return
    /// [`HalError::Unsupported`]: real faults are tested by really unplugging things, per the
    /// WO-006 evidence list.
    fn inject_fault(&mut self, fault: Fault) -> Result<(), HalError>;

    /// Captured output frames (null backend with `capture_frames > 0`, after `stop` or in manual
    /// mode). `None` when capture is off or unsupported.
    #[must_use]
    fn captured(&self) -> Option<&[f32]> {
        None
    }
}

/// A backend: enumeration, capabilities, and opening streams.
pub trait HalBackend: Send + Sync {
    /// Which backend this is.
    #[must_use]
    fn kind(&self) -> BackendKind;

    /// Human name for logs (`WASAPI exclusive`, ...).
    #[must_use]
    fn display_name(&self) -> &'static str;

    /// Every active device this backend can see. The OS default output device is **first**
    /// (conformance-tested), so `DeviceId::default_of` is meaningful without a second lookup.
    fn enumerate(&self) -> Result<Vec<DeviceInfo>, HalError>;

    /// Probe what a device can do. Cheap enough to call per device at startup; the results are
    /// what `sparq devices --caps` prints and what the open ladder consults.
    fn capabilities(&self, dev: &DeviceId) -> Result<Capabilities, HalError>;

    /// Negotiate and open (but do not start) a stream. All buffers exist when this returns —
    /// `start` may not allocate (plan §5.2).
    fn open(
        &self,
        dev: &DeviceId,
        cfg: &StreamConfig,
        opts: &OpenOptions,
        process: ProcessFn,
    ) -> Result<Box<dyn AudioStream>, HalError>;
}

/// Every backend compiled into this binary, default device first per backend.
#[must_use]
pub fn available_backends() -> Vec<Box<dyn HalBackend>> {
    #[allow(unused_mut)] // the pushes below exist only on Windows with the feature
    let mut v: Vec<Box<dyn HalBackend>> = vec![Box::new(null::NullBackend::new())];
    #[cfg(all(windows, feature = "hal-wasapi"))]
    {
        v.push(Box::new(wasapi::WasapiBackend::exclusive()));
        v.push(Box::new(wasapi::WasapiBackend::shared()));
    }
    v
}

/// Look a backend up by kind, with an honest error for backends that are not compiled in.
pub fn backend_by_kind(kind: BackendKind) -> Result<Box<dyn HalBackend>, HalError> {
    available_backends().into_iter().find(|b| b.kind() == kind).ok_or_else(|| match kind {
        BackendKind::Asio => HalError::Unsupported(String::from(
            "the ASIO backend is WO-006 increment 2 (allowlist entry 2 reserved); until then \
                 WASAPI exclusive is the low-latency path on Windows",
        )),
        k => HalError::Unsupported(format!(
            "backend `{k}` is not compiled into this binary (features: hal-wasapi; \
                 platform: {})",
            std::env::consts::OS
        )),
    })
}

/// Look a backend up by CLI name.
pub fn backend_by_name(name: &str) -> Result<Box<dyn HalBackend>, HalError> {
    backend_by_kind(BackendKind::from_name(name)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    #[test]
    fn backend_names_roundtrip() {
        for k in [
            BackendKind::Null,
            BackendKind::WasapiExclusive,
            BackendKind::WasapiShared,
            BackendKind::Asio,
        ] {
            assert_eq!(BackendKind::from_name(k.as_str()), Ok(k), "{} must roundtrip", k.as_str());
        }
        // aliases
        assert_eq!(BackendKind::from_name("WASAPI-X"), Ok(BackendKind::WasapiExclusive));
        assert_eq!(BackendKind::from_name("shared"), Ok(BackendKind::WasapiShared));
        // a misspelling must never become another backend (defect #29's lesson, applied to names)
        assert!(BackendKind::from_name("wasap").is_err());
        assert!(BackendKind::from_name("").is_err());
    }

    #[test]
    fn null_backend_is_always_compiled_in() {
        assert!(compiled_backends().contains(&BackendKind::Null));
        assert!(backend_by_kind(BackendKind::Null).is_ok());
    }

    #[test]
    fn asio_request_is_an_honest_refusal_not_a_fallback() {
        // Increment 1 has no ASIO. The refusal must name the work order, and must NOT silently
        // hand back another backend — a fallback here would play a "works on my machine" trick
        // on exactly the machine that matters.
        match backend_by_kind(BackendKind::Asio) {
            Err(HalError::Unsupported(m)) => {
                assert!(m.contains("WO-006"), "refusal must name the work order: {m}");
                assert!(m.contains("increment 2"));
            },
            // Box<dyn HalBackend> has no Debug: match the arms instead of printing the Result.
            Err(e) => panic!("expected an honest refusal, got Err({e})"),
            Ok(b) => panic!("expected an honest refusal, got backend `{}`", b.display_name()),
        }
    }

    #[test]
    fn rate_range_contains_is_inclusive() {
        let r = RateRange { min: 44_100, max: 48_000 };
        assert!(r.contains(44_100) && r.contains(48_000) && r.contains(46_000));
        assert!(!r.contains(44_099) && !r.contains(48_001));
    }

    #[test]
    fn latency_report_math_is_exact_at_48k() {
        let r = LatencyReport {
            sample_rate: 48_000,
            block_frames: 96,
            hw_period_frames: Some(96),
            output_pipeline_frames: 192,
            input_pipeline_frames: 96,
            measured_roundtrip_frames: None,
        };
        assert!((r.output_ms() - 4.0).abs() < 1e-9, "192 frames at 48 kHz is exactly 4 ms");
        assert!((r.roundtrip_ms() - 6.0).abs() < 1e-9);
        assert!(r.summary().contains("est"), "the summary must not present estimates as measured");
    }

    #[test]
    fn error_hints_only_where_actionable() {
        assert!(HalError::Busy(String::new()).hint().is_some());
        assert!(HalError::Removed(String::new()).hint().is_some());
        assert!(HalError::System(String::new()).hint().is_none());
        // with_hint never loses the message
        let e = HalError::Format(String::from("48000/8ch refused"));
        assert!(e.with_hint().contains("48000/8ch refused"));
    }
}
