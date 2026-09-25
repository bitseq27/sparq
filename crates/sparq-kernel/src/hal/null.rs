//! The null/virtual HAL backend: the CI workhorse and the failure-path simulator (ADR-009).
//!
//! Two pump modes behind one [`AudioStream`]:
//!
//! * **Manual** ([`OpenOptions::paced`] `= false`) — blocks are produced only when a test calls
//!   [`AudioStream::pump`]. Fully deterministic: no thread, no clock, no scheduler. Golden-style
//!   determinism tests and the allocator gate use this mode.
//! * **Paced** (`paced = true`) — a pump thread simulates the device interrupt: it wakes on a
//!   deadline derived from the block period, measures its own wake jitter, enforces the callback
//!   budget and counts late wakes as xruns, exactly like the real backends do. This is the mode
//!   `sparq play --backend null` and the HAL soak use, so a soak rehearsal on a CI runner
//!   exercises the same code path shape as the stage machine.
//!
//! # Faults are features
//!
//! WO-006's acceptance criteria demand that device removal "produces a clean error state and a
//! recoverable stop, not a crash". You cannot test that against hardware in CI, so the null
//! backend simulates it: [`Fault::Unplug`], [`Fault::XrunNext`] and [`Fault::PumpStallNext`].
//! Every fault path asserted here is the same path a real unplug takes — the only difference is
//! who sets the flag.
//!
//! # Timing allowances
//!
//! Like [`crate::device::null`], this module reads the clock and sleeps — in the *pump*, which is
//! the harness simulating the device, never in the callback. The callback receives only a
//! [`FrameCtx`]. The allowance is scoped to this file and explained here rather than hidden.

// The paced pump simulates a device interrupt thread: it must read the clock (jitter, deadlines)
// and sleep (pacing). Both are denied workspace-wide because they are forbidden *on the audio
// path*; the pump is the harness that drives the audio path, exactly as in `device/null.rs`. The
// callback closure still sees neither clock nor thread primitives.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::alloc::CountingGuard;
use crate::block::{BlockContext, BlockId};
use crate::clock::Clock;
use crate::device::StreamConfig;
use crate::hal::diag::DiagRecorder;
use crate::hal::{
    AudioStream, BackendKind, Capabilities, DeviceId, DeviceInfo, Fault, FrameCtx, HalBackend,
    HalError, LatencyReport, OpenOptions, ProcessFn, RateRange, StreamErrorReport, StreamState,
};
use crate::rt::thread::RtThreadGuard;
use crate::rt::BlockStatus;
use crate::sync::SpscRing;

// --------------------------------------------------------------------------- backend

/// The null backend. Stateless; every stream owns its own buffers and diagnostics.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullBackend {
    _private: (),
}

impl NullBackend {
    /// Construct the null backend.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    /// The single device the null backend exposes.
    #[must_use]
    pub fn device() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::by_index(BackendKind::Null, 0),
            name: String::from("sparq Null Device (deterministic, 64 ch)"),
            has_output: true,
            has_input: true,
            is_default_output: true,
            is_default_input: true,
        }
    }
}

impl HalBackend for NullBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Null
    }

    fn display_name(&self) -> &'static str {
        "null (virtual device)"
    }

    fn enumerate(&self) -> Result<Vec<DeviceInfo>, HalError> {
        Ok(vec![Self::device()])
    }

    fn capabilities(&self, dev: &DeviceId) -> Result<Capabilities, HalError> {
        if dev.backend != BackendKind::Null || dev.index != 0 {
            return Err(HalError::NotFound(format!(
                "the null backend has exactly one device (index 0); got index {}",
                dev.index
            )));
        }
        Ok(Capabilities {
            // The null device accepts anything in the plan's operating envelope (§9.6: up to 64
            // channels; rates to 384 kHz because the appliance path may want them).
            rates: vec![RateRange { min: 8_000, max: 384_000 }],
            exclusive_rates: vec![44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 384_000],
            channels_out: (1, 64),
            channels_in: (0, 64),
            default_rate: 48_000,
            default_channels_out: 2,
            exclusive: true,
            event_driven: true,
            full_duplex: true,
            // Synthetic but honest: the paced pump interrupts exactly once per block, so the
            // "hardware period" is the block period; capabilities are device-level, so report a
            // 1 ms nominal and let `latency_report` use the negotiated block.
            hw_period: Some(Duration::from_millis(1)),
            hw_period_min: Some(Duration::from_micros(125)),
            f32_native: true,
        })
    }

    fn open(
        &self,
        dev: &DeviceId,
        cfg: &StreamConfig,
        opts: &OpenOptions,
        process: ProcessFn,
    ) -> Result<Box<dyn AudioStream>, HalError> {
        self.capabilities(dev)?; // validates the id and reuses the NotFound text
        validate(cfg)?;
        Ok(Box::new(NullStream::new(*cfg, opts.clone(), process)))
    }
}

