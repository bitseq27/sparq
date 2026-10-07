//! The sources door, host side (WO-020 INC4 §8.3): D4's binding-resolution as a provider trait.
//!
//! The contract's `sources.snapshot(display-id, source-id)` is one door with two guests: the audio
//! analysis rings (`port` bindings, WO-012) and the stream plane (`stream` bindings, ADR-11). This
//! module is the host half of the stream plane: it resolves a `(display, source)` pair against the
//! manifest's declared bindings and the CURRENT param snapshot (the `param:<id>` indirection — the
//! host owns both the ParamSet and the registry, so it resolves the enum to a stream id at snapshot
//! time, D4), and it names the two implementations of the provider trait:
//!
//! * [`ReplayProvider`] (feature `streams`) — fixture windows from `reference/fixtures/observatory/`,
//!   the hermetic door for the sandbox shell, the at-rest wall, tests and goldens;
//! * the device's `BrokerProvider` — the live broker's windows behind the same trait (landed
//!   INC5; the socket itself rides `sparq-streams`' `streams-net`, and this read face rides the
//!   same `streams` feature as the replay provider).
//!
//! The wasmtime `sources` import (INC5) will be a thin door over this same trait, which is why the
//! sandbox can prove every binding behaviour — `param:` indirection, OFF, KEY NEEDED, unknown-id —
//! with no runtime anywhere.
//!
//! Nothing here runs on the audio thread (the sources door is UI-thread only, sources.wit); reads
//! are `read_to_string`-class per the `clippy.toml` fs ban, and no clock is read — freshness is a
//! function of the injected `now_unix` (the determinism firewall, D11).

use sparq_module_api::params::ParamSet;
use sparq_module_api::toml::{self, Value};
use sparq_streams::registry::Registry;
use sparq_streams::window::{StreamStatus, Window};
// The broker mirror's lock is held for a struct copy on the CONTROL/UI path — never on an audio
// path (the stream plane has no audio path at all, plan D5). The disallowed-type lint is the
// audio thread's rule; this is the defect-#66 allow-with-reason precedent (app `ui/live.rs`).
#[cfg(feature = "streams")]
#[allow(clippy::disallowed_types)]
use std::sync::Mutex;

/// The shared broker handle: the driver thread polls it, the provider reads it. The `Mutex` is
/// the control-plane mirror idiom (`HealthMirror` in the app's `ui/live.rs`) — one allow, here,
/// with the reason; every other mention rides the alias.
#[cfg(feature = "streams")]
#[allow(clippy::disallowed_types)] // control-plane mirror; see the import note above
pub type SharedBroker = std::sync::Arc<Mutex<sparq_streams::broker::Broker>>;

/// One declared source binding on a display (the manifest's `ui.displays[].sources[]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// An audio/analysis ring binding (the WO-012 door; unchanged by WO-020).
    Port(String),
    /// A fixed broker stream binding (`stream = "swpc.kp"`).
    StreamFixed(String),
    /// A param indirection (`stream = "param:cell_01"`): whichever stream the named enum param
    /// currently selects. The options list is captured at parse so resolution is a lookup.
    StreamParam {
        /// The param id.
        param: String,
        /// The param's option VALUES in order (index 0 first) — `""` is the OFF sentinel.
        options: Vec<String>,
    },
}

/// What a `(display, source)` pair resolves to against a param snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Serve this registry stream id.
    Stream(String),
    /// The cell is OFF (the param selected the empty option) — serve nothing, draw at rest.
    Off,
    /// An audio/analysis binding — the other door (not a stream).
    Port(String),
    /// The binding names a stream the registry does not know. Stage 1 refuses these at hand-in
    /// (`E-STREAM-UNKNOWN`); at runtime the provider serves nothing and says so in words rather
    /// than fetching a ghost.
    Unknown(String),
    /// A `param:` indirection names a param the module does not declare, or a non-enum param.
    /// Stage 1 refuses these too (`E-CROSS-FIELD`); runtime reads OFF with words.
    BadParam(String),
}

