//! The loader: engine, compile cache, the two-instance pair, fuel/epoch, and the `Module` face.
//!
//! instrument-host.md §2 as code. One [`InstrumentRuntime`] per host (one `Engine`, one `Linker`
//! defining EXACTLY the four imported interfaces — capability by absence: a component that
//! imports anything else fails to instantiate, and stage 3 reports that failure as the capability
//! honour proof). Per package, [`InstrumentRuntime::load_package`] runs the §2 order: compile
//! (cached by wasmtime's own content-addressed cache — no `unsafe` in sparq code, the allowlist's
//! Tier-2 rule) → instantiate TWICE (decision D: audio + display, sharing nothing mutable) →
//! cross-check `guest.id()` against the manifest (§2.3 — a lying id never enters the graph) →
//! `prepare` before anything else, on both instances (§2.4 — a prepare that traps or refuses
//! refuses the LOAD, in words).
//!
//! Fuel and epoch map onto the EXISTING watchdog vocabulary (§3 — never a second mechanism):
//! a `process` that exhausts its per-block budget traps `OutOfFuel`, which
//! [`WasmInstrument::process`] answers as `BlockStatus::Overrun` — the executor's auto-bypass
//! with words in the UI, the set continues. The memory limiter is wasmtime's own
//! ([`wasmtime::StoreLimits`]), built from `[capabilities] max_memory_mb`, so an under-declaring
//! guest gets a trap the watchdog turns into words, not a host OOM.
//!
//! v1's honest limits (refused in words at LOAD, never half-working): ported instruments (the
//! audio-path conversion lands with the first ported instrument and the Phase-5 data-port
//! arbiter). The port-less `process` is real: the exact negotiated empty shapes, checked both
//! ways, and its marshalling is bounded (the param snapshot and empty port lists) — the
//! zero-allocation audio path the native modules prove is wasmtime's arena work, a recorded
//! LATER, not a silent gap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wasmtime::component::HasSelf;
use wasmtime::{Engine, Store};

use super::bindings::{self, Instrument};
use super::convert;
use super::doors::InstrumentHost;
use super::{ClockReading, Phase};
use crate::package::{self, Package};
use crate::sources::{ManifestSources, StreamProvider};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;

/// `prepare` gets a larger budget than a block: it is where ALL guest allocation happens (the
/// wall parses its embedded coastline, the SDK parses the token bundle), once per load, on the
/// control thread. 64 × the per-block budget is the declared rule — generous enough that an
/// honest prepare never trips it, small enough that a runaway prepare is refused at load (§2.4)
/// rather than at first draw. `configure`/`message` ride the same control-thread budget.
pub const PREPARE_FUEL_MULTIPLIER: u64 = 64;

/// The per-store epoch deadline in watchdog ticks (§3's cheap insurance). Nobody increments the
/// epoch during a gate run, so it never fires there; the production incrementer lands with the
/// INC5b watchdog wiring, where the tick count × the watchdog period IS the deadline.
pub const EPOCH_DEADLINE_TICKS: u64 = 1_000;

/// The static sentence the native `ModuleError` carries when the guest refused (the native
/// contract's error type holds `&'static str`; the guest's OWN words are captured in the
/// instrument's diagnostics and the load/validate reports — leaked per-call strings would be
/// unbounded growth on the control thread, and a static pointer with the words captured beside
/// it loses nothing the operator can read).
pub const GUEST_REFUSED: &str =
    "the wasm guest refused in words — its sentence is in the instrument's diagnostics \
     (the load words / `sparq mod validate` report / the shell's log)";
/// The static sentence for a trap (fuel, epoch, or a wasm fault).
pub const GUEST_TRAPPED: &str =
    "the wasm guest trapped — the sentence (fuel/epoch/fault) is in the instrument's diagnostics";

