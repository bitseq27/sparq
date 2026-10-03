//! The live audio session (WO-012 increment 2, reworked in increment 4): a HAL stream whose
//! callback holds the `AudioEngine`, a control side that stages from the canvas, and a
//! per-frame drain that fills the canvas's wire levels, scope traces and meter bars from the
//! engine's OWN publications — never faked, in either direction.
//!
//! # The audio-control thread (increment 4, D1 — the device blocker)
//!
//! The WASAPI HAL runs every control-side call inside a **persistent per-thread MTA**
//! (`ensure_control_com`). A console host's main thread is uninitialised, so `sparq play`
//! works; a winit host's UI thread is **STA** (the windowing stack OLE-initialises it) and can
//! never become MTA — the operator's first PLAY on SATURN refused with exactly that sentence.
//! So the session spawns ONE dedicated audio-control thread per session: it is the only thread
//! that touches the backend (enumerate → probe → open → start → stop → drop, plus pump/fault/
//! capture on the manual null). It becomes MTA simply by calling the HAL. The UI thread keeps
//! what is thread-safe by construction: the `SharedEngine` control half, the node map, the
//! drain scratch, and a mirrored health/diag snapshot the worker polls on a 50 ms cadence.
//!
//! The rest of the shape is increment 2's, per `WO012-INC2-PLAN.md`: two-phase open built for
//! the NEGOTIATED config; the callback owns the `AudioEngine` and does `render_block` and
//! nothing else; ONE op→sync door (`PatchChanges`: params → command ring with zero re-stages,
//! structural + master handovers → boundary re-stage); the bounded per-frame drain; `Removed`/
//! `Failed` ends the session in ONE honest line with the canvas untouched; STOP prints the
//! measured evidence and teardown drops the engine on a control thread — the worker's.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
// The mirror's lock is held for a struct copy on the CONTROL path (worker poll, UI frame
// read) — never on the audio path, which is why the disallowed-type lint is relaxed here with
// the reason stated, the defect-#66 precedent. The audio thread never sees this Mutex.
use std::sync::Arc;
#[allow(clippy::disallowed_types)]
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use sparq_audio::engine::{AnalysisUpdate, AudioEngine, MeterUpdate, SharedEngine};
use sparq_audio::executor::ExecConfig;
use sparq_kernel::device::StreamConfig;
use sparq_kernel::hal::{
    backend_by_kind, AudioStream, BackendKind, DeviceInfo, Fault, OpenOptions, ProcessFn,
    StreamState,
};
use sparq_kernel::rt::BlockStatus;
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;
use sparq_ui::canvas::interact::PatchChanges;
use sparq_ui::canvas::levels::{LiveMeters, NodeLevels, StereoMeter};
use sparq_ui::canvas::model::{Graph as CanvasGraph, NodeId as CanvasNodeId};
use sparq_ui::canvas::scope::{ScopeTraces, ScopeView};
use sparq_ui::canvas::SCOPE_ID;

/// Meter-ring drain batch (fixed scratch, never a fresh Vec per frame).
const METER_BATCH: usize = 64;
/// Analysis-ring drain batch — those payloads carry waveforms, so the batch stays small.
const ANALYSIS_BATCH: usize = 16;
/// Hard per-frame drain budgets: a frame that fell behind catches up across frames, and the
/// rings refuse-and-count what they cannot hold — the audio thread never waits for this reader.
const METER_BUDGET: usize = 4096;
const ANALYSIS_BUDGET: usize = 1024;
/// How long the UI thread waits for one answer from the audio-control thread. A driver that
/// owns its thread longer than this is a device fault, and the session says so in words
/// instead of freezing the shell.
const WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// The worker's health-poll cadence while idle: an unplug between commands still reaches the
/// shell's per-frame health gate within one poll.
const HEALTH_POLL: Duration = Duration::from_millis(50);
/// `motion.toml`'s phosphor half-life, in seconds — the peak-hold blocks decay on AUDIO time
/// (blocks at the negotiated rate), never on a wall clock the display path could lie about.
const HOLD_HALF_LIFE_S: f64 = 0.3;
/// The clip threshold (operator round 4, D2): a per-channel block peak that REACHES full scale
/// has clipped, and the latch the meter wears says so until the session ends. `>=`, not `>` —
/// a sample sitting exactly at 1.0 is at the rail, and the rail is the claim.
const CLIP_THRESHOLD: f32 = 1.0;

/// What the session was asked to open with. The shell's PLAY uses [`LiveOptions::default`];
/// the audit and tests pin the null backend in manual mode so nothing depends on a device or
/// a wall clock.
#[derive(Clone, Copy, Debug)]
pub struct LiveOptions {
    /// Force a backend. `None` = the automatic choice: `wasapi-shared` where it is compiled in
    /// on Windows, the null device everywhere else (said out loud in the log).
    pub backend: Option<BackendKind>,
    /// Null backend only: a paced pump thread (`true`) or manual [`LiveSession::pump_manual`]
    /// stepping (`false` — the deterministic mode the audit runs).
    pub paced: bool,
    /// Null backend only: keep this many output frames for inspection (smoke evidence; 0 in the
    /// shell so a long session cannot exhaust memory).
    pub capture_frames: usize,
}

impl Default for LiveOptions {
    fn default() -> Self {
        Self { backend: None, paced: true, capture_frames: 0 }
    }
}

// --------------------------------------------------------------------------- the worker

/// Commands to the audio-control thread. Every one answers, so the protocol is request/response
/// on a control path — never on the audio path.
enum ToWorker {
    /// Enumerate, probe-open, and report the negotiated truth (the probe stream is dropped).
    Probe { kind: BackendKind, request: StreamConfig },
    /// The real open with the caller's engine in the callback, then start. The engine rides in
    /// a `Box` so the command enum stays small (the channel allocates once, on the control
    /// path, at session start — never per block).
    Start { engine: Box<AudioEngine>, cfg: StreamConfig, paced: bool, capture_frames: usize },
    /// Manual-mode stepping (null backend). Device backends answer with their own refusal.
    Pump(u64),
    /// Ask the null backend to simulate a fault.
    Fault(Fault),
    /// A copy of the captured output frames (null backend with capture on).
    Capture,
    /// Stop, report the measured evidence, drop the stream HERE, and exit.
    Stop,
}

/// The measured evidence the stop path carries home — plain owned data, no borrows of the
/// stream escape the worker.
#[derive(Debug)]
struct StopEvidence {
    blocks: u64,
    xruns: u64,
    overruns: u64,
    allocs: u64,
    state: StreamState,
    last_error: Option<String>,
}

#[derive(Debug)]
enum FromWorker {
    Probed { cfg: StreamConfig, device: String, backend: String },
    Started { latency: String },
    Pumped(u64),
    Faulted,
    Captured(Vec<f32>),
    Stopped(Box<StopEvidence>),
    Failed(String),
}

