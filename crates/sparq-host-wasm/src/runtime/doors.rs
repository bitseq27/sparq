//! The four import doors as one store-data type: `tokens`, `host`, `assets`, `sources`.
//!
//! One `InstrumentHost` per instance (per `Store`), because the two instances share nothing
//! mutable (decision D) — the audio instance's doors answer control-thread calls and REFUSE the
//! sources door (it has no display binding); the display instance's doors serve `sources` and
//! read the same injected clock, never a wall clock (host.wit rule 1).
//!
//! Every door is a pure function of injected state: the clock is a [`ClockReading`] the caller
//! sets, the streams are a `StreamProvider` snapshot, the seeds derive from the kernel's seed
//! tree (ADR-007 — one derivation, not a second), and the token bundle is the checked-in
//! artefact. Nothing here reads a wall clock, touches the network, or opens a file at call time.

use std::sync::Arc;

use wasmtime::component::{Resource, ResourceTable};

use super::bindings::sparq::instrument::{assets, host, sources, tokens, types};
use super::bindings::AssetRep;
use super::{host_api_version, ClockReading, Phase};
use crate::sources::{ManifestSources, Resolution, StreamProvider};
use sparq_module_api::params::ParamSet;

/// The token bundle the `tokens` door serves: the checked-in envelope's version and the
/// `tokens.json` artefact verbatim (tokens.wit: "the tokens.json artefact, verbatim"; the
/// envelope's `payload_sha256` IS this file's hash — pinned by `sparq-module-api`'s
/// `token_bundle` test, so serving these bytes is serving the pinned artefact).
#[derive(Clone, Debug)]
pub struct TokenBundle {
    /// The bundle's semver (the envelope's `version` object).
    pub version: types::Semver,
    /// `design/tokens/generated/tokens.json`, verbatim.
    pub payload: Arc<Vec<u8>>,
}

/// The checked-in token bundle, embedded — the same bytes `tools/token_gen.py --check` pins.
const TOKENS_JSON: &[u8] = include_bytes!("../../../../design/tokens/generated/tokens.json");
const TOKEN_BUNDLE_JSON: &[u8] =
    include_bytes!("../../../../design/tokens/generated/token-bundle.json");

impl TokenBundle {
    /// Loads the embedded bundle. The version comes from the envelope (never hand-typed, so a
    /// token version bump moves the door with it).
    ///
    /// # Errors
    /// A sentence naming the parse failure — the envelope is checked in and machine-generated,
    /// so a failure here is a broken check-in, not a runtime state.
    pub fn load() -> Result<Self, String> {
        let envelope: serde_json::Value = serde_json::from_slice(TOKEN_BUNDLE_JSON)
            .map_err(|e| format!("token-bundle.json: {e}"))?;
        let v = envelope.get("version").ok_or("token-bundle.json carries no version")?;
        let u32_at = |k: &str| -> Result<u32, String> {
            u32::try_from(v.get(k).and_then(serde_json::Value::as_i64).unwrap_or(-1))
                .map_err(|_| format!("token-bundle.json version.{k} is not a u32"))
        };
        Ok(Self {
            version: types::Semver {
                major: u32_at("major")?,
                minor: u32_at("minor")?,
                patch: u32_at("patch")?,
            },
            payload: Arc::new(TOKENS_JSON.to_vec()),
        })
    }
}

/// wasmtime's [`StoreLimits`](wasmtime::StoreLimits) plus a high-water counter — the memory
/// MEASUREMENT stage 5 reports against the declared class (the limiter enforces; this records
/// what the guest actually asked for). Delegation, not reimplementation: every decision is
/// wasmtime's own.
pub struct TrackingLimiter {
    inner: wasmtime::StoreLimits,
    /// The largest linear-memory size the guest ever requested (bytes).
    pub high_water: usize,
}

impl TrackingLimiter {
    /// A limiter capped at `mb` megabytes.
    #[must_use]
    pub fn new(mb: u32) -> Self {
        Self {
            inner: wasmtime::StoreLimitsBuilder::new()
                .memory_size(usize::try_from(mb).unwrap_or(u32::MAX as usize) * 1024 * 1024)
                .build(),
            high_water: 0,
        }
    }
}

impl wasmtime::ResourceLimiter for TrackingLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.high_water = self.high_water.max(desired);
        self.inner.memory_growing(current, desired, maximum)
    }
    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.inner.table_growing(current, desired, maximum)
    }
}