/// What a load needs from the world: the stream provider behind the sources door, the evaluation
/// clock (injected — D11), the seed tree's root and this node's path (ADR-007), and the audio
/// shape `prepare` is negotiated against.
#[derive(Clone)]
pub struct LoadContext {
    /// The stream provider (Replay in gates, Broker on the device).
    pub provider: Arc<dyn StreamProvider + Send + Sync>,
    /// The freshness clock served with the windows (injected, never read).
    pub now_unix: i64,
    /// The session seed tree's root.
    pub seed_root: u64,
    /// This node's graph path (seed namespace).
    pub node_path: Vec<String>,
    /// Negotiated sample rate.
    pub sample_rate: u32,
    /// Negotiated block frames.
    pub block_frames: u32,
}

/// One instance and its store — the unit the §4 ordering moves. Not `Sync` (one owner thread);
/// `Send` (the audio instance is BUILT on the control thread and handed to the audio thread —
/// the executor's existing adoption path).
pub struct GuestInstance {
    /// The store (doors' state, fuel, limiter).
    pub store: Store<InstrumentHost>,
    /// The instantiated guest exports.
    pub inst: Instrument,
}

/// The generated guest export handle (the call surface both instances share; `Clone` is a
/// handle clone — the typed function indices — not a guest copy).
type GuestExports<'a> = &'a bindings::exports::sparq::instrument::guest::Guest;

impl GuestInstance {
    /// One metered call: fuel set to `budget` before, burned measured after, traps classified
    /// into the watchdog's words (§3). The returned fuel count is the golden report's
    /// determinism evidence — a fuel number that moves without a wasm hash change is a defect.
    /// Public because the gate's stages drive the same lifecycle the executor
    /// does — a stage that used a second call path would measure something else.
    pub fn budget_call<R>(
        &mut self,
        budget: u64,
        call: impl FnOnce(&mut Store<InstrumentHost>, GuestExports<'_>) -> wasmtime::Result<R>,
    ) -> Result<(R, u64), String> {
        self.store.set_fuel(budget).map_err(|e| format!("fuel: {e}"))?;
        let guest = self.inst.sparq_instrument_guest().clone();
        let out = call(&mut self.store, &guest);
        let left = self.store.get_fuel().unwrap_or(0);
        let burned = budget.saturating_sub(left);
        match out {
            Ok(r) => Ok((r, burned)),
            Err(e) => Err(trap_words(&e, budget, burned)),
        }
    }

    /// The diagnostics captured through the doors (logs + RT violations), drained.
    pub fn take_diagnostics(&mut self) -> Vec<String> {
        let host = self.store.data_mut();
        let mut out = std::mem::take(&mut host.violations);
        for (level, msg) in std::mem::take(&mut host.logs) {
            out.push(format!("guest log ({level}): {msg}"));
        }
        out
    }
}

/// Classifies a wasmtime failure into the watchdog's words (§3's mapping table): fuel and epoch
/// get their own sentences (they drive Overrun), everything else prints verbatim (a trap
/// backtrace from a component built with its name section is actionable — the reason the
/// release profile keeps it).
#[must_use]
pub fn trap_words(err: &wasmtime::Error, budget: u64, burned: u64) -> String {
    if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
        match trap {
            wasmtime::Trap::OutOfFuel => format!(
                "fuel budget exhausted ({burned} of {budget} burned) — the guest overran its \
                 declared max_fuel; the watchdog auto-bypasses with these words (guide §6)"
            ),
            wasmtime::Trap::Interrupt => format!(
                "epoch deadline exceeded (budget {budget}) — the watchdog's tick interrupted the \
                 call; treated as an overrun (instrument-host §3)"
            ),
            other => format!("wasm trap: {other} — {err}"),
        }
    } else {
        format!("wasm call failed: {err}")
    }
}

/// The host's runtime: one engine (fuel + epoch + component model + the compile cache), one
/// linker (the four doors and NOTHING else), and the loads it produced.
pub struct InstrumentRuntime {
    engine: Engine,
    linker: wasmtime::component::Linker<InstrumentHost>,
    /// The cache directory in use (`None` = caching off — every launch recompiles).
    pub cache_dir: Option<PathBuf>,
}