/// The worker's mirror of the stream's health and diag, polled on [`HEALTH_POLL`] and after
/// every command, so the shell's per-frame gate and the top-bar diagnostics read a snapshot
/// without touching the stream (which lives, and dies, on the worker). Reading diag/state is
/// COM-free (atomics and cached config), which is why the mirror may poll freely.
#[derive(Clone, Debug)]
struct HealthSnapshot {
    state: StreamState,
    last_error: Option<String>,
    rate: u32,
    block: usize,
    xruns: u64,
}

impl Default for HealthSnapshot {
    fn default() -> Self {
        Self { state: StreamState::Idle, last_error: None, rate: 0, block: 0, xruns: 0 }
    }
}

#[allow(clippy::disallowed_types)] // control-path mirror; see the import note above
type HealthMirror = Arc<Mutex<HealthSnapshot>>;

fn mirror(stream: &Option<Box<dyn AudioStream>>, health: &HealthMirror) {
    let snap = match stream {
        Some(s) => {
            let rep = s.error_report();
            let diag = s.diag();
            let cfg = s.actual_config();
            HealthSnapshot {
                state: rep.state,
                last_error: rep.last_error,
                rate: cfg.sample_rate,
                block: cfg.block_frames,
                xruns: diag.xruns,
            }
        },
        None => HealthSnapshot { state: StreamState::Stopped, ..HealthSnapshot::default() },
    };
    if let Ok(mut g) = health.lock() {
        *g = snap;
    }
}

/// The audio-control thread's loop (D1). It owns the stream from open to drop; its apartment
/// (MTA on Windows, via the HAL's own `ensure_control_com`) begins with its first HAL call and
/// ends with the thread.
fn worker_run(rx: Receiver<ToWorker>, tx: Sender<FromWorker>, health: HealthMirror) {
    let mut stream: Option<Box<dyn AudioStream>> = None;
    let mut device: Option<DeviceInfo> = None;
    loop {
        let cmd = match rx.recv_timeout(HEALTH_POLL) {
            Ok(c) => c,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                mirror(&stream, &health);
                continue;
            },
            // The session vanished without a Stop (a panic path): drop the stream HERE and
            // exit — teardown on a control thread, never in a device callback.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let is_stop = matches!(cmd, ToWorker::Stop);
        let reply = match cmd {
            ToWorker::Probe { kind, request } => probe(&mut device, kind, &request),
            ToWorker::Start { engine, cfg, paced, capture_frames } => {
                start(&mut stream, device.take(), engine, &cfg, paced, capture_frames)
            },
            ToWorker::Pump(n) => match stream.as_mut() {
                Some(s) => match s.pump(n) {
                    Ok(done) => FromWorker::Pumped(done),
                    Err(e) => FromWorker::Failed(e.with_hint()),
                },
                None => FromWorker::Failed("pump: no stream is open".to_string()),
            },
            ToWorker::Fault(f) => match stream.as_mut() {
                Some(s) => match s.inject_fault(f) {
                    Ok(()) => FromWorker::Faulted,
                    Err(e) => FromWorker::Failed(e.with_hint()),
                },
                None => FromWorker::Failed("fault: no stream is open".to_string()),
            },
            ToWorker::Capture => FromWorker::Captured(
                stream.as_ref().and_then(|s| s.captured()).map(<[f32]>::to_vec).unwrap_or_default(),
            ),
            ToWorker::Stop => {
                let ev = match stream.take() {
                    Some(mut s) => {
                        // Callable from every state; blocking here is fine — this IS the
                        // control thread the contract names.
                        let _ = s.stop();
                        let diag = s.diag();
                        let rep = s.error_report();
                        StopEvidence {
                            blocks: diag.blocks,
                            xruns: diag.xruns,
                            overruns: diag.overruns,
                            allocs: diag.allocations,
                            state: rep.state,
                            last_error: rep.last_error,
                        }
                    },
                    None => StopEvidence {
                        blocks: 0,
                        xruns: 0,
                        overruns: 0,
                        allocs: 0,
                        state: StreamState::Stopped,
                        last_error: None,
                    },
                };
                mirror(&stream, &health);
                FromWorker::Stopped(Box::new(ev))
            },
        };
        mirror(&stream, &health);
        if tx.send(reply).is_err() {
            break; // nobody is listening any more: wind down
        }
        if is_stop {
            break; // the stream is dropped above; the apartment ends with this thread
        }
    }
    drop(stream);
}

fn probe(device: &mut Option<DeviceInfo>, kind: BackendKind, request: &StreamConfig) -> FromWorker {
    let out = (|| -> Result<FromWorker, String> {
        let backend = backend_by_kind(kind).map_err(|e| e.with_hint())?;
        let devices = backend
            .enumerate()
            .map_err(|e| format!("enumerating {kind} devices: {}", e.with_hint()))?;
        let dev = devices.first().ok_or_else(|| {
            format!("backend `{kind}` reports NO output devices — check `sparq devices`")
        })?;
        let probe_cb: ProcessFn = Box::new(|fctx| {
            fctx.zero_out();
            BlockStatus::Ok
        });
        let cfg = {
            let probe = backend
                .open(&dev.id, request, &OpenOptions::probe(), probe_cb)
                .map_err(|e| format!("probe open failed: {}", e.with_hint()))?;
            probe.actual_config()
        };
        let names = FromWorker::Probed {
            cfg,
            device: dev.name.clone(),
            backend: backend.display_name().to_string(),
        };
        *device = Some(dev.clone());
        Ok(names)
    })();
    out.unwrap_or_else(FromWorker::Failed)
}

fn start(
    stream: &mut Option<Box<dyn AudioStream>>,
    device: Option<DeviceInfo>,
    engine: Box<AudioEngine>,
    cfg: &StreamConfig,
    paced: bool,
    capture_frames: usize,
) -> FromWorker {
    let out = (|| -> Result<FromWorker, String> {
        let dev =
            device.ok_or_else(|| "probe never ran — open order is a session bug".to_string())?;
        let backend = backend_by_kind(dev.id.backend).map_err(|e| e.with_hint())?;
        let mut audio = *engine;
        let proc: ProcessFn = Box::new(move |fctx| match audio.render_block(fctx.out) {
            Ok(()) => BlockStatus::Ok,
            Err(_) => {
                // A refusal here means a staged patch lost its master — a control-side bug.
                // Silence is the honest output; DeviceError is the honest histogram entry.
                fctx.zero_out();
                BlockStatus::DeviceError
            },
        });
        let opts = OpenOptions {
            audit_allocations: true,
            paced,
            capture_frames,
            ideal_processor: None,
            working_set_lock: true,
            log_negotiation: false,
        };
        let mut s = backend
            .open(&dev.id, cfg, &opts, proc)
            .map_err(|e| format!("open failed: {}", e.with_hint()))?;
        let latency = s.latency_report().summary();
        s.start().map_err(|e| format!("start failed: {}", e.with_hint()))?;
        *stream = Some(s);
        Ok(FromWorker::Started { latency })
    })();
    out.unwrap_or_else(FromWorker::Failed)
}