/// The store data: every door's state, the phase tracker, the diagnostics capture, and the
/// memory limiter's counters. One per instance.
pub struct InstrumentHost {
    /// The memory limiter (from `[capabilities] max_memory_mb`), enforced by wasmtime itself,
    /// with the high-water measurement the report needs.
    pub limits: TrackingLimiter,
    /// The assets door's handle table.
    pub table: ResourceTable,
    /// The manifest's declared asset hashes (`state.assets[]`, `blake3:<hex>` spellings).
    pub declared_assets: Vec<String>,
    /// The token bundle in force.
    pub bundle: TokenBundle,
    /// The session seed tree's root for this instance (ADR-007).
    pub seed_root: u64,
    /// This instance's graph path — the seed derivation's namespace (`["canvas", "node-3"]`).
    pub node_path: Vec<String>,
    /// The injected clocks (`host.now` answers THIS, never a wall clock).
    pub clock: ClockReading,
    /// Which phase the guest is in — the RT discipline's evidence.
    pub phase: Phase,
    /// Captured `host.log` messages, `(level, message)`.
    pub logs: Vec<(String, String)>,
    /// RT violations and door refusals, in words (the validator's report lines).
    pub violations: Vec<String>,
    /// The manifest's parsed source bindings (D4's resolution table).
    pub bindings: ManifestSources,
    /// The host-side param snapshot the `param:` indirection resolves against.
    pub params: ParamSet,
    /// The stream provider (Replay in the sandbox/gates, Broker on the device).
    pub provider: Arc<dyn StreamProvider + Send + Sync>,
    /// The evaluation clock for window freshness (injected — D11).
    pub now_unix: i64,
    /// Whether the sources door is open on this instance (the display instance's only; the audio
    /// instance answers None in words — it has no display binding, §2.2).
    pub sources_open: bool,
    /// Measurement: sources snapshots served (Some) / at-rest answers (None).
    pub sources_served: usize,
    /// Measurement: sources snapshots answered at rest.
    pub sources_empty: usize,
}

impl InstrumentHost {
    /// A host for one instance. `sources_open` is `false` for the audio instance, `true` for the
    /// display instance (decision D's split).
    ///
    /// # Errors
    /// A sentence naming the token-bundle load failure.
    pub fn new(
        max_memory_mb: u32,
        provider: Arc<dyn StreamProvider + Send + Sync>,
        bindings: ManifestSources,
        params: ParamSet,
        now_unix: i64,
        sources_open: bool,
    ) -> Result<Self, String> {
        let limits = TrackingLimiter::new(max_memory_mb);
        Ok(Self {
            limits,
            table: ResourceTable::new(),
            declared_assets: Vec::new(),
            bundle: TokenBundle::load()?,
            seed_root: 0,
            node_path: Vec::new(),
            clock: ClockReading::default(),
            phase: Phase::Control,
            logs: Vec::new(),
            violations: Vec::new(),
            bindings,
            params,
            provider,
            now_unix,
            sources_open,
            sources_served: 0,
            sources_empty: 0,
        })
    }

    fn note_violation(&mut self, words: String) {
        self.violations.push(words);
    }
}

// The four doors. The generated traits take `&mut self` where `self` is the store data itself
// (the `HasSelf` projection), so every door reads/writes the fields above and nothing else.

// The `types`/`audio`/`display` interfaces carry only shared vocabulary — their `Host` traits
// are empty, but the linker's bounds require the store data to implement them.
impl types::Host for InstrumentHost {}
impl super::bindings::sparq::instrument::audio::Host for InstrumentHost {}
impl super::bindings::sparq::instrument::display::Host for InstrumentHost {}

impl tokens::Host for InstrumentHost {
    fn version(&mut self) -> types::Semver {
        self.bundle.version.clone()
    }
    fn get_bundle(&mut self) -> tokens::Bundle {
        tokens::Bundle {
            version: self.bundle.version.clone(),
            encoding: tokens::BundleEncoding::Json,
            payload: (*self.bundle.payload).clone(),
        }
    }
}

impl host::Host for InstrumentHost {
    fn now(&mut self) -> host::TimeInfo {
        if self.phase == Phase::Process {
            // Lawful inside process ONLY for block-input.t-sample; host.now during process is
            // the control-thread door reached from the audio thread — captured, per host.wit.
            self.note_violation(
                "host.now() called during process — the only lawful clock inside a block is \
                 block-input.t-sample (host.wit rule 1); captured as an RT violation"
                    .to_string(),
            );
        }
        self.clock.to_time_info()
    }

    fn random_seed(&mut self, stream_name: String) -> u64 {
        if self.phase != Phase::Control {
            self.note_violation(format!(
                "host.random-seed(\"{stream_name}\") called outside the control thread — the seed \
                 tree is a prepare-time door (host.wit rule 2); captured as an RT violation"
            ));
        }
        // ADR-007's ONE derivation: session root → this instance's graph path → the named stream.
        let mut path: Vec<&str> = self.node_path.iter().map(String::as_str).collect();
        path.push(stream_name.as_str());
        sparq_kernel::seed::derive_seed(self.seed_root, &path)
    }

    fn log(&mut self, level: types::LogLevel, message: String) {
        if self.phase != Phase::Control {
            self.note_violation(format!(
                "host.log({}) during {}: {:?} — diagnostics are a control-thread door; calls \
                 from process/draw are RT violations the hand-in report names (host.wit rule 3)",
                level_word(&level),
                match self.phase {
                    Phase::Process => "process",
                    Phase::Draw => "draw",
                    Phase::Control => "control",
                },
                message
            ));
        }
        self.logs.push((level_word(&level).to_string(), message));
    }

    fn api_version(&mut self) -> types::Semver {
        host_api_version()
    }
}