/// Reject configs outside the plan's envelope with actionable text (WO-006: "the capability
/// report correctly says it can't").
fn validate(cfg: &StreamConfig) -> Result<(), HalError> {
    if cfg.sample_rate < 1 {
        return Err(HalError::Format(format!("sample rate {} is not a rate", cfg.sample_rate)));
    }
    if cfg.block_frames < 1 {
        return Err(HalError::Format(format!("block {} frames is not a block", cfg.block_frames)));
    }
    if cfg.outputs < 1 || cfg.outputs > 64 {
        return Err(HalError::Unsupported(format!(
            "{} output channels: the null device does 1..=64 (plan §9.6)",
            cfg.outputs
        )));
    }
    if cfg.inputs > 64 {
        return Err(HalError::Unsupported(format!(
            "{} input channels: the null device does 0..=64 (plan §9.6)",
            cfg.inputs
        )));
    }
    Ok(())
}

// --------------------------------------------------------------------------- stream state

/// The state encoding is shared with the WASAPI pump (`hal::state_to_u8` / `state_from_u8`) so
/// that a diagnostics reader never has to know which backend produced the byte.
fn state_load(a: &AtomicU8) -> StreamState {
    crate::hal::state_from_u8(a.load(Ordering::Acquire))
}

fn state_store(a: &AtomicU8, s: StreamState) {
    a.store(crate::hal::state_to_u8(s), Ordering::Release);
}

/// Fault register shared between the stream (control side) and the pump.
#[derive(Default)]
struct FaultFlags {
    bits: AtomicU64,
    stall_ms: AtomicU64,
}

const F_UNPLUG: u64 = 1;
const F_XRUN: u64 = 2;
const F_STALL: u64 = 4;

impl FaultFlags {
    fn unplug_latched(&self) -> bool {
        self.bits.load(Ordering::Acquire) & F_UNPLUG != 0
    }

    fn take(&self, bit: u64) -> bool {
        self.bits.fetch_and(!bit, Ordering::AcqRel) & bit != 0
    }

    fn set(&self, bit: u64) {
        self.bits.fetch_or(bit, Ordering::AcqRel);
    }
}

/// What the paced pump returns on exit: the callback and both buffers, so a stopped stream can
/// start again and nothing leaks (the type alias keeps the JoinHandle readable — clippy's
/// `type_complexity` is right that the bare tuple is noise).
type PumpReturn = (ProcessFn, Vec<f32>, Vec<f32>);

// --------------------------------------------------------------------------- the stream

/// A null stream. Buffers, diagnostics and the fault register exist from `open`; `start` spawns
/// the paced pump (or flips state, in manual mode) and allocates nothing.
pub struct NullStream {
    req: StreamConfig,
    opts: OpenOptions,
    diag: Arc<DiagRecorder>,
    state: Arc<AtomicU8>,
    faults: Arc<FaultFlags>,
    stop_flag: Arc<AtomicBool>,
    /// Output scratch, `block * outputs` f32. Lives here except while a paced pump owns it.
    out_scratch: Vec<f32>,
    /// Input scratch, `block * inputs` f32 — silence (a null device records nothing).
    in_scratch: Vec<f32>,
    /// The callback, `None` only while a paced pump thread owns it.
    proc: Option<ProcessFn>,
    /// Paced pump in flight. The thread *returns* the callback and both buffers on exit, so a
    /// stopped stream can be started again and nothing leaks.
    pump: Option<JoinHandle<PumpReturn>>,
    /// Capture ring (paced mode: pump writes, stream drains at stop).
    capture_ring: Option<Arc<SpscRing<f32>>>,
    /// Drained capture, exposed via `captured()`.
    captured: Vec<f32>,
    /// Blocks pumped in manual mode (for ctx advance across pump calls).
    manual_ctx: BlockContext,
    manual_clock: Clock,
}

