//! The instrument launch registration + the display adapter (WO-020 INC6 S4, plan D17a/D17c).
//!
//! **DEVICE-FIRST-COMPILE cell** — the §8 wall stands: anything pulling wasmtime OOMs the 1 GB
//! sandbox in `cranelift-assembler-x64`, so this module is WRITTEN against the vendored runtime
//! API (`runtime::load`'s `InstrumentRuntime`/`LoadContext`/`LoadedInstrument`/`GuestInstance`/
//! `WasmInstrument`, `runtime::convert`'s D13 interchange writer, `package`'s discovery) and
//! DESK-CHECKED line by line against INC5's own call idioms in `runtime/stages.rs` — the same
//! `budget_call(…, |s, g| g.call_draw(s, &frame))` lifecycle, never a second call path. Its
//! first real compile is on SATURN, and `scripts/test008.bat` is the run sheet. The sandbox
//! proves the seams this module FEEDS: the `LiveDisplay` trait, the D19 pacer, the skip/bypass
//! counter and the band switch all run against scripted mocks in the headless smoke — the
//! trait boundary is where the desk-check ends and the measurement begins.
//!
//! What the launch does (D17a, the plan's own order):
//!
//! 1. **The engine:** `InstrumentRuntime::new` — wasmtime with fuel + epoch + the component
//!    model and its compile cache (INC5's loader; the device already compiled it green in the
//!    session-4..8 runs).
//! 2. **The provider behind the sources door:** the live broker's read face when the driver
//!    thread runs (S4's swap — D17b), else the replay fixtures (at rest, honestly). No
//!    provider at all → the registration REFUSES in words: a sources-blind instrument would
//!    draw holes as data.
//! 3. **Per discovered loadable package:** ONE launch load → the manifest, the caps and the
//!    two-instance split (§2.2): the DISPLAY half parks on the shell's pending shelf (the tick
//!    attaches it to the module's first canvas node — the UI thread owns it, D17c), and the
//!    AUDIO half registers through the registry's state-capturing factory door
//!    (instrument-host.md §2.5) — a reload per instantiation, because a wasm `Store` has one
//!    owner and a factory is `Fn() -> Box<dyn Module>`. The executor then adopts it like any
//!    module and PLAY's refusal (#58) lifts STRUCTURALLY — the defect's own rule: the canvas
//!    never shows a module the registry does not have, and now the registry has it. A package
//!    that refuses to load is WORDS in the log and an absent browser row, never a
//!    half-instrument.

use std::path::Path;
use std::sync::Arc;

use sparq_host_wasm::package;
use sparq_host_wasm::runtime::convert;
use sparq_host_wasm::runtime::load::{GuestInstance, InstrumentRuntime, LoadContext};
use sparq_host_wasm::sources::{BrokerProvider, StreamProvider};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::registry::SharedFactory;

use crate::ui::live_display::{DisplayFrame, DisplayLod, LiveDisplay};
use crate::ui::shell_ui::ShellUi;

/// The display instance riding the shell's seam (D17c): the runtime's display `GuestInstance`
/// behind [`LiveDisplay`] — `budget_call` at the declared `max_fuel` per draw (§3's watchdog
/// arithmetic: fuel and epoch traps classify into their own sentences, `trap_words`, and the
/// pacer counts those Errs — five consecutive bypass THIS instance only; the audio instance is
/// untouched, decision D's isolation), then the WIT `surface` through the D13 interchange into
/// the SAME parser the at-rest shelf reads (one interchange, two producers — stage 4's
/// bit-exactness property, now on the live path).
pub struct RuntimeDisplay {
    inst: GuestInstance,
    max_fuel: u64,
    display_id: String,
}

impl RuntimeDisplay {
    /// Wraps a loaded display instance. `display_id` is the manifest's first declared display
    /// (v0 draws `ui.displays[0]` — the Observatory's `wall`).
    #[must_use]
    pub fn new(inst: GuestInstance, max_fuel: u64, display_id: String) -> Self {
        Self { inst, max_fuel, display_id }
    }
}