/// The backend choice (D4 of increment 2, unchanged): `wasapi-shared` where it is compiled in
/// on Windows — the device-proven interim vehicle — and the null device everywhere else,
/// SAID OUT LOUD, because a session that makes no sound must never pretend otherwise.
fn automatic_backend(log: &mut Vec<String>) -> BackendKind {
    #[cfg(all(windows, feature = "hal-wasapi"))]
    {
        let _ = log;
        BackendKind::WasapiShared
    }
    #[cfg(not(all(windows, feature = "hal-wasapi")))]
    {
        log.push(
            "live: no device backend in this build — running against the NULL device (no sound \
             leaves the process; the meters and the engine are real)"
                .to_string(),
        );
        BackendKind::Null
    }
}

// --------------------------------------------------------------------------- the session

/// One running live session. The UI thread owns this; the HAL stream lives on the session's
/// audio-control thread (D1). `None` in the shell means nothing is playing.
pub struct LiveSession {
    tx: Sender<ToWorker>,
    rx: Receiver<FromWorker>,
    worker: Option<JoinHandle<()>>,
    health: HealthMirror,
    control: SharedEngine,
    /// The config the executor was built at — the NEGOTIATED truth; every re-stage rebuilds at
    /// exactly these numbers, because the callback's buffer is exactly this shape.
    exec_cfg: ExecConfig,
    /// canvas node → kernel node (the sync door's aim for `set_params`).
    map: HashMap<CanvasNodeId, sparq_kernel::graph::NodeId>,
    /// kernel node (u32) → canvas node (the drain's way home).
    inv: HashMap<u32, CanvasNodeId>,
    /// The canvas master the live executor was staged with — a handover is structural (D7).
    staged_master: CanvasNodeId,
    /// The session's level set: last-wins per (node, port) across drains, reset on re-stage.
    levels: NodeLevels,
    /// Per-node fold tracking: node → (newest block seen, max port peak in that block).
    fold: HashMap<CanvasNodeId, (u64, f32)>,
    /// The scope traces (WO-013 increment 6): session-owned display state the painter reads.
    traces: ScopeTraces,
    /// The resolved scope bindings; a change means a rebind and resets the traces.
    bindings: Vec<ScopeBinding>,
    /// The stereo meter bars' source (WO-012 increment 4): per (node, output port) peaks and
    /// peak-holds, decayed on audio time. The painter reads it; at rest the shell hands over
    /// an empty map and the wells stay empty.
    meters: LiveMeters,
    meter_scratch: [MeterUpdate; METER_BATCH],
    analysis_scratch: [AnalysisUpdate; ANALYSIS_BATCH],
    /// The last build refusal already said, so a broken patch logs each DISTINCT refusal once.
    last_refusal: Option<String>,
    /// The param-ring refusal was already said this episode (reset by a clean sync).
    param_refusal_said: bool,
    kind: BackendKind,
    /// The probe's device name — the Main Out card reads it (operator ruling 2026-10-01: the
    /// master output shows WHICH driver is under it, in words, while it plays).
    device_name: String,
    /// The probe's backend display name (same ruling).
    backend_name: String,
    /// Set once STOP has run, so `Drop` never double-stops.
    stopped: bool,
}

/// A failed open leaves no session: stop the worker, join it, and return the sentence.
fn cleanup(
    tx: Sender<ToWorker>,
    rx: &Receiver<FromWorker>,
    worker: &mut Option<JoinHandle<()>>,
    e: String,
) -> String {
    let _ = tx.send(ToWorker::Stop);
    let _ = rx.recv_timeout(WORKER_REPLY_TIMEOUT);
    if let Some(w) = worker.take() {
        let _ = w.join();
    }
    e
}

/// One scope binding: a scope node's input axis, resolved to the SOURCE `(node, port)` whose
/// published waveform it displays — "the wire is the binding, the ring is the payload".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScopeBinding {
    scope: CanvasNodeId,
    src: CanvasNodeId,
    src_port: u32,
    axis_y: bool,
}