impl InstrumentRuntime {
    /// Builds the engine and linker. `cache_dir`: wasmtime's content-addressed compile cache
    /// (§2.1) — a component whose bytes are unchanged loads without recompiling.
    ///
    /// # Errors
    /// A sentence naming the config/engine/linker failure.
    pub fn new(cache_dir: Option<PathBuf>) -> Result<Self, String> {
        let mut cfg = wasmtime::Config::new();
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        if let Some(dir) = &cache_dir {
            let mut cc = wasmtime::CacheConfig::new();
            cc.with_directory(dir);
            let cache = wasmtime::Cache::new(cc).map_err(|e| format!("wasm cache: {e}"))?;
            cfg.cache(Some(cache));
        }
        // No WASI, no `wasi:*` preopens, nothing else: capability by absence (decision E).
        let engine = Engine::new(&cfg).map_err(|e| format!("engine: {e}"))?;
        let mut linker = wasmtime::component::Linker::new(&engine);
        Instrument::add_to_linker::<InstrumentHost, HasSelf<InstrumentHost>>(&mut linker, |s| s)
            .map_err(|e| format!("linker: {e}"))?;
        Ok(Self { engine, linker, cache_dir })
    }

    /// The engine (the epoch watchdog thread increments it — §3's cheap insurance).
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Compiles one component's bytes (the cache makes a repeat launch cheap).
    ///
    /// # Errors
    /// A sentence naming the compile failure (a malformed component says so here, at load,
    /// never at first draw).
    pub fn compile(&self, bytes: &[u8]) -> Result<wasmtime::component::Component, String> {
        wasmtime::component::Component::new(&self.engine, bytes)
            .map_err(|e| format!("component compile: {e}"))
    }

    /// Loads one package directory through the full §2 order.
    ///
    /// # Errors
    /// Words — every refusal names the file/field/value and the fix: an unreadable package, a
    /// manifest that does not decode, a non-instrument layer, a ported instrument (v1's honest
    /// limit, named), a missing component, a compile failure, an identity disagreement (§2.3),
    /// or a `prepare` that traps or refuses (§2.4).
    pub fn load_package(&self, dir: &Path, ctx: &LoadContext) -> Result<LoadedInstrument, String> {
        let pkg = package::open(dir);
        if let Err(report) = package::is_loadable(&pkg) {
            return Err(format!("package at {} is not loadable:\n{report}", dir.display()));
        }
        let manifest_text = pkg
            .manifest_text
            .clone()
            .ok_or_else(|| format!("{}: no readable sparqmod.toml", dir.display()))?;
        let manifest = sparq_module_api::decode(&manifest_text)
            .map_err(|r| format!("the manifest does not validate:\n{r}"))?;
        self.load_validated(&pkg, manifest_text, manifest, ctx)
    }