impl LiveDisplay for RuntimeDisplay {
    fn draw(&mut self, frame: &DisplayFrame) -> Result<Vec<sparq_ui::displaylist::Item>, String> {
        use sparq_host_wasm::runtime::bindings::sparq::instrument::display as wit;
        // The host-side frame becomes the contract's FrameContext — field for field (the
        // mirror is why the pacer's seam type exists: the sandbox proves the values, the
        // device proves the boundary).
        let wit_frame = wit::FrameContext {
            display_id: frame.display_id.clone(),
            width_px: frame.width_px,
            height_px: frame.height_px,
            lod: match frame.lod {
                DisplayLod::Full => wit::Lod::Full,
                DisplayLod::Simplified => wit::Lod::Simplified,
                DisplayLod::Dot => wit::Lod::Dot,
            },
            time_sec: frame.time_sec,
        };
        // The metered draw — stages.rs's own idiom: fuel set before, burned measured after,
        // traps classified into the watchdog's words. The fuel count is dropped here (the
        // gate's stages record it; the shell's honesty is the skip count, not the number).
        let budget = self.max_fuel;
        let (surface, _fuel) = self.inst.budget_call(budget, |s, g| g.call_draw(s, &wit_frame))?;
        // D13's interchange: the surface becomes the same JSON the harness writes, and the
        // at-rest parser reads it back — the live shelf and the at-rest shelf speak one tongue.
        let json =
            convert::surface_to_ir_json(&surface, frame.width_px as f32, frame.height_px as f32);
        let text = convert::ir_json_pretty(&json)?;
        let parsed =
            sparq_ui::json::parse(&text).map_err(|e| format!("the interchange refused: {e}"))?;
        sparq_ui::displaylist::parse_surface(&parsed)
    }

    fn display_id(&self) -> &str {
        &self.display_id
    }
}

/// The factory's refusal module: what a RELOAD failure after a successful launch load becomes
/// (a compile-cache eviction, a disk fault — the exceptions, not the path). It is the §6.3
/// shape on the executor's side: silence substituted, `Failed` flagged, the words live in the
/// launch log — never a stub that pretends to make sound.
struct LoadRefused {
    id: String,
}

impl Module for LoadRefused {
    fn id(&self) -> &str {
        &self.id
    }
    fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        for s in ctx.output().iter_mut() {
            *s = 0.0;
        }
        BlockStatus::Failed
    }
    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message(
            "the package refused to reload — the launch log carries the words",
        ))
    }
}