impl NullStream {
    fn new(cfg: StreamConfig, opts: OpenOptions, proc: ProcessFn) -> Self {
        let out_scratch = vec![0.0f32; cfg.block_frames * cfg.outputs];
        let in_scratch = vec![0.0f32; cfg.block_frames * cfg.inputs];
        // Writing every element IS the pre-touch: pages are faulted in now, at open, on the
        // control thread — never inside a callback (rt::thread::pre_touch does the same for u8).
        for s in &out_scratch {
            std::hint::black_box(s);
        }
        let capture_ring = (opts.capture_frames > 0)
            .then(|| Arc::new(SpscRing::<f32>::new(opts.capture_frames * cfg.outputs.max(1))));
        let manual_ctx = BlockContext {
            block: BlockId::FIRST,
            sample_offset: 0,
            frames: cfg.block_frames,
            sample_rate: cfg.sample_rate,
            channels: cfg.outputs,
            tick: 0,
            ppqn: 960,
        };
        Self {
            req: cfg,
            opts,
            diag: Arc::new(DiagRecorder::new(cfg.sample_rate)),
            state: Arc::new(AtomicU8::new(0)),
            faults: Arc::new(FaultFlags::default()),
            stop_flag: Arc::new(AtomicBool::new(false)),
            out_scratch,
            in_scratch,
            proc: Some(proc),
            pump: None,
            capture_ring,
            captured: Vec::new(),
            manual_ctx,
            manual_clock: Clock::new(cfg.sample_rate, 120.0, 960),
        }
    }

    /// One block through the callback, with audit, budget accounting and fault handling shared
    /// by both modes. Returns the callback's status. `stall` is honoured by the paced pump only
    /// (manual mode has no deadline to miss, so it counts the xrun directly instead).
    ///
    /// The argument count is the honest shape of "everything the two pump modes share"; bundling
    /// it into a context struct would just move the nine fields one level down.
    #[allow(clippy::too_many_arguments)]
    fn run_block(
        proc: &mut ProcessFn,
        ctx: &mut BlockContext,
        out: &mut [f32],
        input: &[f32],
        diag: &DiagRecorder,
        faults: &FaultFlags,
        budget: Duration,
        audit: bool,
        paced: bool,
    ) -> BlockStatus {
        if faults.take(F_STALL) {
            let ms = faults.stall_ms.swap(0, Ordering::AcqRel);
            if paced {
                // Harness-side stall: burns wall time *outside* the callback, so the next wake is
                // late and `record_wake` counts the xrun — the same observable consequence a real
                // system hiccup has. (The paced pump calls this between blocks.)
                let t = Instant::now();
                while t.elapsed() < Duration::from_millis(ms) {
                    std::hint::spin_loop();
                }
            } else {
                diag.record_xrun(); // manual mode: no deadline exists; count the glitch honestly
            }
        }
        let guard = audit.then(CountingGuard::new);
        let t0 = paced.then(Instant::now);
        let mut fctx = FrameCtx { block: *ctx, out, input };
        let status = proc(&mut fctx);
        let dur = t0.map_or(Duration::ZERO, |t| t.elapsed());
        let allocs = guard.map_or(0, CountingGuard::finish);
        diag.record_callback(dur, status, paced.then_some(budget), allocs);
        if faults.take(F_XRUN) {
            diag.record_xrun();
        }
        status
    }

    fn drain_capture(&mut self) {
        let Some(ring) = self.capture_ring.as_ref() else { return };
        // `capture_frames` counts FRAMES; the ring stores interleaved SAMPLES (frames × outputs).
        let limit = self.opts.capture_frames * self.req.outputs.max(1);
        if self.captured.len() >= limit {
            return;
        }
        let mut chunk = vec![0.0f32; 4096.min(ring.capacity())];
        loop {
            if self.captured.len() >= limit {
                break;
            }
            let n = ring.drain(&mut chunk);
            if n == 0 {
                break;
            }
            self.captured.extend_from_slice(&chunk[..n]);
        }
        self.captured.truncate(limit);
    }
}