    /// [`load_package`] for an already-decoded manifest (the app's launch path decodes once for
    /// the registry and hands the same validation here — one decode, two consumers).
    ///
    /// # Errors
    /// As [`load_package`], minus the decode.
    pub fn load_validated(
        &self,
        pkg: &Package,
        manifest_text: String,
        manifest: sparq_module_api::manifest::ValidatedManifest,
        ctx: &LoadContext,
    ) -> Result<LoadedInstrument, String> {
        // The layer rule: this loader is the instrument tier's door (ADR-010). A backbone module
        // is native by the admission rule; its package has no business here.
        let layer = manifest.manifest().classification.layer.as_deref().unwrap_or("backbone");
        if layer != "instrument" {
            return Err(format!(
                "`{}` declares classification.layer = `{layer}` — only `instrument` packages load \
                 through the wasm host (ADR-010's admission rule: backbone is native); fix: this \
                 package belongs in modules/, not instruments/",
                manifest.id()
            ));
        }
        // v1's honest limit: the port-less instrument path is the one that exists, is tested and
        // ships (plan D5 — the Observatory is port-less). A ported instrument refuses AT LOAD,
        // in words, rather than mis-rendering an audio path nobody has proven.
        if !manifest.ports().is_empty() {
            return Err(format!(
                "`{}` declares {} port(s) — the v1 loader serves PORT-LESS instruments (the \
                 audio-path conversion lands with the first ported instrument and the Phase-5 \
                 data-port arbiter); fix: ship port-less, or wait for the ported loader — never \
                 a silent half-path",
                manifest.id(),
                manifest.ports().len()
            ));
        }
        let caps = capabilities_of(&manifest_text);
        let wasm_path =
            pkg.inventory.get(package::Role::Component).map(|f| pkg.dir.join(&f.name)).ok_or_else(
                || format!("{}: no *.wasm component in the package", pkg.dir.display()),
            )?;
        let bytes = super::read_bytes(&wasm_path)?;
        let component_sha = convert::sha256_hex(&bytes);
        let component = self.compile(&bytes)?;
        let pre =
            self.linker.instantiate_pre(&component).map_err(|e| format!("instantiate_pre: {e}"))?;
        let pre = bindings::InstrumentPre::new(pre).map_err(|e| {
            format!(
                "the component's exports do not match the frozen WIT (sparq:instrument/guest@1.1.0 \
                 expected): {e}"
            )
        })?;

        let bindings_table = ManifestSources::parse(&manifest_text)
            .map_err(|e| format!("manifest source bindings: {e}"))?;
        let defaults = convert::params_defaults_from_manifest(&manifest_text)?;
        let n_params = defaults.len();
        let declared_assets = declared_asset_hashes(&manifest_text);
        let params = ParamSet::new(1, &defaults)
            .ok_or_else(|| format!("{} params exceed MAX_PARAMS (32)", defaults.len()))?;

        let (mut audio, instantiate_fuel) =
            self.spawn(&pre, ctx, caps, &bindings_table, params, false, &declared_assets)?;
        let (mut display, display_instantiate_fuel) =
            self.spawn(&pre, ctx, caps, &bindings_table, params, true, &declared_assets)?;

        // §2.3 — identity BEFORE anything else: a lying id never enters the graph. The call
        // rides the CONTROL budget, not the per-block one: the first export call into a fresh
        // instance also pays the guest's lazy initialisation (the SDK builds its state on first
        // call — device session 6 measured `id()` alone exhausting the 2 M per-block budget).
        // The per-block budget is process's contract and nothing else's.
        let (said, _) = audio
            .budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |store, guest| {
                guest.call_id(store)
            })
            .map_err(|e| format!("guest.id() call failed: {e}"))?;
        if said != manifest.id() {
            return Err(format!(
                "the component says its id is `{said}` but the manifest declares `{}` — a module \
                 that lies about who it is breaks patch provenance silently; the load is refused \
                 (instrument-host §2.3). Fix: rebuild the component from the source that matches \
                 the manifest",
                manifest.id()
            ));
        }