impl LiveSession {
    /// Open, build for the negotiated truth, and start — all HAL work on the audio-control
    /// thread (D1). Every refusal is a sentence with the remedy; every success logs the
    /// honesty lines (D3 of increment 2). The log vector is the shell's.
    ///
    /// # Errors
    /// A sentence naming what refused: no device, a failed probe or real open, a failed start,
    /// a silent worker, or a patch the executor refused (its own words, passed through).
    pub fn start(
        graph: &CanvasGraph,
        master: CanvasNodeId,
        modules: &Registry,
        opts: LiveOptions,
        log: &mut Vec<String>,
    ) -> Result<Self, String> {
        let kind = match opts.backend {
            Some(k) => k,
            None => automatic_backend(log),
        };
        // The audio-control thread comes FIRST: every HAL call below rides it (D1). It becomes
        // MTA (on Windows) by simply calling the HAL; the UI thread never does.
        let (tx, rx_worker) = channel();
        let (tx_worker, rx) = channel();
        #[allow(clippy::disallowed_types)] // control-path mirror; see the import note
        let health: HealthMirror = Arc::new(Mutex::new(HealthSnapshot::default()));
        let h2 = Arc::clone(&health);
        let mut worker = Some(
            std::thread::Builder::new()
                .name("sparq-audio-control".to_string())
                .spawn(move || worker_run(rx_worker, tx_worker, h2))
                .map_err(|e| format!("the audio-control thread would not start: {e}"))?,
        );

        // ---- phase 1: the probe, on the worker ------------------------------------------------
        let request = StreamConfig {
            sample_rate: 48_000,
            block_frames: 64,
            inputs: 0,
            outputs: 2,
            exclusive: kind == BackendKind::WasapiExclusive,
        };
        if tx.send(ToWorker::Probe { kind, request }).is_err() {
            return Err("the audio-control thread is gone".to_string());
        }
        let (cfg, device, backend) = match rx.recv_timeout(WORKER_REPLY_TIMEOUT) {
            Ok(FromWorker::Probed { cfg, device, backend }) => (cfg, device, backend),
            Ok(FromWorker::Failed(e)) => return Err(cleanup(tx, &rx, &mut worker, e)),
            Ok(other) => {
                return Err(cleanup(tx, &rx, &mut worker, format!("probe answered {other:?}")))
            },
            Err(_) => {
                return Err(cleanup(
                    tx,
                    &rx,
                    &mut worker,
                    "the audio-control thread did not answer the probe in 5 s — the device                      driver owns it"
                        .to_string(),
                ))
            },
        };
        let _ = device; // the worker keeps it for the real open
        log.push(format!("PLAY · {backend} · device `{device}`"));
        let asked_differs =
            cfg.sample_rate != request.sample_rate || cfg.outputs != request.outputs;
        log.push(format!(
            "PLAY · NEGOTIATED {} Hz · {} ch · block {}{} — the executor is built for THIS, \
             not for the request",
            cfg.sample_rate,
            cfg.outputs,
            cfg.block_frames,
            if asked_differs {
                format!(" (asked {} Hz · {} ch)", request.sample_rate, request.outputs)
            } else {
                String::new()
            }
        ));

        // ---- phase 2: build HERE (pure Rust), open+start on the worker ------------------------
        let exec_cfg = ExecConfig::new(cfg.sample_rate, cfg.block_frames, cfg.outputs);
        // The trim-gain map comes out of the door for the live path; the SHIPPED behaviour is
        // the structural re-stage (silent since D1: the gain instance adopts on its synthetic
        // wire key and its glide rides the drag), so the session drops the map today — the
        // declared LATER set_params fast path lifts it from the same door.
        let (executor, k_master, map, _trim_gains) =
            crate::bridge::build_with_map_at(graph, master, modules, exec_cfg)?;
        let inv = map.iter().map(|(c, k)| (k.0, *c)).collect();
        let (control, audio_engine) = SharedEngine::new(executor, k_master);
        if tx
            .send(ToWorker::Start {
                engine: Box::new(audio_engine),
                cfg,
                paced: opts.paced,
                capture_frames: opts.capture_frames,
            })
            .is_err()
        {
            return Err(cleanup(tx, &rx, &mut worker, "the audio-control thread is gone".into()));
        }
        let latency = match rx.recv_timeout(WORKER_REPLY_TIMEOUT) {
            Ok(FromWorker::Started { latency }) => latency,
            Ok(FromWorker::Failed(e)) => return Err(cleanup(tx, &rx, &mut worker, e)),
            Ok(other) => {
                return Err(cleanup(tx, &rx, &mut worker, format!("start answered {other:?}")))
            },
            Err(_) => {
                return Err(cleanup(
                    tx,
                    &rx,
                    &mut worker,
                    "the audio-control thread did not answer the start in 5 s — the device                      driver owns it"
                        .to_string(),
                ))
            },
        };
        log.push(format!("PLAY · latency {latency}"));
        log.push(
            "PLAY · running — wires animate from the engine's own meters; edits cross live"
                .to_string(),
        );
        Ok(Self {
            tx,
            rx,
            worker,
            health,
            control,
            exec_cfg,
            map,
            inv,
            staged_master: master,
            levels: NodeLevels::new(),
            fold: HashMap::new(),
            traces: ScopeTraces::new(),
            bindings: Vec::new(),
            meters: LiveMeters::new(),
            meter_scratch: [MeterUpdate::default(); METER_BATCH],
            analysis_scratch: [AnalysisUpdate::default(); ANALYSIS_BATCH],
            last_refusal: None,
            param_refusal_said: false,
            kind,
            device_name: device,
            backend_name: backend,
            stopped: false,
        })
    }

    /// Send one command; a dead worker is a sentence, not a panic.
    fn send(&self, cmd: ToWorker) -> Result<(), String> {
        self.tx.send(cmd).map_err(|_| "the audio-control thread is gone".to_string())
    }

    /// Wait for one answer, bounded: a driver that owns its thread overlong is a device fault
    /// the shell reports in words instead of freezing on.
    fn recv(&self) -> Result<FromWorker, String> {
        self.rx.recv_timeout(WORKER_REPLY_TIMEOUT).map_err(|_| {
            "the audio-control thread did not answer in 5 s — the device driver owns it; stop \
             the session and re-run `sparq devices`"
                .to_string()
        })
    }

    /// Is the stream still usable? The shell asks once per frame off the mirror (no round
    /// trip); `Removed`/`Failed` end the session with one honest line (D3).
    #[must_use]
    pub fn health(&self) -> StreamState {
        self.health.lock().map(|g| g.state).unwrap_or(StreamState::Failed)
    }