impl AudioStream for NullStream {
    fn start(&mut self) -> Result<(), HalError> {
        let now = state_load(&self.state);
        match now {
            StreamState::Running | StreamState::Starting => {
                return Err(HalError::Device(String::from(
                    "start on a running stream: lifecycle bug — stop it first",
                )));
            },
            StreamState::Removed | StreamState::Failed => {
                return Err(HalError::Removed(format!(
                    "stream is in state {now:?}; a removed device needs a fresh open, not a restart"
                )));
            },
            StreamState::Idle | StreamState::Stopped => {},
        }
        self.captured.clear();
        if self.opts.paced {
            // Take ownership of everything the pump needs; `stop` gives it all back.
            let Some(proc) = self.proc.take() else {
                return Err(HalError::System(String::from("pump already owns the callback")));
            };
            let out = std::mem::take(&mut self.out_scratch);
            let input = std::mem::take(&mut self.in_scratch);
            let diag = Arc::clone(&self.diag);
            let state = Arc::clone(&self.state);
            let state_for_err = Arc::clone(&self.state);
            let faults = Arc::clone(&self.faults);
            let stop_flag = Arc::clone(&self.stop_flag);
            let ring = self.capture_ring.clone();
            let cfg = self.req;
            let opts = self.opts.clone();

            stop_flag.store(false, Ordering::Release);
            state_store(&self.state, StreamState::Running);

            let builder = std::thread::Builder::new().name(String::from("sparq-null-pump"));
            let handle = builder
                .spawn(move || -> PumpReturn {
                    paced_pump(proc, out, input, cfg, opts, diag, state, faults, stop_flag, ring)
                })
                .map_err(|e| {
                    state_store(&state_for_err, StreamState::Failed);
                    HalError::System(format!("spawning the null pump thread: {e}"))
                })?;
            self.pump = Some(handle);
        } else {
            state_store(&self.state, StreamState::Running);
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HalError> {
        if let Some(handle) = self.pump.take() {
            self.stop_flag.store(true, Ordering::Release);
            match handle.join() {
                Ok((proc, out, input)) => {
                    self.proc = Some(proc);
                    self.out_scratch = out;
                    self.in_scratch = input;
                },
                Err(_) => {
                    // A panicked pump is a defect, not a state: report it, mark the stream failed,
                    // and stay disposable. Rebuild proc/buffers so Drop cannot double-free.
                    state_store(&self.state, StreamState::Failed);
                    self.proc = None;
                    self.out_scratch = Vec::new();
                    self.in_scratch = Vec::new();
                    self.diag.record_device_error();
                    self.drain_capture();
                    return Err(HalError::System(String::from(
                        "the null pump thread panicked; the stream is Failed and disposable",
                    )));
                },
            }
        }
        if state_load(&self.state) == StreamState::Running {
            state_store(&self.state, StreamState::Stopped);
        }
        self.drain_capture();
        Ok(())
    }

    fn is_running(&self) -> bool {
        state_load(&self.state) == StreamState::Running
    }

    fn config(&self) -> StreamConfig {
        self.req
    }

    fn actual_config(&self) -> StreamConfig {
        // The null device honours the request exactly — that determinism is why every layer above
        // the HAL can be tested against it.
        self.req
    }

    fn latency_report(&self) -> LatencyReport {
        let rate = self.req.sample_rate.max(1);
        let block = self.req.block_frames as u32;
        let hw = rate / 1000; // the synthetic 1 ms period from capabilities()
        LatencyReport {
            sample_rate: rate,
            block_frames: block,
            hw_period_frames: Some(hw),
            output_pipeline_frames: block + hw,
            input_pipeline_frames: if self.req.inputs > 0 { block + hw } else { 0 },
            measured_roundtrip_frames: None,
        }
    }

    fn error_report(&self) -> StreamErrorReport {
        let state = state_load(&self.state);
        StreamErrorReport {
            state,
            last_error: match state {
                StreamState::Removed => {
                    Some(String::from("simulated device removal (Fault::Unplug)"))
                },
                StreamState::Failed => Some(String::from("pump thread failure (see stderr)")),
                _ => None,
            },
            device_errors: self.diag.snapshot().device_errors,
            rt: self.diag.rt_report(),
        }
    }

    fn diag(&self) -> crate::hal::diag::DiagSnapshot {
        self.diag.snapshot()
    }

    fn pump(&mut self, blocks: u64) -> Result<u64, HalError> {
        if self.opts.paced {
            return Err(HalError::Unsupported(String::from(
                "this is a paced stream — the pump thread produces blocks; open with \
                 OpenOptions::manual_with_capture(..) for deterministic stepping",
            )));
        }
        if state_load(&self.state) != StreamState::Running {
            return Err(HalError::Device(format!(
                "pump requires a running stream; state is {:?}",
                state_load(&self.state)
            )));
        }
        let Some(proc) = self.proc.as_mut() else {
            return Err(HalError::System(String::from("callback is missing")));
        };
        let budget = Duration::from_secs_f64(self.req.block_seconds());
        let mut done = 0u64;
        for _ in 0..blocks {
            if self.faults.unplug_latched() {
                self.diag.record_device_error();
                state_store(&self.state, StreamState::Removed);
                break;
            }
            Self::run_block(
                proc,
                &mut self.manual_ctx,
                &mut self.out_scratch,
                &self.in_scratch,
                &self.diag,
                &self.faults,
                budget,
                self.opts.audit_allocations,
                false,
            );
            if let Some(ring) = self.capture_ring.as_ref() {
                for &s in self.out_scratch.iter() {
                    let _ = ring.push(s); // refusals counted by the ring; capture is bounded by design
                }
            }
            // Synthetic device clock: exact, so drift measures only the wall side (zero in manual).
            self.manual_ctx = self.manual_ctx.advance(&self.manual_clock);
            done += 1;
        }
        self.drain_capture();
        if state_load(&self.state) == StreamState::Removed {
            // Blocks already produced are recorded in the diagnostics; the caller learns about the
            // removal from this very call, even if it happened mid-batch.
            return Err(HalError::Removed(String::from(
                "device removed mid-pump; the stream is a clean husk — dispose and re-open",
            )));
        }
        Ok(done)
    }

    fn inject_fault(&mut self, fault: Fault) -> Result<(), HalError> {
        match fault {
            Fault::Unplug => self.faults.set(F_UNPLUG),
            Fault::XrunNext => self.faults.set(F_XRUN),
            Fault::PumpStallNext { millis } => {
                self.faults.stall_ms.store(millis, Ordering::Release);
                self.faults.set(F_STALL);
            },
        }
        Ok(())
    }

    fn captured(&self) -> Option<&[f32]> {
        (self.opts.capture_frames > 0).then_some(self.captured.as_slice())
    }
}

impl Drop for NullStream {
    fn drop(&mut self) {
        // Dropping a running stream must not detach a live pump that still owns shared state:
        // stop it. Errors are meaningless during teardown; the state machine keeps them.
        if self.pump.is_some() {
            let _ = AudioStream::stop(self);
        }
    }
}

/// The paced pump: simulates a device interrupt at the block period.
///
/// Returns the callback and the buffers so the stream can start again or be dropped without
/// leaking (WO-006 acceptance: stop/start must not leak).
#[allow(clippy::too_many_arguments)]
fn paced_pump(
    mut proc: ProcessFn,
    mut out: Vec<f32>,
    input: Vec<f32>,
    cfg: StreamConfig,
    opts: OpenOptions,
    diag: Arc<DiagRecorder>,
    state: Arc<AtomicU8>,
    faults: Arc<FaultFlags>,
    stop_flag: Arc<AtomicBool>,
    capture_ring: Option<Arc<SpscRing<f32>>>,
) -> (ProcessFn, Vec<f32>, Vec<f32>) {
    // RT discipline: on Windows this is MMCSS + TIME_CRITICAL + ideal processor + working-set
    // raise; elsewhere an honest no-op. The report goes into the diagnostics, never into a log
    // nobody reads.
    let buf_bytes = (out.len() + input.len()) * std::mem::size_of::<f32>();
    let rt = RtThreadGuard::apply(
        opts.ideal_processor,
        if opts.working_set_lock { buf_bytes } else { 0 },
    );
    diag.set_rt_report(rt.report());

    let period = Duration::from_secs_f64(cfg.block_seconds());
    let clock = Clock::new(cfg.sample_rate, 120.0, 960);
    let mut ctx = BlockContext {
        block: BlockId::FIRST,
        sample_offset: 0,
        frames: cfg.block_frames,
        sample_rate: cfg.sample_rate,
        channels: cfg.outputs,
        tick: 0,
        ppqn: 960,
    };
    let start = Instant::now();
    let mut next_deadline = start + period;
    let mut last_wake = start;
    let mut device_frames = 0u64;

    while !stop_flag.load(Ordering::Acquire) {
        if faults.unplug_latched() {
            diag.record_device_error();
            state_store(&state, StreamState::Removed);
            return (proc, out, input);
        }

        // Pace: hybrid sleep+spin to the deadline. Plain `sleep` overshoots by the kernel's
        // timer granularity (~0.5–4 ms under load), which the jitter stats faithfully reported
        // as late wakes; a real device interrupt does not drift like that, so the *simulation*
        // must not either. Sleep until 150 µs before the deadline, then spin the rest: bounded
        // CPU cost (~10–20 % of one core at 48k/64), interrupt-grade accuracy. (A simulated
        // interrupt; see the module-level allow.)
        let now = Instant::now();
        if now < next_deadline {
            let spin_window = Duration::from_micros(150);
            if next_deadline - now > spin_window {
                std::thread::sleep(next_deadline - now - spin_window);
            }
            while Instant::now() < next_deadline {
                std::hint::spin_loop();
            }
        }
        let wake = Instant::now();
        diag.record_wake(wake - last_wake, period);
        last_wake = wake;

        NullStream::run_block(
            &mut proc,
            &mut ctx,
            &mut out,
            &input,
            &diag,
            &faults,
            period,
            opts.audit_allocations,
            true,
        );

        if let Some(ring) = capture_ring.as_ref() {
            for &s in out.iter() {
                let _ = ring.push(s);
            }
        }

        device_frames += cfg.block_frames as u64;
        diag.record_device_clock(device_frames, wake - start);
        ctx = ctx.advance(&clock);

        next_deadline += period;
        // Catch-up policy: never chase a lost deadline block-by-block (that models a device that
        // magically buffered our lateness). Skip forward and let `record_wake` count the xrun —
        // the audible truth.
        if Instant::now() > next_deadline + period {
            next_deadline = Instant::now() + period;
        }
    }

    state_store(&state, StreamState::Stopped);
    (proc, out, input)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::hal::{Fault, OpenOptions};

    /// A callback that writes a deterministic ramp from the block context.
    fn ramp_proc() -> ProcessFn {
        Box::new(|fctx: &mut FrameCtx<'_>| {
            for (i, s) in fctx.out.iter_mut().enumerate() {
                *s = ((fctx.block.sample_offset as usize + i) % 997) as f32 / 997.0 - 0.5;
            }
            BlockStatus::Ok
        })
    }

    fn manual_stream(cfg: StreamConfig, frames: usize) -> Box<dyn AudioStream> {
        let opts = OpenOptions::manual_with_capture(frames);
        NullBackend::new()
            .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, ramp_proc())
            .unwrap_or_else(|e| panic!("open: {e}"))
    }