        // §2.4 — prepare before anything else, on BOTH instances (each allocates its own).
        let native_res = portless_resources(ctx);
        let wit_res = convert::resources_to_wit(
            &native_res,
            caps.max_fuel,
            &audio.store.data().bundle.version,
        );
        let prepare_budget = caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER);
        let prepare_fuel = prepare_once(&mut audio, &wit_res, prepare_budget)
            .map_err(|e| format!("prepare (audio instance) refused the load: {e}"))?;
        let display_prepare_fuel = prepare_once(&mut display, &wit_res, prepare_budget)
            .map_err(|e| format!("prepare (display instance) refused the load: {e}"))?;

        Ok(LoadedInstrument {
            audio,
            display,
            manifest,
            manifest_text,
            caps,
            n_params,
            component_sha,
            instantiate_fuel,
            display_instantiate_fuel,
            prepare_fuel,
            display_prepare_fuel,
        })
    }

    fn spawn(
        &self,
        pre: &bindings::InstrumentPre<InstrumentHost>,
        ctx: &LoadContext,
        caps: Caps,
        bindings_table: &ManifestSources,
        params: ParamSet,
        sources_open: bool,
        declared_assets: &[String],
    ) -> Result<(GuestInstance, u64), String> {
        let mut host = InstrumentHost::new(
            caps.max_memory_mb,
            Arc::clone(&ctx.provider),
            bindings_table.clone(),
            params,
            ctx.now_unix,
            sources_open,
        )?;
        host.declared_assets = declared_assets.to_vec();
        host.seed_root = ctx.seed_root;
        host.node_path = ctx.node_path.clone();
        let mut store = Store::new(&self.engine, host);
        store.epoch_deadline_trap();
        // `epoch_deadline_trap()` arms the trap but leaves the deadline at ZERO — the vendored
        // doc's own warning: "it's required to call Store::set_epoch_deadline or otherwise wasm
        // will always immediately trap". Device session 5 measured exactly that (instantiation
        // is the first wasm to run). The tick budget is generous: the stages have no epoch
        // incrementer at all, and in production the watchdog thread's cadence (INC5b) turns
        // these ticks into the §3 wall-clock deadline.
        store.set_epoch_deadline(EPOCH_DEADLINE_TICKS);
        // consume_fuel(true) starts the store at ZERO fuel, and instantiation itself executes
        // wasm (the component's start sections and lowering trampolines) — fuel the store
        // BEFORE instantiate or the load traps with "all fuel consumed" before the guest ever
        // runs. Device session 4 found exactly that; the budget is prepare's (instantiation is
        // load-time work, and every metered call re-sets its own budget afterwards).
        let instantiate_budget = caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER);
        store.set_fuel(instantiate_budget).map_err(|e| format!("fuel: {e}"))?;
        // The memory limiter is wasmtime's own: the declared MB, enforced at the allocation
        // site — an under-declaring guest traps (→ the watchdog's words), never a host OOM.
        store.limiter(|host: &mut InstrumentHost| -> &mut dyn wasmtime::ResourceLimiter {
            &mut host.limits
        });
        let inst = match pre.instantiate(&mut store) {
            Ok(inst) => inst,
            Err(e) => {
                let burned = instantiate_budget.saturating_sub(store.get_fuel().unwrap_or(0));
                let words = trap_words(&e, instantiate_budget, burned);
                // A non-trap instantiate failure is the linker talking: the component imports
                // something the four doors do not provide — capability by absence, and THAT
                // failure is the capability honour proof (stage 3). Traps keep their own words
                // (session 4's lesson: do not blame imports for a fuel exhaustion).
                let capability = if e.downcast_ref::<wasmtime::Trap>().is_none() {
                    " — a component that imports anything beyond the four doors \
                     (tokens/host/assets/sources) cannot link: capability by absence, and this \
                     failure IS the capability honour proof (stage 3)"
                } else {
                    ""
                };
                return Err(format!("instantiate: {words}{capability}"));
            },
        };
        let instantiate_fuel = instantiate_budget.saturating_sub(store.get_fuel().unwrap_or(0));
        Ok((GuestInstance { store, inst }, instantiate_fuel))
    }
}

/// The port-less negotiation: zero ports every way, the ctx's rate and block (§2.4's resolved
/// channel COUNTS — for a port-less instrument that is the empty list, and the manifest's own
/// port table has already been checked empty above).
fn portless_resources(ctx: &LoadContext) -> Resources {
    Resources {
        sample_rate: ctx.sample_rate,
        block_frames: ctx.block_frames as usize,
        input_channels: 0,
        output_channels: 0,
        audio_in_channels: [0; sparq_module_api::module::MAX_PORTS_PER_CLASS],
        audio_in_count: 0,
        audio_out_channels: [0; sparq_module_api::module::MAX_PORTS_PER_CLASS],
        audio_out_count: 0,
        oversampling: sparq_module_api::module::Oversampling::None,
        voices: 0,
        arena_bytes: 0,
    }
}