/// The log level's word (the intent log shows words, never ordinals).
#[must_use]
pub fn level_word(level: &types::LogLevel) -> &'static str {
    match level {
        types::LogLevel::Trace => "trace",
        types::LogLevel::Debug => "debug",
        types::LogLevel::Info => "info",
        types::LogLevel::Warn => "warn",
        types::LogLevel::Error => "error",
    }
}

impl assets::HostAsset for InstrumentHost {
    fn size(&mut self, self_: Resource<AssetRep>) -> u64 {
        match self.table.get(&self_) {
            Ok(rep) => u64::try_from(rep.bytes.len()).unwrap_or(u64::MAX),
            Err(_) => 0, // an unknown handle has no size; the read below says so in words
        }
    }
    fn read(
        &mut self,
        self_: Resource<AssetRep>,
        offset: u64,
        len: u32,
    ) -> Result<Vec<u8>, assets::AssetError> {
        let rep = self.table.get(&self_).map_err(|e| {
            assets::AssetError::Io(format!(
                "unknown asset handle ({e:?}) — the host never opened one"
            ))
        })?;
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(rep.bytes.len());
        let end =
            start.saturating_add(usize::try_from(len).unwrap_or(usize::MAX)).min(rep.bytes.len());
        Ok(rep.bytes[start..end].to_vec())
    }
    fn drop(&mut self, rep: Resource<AssetRep>) -> wasmtime::Result<()> {
        // A dropped handle frees its table slot; an unknown one is already free (idempotent, in
        // the WIT's spirit: the handle dies with the instance regardless).
        let _ = self.table.delete(rep);
        Ok(())
    }
}

impl assets::Host for InstrumentHost {
    fn open(&mut self, hash: String) -> Result<Resource<AssetRep>, assets::AssetError> {
        if self.phase != Phase::Control {
            return Err(assets::AssetError::Denied);
        }
        if !self.declared_assets.iter().any(|h| *h == hash) {
            // The entitlement check: undeclared hashes never reach the store (assets.wit: the
            // manifest's assets[] IS the scope; fs-read capability would widen it — v1 has none).
            return Err(assets::AssetError::Undeclared);
        }
        // Declared but absent: the content-addressed library store is Phase 3. Honest
        // `not-found` (the WIT's own case: "fetch pending or offline"), never an empty read.
        Err(assets::AssetError::NotFound)
    }
}

impl sources::Host for InstrumentHost {
    fn snapshot(
        &mut self,
        display_id: String,
        source_id: String,
    ) -> Option<sources::StreamSnapshot> {
        if self.phase == Phase::Process {
            self.note_violation(format!(
                "sources.snapshot({display_id}, {source_id}) called during process — the door is \
                 UI-thread only (sources.wit); captured as an RT violation, answered at rest"
            ));
            return None;
        }
        if !self.sources_open {
            self.note_violation(format!(
                "sources.snapshot({display_id}, {source_id}) on the AUDIO instance — it has no \
                 display binding (§2.2); answered at rest in words"
            ));
            return None;
        }
        let at_rest = |host: &mut Self, why: String| {
            host.sources_empty += 1;
            host.note_violation(why);
            None
        };
        match self.bindings.resolve(self.provider.registry(), &display_id, &source_id, &self.params)
        {
            Resolution::Stream(id) => {
                let window = self.provider.window(&id)?;
                // The served records are the window's snapshot at the injected clock: flags
                // stamped (Stale when the window is not LIVE). This is sources.wit decision C
                // ("the host re-stamps when it serves") AND the cross-boundary parity rule: the
                // guest has no clock, so it derives freshness from the flags — serving the
                // stamped snapshot makes the guest's derivation equal the broker's own
                // (window.status(now)), which is what the harness's golden was rendered with.
                let records = window.snapshot(self.now_unix);
                if records.is_empty() {
                    self.sources_empty += 1;
                    return None;
                }
                self.sources_served += 1;
                Some(sources::StreamSnapshot::Window(
                    records.iter().map(super::convert::record_to_wit).collect(),
                ))
            },
            Resolution::Off => {
                self.sources_empty += 1;
                None // the cell is OFF by param choice — at rest, silently (not a defect)
            },
            Resolution::Port(port) => at_rest(
                self,
                format!(
                    "source `{source_id}` binds port `{port}` — the analysis-ring door for wasm \
                     instruments arrives with the live display binding work; answered at rest, \
                     in words (v1 serves stream bindings)"
                ),
            ),
            Resolution::Unknown(id) => at_rest(
                self,
                format!(
                    "source `{source_id}` names stream `{id}` which the registry does not know — \
                     stage 1 refuses these at hand-in (E-STREAM-UNKNOWN); answered at rest, never \
                     a ghost fetch"
                ),
            ),
            Resolution::BadParam(param) => at_rest(
                self,
                format!(
                    "source `{source_id}` indirection `param:{param}` is not a declared enum — \
                     stage 1 refuses these (E-CROSS-FIELD); answered at rest, in words"
                ),
            ),
        }
    }
}