/// The parsed binding table of one manifest: every display's sources, plus the param order/options
/// the `param:` indirection resolves against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManifestSources {
    /// `(display_id, source_id, binding)` in manifest order.
    pub sources: Vec<(String, String, Binding)>,
    /// `(param_id, is_enum, option values)` in manifest declaration order — the index IS the
    /// ParamSet position (the snapshot is positional, types.wit).
    pub params: Vec<(String, bool, Vec<String>)>,
}

impl ManifestSources {
    /// Parses the binding table from a manifest's TOML text.
    ///
    /// # Errors
    /// A string naming the parse defect. A manifest with no `ui.displays` yields an empty table
    /// (a backbone module has no sources — legal, not an error).
    pub fn parse(text: &str) -> Result<Self, String> {
        let root = toml::parse(text).map_err(|e| format!("manifest TOML: {e}"))?;
        let mut out = Self::default();
        for p in root.tables("params").unwrap_or_default() {
            let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            let is_enum = p.get("type").and_then(Value::as_str) == Some("enum");
            let options = p
                .get("options")
                .and_then(Value::as_array)
                .map(|opts| {
                    opts.iter()
                        .map(|o| {
                            o.as_table()
                                .and_then(|t| t.get("value"))
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string()
                        })
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            out.params.push((id, is_enum, options));
        }
        let ui = match root.get("ui").and_then(Value::as_table) {
            Some(u) => u,
            None => return Ok(out),
        };
        for d in ui.tables("displays").unwrap_or_default() {
            let did = d.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            for s in d.tables("sources").unwrap_or_default() {
                let sid = s.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let binding = match (
                    s.get("port").and_then(Value::as_str),
                    s.get("stream").and_then(Value::as_str),
                ) {
                    (Some(p), _) => Binding::Port(p.to_string()),
                    (None, Some(st)) => match st.strip_prefix("param:") {
                        Some(pid) => {
                            let options = out
                                .params
                                .iter()
                                .find(|(id, _, _)| id == pid)
                                .map(|(_, _, o)| o.clone())
                                .unwrap_or_default();
                            Binding::StreamParam { param: pid.to_string(), options }
                        },
                        None => Binding::StreamFixed(st.to_string()),
                    },
                    (None, None) => continue, // stage 1 refuses; the runtime table skips
                };
                out.sources.push((did.clone(), sid, binding));
            }
        }
        Ok(out)
    }

    /// The binding for a `(display, source)` pair, if declared.
    #[must_use]
    pub fn binding(&self, display_id: &str, source_id: &str) -> Option<&Binding> {
        self.sources.iter().find(|(d, s, _)| d == display_id && s == source_id).map(|(_, _, b)| b)
    }

    /// Resolves a `(display, source)` pair against a param snapshot and the registry (D4). The
    /// enum's current option index rides the ParamSet positionally; `""` is OFF; a non-empty option
    /// is a stream id the registry must know (else [`Resolution::Unknown`]).
    #[must_use]
    pub fn resolve(
        &self,
        registry: &Registry,
        display_id: &str,
        source_id: &str,
        params: &ParamSet,
    ) -> Resolution {
        match self.binding(display_id, source_id) {
            None => Resolution::Off, // an undeclared source serves nothing (at-rest), never a guess
            Some(Binding::Port(p)) => Resolution::Port(p.clone()),
            Some(Binding::StreamFixed(id)) => {
                if registry.contains(id) {
                    Resolution::Stream(id.clone())
                } else {
                    Resolution::Unknown(id.clone())
                }
            },
            Some(Binding::StreamParam { param, options }) => {
                let idx = match self.params.iter().position(|(id, _, _)| id == param) {
                    Some(i) => i,
                    None => return Resolution::BadParam(param.clone()),
                };
                let is_enum = self.params[idx].1;
                if !is_enum {
                    return Resolution::BadParam(param.clone());
                }
                // The enum rides its option INDEX as a raw f32 (types.wit param-set convention).
                let raw = params.at(idx);
                if !raw.is_finite() {
                    return Resolution::Off;
                }
                let opt = raw.round() as usize;
                let value = match options.get(opt) {
                    Some(v) => v.clone(),
                    None => return Resolution::Off, // past the options: OFF, not a wrap
                };
                if value.is_empty() {
                    return Resolution::Off;
                }
                if registry.contains(&value) {
                    Resolution::Stream(value)
                } else {
                    Resolution::Unknown(value)
                }
            },
        }
    }
}

/// A source of stream windows behind the contract's door. Two implementations: [`ReplayProvider`]
/// (fixtures, hermetic) and the device's [`BrokerProvider`] (the live broker's windows). The wasmtime
/// `sources` import will be a thin door over this trait.
pub trait StreamProvider {
    /// The registry the provider serves against (ids, cadences, key states).
    fn registry(&self) -> &Registry;
    /// A snapshot copy of a stream's window (`None` = unconnected / stale-expired / key-needed —
    /// the caller draws at rest). Sources are read-only COPIES (sources.wit), so cloning is the
    /// contract, not a cost to optimise away here.
    fn window(&self, stream_id: &str) -> Option<Window>;
    /// The freshness of a stream at `now_unix` (the injected clock — D11).
    fn status(&self, stream_id: &str, now_unix: i64) -> StreamStatus;
    /// The KEY NEEDED env var for a stream whose key is unset (D14), else `None`.
    fn key_needed_env(&self, stream_id: &str) -> Option<String>;
}

/// The hermetic provider: recorded fixtures, replayed into windows (plan §8.3). This is what lets
/// the sandbox shell show the fixture-fed wall and the goldens render bit-exact, with no socket.
#[cfg(feature = "streams")]
#[derive(Clone, Debug)]
pub struct ReplayProvider {
    registry: Registry,
    windows: std::collections::HashMap<String, Window>,
    /// The evaluation clock the replay was loaded at (the newest fetch across the set) — the label
    /// and statuses read LIVE against it. Injected, never read from a wall clock (D11).
    now: i64,
}

#[cfg(feature = "streams")]
impl ReplayProvider {
    /// Loads every recorded fixture in `dir` into windows.
    ///
    /// # Errors
    /// A string naming the replay defect (missing fixtures, a normalizer refusal).
    pub fn load(dir: &std::path::Path) -> Result<Self, String> {
        let registry = Registry::load().map_err(|e| format!("registry: {e}"))?;
        let replayed = sparq_streams::replay::replay_all(&registry, dir, None);
        let mut windows = std::collections::HashMap::new();
        let mut now = 0i64;
        let mut failures = Vec::new();
        for (id, res) in replayed {
            match res {
                Ok(w) => {
                    if let Some(f) = w.last_fetch_unix() {
                        now = now.max(f);
                    }
                    windows.insert(id, w);
                },
                Err(e) => failures.push(format!("{id}: {e}")),
            }
        }
        if !failures.is_empty() {
            return Err(format!("fixture replay failed: {}", failures.join("; ")));
        }
        Ok(Self { registry, windows, now })
    }

    /// The evaluation clock the windows were loaded at.
    #[must_use]
    pub fn now(&self) -> i64 {
        self.now
    }
}

#[cfg(feature = "streams")]
impl StreamProvider for ReplayProvider {
    fn registry(&self) -> &Registry {
        &self.registry
    }
    fn window(&self, stream_id: &str) -> Option<Window> {
        self.windows.get(stream_id).cloned()
    }
    fn status(&self, stream_id: &str, now_unix: i64) -> StreamStatus {
        match self.windows.get(stream_id) {
            Some(w) => w.status(now_unix),
            None => StreamStatus::Offline,
        }
    }
    fn key_needed_env(&self, stream_id: &str) -> Option<String> {
        let def = self.registry.get(stream_id)?;
        match def.key_state(|k| std::env::var(k).ok()) {
            sparq_streams::registry::KeyState::KeyNeeded => def.key_env.clone(),
            _ => None,
        }
    }
}

/// The device's live provider (INC5): the broker's windows behind the SAME trait the replay
/// provider wears — the swap the shell performs when the build carries `streams-net` (plan §8.3:
/// one door, two guests). The broker itself lives in `sparq-streams` (cadence floors, the
/// replace/accumulate rules, the last-good cache); a driver thread the app owns polls it through
/// an injected transport, and this provider is the read face: `window`/`status` clone out of the
/// shared broker, `key_needed_env` reads the registry. The lock is the control-plane's usual
/// `Arc<Mutex<…>>` (the defect-#66 precedent, `ui/live.rs`): held for a struct copy on the UI
/// thread, never on an audio path — the stream plane has no audio path at all (plan D5).
#[cfg(feature = "streams")]
pub struct BrokerProvider {
    registry: Registry,
    broker: SharedBroker,
}

#[cfg(feature = "streams")]
impl BrokerProvider {
    /// Wraps a broker the caller (the app's driver thread) keeps a handle to.
    ///
    /// # Errors
    /// A string naming the registry load failure (the same checked-in bytes every other door reads).
    pub fn new(broker: SharedBroker) -> Result<Self, String> {
        let registry = Registry::load().map_err(|e| format!("registry: {e}"))?;
        Ok(Self { registry, broker })
    }

    /// The shared broker handle — the driver thread polls through the same `Arc` this reads.
    #[must_use]
    pub fn broker(&self) -> SharedBroker {
        std::sync::Arc::clone(&self.broker)
    }
}

#[cfg(feature = "streams")]
impl StreamProvider for BrokerProvider {
    fn registry(&self) -> &Registry {
        &self.registry
    }
    fn window(&self, stream_id: &str) -> Option<Window> {
        // A poisoned lock answers None (draw at rest, in words) rather than panicking the UI
        // thread — the shell survives every broker failure (§6.3).
        let broker = self.broker.lock().ok()?;
        broker.window(stream_id).cloned()
    }
    fn status(&self, stream_id: &str, now_unix: i64) -> StreamStatus {
        match self.broker.lock() {
            Ok(broker) => broker.status(stream_id, now_unix),
            Err(_) => StreamStatus::Offline,
        }
    }
    fn key_needed_env(&self, stream_id: &str) -> Option<String> {
        let def = self.registry.get(stream_id)?;
        match def.key_state(|k| std::env::var(k).ok()) {
            sparq_streams::registry::KeyState::KeyNeeded => def.key_env.clone(),
            _ => None,
        }
    }
}

/// The §5.2 toolbar status label — the shell's own words for the broker state, computed from the
/// resolved bindings of one display and the provider's statuses. Never guest-drawn (the guest has
/// no clock and no env); always in words, never a bare colour.
///
/// Example: `LIVE 14/16 · 1 STALE · 1 KEY NEEDED`.
#[must_use]
pub fn status_label<P: StreamProvider>(
    bindings: &ManifestSources,
    display_id: &str,
    params: &ParamSet,
    provider: &P,
    now_unix: i64,
) -> String {
    let mut live = 0usize;
    let mut stale = 0usize;
    let mut offline = 0usize;
    let mut keyed = 0usize;
    let mut off = 0usize;
    let mut total = 0usize;
    for (_, sid, _) in bindings.sources.iter().filter(|(d, _, _)| d == display_id) {
        if matches!(bindings.binding(display_id, sid), Some(Binding::Port(_))) {
            continue; // the audio/analysis door is not a stream; the label counts streams (§5.2)
        }
        total += 1;
        match bindings.resolve(provider.registry(), display_id, sid, params) {
            Resolution::Off => off += 1,
            Resolution::Stream(id) => {
                if provider.key_needed_env(&id).is_some() {
                    keyed += 1;
                } else {
                    match provider.status(&id, now_unix) {
                        StreamStatus::Live => live += 1,
                        StreamStatus::Stale => stale += 1,
                        StreamStatus::Offline => offline += 1,
                    }
                }
            },
            Resolution::Unknown(_) | Resolution::BadParam(_) | Resolution::Port(_) => offline += 1,
        }
    }
    let mut parts = Vec::new();
    parts.push(format!("LIVE {live}/{total}"));
    if stale > 0 {
        parts.push(format!("{stale} STALE"));
    }
    if offline > 0 {
        parts.push(format!("{offline} OFFLINE"));
    }
    if keyed > 0 {
        parts.push(format!("{keyed} KEY NEEDED"));
    }
    if off > 0 {
        parts.push(format!("{off} OFF"));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A two-cell manifest fragment in the Observatory's shape (fixed + param bindings + OFF).
    const MANIFEST: &str = r#"
[[params]]
id = "cell_01"
type = "enum"
options = [ { value = "", label = "OFF" }, { value = "swpc.aurora", label = "Aurora" }, { value = "geo.quakes-hour", label = "Quakes" } ]

[[params]]
id = "cell_02"
type = "enum"
options = [ { value = "", label = "OFF" }, { value = "swpc.kp", label = "Kp" } ]

[ui]
[[ui.displays]]
id = "wall"
[[ui.displays.sources]]
id = "cell-01"
stream = "param:cell_01"
[[ui.displays.sources]]
id = "cell-02"
stream = "param:cell_02"
[[ui.displays.sources]]
id = "fixed"
stream = "swpc.wwv"
[[ui.displays.sources]]
id = "ring"
port = "out"
"#;

    fn bindings() -> ManifestSources {
        ManifestSources::parse(MANIFEST).unwrap()
    }

    fn registry() -> Registry {
        Registry::load().unwrap()
    }

    fn params_with(cell1: f32, cell2: f32) -> ParamSet {
        // Param order: cell_01 = 0, cell_02 = 1.
        ParamSet::new(1, &[cell1, cell2]).unwrap()
    }

    #[test]
    fn the_param_indirection_resolves_to_the_selected_stream() {
        let b = bindings();
        let r = registry();
        // cell_01 option 1 = swpc.aurora.
        let res = b.resolve(&r, "wall", "cell-01", &params_with(1.0, 0.0));
        assert_eq!(res, Resolution::Stream("swpc.aurora".into()));
        // option 2 = geo.quakes-hour.
        let res = b.resolve(&r, "wall", "cell-01", &params_with(2.0, 0.0));
        assert_eq!(res, Resolution::Stream("geo.quakes-hour".into()));
    }

    #[test]
    fn the_off_option_resolves_to_off() {
        let b = bindings();
        let r = registry();
        assert_eq!(b.resolve(&r, "wall", "cell-01", &params_with(0.0, 0.0)), Resolution::Off);
    }

    #[test]
    fn an_index_past_the_options_is_off_not_a_wrap() {
        let b = bindings();
        let r = registry();
        assert_eq!(b.resolve(&r, "wall", "cell-02", &params_with(0.0, 99.0)), Resolution::Off);
    }

    #[test]
    fn a_fixed_binding_resolves_and_a_port_stays_a_port() {
        let b = bindings();
        let r = registry();
        assert_eq!(
            b.resolve(&r, "wall", "fixed", &params_with(0.0, 0.0)),
            Resolution::Stream("swpc.wwv".into())
        );
        assert_eq!(
            b.resolve(&r, "wall", "ring", &params_with(0.0, 0.0)),
            Resolution::Port("out".into())
        );
    }

    #[test]
    fn an_undeclared_source_is_off_never_a_guess() {
        let b = bindings();
        let r = registry();
        assert_eq!(b.resolve(&r, "wall", "nope", &params_with(0.0, 0.0)), Resolution::Off);
    }

    #[cfg(feature = "streams")]
    #[test]
    fn the_broker_provider_serves_live_windows_behind_the_same_trait() {
        // One real broker poll over a fixture-backed transport (hermetic — the socket is the
        // injected seam), then the provider read face: same trait, same label grammar as replay.
        use sparq_streams::broker::Broker;
        use sparq_streams::fetch::{self, Transport, TransportResponse};
        struct KpOnly(String);
        impl Transport for KpOnly {
            fn get(&self, url: &str) -> TransportResponse {
                if url == self.0 {
                    let body = std::fs::read_to_string(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../reference/fixtures/observatory/swpc.kp.json"
                    ))
                    .unwrap();
                    TransportResponse {
                        status: 200,
                        content_type: "application/json".into(),
                        body,
                        note: String::new(),
                    }
                } else {
                    TransportResponse {
                        status: 404,
                        content_type: "".into(),
                        body: "".into(),
                        note: "".into(),
                    }
                }
            }
        }
        let reg = Registry::load().unwrap();
        let now = 1_791_290_096i64; // 2026-10-06T12:34:56Z
        let kp_url = fetch::fill_endpoint(&reg, "swpc.kp", now, |_| None).unwrap().url;
        let mut broker = Broker::new(reg, None);
        let out = broker.poll(&["swpc.kp".into()], &KpOnly(kp_url.clone()), now, |_| None, |_| ());
        assert!(out[0].ok, "{}", out[0].words);
        #[allow(clippy::disallowed_types)]
        // the one construction site; the alias carries the reason
        let shared = SharedBroker::new(Mutex::new(broker));
        let prov = BrokerProvider::new(std::sync::Arc::clone(&shared)).unwrap();
        // The trait face: window, status, key words — the same shapes ReplayProvider answers.
        let w = prov.window("swpc.kp").expect("the broker polled kp green");
        assert!(!w.is_empty());
        assert_eq!(prov.status("swpc.kp", now), StreamStatus::Live);
        assert_eq!(prov.status("swpc.kp", now + 10 * 300), StreamStatus::Offline);
        assert!(prov.window("geo.firms").is_none(), "a keyless stream has no window");
        assert_eq!(prov.key_needed_env("geo.firms").as_deref(), Some("SPARQ_FIRMS_KEY"));
        assert_eq!(prov.key_needed_env("swpc.kp"), None);
        // The §5.2 label over the test manifest: cell_02 selects kp (LIVE — just polled);
        // cell_01 selects quakes-hour and the fixed binding is wwv — neither was ever polled,
        // so both read OFFLINE. The port binding is not a stream and is not counted (§5.2).
        let b = bindings();
        let label = status_label(&b, "wall", &params_with(2.0, 1.0), &prov, now);
        assert_eq!(label, "LIVE 1/3 · 2 OFFLINE");
        // The driver and the provider share one broker: a second poll is visible through the read face.
        let mut bk = shared.lock().unwrap();
        let out2 = bk.poll(&["swpc.kp".into()], &KpOnly(kp_url), now + 300, |_| None, |_| ());
        assert!(out2[0].ok, "{}", out2[0].words);
        drop(bk);
        assert_eq!(prov.status("swpc.kp", now + 300), StreamStatus::Live);
    }

    #[cfg(feature = "streams")]
    #[test]
    fn the_replay_provider_serves_fixture_windows_and_a_label() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/fixtures/observatory");
        let prov = ReplayProvider::load(&dir).unwrap();
        let w = prov.window("swpc.aurora").expect("the aurora fixture replays");
        assert!(!w.is_empty());
        assert_eq!(prov.status("swpc.aurora", prov.now()), StreamStatus::Live);
        // The label over the fragment's four sources at the default params (cell1 OFF, cell2 OFF).
        let b = bindings();
        let label = status_label(&b, "wall", &params_with(1.0, 1.0), &prov, prov.now());
        assert!(label.starts_with("LIVE 3/3"), "{label}"); // aurora + kp + wwv, all live
        let label_off = status_label(&b, "wall", &params_with(0.0, 0.0), &prov, prov.now());
        assert!(label_off.contains("2 OFF") && label_off.starts_with("LIVE 1/3"), "{label_off}");
    }
}