    /// The last device-level failure text, for the honest line.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.health.lock().ok().and_then(|g| g.last_error.clone())
    }

    /// The top bar's live diagnostics line (increment 3): the NEGOTIATED rate and block, the
    /// measured xrun count and boundary swaps — every number with its unit, every value read
    /// (mirror + control stats), none invented. No worker round trip: the mirror polls.
    #[must_use]
    pub fn diag_line(&self) -> String {
        let snap = self.health.lock().map(|g| g.clone()).unwrap_or_default();
        format!(
            "{:.1} kHz · {} fr · xrun {} · swap {}",
            f64::from(snap.rate) / 1000.0,
            snap.block,
            snap.xruns,
            self.control.stats().swap.swaps,
        )
    }

    /// The Main Out card's driver readout (operator ruling 2026-10-01): WHICH driver is under
    /// the master, and the negotiated truth it opened with — backend · device on the first
    /// line, rate · channels · block · sample format on the second. Every value is the
    /// session's own (the probe's names, the negotiated config, the mirror's counters);
    /// nothing is invented, and the format is the HAL contract's interleaved f32 — the one
    /// format `ProcessFn` carries.
    #[must_use]
    pub fn driver_lines(&self) -> [String; 2] {
        let snap = self.health.lock().map(|g| g.clone()).unwrap_or_default();
        let rate = if snap.rate > 0 { snap.rate } else { self.exec_cfg.sample_rate };
        let block = if snap.block > 0 { snap.block } else { self.exec_cfg.block_frames };
        // Line two is composed to FIT the card's info band at zoom 1 (28 xs chars): kHz not
        // Hz, `f32` not `32-bit float` — the band clips with an ellipsis if a device name or
        // a negotiated oddity outruns it, but the negotiated truth never loses its units.
        let khz = if rate % 1000 == 0 {
            format!("{} kHz", rate / 1000)
        } else {
            format!("{:.1} kHz", f64::from(rate) / 1000.0)
        };
        [
            format!("{} · {}", self.backend_name, self.device_name),
            format!("{khz} · {} ch · {} fr · f32", self.exec_cfg.device_channels, block),
        ]
    }

    /// The cross-thread counters — what the evidence line reads, and what the smokes assert
    /// on (state, not log lines): swaps, commands applied/refused, ring refusals.
    #[must_use]
    pub fn stats(&self) -> sparq_audio::engine::EngineStats {
        self.control.stats()
    }

    /// Manual-mode stepping (null backend, `paced = false`): produce exactly `blocks` blocks
    /// synchronously ON THE WORKER — the audit's deterministic pump. Device backends refuse in
    /// words (hardware paces itself).
    ///
    /// # Errors
    /// The HAL's own sentence, or a silent worker.
    pub fn pump_manual(&mut self, blocks: u64) -> Result<u64, String> {
        self.send(ToWorker::Pump(blocks))?;
        match self.recv()? {
            FromWorker::Pumped(n) => Ok(n),
            FromWorker::Failed(e) => Err(e),
            other => Err(format!("pump answered {other:?}")),
        }
    }

    /// Ask the null backend to simulate a fault (the unplug smoke). Device backends refuse in
    /// words — real faults are tested by really unplugging things, on SATURN.
    ///
    /// # Errors
    /// The HAL's own sentence.
    pub fn inject_fault(&mut self, fault: Fault) -> Result<(), String> {
        self.send(ToWorker::Fault(fault))?;
        match self.recv()? {
            FromWorker::Faulted => Ok(()),
            FromWorker::Failed(e) => Err(e),
            other => Err(format!("fault answered {other:?}")),
        }
    }

    /// A copy of the captured output frames (null backend with `capture_frames > 0`) — the
    /// smoke's proof that the callback rendered the CANVAS patch, not silence. The copy
    /// crosses from the worker; `None` when capture is off or empty.
    #[must_use]
    pub fn captured(&mut self) -> Option<Vec<f32>> {
        self.send(ToWorker::Capture).ok()?;
        match self.recv() {
            Ok(FromWorker::Captured(v)) if !v.is_empty() => Some(v),
            _ => None,
        }
    }

    /// The one op→sync door (D7 of increment 2). Param edits cross as command-ring snapshots;
    /// a structural change — or a master handover — rebuilds at the negotiated config and
    /// stages for the next block boundary. Staging itself never refuses (the hot-swap slot
    /// SUPERSEDES a waiting patch and counts it); what can refuse is the BUILD, and a build
    /// refusal leaves the live patch playing and says so in words.
    pub fn sync(
        &mut self,
        graph: &CanvasGraph,
        modules: &Registry,
        master: CanvasNodeId,
        changes: &PatchChanges,
        log: &mut Vec<String>,
    ) {
        let mut refused = false;
        for &node in &changes.param_nodes {
            let (Some(kid), Some(n)) = (self.map.get(&node), graph.node(node)) else {
                continue; // a node the live patch does not have (yet) — a re-stage below fixes it
            };
            let values = n.effective_params();
            let Some(params) = ParamSet::new(1, &values) else { continue };
            if !self.control.set_params(*kid, params) {
                refused = true;
            }
        }
        if refused {
            if !self.param_refusal_said {
                self.param_refusal_said = true;
                log.push(format!(
                    "live: param command REFUSED — the command ring is full (the audio side is \
                     behind); {} refusal(s) counted; the edit stays on the canvas and re-crosses \
                     with the next change",
                    self.control.stats().cmd_queue_refusals,
                ));
            }
        } else {
            self.param_refusal_said = false;
        }
        if changes.structural || master != self.staged_master {
            self.restage(graph, modules, master, log);
        }
        self.sync_scopes(graph);
    }

    /// Rebuild at the negotiated config and stage for the next boundary.
    fn restage(
        &mut self,
        graph: &CanvasGraph,
        modules: &Registry,
        master: CanvasNodeId,
        log: &mut Vec<String>,
    ) {
        match crate::bridge::build_with_map_at(graph, master, modules, self.exec_cfg) {
            // The trim-gain map: dropped here like at session start — the re-stage IS the
            // shipped trim path (adoption + glide keep it silent); the map is the declared
            // LATER fast path's door.
            Ok((executor, k_master, map, _trim_gains)) => {
                let superseded = self.control.stage(executor, k_master);
                self.map = map;
                self.inv = self.map.iter().map(|(c, k)| (k.0, *c)).collect();
                self.staged_master = master;
                // A new patch is a new identity: stale levels would be a lie in motion.
                self.levels = NodeLevels::new();
                self.fold.clear();
                self.last_refusal = None;
                log.push(format!(
                    "live: re-staged — the edit goes audible at the next block boundary{} \
                     (swaps so far: {})",
                    if superseded {
                        " (this stage superseded one the audio side had not consumed)"
                    } else {
                        ""
                    },
                    self.control.stats().swap.swaps,
                ));
            },
            Err(e) => {
                if self.last_refusal.as_deref() != Some(e.as_str()) {
                    log.push(format!(
                        "live: re-stage REFUSED: {e} — the live patch keeps playing; fix the \
                         patch and the next edit tries again"
                    ));
                    self.last_refusal = Some(e);
                }
            },
        }
    }

    /// Resolve the scope bindings from the CURRENT graph (per frame — structurally impossible
    /// to stale), size each trace at the negotiated rate and the node's own timebase, prune
    /// scopes that left, and reset the traces when any binding changed.
    fn sync_scopes(&mut self, graph: &CanvasGraph) {
        let mut bindings = Vec::new();
        let mut scopes: Vec<(CanvasNodeId, usize)> = Vec::new();
        for n in graph.nodes() {
            if n.spec.module_id != SCOPE_ID {
                continue;
            }
            // The view params, clamped by the model (D10 of inc 6): index order is the
            // manifest's own — timebase, mode, trigger, gain, colormap — pinned by a test.
            let p = n.effective_params();
            let view = ScopeView::from_params(
                p.first().copied().unwrap_or(20.0),
                p.get(1).copied().unwrap_or(0.0),
                p.get(2).copied().unwrap_or(0.0),
                p.get(3).copied().unwrap_or(1.0),
            );
            let cap = view.capacity_at(self.exec_cfg.sample_rate, self.exec_cfg.block_frames);
            scopes.push((n.id, cap));
            for w in graph.wires() {
                if w.to.node != n.id || w.to.index > 1 {
                    continue;
                }
                bindings.push(ScopeBinding {
                    scope: n.id,
                    src: w.from.node,
                    src_port: w.from.index as u32,
                    axis_y: w.to.index == 1,
                });
            }
        }
        if bindings != self.bindings {
            self.bindings = bindings;
            // A rebind: clear, so no trace carries a dead source's tail into a new one.
            self.traces.clear();
        }
        self.traces.retain(|id| scopes.iter().any(|(s, _)| *s == id));
        for (id, cap) in scopes {
            self.traces.ensure(id, cap);
        }
    }

    /// The session's trace set — the SINGLE source the painter reads per frame (inc 6, D3′).
    #[must_use]
    pub fn traces(&self) -> &ScopeTraces {
        &self.traces
    }

    /// Mutable access for the driver doors (the smokes clear the set to measure the at-rest
    /// rendering; a session feed never goes through here).
    pub fn traces_mut(&mut self) -> &mut ScopeTraces {
        &mut self.traces
    }

    /// The stereo meter bars' source (increment 4): the SINGLE source, like the traces.
    #[must_use]
    pub fn meters(&self) -> &LiveMeters {
        &self.meters
    }

    /// The NEGOTIATED sample rate this session runs at — the display side's rate source (the
    /// response curves' axes and the scope traces' capacities both read the negotiated truth,
    /// never an assumed 48 kHz).
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.exec_cfg.sample_rate
    }

    /// The per-frame drain (D11 of increment 2, extended in increment 4): bounded, batched,
    /// last-wins. Audio port levels and the stereo bars from the meter ring, the folded node
    /// level = max of its newest block's port peaks, cv port levels and scope traces from the
    /// analysis ring by the increment-5 magnitude rule. Returns the level set; the painter
    /// reads traces and meters through their own accessors.
    pub fn drain(&mut self) -> &NodeLevels {
        let half_blocks = (HOLD_HALF_LIFE_S * f64::from(self.exec_cfg.sample_rate)
            / self.exec_cfg.block_frames as f64)
            .max(1.0);
        let mut newest_block = 0u64;
        let mut budget = METER_BUDGET;
        while budget > 0 {
            let n = self.control.read_meters(&mut self.meter_scratch);
            if n == 0 {
                break;
            }
            budget -= budget.min(n);
            for u in &self.meter_scratch[..n] {
                let Some(&cid) = self.inv.get(&u.node) else { continue };
                if u.port == MeterUpdate::FOLDED {
                    // A cv-only node: its honest zeros keep the fold truthful.
                    self.levels.set(cid, u.peak);
                    continue;
                }
                self.levels.set_port(cid, u.port as usize, u.peak);
                // The stereo bars (increment 4): per-channel peaks plus peak-holds decayed on
                // AUDIO time — blocks, not frames, so the display cannot lie about tempo.
                let key = (cid, u.port as usize);
                let prev = self.meters.get(&key).copied().unwrap_or_default();
                let age = u.block.saturating_sub(prev.block) as f64;
                let decay = 0.5f32.powf((age / half_blocks) as f32);
                newest_block = newest_block.max(u.block);
                self.meters.insert(
                    key,
                    StereoMeter {
                        l: u.peak_l,
                        r: u.peak_r,
                        hold_l: (prev.hold_l * decay).max(u.peak_l),
                        hold_r: (prev.hold_r * decay).max(u.peak_r),
                        // The clip latch (operator round 4, D2): once per channel, held until
                        // the session ends — the peak-hold's own state machinery hosts it, so
                        // a transient that clipped 40 seconds ago still says so, and a fresh
                        // session starts unlatched by construction (a STOP clears the word).
                        clip_l: prev.clip_l || u.peak_l >= CLIP_THRESHOLD,
                        clip_r: prev.clip_r || u.peak_r >= CLIP_THRESHOLD,
                        block: u.block,
                    },
                );
                match self.fold.get_mut(&cid) {
                    Some((block, max)) if u.block >= *block => {
                        if u.block > *block {
                            *block = u.block;
                            *max = u.peak;
                        } else {
                            *max = (*max).max(u.peak);
                        }
                    },
                    Some(_) => {}, // an older block than the fold already saw: last-wins
                    None => {
                        self.fold.insert(cid, (u.block, u.peak));
                    },
                }
            }
            for (&cid, &(_, max)) in &self.fold {
                self.levels.set(cid, max);
            }
        }
        // Peak-holds decay on the blocks that passed while nobody updated a slot — audio
        // time, so a paused session holds its bars and a playing one decays them at tempo.
        if newest_block > 0 {
            for m in self.meters.values_mut() {
                if m.block < newest_block {
                    let age = (newest_block - m.block) as f64;
                    let decay = 0.5f32.powf((age / half_blocks) as f32);
                    m.hold_l *= decay;
                    m.hold_r *= decay;
                }
            }
        }
        let mut budget = ANALYSIS_BUDGET;
        while budget > 0 {
            let n = self.control.read_analysis(&mut self.analysis_scratch);
            if n == 0 {
                break;
            }
            budget -= budget.min(n);
            for a in &self.analysis_scratch[..n] {
                let Some(&cid) = self.inv.get(&a.node) else { continue };
                let peak = a.wave().iter().fold(0.0f32, |m, &s| m.max(s.abs()));
                self.levels.set_port(cid, a.port as usize, peak.clamp(0.0, 1.0));
                // The scope feed (WO-013 increment 6): every binding whose SOURCE is this
                // (node, port) accumulates the SAME payload — one pass, no second read.
                for b in &self.bindings {
                    if b.src == cid && b.src_port == a.port {
                        if let Some(t) = self.traces.get_mut(b.scope) {
                            if b.axis_y {
                                t.y.push(a.wave());
                            } else {
                                t.x.push(a.wave());
                            }
                        }
                    }
                }
            }
        }
        &self.levels
    }

    /// Stop: tell the worker to join the pump, report the MEASURED evidence (D3 — every number
    /// read, none invented), and let the stream (with the audio engine inside it) drop THERE,
    /// on the audio-control thread — the teardown point the contract names.
    pub fn stop(mut self, verb: &str, log: &mut Vec<String>) {
        self.stopped = true;
        let ev = match self.tx.send(ToWorker::Stop) {
            Ok(()) => match self.recv() {
                Ok(FromWorker::Stopped(ev)) => Some(*ev),
                _ => None,
            },
            Err(_) => None,
        }
        .unwrap_or(StopEvidence {
            blocks: 0,
            xruns: 0,
            overruns: 0,
            allocs: 0,
            state: self.health(),
            last_error: self.last_error(),
        });
        // Retirement hygiene: every superseded patch freed exactly once, here, not in a
        // callback.
        let mut reclaimed = 0usize;
        while self.control.reclaim().is_some() {
            reclaimed += 1;
        }
        let stats = self.control.stats();
        log.push(format!(
            "{verb} · {} — {} blocks · xruns {} · overruns {} · callback allocs {} · swaps {} · \
             cmds {} applied / {} refused · meter drops {} · analysis drops {} · {} reclaimed · \
             final state {:?}",
            self.kind,
            ev.blocks,
            ev.xruns,
            ev.overruns,
            ev.allocs,
            stats.swap.swaps,
            stats.cmd_applied,
            stats.cmd_refused,
            stats.meter_refusals,
            stats.analysis_refusals,
            reclaimed,
            ev.state,
        ));
        if let Some(err) = ev.last_error {
            log.push(format!("{verb} · the stream's last error, verbatim: {err}"));
        }
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        if self.stopped {
            return;
        }
        // A session dropped without STOP (a panic path, a failed start already wound down):
        // the worker still gets its Stop and is joined — a session that leaked its thread
        // would leak the device.
        let _ = self.tx.send(ToWorker::Stop);
        let _ = self.rx.recv_timeout(WORKER_REPLY_TIMEOUT);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use sparq_audio::modules::register_builtins;

    fn registry() -> Registry {
        let mut r = Registry::new();
        register_builtins(&mut r).unwrap();
        r
    }

    fn manual_opts(capture: usize) -> LiveOptions {
        LiveOptions { backend: Some(BackendKind::Null), paced: false, capture_frames: capture }
    }

    fn find(graph: &CanvasGraph, module: &str) -> CanvasNodeId {
        graph.nodes().iter().find(|n| n.spec.module_id == module).map(|n| n.id).expect("node")
    }

    /// The demo patch (sine → gain master + a bare out/main), started on the manual null.
    fn session(log: &mut Vec<String>) -> (LiveSession, CanvasGraph, Registry) {
        let reg = registry();
        let graph = crate::bridge::demo_graph(&reg).unwrap();
        let master = find(&graph, "sparq/util/gain");
        let s = LiveSession::start(&graph, master, &reg, manual_opts(64 * 2 * 8), log).unwrap();
        (s, graph, reg)
    }

    #[test]
    fn the_live_session_renders_the_canvas_patch_and_the_levels_are_real() {
        let mut log = Vec::new();
        let (mut s, graph, _reg) = session(&mut log);
        assert_eq!(s.health(), StreamState::Running);

        s.pump_manual(8).unwrap();
        let levels = s.drain().clone();
        let sine = find(&graph, "sparq/syn/sine");
        let gain = find(&graph, "sparq/util/gain");
        assert!(levels.get(sine) > 0.0, "the sine's live level is the metered signal");
        assert!(levels.get(gain) > 0.0, "the gain's live level is the metered signal");
        // The capture is the device-side proof: the callback rendered the patch, not silence.
        let cap = s.captured().expect("capture is on");
        let peak = cap.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.3, "the captured device buffer carries the 0.5-amp demo sine: {peak}");

        // The honesty lines exist and carry the negotiated numbers.
        assert!(log.iter().any(|l| l.contains("NEGOTIATED 48000 Hz")), "{log:?}");
        assert!(log.iter().any(|l| l.contains("latency")), "{log:?}");

        s.stop("STOP", &mut log);
        assert!(
            log.iter().any(|l| l.starts_with("STOP ·") && l.contains("8 blocks")),
            "the evidence line carries the measured block count: {log:?}"
        );
        assert!(log.iter().any(|l| l.contains("xruns 0")), "{log:?}");
    }

    #[test]
    fn a_param_edit_crosses_the_command_ring_without_a_restage_and_the_level_follows() {
        let mut log = Vec::new();
        let (mut s, mut graph, reg) = session(&mut log);
        let sine = find(&graph, "sparq/syn/sine");
        s.pump_manual(4).unwrap();
        s.drain();
        let before = s.levels.get(sine);

        // Amp 0.5 → 0.125 through the model, then the door: param-dirty only, NOT structural.
        graph.op_set_param(sine, 1, 0.125).unwrap();
        let changes = PatchChanges { structural: false, param_nodes: vec![sine] };
        s.sync(&graph, &reg, s.staged_master, &changes, &mut log);
        s.pump_manual(4).unwrap();
        s.drain();
        let after = s.levels.get(sine);
        assert!(after < before / 2.0, "the level FOLLOWS the signal: {before} → {after}");
        assert!(
            !log.iter().any(|l| l.contains("re-staged")),
            "a param edit must NOT re-stage (no click): {log:?}"
        );

        s.stop("STOP", &mut log);
        assert!(log.iter().any(|l| l.contains("cmds 1 applied / 0 refused")), "{log:?}");
        assert!(log.iter().any(|l| l.contains("swaps 0")), "no boundary swap happened: {log:?}");
    }

    #[test]
    fn a_structural_edit_restages_at_the_boundary_and_the_clock_survives() {
        let mut log = Vec::new();
        let (mut s, mut graph, reg) = session(&mut log);
        let master = s.staged_master;
        s.pump_manual(4).unwrap();

        // Wire the bare out/main in (the demo's fourth node): a structural change.
        // util/gain's ports are in=0, out=1 — the wire runs OUTPUT → input.
        let out_main = find(&graph, "sparq/out/main");
        let gain = find(&graph, "sparq/util/gain");
        graph.op_add_wire(
            sparq_ui::canvas::model::PortRef::new(gain, 1),
            sparq_ui::canvas::model::PortRef::new(out_main, 0),
        );
        let changes = PatchChanges { structural: true, param_nodes: Vec::new() };
        s.sync(&graph, &reg, master, &changes, &mut log);
        assert!(log.iter().any(|l| l.contains("re-staged")), "{log:?}");

        s.pump_manual(4).unwrap();
        let stats = s.control.stats();
        assert!(stats.swap.swaps >= 1, "the boundary swap happened: {stats:?}");
        s.stop("STOP", &mut log);
        assert!(log.iter().any(|l| l.starts_with("STOP ·") && l.contains("8 blocks")), "{log:?}");
    }

    #[test]
    fn an_unplugged_device_ends_the_session_in_words_and_the_stop_still_reports() {
        let mut log = Vec::new();
        let (mut s, _graph, _reg) = session(&mut log);
        s.pump_manual(2).unwrap();
        s.inject_fault(Fault::Unplug).unwrap();
        // The fault is observed by the PUMP: the next step flips the state and refuses to run.
        let pumped = s.pump_manual(1);
        assert!(
            pumped.is_err() || pumped == Ok(0),
            "the unplugged pump does not produce: {pumped:?}"
        );
        assert_eq!(s.health(), StreamState::Removed, "the mirror reports what happened");
        s.stop("PANIC", &mut log);
        assert!(
            log.iter().any(|l| l.starts_with("PANIC ·") && l.contains("Removed")),
            "the final state is in the evidence line: {log:?}"
        );
    }

    #[test]
    fn a_master_handover_restages_even_with_no_ops_pending() {
        let mut log = Vec::new();
        let (mut s, graph, reg) = session(&mut log);
        s.pump_manual(2).unwrap();
        let out_main = find(&graph, "sparq/out/main");
        let empty = PatchChanges::default();
        s.sync(&graph, &reg, out_main, &empty, &mut log);
        assert!(
            log.iter().any(|l| l.contains("re-staged")),
            "a master handover is structural: {log:?}"
        );
        assert_eq!(s.staged_master, out_main);
        s.pump_manual(2).unwrap();
        s.stop("STOP", &mut log);
    }

    #[test]
    fn two_sessions_run_back_to_back_and_a_dropped_session_still_releases_the_device() {
        // The worker thread is per-session: the second open proves the first dropped clean
        // (no leaked thread, no leaked device), and a session dropped WITHOUT stop winds
        // itself down through Drop — a session that leaked its thread would leak the device.
        let mut log = Vec::new();
        let (s, _graph, _reg) = session(&mut log);
        s.stop("STOP", &mut log);
        let (s2, _g2, _r2) = session(&mut log);
        assert_eq!(s2.health(), StreamState::Running);
        drop(s2); // no stop: Drop sends Stop and joins
        let (s3, _g3, _r3) = session(&mut log);
        assert_eq!(s3.health(), StreamState::Running);
        s3.stop("STOP", &mut log);
    }

    #[test]
    fn the_out_main_node_publishes_stereo_peaks_for_the_meter_bars() {
        // Increment 4, D3: the meter ring carries per-channel peaks; a stereo port's bars read
        // L and R separately, and a MONO port pins peak_l == peak_r == peak (the additive claim).
        let mut log = Vec::new();
        let reg = registry();
        let mut graph = crate::bridge::demo_graph(&reg).unwrap();
        let gain = find(&graph, "sparq/util/gain");
        let out_main = find(&graph, "sparq/out/main");
        graph.op_add_wire(
            sparq_ui::canvas::model::PortRef::new(gain, 1),
            sparq_ui::canvas::model::PortRef::new(out_main, 0),
        );
        let mut s = LiveSession::start(&graph, out_main, &reg, manual_opts(0), &mut log).unwrap();
        s.sync(&graph, &reg, out_main, &PatchChanges::default(), &mut log);
        s.pump_manual(8).unwrap();
        s.drain();
        let out_port = 1usize; // out/main's ports are in=0, out=1
        let m = s.meters().get(&(out_main, out_port)).expect("the master's stereo meter");
        assert!(m.l > 0.3 && m.r > 0.3, "both channels meter the demo sine: {:?}", m);
        assert!(m.hold_l >= m.l && m.hold_r >= m.r, "the hold never sits below the live peak");
        // The mono pin: the sine's single-channel port duplicates.
        let sine = find(&graph, "sparq/syn/sine");
        let mono = s.meters().get(&(sine, 0)).expect("the sine's meter");
        assert_eq!(mono.l, mono.r, "a mono port duplicates its channel");
        assert!(mono.l > 0.3, "and it is the signal: {:?}", mono);
        s.stop("STOP", &mut log);
    }

    /// A scope rig: sine → gain (master), gain → tap, tap.wave → scope.x, plus a SPARE scope
    /// with no wires (the unbound case).
    fn scope_world(
        reg: &Registry,
    ) -> (CanvasGraph, CanvasNodeId, CanvasNodeId, CanvasNodeId, CanvasNodeId) {
        use sparq_ui::canvas::model::{NodeSpec, Op, PortRef};
        use sparq_ui::geom::Vec2 as SpVec2;
        let mut g = CanvasGraph::new();
        let spec = |id: &str| NodeSpec::from_manifest(reg.get(id).unwrap().manifest());
        let nid = |op: Op| match op {
            Op::AddNode(n) => n.id,
            _ => panic!("spawn failed"),
        };
        let sine = nid(g.op_add_node(spec("sparq/syn/sine"), SpVec2::new(0.0, 0.0)));
        let gain = nid(g.op_add_node(spec("sparq/util/gain"), SpVec2::new(300.0, 0.0)));
        let tap = nid(g.op_add_node(spec("sparq/ana/tap"), SpVec2::new(600.0, 0.0)));
        let scope = nid(g.op_add_node(spec("sparq/dsp/scope"), SpVec2::new(900.0, 0.0)));
        let spare = nid(g.op_add_node(spec("sparq/dsp/scope"), SpVec2::new(900.0, 300.0)));
        g.op_add_wire(PortRef::new(sine, 0), PortRef::new(gain, 0));
        g.op_add_wire(PortRef::new(gain, 1), PortRef::new(tap, 0));
        g.op_add_wire(PortRef::new(tap, 1), PortRef::new(scope, 0));
        (g, gain, tap, scope, spare)
    }

    #[test]
    fn scope_traces_fill_from_the_bound_source_and_an_unbound_scope_stays_flat() {
        let mut log = Vec::new();
        let reg = registry();
        let (graph, master, _tap, scope, spare) = scope_world(&reg);
        let mut s = LiveSession::start(&graph, master, &reg, manual_opts(0), &mut log).unwrap();
        s.sync(&graph, &reg, master, &PatchChanges::default(), &mut log);
        s.pump_manual(8).unwrap();
        s.drain();

        let t = s.traces.get(scope).expect("the bound scope has a trace");
        assert!(!t.x.is_empty(), "the trace accumulated the published waveforms");
        assert!(
            t.x.samples().iter().any(|v| *v > 0.0) && t.x.samples().iter().any(|v| *v < 0.0),
            "it is the SIGNED sine — a real waveform, not an envelope"
        );
        let peak = t.x.peak();
        assert!((0.3..=0.51).contains(&peak), "the 0.5-amp demo sine, at tap gain 1.0: {peak}");
        assert!(t.y.is_empty(), "waveform mode feeds x only; y stays at rest");
        let u =
            s.traces.get(spare).expect("the spare is ensured (an empty trace, not a missing one)");
        assert!(u.x.is_empty() && u.y.is_empty(), "the unbound scope shows the rest line");
        s.stop("STOP", &mut log);
    }

    #[test]
    fn a_rebind_resets_the_traces_so_no_dead_tail_survives() {
        let mut log = Vec::new();
        let reg = registry();
        let (mut graph, master, tap, scope, spare) = scope_world(&reg);
        let mut s = LiveSession::start(&graph, master, &reg, manual_opts(0), &mut log).unwrap();
        s.sync(&graph, &reg, master, &PatchChanges::default(), &mut log);
        s.pump_manual(4).unwrap();
        s.drain();
        assert!(!s.traces.get(scope).unwrap().x.is_empty(), "fed before the rebind");

        use sparq_ui::canvas::model::PortRef;
        graph.op_add_wire(PortRef::new(tap, 1), PortRef::new(spare, 0));
        let changes = PatchChanges { structural: true, param_nodes: Vec::new() };
        s.sync(&graph, &reg, master, &changes, &mut log);
        assert!(
            s.traces.get(scope).map(|t| t.x.is_empty()).unwrap_or(false),
            "the rebind cleared the old tail"
        );
        s.pump_manual(4).unwrap();
        s.drain();
        assert!(
            !s.traces.get(scope).unwrap().x.is_empty()
                && !s.traces.get(spare).unwrap().x.is_empty(),
            "both scopes now read the same source, from the rebind onward"
        );
        s.stop("STOP", &mut log);
    }

    #[test]
    fn the_scope_param_order_is_the_manifests_own() {
        let reg = registry();
        let m = reg.get("sparq/dsp/scope").unwrap().manifest().manifest();
        let ids: Vec<&str> = m.params.iter().map(|p| p.id.as_deref().unwrap_or("")).collect();
        assert_eq!(ids, ["timebase", "mode", "trigger", "gain", "colormap"]);
    }
}