/// prepare with its budget, mapping a guest refusal or a trap to load-refusing words.
fn prepare_once(
    inst: &mut GuestInstance,
    res: &bindings::sparq::instrument::types::Resources,
    budget: u64,
) -> Result<u64, String> {
    let (outcome, burned) =
        inst.budget_call(budget, |store, guest| guest.call_prepare(store, res))?;
    outcome.map_err(|e| guest_error_words("prepare", &e))?;
    Ok(burned)
}

/// A guest `module-error` as the host's words (the guest's own sentence, with the door named).
#[must_use]
pub fn guest_error_words(
    door: &str,
    e: &bindings::sparq::instrument::types::ModuleError,
) -> String {
    use bindings::sparq::instrument::types::ModuleError as E;
    let (kind, words) = match e {
        E::Unsupported(w) => ("unsupported", w),
        E::State(w) => ("state", w),
        E::Resources(w) => ("resources", w),
        E::Message(w) => ("message", w),
    };
    format!("the guest refused {door} ({kind}): {words}")
}

/// The `[capabilities]` knobs the sandbox enforces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    /// The per-block fuel budget (`resources.fuel-per-block` IS this, §2.4).
    pub max_fuel: u64,
    /// The memory ceiling in MB (wasmtime's limiter enforces it).
    pub max_memory_mb: u32,
}

fn capabilities_of(manifest_text: &str) -> Caps {
    // The defaults are the declared-budget stage's floor: stage 5 (static) has already refused a
    // package with no sane max_fuel/max_memory_mb, so a parse miss here means the gate ran in a
    // feature-off build — fall back to strict sane values rather than trusting.
    let parsed = sparq_module_api::toml::parse(manifest_text).ok();
    let caps = parsed
        .as_ref()
        .and_then(|root| root.get("capabilities"))
        .and_then(sparq_module_api::toml::Value::as_table);
    let fuel = caps
        .and_then(|c| c.get("max_fuel"))
        .and_then(sparq_module_api::toml::Value::as_i64)
        .filter(|&v| v > 0)
        .map_or(1_000_000, |v| v as u64);
    let mem = caps
        .and_then(|c| c.get("max_memory_mb"))
        .and_then(sparq_module_api::toml::Value::as_u32)
        .filter(|&v| v > 0)
        .unwrap_or(64);
    Caps { max_fuel: fuel, max_memory_mb: mem }
}