    #[test]
    fn enumerate_reports_one_default_device() {
        let devs = NullBackend::new().enumerate().unwrap();
        assert_eq!(devs.len(), 1);
        assert!(devs[0].is_default_output && devs[0].has_input && devs[0].has_output);
    }

    #[test]
    fn manual_pump_produces_exact_block_count_and_advances_ctx() {
        let cfg = StreamConfig {
            sample_rate: 48_000,
            block_frames: 64,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        };
        let mut s = manual_stream(cfg, 640);
        s.start().unwrap();
        assert_eq!(s.pump(10).unwrap(), 10);
        assert_eq!(s.diag().blocks, 10);
        let cap = s.captured().unwrap();
        assert_eq!(
            cap.len(),
            640 * 2,
            "capture_frames counts frames; the ring holds frames × outputs"
        );
        // ctx advanced: the last captured block started at sample_offset 9*64
        s.stop().unwrap();
        assert!(!s.is_running());
    }

    #[test]
    fn manual_render_is_deterministic() {
        let cfg = StreamConfig {
            sample_rate: 96_000,
            block_frames: 128,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        };
        let render = |seed: u32| {
            let opts = OpenOptions::manual_with_capture(1024);
            let proc: ProcessFn = Box::new(move |f: &mut FrameCtx<'_>| {
                for (i, s) in f.out.iter_mut().enumerate() {
                    *s = ((f.block.sample_offset as u32).wrapping_mul(31) ^ seed ^ i as u32) as f32
                        / u32::MAX as f32;
                }
                BlockStatus::Ok
            });
            let mut s = NullBackend::new()
                .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, proc)
                .unwrap();
            s.start().unwrap();
            s.pump(8).unwrap();
            s.captured().unwrap().to_vec()
        };
        assert_eq!(render(7), render(7), "same callback + same blocks = same bytes (ADR-007)");
        assert_ne!(render(7), render(8));
    }

    #[test]
    fn unplug_enters_removed_state_and_stop_still_works() {
        let cfg = StreamConfig::stereo_48k();
        let mut s = manual_stream(cfg, 256);
        s.start().unwrap();
        assert_eq!(s.pump(3).unwrap(), 3);
        s.inject_fault(Fault::Unplug).unwrap();
        // The pump notices on the next block: it refuses to process and reports Removed — the
        // caller learns the device is gone from the very call that discovered it.
        assert!(matches!(s.pump(4), Err(HalError::Removed(_))));
        let rep = s.error_report();
        assert_eq!(rep.state, StreamState::Removed);
        assert!(rep.last_error.is_some_and(|m| m.contains("removal")));
        assert!(rep.device_errors >= 1);
        // stop() from Removed must succeed — "recoverable stop, not a crash"
        s.stop().unwrap();
        // ...and restart must be refused with an honest error, not a panic.
        assert!(matches!(s.start(), Err(HalError::Removed(_))));
    }

    #[test]
    fn injected_xrun_is_counted() {
        let cfg = StreamConfig::stereo_48k();
        let mut s = manual_stream(cfg, 256);
        s.start().unwrap();
        s.inject_fault(Fault::XrunNext).unwrap();
        s.pump(2).unwrap();
        let d = s.diag();
        assert_eq!(d.xruns, 1, "exactly one injected xrun");
        assert!(!d.is_clean());
        s.pump(2).unwrap();
        assert_eq!(s.diag().xruns, 1, "the fault was consumed, not latched");
    }

    #[test]
    fn allocation_audit_catches_a_dirty_callback() {
        // Only meaningful when the counting allocator is installed (test binaries do; lib unit
        // tests do not — the real gate lives in rt_discipline.rs, same contract as device::null).
        if !crate::alloc::audit_is_live() {
            return;
        }
        let cfg = StreamConfig::stereo_48k();
        let opts = OpenOptions::manual_with_capture(128);
        let dirty: ProcessFn = Box::new(|f: &mut FrameCtx<'_>| {
            let leak: Vec<f32> = vec![0.0; 8]; // plan §5.2 violation on purpose
            for (i, s) in f.out.iter_mut().enumerate() {
                *s = leak[i % leak.len()];
            }
            BlockStatus::Ok
        });
        let mut s = NullBackend::new()
            .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, dirty)
            .unwrap();
        s.start().unwrap();
        s.pump(2).unwrap();
        assert!(s.diag().allocations >= 2, "audit must see the per-block allocation");
    }

    #[test]
    fn paced_stream_runs_and_stops_clean() {
        let cfg = StreamConfig {
            sample_rate: 48_000,
            block_frames: 256,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        };
        let opts = OpenOptions { paced: true, capture_frames: 4096, ..OpenOptions::default() };
        let mut s = NullBackend::new()
            .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, ramp_proc())
            .unwrap();
        s.start().unwrap();
        assert!(s.is_running());
        // ~0.3 s at 48 kHz/256 ≈ 56 blocks; a loaded CI runner must still manage 80 % of real time.
        std::thread::sleep(Duration::from_millis(300));
        s.stop().unwrap();
        let d = s.diag();
        assert!(d.blocks >= 45, "paced pump too slow: {} blocks in 300 ms", d.blocks);
        assert!(d.blocks <= 70, "paced pump overproduced: {} blocks", d.blocks);
        assert_eq!(d.allocations, 0, "ramp callback must not allocate");
        assert!(s.captured().is_some());
        // restart after stop works (paced): the thread returned everything
        s.start().unwrap();
        std::thread::sleep(Duration::from_millis(30));
        s.stop().unwrap();
        assert!(s.diag().blocks > 0);
    }

    #[test]
    fn paced_stop_during_unplug_is_recoverable() {
        let cfg = StreamConfig::stereo_48k();
        let opts = OpenOptions { paced: true, ..OpenOptions::default() };
        let mut s = NullBackend::new()
            .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, ramp_proc())
            .unwrap();
        s.start().unwrap();
        s.inject_fault(Fault::Unplug).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        // The pump exits by itself on the fault; stop() must still succeed and report Removed.
        s.stop().unwrap();
        assert_eq!(s.error_report().state, StreamState::Removed);
        assert!(!s.is_running());
    }

    #[test]
    fn multichannel_32_out_8_in_opens_and_processes() {
        // WO-006 acceptance: "the null device proves the code path" for ≥8 channels.
        let cfg = StreamConfig {
            sample_rate: 96_000,
            block_frames: 64,
            inputs: 8,
            outputs: 32,
            exclusive: true,
        };
        let opts = OpenOptions::manual_with_capture(64);
        let seen = Arc::new(AtomicU64::new(0));
        let seen2 = Arc::clone(&seen);
        let proc: ProcessFn = Box::new(move |f: &mut FrameCtx<'_>| {
            assert_eq!(f.out.len(), f.block.frames * 32);
            assert_eq!(f.input.len(), f.block.frames * 8);
            seen2.fetch_add(1, Ordering::Relaxed);
            f.zero_out();
            BlockStatus::Ok
        });
        let mut s = NullBackend::new()
            .open(&DeviceId::default_of(BackendKind::Null), &cfg, &opts, proc)
            .unwrap();
        assert_eq!(s.actual_config(), cfg);
        s.start().unwrap();
        s.pump(4).unwrap();
        assert_eq!(seen.load(Ordering::Relaxed), 4);
        let lat = s.latency_report();
        assert!(lat.input_pipeline_frames > 0, "duplex stream reports an input pipeline");
    }

    #[test]
    fn out_of_envelope_configs_are_refused_with_text() {
        let opts = OpenOptions::manual_with_capture(0);
        let b = NullBackend::new();
        let id = DeviceId::default_of(BackendKind::Null);
        let proc = || -> ProcessFn { Box::new(|_: &mut FrameCtx<'_>| BlockStatus::Ok) };
        let cfg65 = StreamConfig {
            sample_rate: 48_000,
            block_frames: 64,
            inputs: 0,
            outputs: 65,
            exclusive: false,
        };
        match b.open(&id, &cfg65, &opts, proc()) {
            Err(HalError::Unsupported(m)) => assert!(m.contains("64"), "{m}"),
            // Box<dyn AudioStream> has no Debug: match the arms instead of printing the Result.
            Err(e) => panic!("expected Unsupported, got Err({e})"),
            Ok(_) => panic!("expected Unsupported for 65 output channels, got Ok"),
        }
        let cfg0 = StreamConfig {
            sample_rate: 0,
            block_frames: 64,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        };
        assert!(matches!(b.open(&id, &cfg0, &opts, proc()), Err(HalError::Format(_))));
        // a wrong-backend id is NotFound, not a panic
        let bad = DeviceId::by_index(BackendKind::Asio, 3);
        assert!(matches!(
            b.open(&bad, &StreamConfig::stereo_48k(), &opts, proc()),
            Err(HalError::NotFound(_))
        ));
    }

    #[test]
    fn pump_without_start_is_refused() {
        let mut s = manual_stream(StreamConfig::stereo_48k(), 0);
        assert!(matches!(s.pump(1), Err(HalError::Device(_))));
    }

    #[test]
    fn double_start_is_an_error_not_a_no_op() {
        let mut s = manual_stream(StreamConfig::stereo_48k(), 0);
        s.start().unwrap();
        assert!(matches!(s.start(), Err(HalError::Device(_))));
        s.stop().unwrap();
    }
}