/// The manifest's first declared display id (`ui.displays[0].id`), or `wall` when the text
/// does not parse one out — the same tolerant read the launch door's own words cover (a
/// package whose manifest does not decode never reaches this function: `load_package`
/// cross-checks the identity first).
fn first_display_id(manifest_text: &str) -> String {
    use sparq_module_api::toml::Value;
    let Ok(root) = sparq_module_api::toml::parse(manifest_text) else {
        return "wall".to_string();
    };
    root.get("ui")
        .and_then(Value::as_table)
        .and_then(|ui| ui.tables("displays"))
        .and_then(|ds| ds.into_iter().next())
        .and_then(|d| d.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("wall")
        .to_string()
}

/// The launch registration (D17a) — called by the WINDOW host after `start_live_streams`
/// (the provider precedence wants the driver up first). Every refusal is a log line with its
/// path and words; the shell never dies here (§6.3), and an instrument that did not register
/// simply has no browser row and no PLAY — the #58 rule, structurally.
pub fn register_discovered(shell: &mut ShellUi) {
    // The provider behind the sources door: the live broker's read face when the driver runs,
    // else the replay fixtures. Computed before any &mut shell work (one borrow at a time).
    #[cfg(feature = "streams-net")]
    let broker_shared = shell.streams_driver.as_ref().map(|d| d.broker());
    #[cfg(not(feature = "streams-net"))]
    let broker_shared: Option<sparq_host_wasm::sources::SharedBroker> = None;
    let provider: Arc<dyn StreamProvider + Send + Sync> = match broker_shared
        .map(|b| BrokerProvider::new(b))
    {
        Some(Ok(bp)) => Arc::new(bp),
        Some(Err(words)) => {
            shell.push_log(format!(
                "instruments: the broker provider refused ({words}) — falling back to the fixtures, in words"
            ));
            match shell.replay.clone() {
                Some(rp) => Arc::new(rp),
                None => {
                    shell.push_log(
                        "instruments: no stream provider at all (no driver, no fixtures) — the launch registration REFUSES: a sources-blind instrument would draw holes as data"
                            .to_string(),
                    );
                    return;
                },
            }
        },
        None => match shell.replay.clone() {
            Some(rp) => Arc::new(rp),
            None => {
                shell.push_log(
                    "instruments: no stream provider at all (no driver, no fixtures) — the launch registration REFUSES: a sources-blind instrument would draw holes as data"
                        .to_string(),
                );
                return;
            },
        },
    };
    // The evaluation clock served with the windows (injected — D11: the plane never reads a
    // wall clock it did not sanction; `fetch::now_unix` IS the sanctioned impure edge, and
    // without `streams-net` the replay's load-time clock is the honest one).
    #[cfg(feature = "streams-net")]
    let now_unix = sparq_streams::fetch::now_unix();
    #[cfg(not(feature = "streams-net"))]
    let now_unix = shell.replay.as_ref().map_or(0, |rp| rp.now());
    // The launch context: seed 0 and an empty node path (the session seed tree is the audio
    // session's — a spawn re-prepares through the executor's own negotiation, and test008's
    // determinism rows are the device's truth); 48 kHz / 256 are the demo's negotiated shape,
    // re-negotiated by `prepare` at PLAY.
    let ctx = LoadContext {
        provider: Arc::clone(&provider),
        now_unix,
        seed_root: 0,
        node_path: Vec::new(),
        sample_rate: 48_000,
        block_frames: 256,
    };
    let rt = match InstrumentRuntime::new(None) {
        Ok(rt) => Arc::new(rt),
        Err(words) => {
            shell.push_log(format!(
                "instruments: the runtime refused to build ({words}) — no instrument registers; the browser rows stay absent, in words"
            ));
            return;
        },
    };
    let mut registered = 0usize;
    for pkg in package::discover(Path::new("instruments")) {
        if let Some(w) = &pkg.io_words {
            shell.push_log(format!("instruments: {}: {w}", pkg.dir.display()));
        }
        let Some(text) = pkg.manifest_text.as_deref() else { continue };
        if let Err(report) = package::is_loadable(&pkg) {
            shell.push_log(format!(
                "instruments: {} is NOT LOADABLE ({report:?}) — an absent browser row, never a half-instrument",
                pkg.dir.display()
            ));
            continue;
        }
        match rt.load_package(&pkg.dir, &ctx) {
            Ok(loaded) => {
                let id = loaded.manifest.id().to_string();
                let max_fuel = loaded.caps.max_fuel;
                let display_id = first_display_id(text);
                let instantiate_fuel = loaded.instantiate_fuel;
                // §2.2's split: the display half to the UI's shelf (the tick attaches it to the
                // module's first node), the audio half… dropped HERE — this launch instance is
                // the load's proof; the FACTORY reloads per instantiation (a Store has one
                // owner, and the executor builds per PLAY). The cost is measured, not hidden:
                shell.push_log(format!(
                    "instruments: {id} loaded — display instance parked (instantiate {instantiate_fuel} fuel); the factory reloads per spawn"
                ));
                let (_audio_proof, display_inst) = loaded.split();
                shell.pending_displays.insert(
                    id.clone(),
                    Box::new(RuntimeDisplay::new(display_inst, max_fuel, display_id)),
                );
                // The registry's instrument door (§2.5): a state-capturing factory. A reload
                // failure at instantiation time becomes LoadRefused — Failed blocks and the
                // launch log's words, never a silent stub.
                let rt2 = Arc::clone(&rt);
                let dir2 = pkg.dir.clone();
                let ctx2 = ctx.clone();
                let id2 = id.clone();
                let factory: SharedFactory = Arc::new(move || {
                    match rt2.load_package(&dir2, &ctx2) {
                        Ok(l) => {
                            let (audio, _display) = l.split();
                            Box::new(audio)
                        },
                        Err(_) => Box::new(LoadRefused { id: id2.clone() }),
                    }
                });
                match shell.modules.register_instrument(text, factory) {
                    Ok(()) => {
                        registered += 1;
                        shell.push_log(format!(
                            "instruments: {id} REGISTERED — the executor adopts it like any module (PLAY's #58 refusal lifts for the port-less instrument)"
                        ));
                    },
                    Err(e) => shell.push_log(format!(
                        "instruments: {id} did not register ({e:?}) — an absent browser row, in words"
                    )),
                }
            },
            Err(words) => shell.push_log(format!(
                "instruments: {} REFUSED to load ({words}) — WORDS in the log and an absent browser row, never a half-instrument",
                pkg.dir.display()
            )),
        }
    }
    shell.push_log(format!(
        "instruments: launch registration done — {registered} package(s) joined the registry"
    ));
}