fn declared_asset_hashes(manifest_text: &str) -> Vec<String> {
    let parsed = sparq_module_api::toml::parse(manifest_text).ok();
    parsed
        .as_ref()
        .and_then(|root| root.get("state"))
        .and_then(sparq_module_api::toml::Value::as_table)
        .and_then(|st| st.get("assets"))
        .and_then(sparq_module_api::toml::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| match v {
                    sparq_module_api::toml::Value::Str(s) => Some(s.clone()),
                    sparq_module_api::toml::Value::Table(t) => t
                        .get("hash")
                        .and_then(sparq_module_api::toml::Value::as_str)
                        .map(str::to_string),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One loaded instrument: the two-instance pair (§2.2), its validated manifest, and the load's
/// measurements. [`LoadedInstrument::split`] hands the audio instance to the executor (as a
/// [`WasmInstrument`], the `Module` face) and the display instance to the shell/validator.
pub struct LoadedInstrument {
    /// The audio instance (control + audio threads; `sources` closed).
    pub audio: GuestInstance,
    /// The display instance (UI thread; `sources` open).
    pub display: GuestInstance,
    /// The validated manifest.
    pub manifest: sparq_module_api::manifest::ValidatedManifest,
    /// The manifest text (the param table and bindings were parsed from it).
    pub manifest_text: String,
    /// The declared caps.
    pub caps: Caps,
    /// How many params the manifest declares (the BlockInput mirror's slice length).
    pub n_params: usize,
    /// The component's sha256 (the cache key's evidence, the report's provenance line).
    pub component_sha: String,
    /// What instantiation itself burned (audio instance) — start sections and trampolines are
    /// wasm too, and the report records them (a load cost that is invisible is a load cost
    /// that surprises someone later).
    pub instantiate_fuel: u64,
    /// What instantiation burned on the display instance.
    pub display_instantiate_fuel: u64,
    /// What `prepare` burned on the audio instance (the report's first measurement).
    pub prepare_fuel: u64,
    /// What `prepare` burned on the display instance.
    pub display_prepare_fuel: u64,
}

impl LoadedInstrument {
    /// Splits the pair: the audio instance becomes the executor's `Module`, the display instance
    /// goes to whoever draws (the shell, the validator's stages).
    #[must_use]
    pub fn split(self) -> (WasmInstrument, GuestInstance) {
        let id = self.manifest.id().to_string();
        let bundle_version = self.audio.store.data().bundle.version.clone();
        (
            WasmInstrument {
                id,
                inner: self.audio,
                caps: self.caps,
                n_params: self.n_params,
                bundle_version,
                block_seq: 0,
                diagnostics: Vec::new(),
                fuel_last_block: 0,
                fuel_max_block: 0,
            },
            self.display,
        )
    }
}

/// The audio instance wearing the native [`Module`] face — what the executor adopts, so a wasm
/// instrument enters the graph exactly where a native module would (the registry's tier-blind
/// rule). Fuel trap → [`BlockStatus::Overrun`] (§3); a shape lie → `Failed` with words; the
/// guest's own refusals are captured in [`WasmInstrument::diagnostics`] (bounded).
pub struct WasmInstrument {
    id: String,
    inner: GuestInstance,
    caps: Caps,
    n_params: usize,
    bundle_version: bindings::sparq::instrument::types::Semver,
    block_seq: u64,
    /// Bounded guest words (the newest 64) — the UI/log face of refusals and traps.
    pub diagnostics: Vec<String>,
    /// Fuel burned by the most recent block (the report's per-block line).
    pub fuel_last_block: u64,
    /// The worst block so far (the report's headline number vs `max_fuel`).
    pub fuel_max_block: u64,
}

impl WasmInstrument {
    /// The bounded diagnostics note (control-thread; the audio path only bumps counters).
    fn note(&mut self, words: &str) {
        self.diagnostics.push(words.to_string());
        if self.diagnostics.len() > 64 {
            self.diagnostics.remove(0);
        }
    }

    /// The display instance's draw door lives on [`GuestInstance`]; stages and the shell call it
    /// there. This helper is the audio-side param mirror the §4 ordering publishes: the executor
    /// sets params through `process`, and the host-side mirror for the sources door rides the
    /// same snapshot.
    pub fn set_param_mirror(&mut self, values: &[f32]) {
        let n = values.len().min(sparq_module_api::params::MAX_PARAMS);
        if let Some(params) = ParamSet::new(1, &values[..n]) {
            self.inner.store.data_mut().params = params;
        }
    }
}

impl Module for WasmInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let budget = self.caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER);
        let r = self.inner.budget_call(budget, |store, guest| guest.call_configure(store, state));
        match r {
            Ok((Ok(()), _)) => Ok(()),
            Ok((Err(e), _)) => {
                self.note(&guest_error_words("configure", &e));
                Err(ModuleError::State(GUEST_REFUSED))
            },
            Err(words) => {
                self.note(&words);
                Err(ModuleError::State(GUEST_TRAPPED))
            },
        }
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        let wit = convert::resources_to_wit(resources, self.caps.max_fuel, &self.bundle_version);
        let budget = self.caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER);
        match prepare_once(&mut self.inner, &wit, budget) {
            Ok(_) => Ok(()),
            Err(words) => {
                self.note(&words);
                Err(ModuleError::Resources(GUEST_REFUSED))
            },
        }
    }

    fn activate(&mut self) -> Result<(), ModuleError> {
        let r =
            self.inner.budget_call(self.caps.max_fuel, |store, guest| guest.call_activate(store));
        match r {
            Ok((Ok(()), _)) => Ok(()),
            Ok((Err(e), _)) => {
                // activate refusals are captured, but the native contract's ModuleError has no
                // activate kind — Resources is the honest nearest (it refused to start).
                self.note(&guest_error_words("activate", &e));
                Err(ModuleError::Resources(GUEST_REFUSED))
            },
            Err(words) => {
                self.note(&words);
                Err(ModuleError::Resources(GUEST_TRAPPED))
            },
        }
    }

    fn deactivate(&mut self) {
        let _ =
            self.inner.budget_call(self.caps.max_fuel, |store, guest| guest.call_deactivate(store));
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        use bindings::sparq::instrument::audio as a;
        use bindings::sparq::instrument::types as t;
        // The RT face: phase + clock go in BEFORE the call, out after — the doors' violation
        // capture depends on the phase being honest for the call's duration.
        {
            let host = self.inner.store.data_mut();
            host.phase = Phase::Process;
            host.clock = ClockReading::from_block(ctx.block);
        }
        let n = self.n_params.min(sparq_module_api::params::MAX_PARAMS);
        let input = a::BlockInput {
            block_id: self.block_seq,
            frames: ctx.block.frames as u32,
            t_sample: ctx.block.sample_offset,
            tick: ctx.block.tick,
            ppqn: ctx.block.ppqn,
            params: t::ParamSet {
                version: ctx.params.version(),
                values: ctx.params.values()[..n].to_vec(),
            },
            audio_in: Vec::new(),
            cv_in: Vec::new(),
            events_in: Vec::new(),
            data_in: Vec::new(),
        };
        self.block_seq = self.block_seq.wrapping_add(1);
        let r = self
            .inner
            .budget_call(self.caps.max_fuel, |store, guest| guest.call_process(store, &input));
        self.inner.store.data_mut().phase = Phase::Control;
        match r {
            Ok((out, fuel)) => {
                self.fuel_last_block = fuel;
                self.fuel_max_block = self.fuel_max_block.max(fuel);
                // The negotiated shapes, checked: a port-less instrument's output is the exact
                // empty lists. A guest that grew ports mid-set is Failed with words, never a
                // silent reinterpretation.
                if !out.audio_out.is_empty()
                    || !out.cv_out.is_empty()
                    || !out.events_out.is_empty()
                    || !out.data_out.is_empty()
                {
                    self.note(
                        "the guest returned non-empty output shapes against a port-less \
                         negotiation — the contract is the exact negotiated shape (audio.wit); \
                         the executor substitutes silence",
                    );
                    return BlockStatus::Failed;
                }
                match out.status {
                    t::BlockStatus::Ok => BlockStatus::Ok,
                    t::BlockStatus::Silenced => BlockStatus::Silenced,
                    t::BlockStatus::Overrun => BlockStatus::Overrun,
                    t::BlockStatus::Failed => BlockStatus::Failed,
                }
            },
            Err(words) => {
                // The audio thread does not format strings into the diagnostics (allocation +
                // time); the fuel counters carry the measurement, and the watchdog's own words
                // come from the trap classification at the next control-thread read. The
                // Overrun/Failed split is the §3 mapping — string matching on OUR OWN sentences,
                // not the guest's.
                self.fuel_last_block = self.caps.max_fuel;
                self.fuel_max_block = self.fuel_max_block.max(self.caps.max_fuel);
                if words.starts_with("fuel budget exhausted") || words.starts_with("epoch deadline")
                {
                    BlockStatus::Overrun
                } else {
                    BlockStatus::Failed
                }
            },
        }
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        let budget = self.caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER);
        let r = self.inner.budget_call(budget, |store, guest| guest.call_message(store, payload));
        match r {
            Ok((Ok(()), _)) => Ok(()),
            Ok((Err(e), _)) => {
                self.note(&guest_error_words("message", &e));
                Err(ModuleError::Message(GUEST_REFUSED))
            },
            Err(words) => {
                self.note(&words);
                Err(ModuleError::Message(GUEST_TRAPPED))
            },
        }
    }
}
