//! The stream registry — `streams.toml` as typed, queryable data.
//!
//! This is the single source of truth for stream ids (plan §3, D10). Three readers share it: the
//! broker (which streams to poll, on what cadence, into what schema), the validator's stream-binding
//! checks (INC2 — does a manifest's `stream = "…"` name a real registry id?), and the manifest
//! generator (INC3 — the 16 × 24-option enum walls are emitted from these rows, never hand-typed).
//! Keeping it as data means "add a stream" is a row here and a normalizer, not a code-path edit
//! scattered across the tree.
//!
//! Parsing rides [`sparq_module_api::toml`], the house's dependency-free TOML *subset* parser (the
//! same one `sparqmod.toml` uses), so this crate stays zero-dependency in its default build: the
//! registry needs no serde, only the subset parser and std. Every rejection names the row and the
//! field, because a registry that loads as something other than what its author wrote is a
//! wrong-data bug with no symptom until a cell draws the wrong feed.

use sparq_module_api::toml::{self, Table, Value};

use crate::schema;

/// The five stream domains, in the picker's canonical group order (§3.1, §5.2). The dropdown groups
/// by these; a domain not in this list is still parsed (the registry is data) but sorts last.
pub const DOMAIN_ORDER: [&str; 5] = ["SW", "SAT", "GEO", "WX", "SP"];

/// One `[[stream]]` row, fully parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamDef {
    /// The stable stream id — what a manifest `stream = "…"` binding names.
    pub id: String,
    /// Domain code: `SW` / `SAT` / `GEO` / `WX` / `SP`.
    pub domain: String,
    /// The dropdown label.
    pub label: String,
    /// The URL template, with `{…}` placeholders the fetcher fills (see `streams.toml` header).
    pub endpoint: String,
    /// The minimum poll interval, in seconds — a floor the scheduler never undercuts (§11.1).
    pub cadence_s: u32,
    /// The renderer hint (§5.5). Advice for the guest, not a hard binding.
    pub view: String,
    /// The frozen record schema, e.g. `observatory/grid@1`.
    pub schema: String,
    /// The unit string the cell's axis/footer carries (token rule 9). May be empty (raw text).
    pub units: String,
    /// The source credit the package README carries (§11.3).
    pub attribution: String,
    /// Env var holding an API key, if this stream needs one.
    pub key_env: Option<String>,
    /// Value used when `key_env` is unset (NASA's `DEMO_KEY`), if any.
    pub key_fallback: Option<String>,
    /// A declared broker-side transform, e.g. `regrid:2deg` (§6.4). Data, not code folklore.
    pub transform: Option<String>,
    /// Whether the endpoint carries `{lat}`/`{lon}` from `SPARQ_OBSERVATORY_LOC`.
    pub uses_location: bool,
    /// Default CO-OPS tide station (`{st}`), if this stream uses one.
    pub default_tide_station: Option<String>,
    /// Default FIRMS country code (`{CC}`), if this stream uses one.
    pub default_country: Option<String>,
    /// Whether a successful fetch APPENDS to the window instead of replacing it (see
    /// `streams.toml`'s field vocabulary). `true` for the SWPC `summary/*` feeds, whose payload is
    /// a single record per poll — history lives in the broker, not in the payload. Default `false`
    /// (replace): a whole-picture payload — a 3-day series, an event set, a grid — IS the window.
    pub accumulate: bool,
}

impl StreamDef {
    /// Splits `schema` into `(id, version)`. A malformed `@version` is a registry error caught at
    /// parse time, so this is total over a loaded registry — but it still returns `Option` rather
    /// than panicking, because a library that panics on its own data is a library that cannot be
    /// trusted at a boundary.
    #[must_use]
    pub fn schema_parts(&self) -> Option<(&str, u32)> {
        schema::parse_schema(&self.schema)
    }

    /// Whether this stream needs an API key at all.
    #[must_use]
    pub fn requires_key(&self) -> bool {
        self.key_env.is_some()
    }

    /// Resolves the key state against an environment lookup. This is where "a stream whose key is
    /// unset is never fetched and reports KEY NEEDED in words" (plan D14) becomes a value the CLI
    /// and the picker can read — never a silent hole, never a crash.
    #[must_use]
    pub fn key_state<F>(&self, env: F) -> KeyState
    where
        F: Fn(&str) -> Option<String>,
    {
        match &self.key_env {
            None => KeyState::NotNeeded,
            Some(var) => match env(var) {
                Some(v) if !v.trim().is_empty() => KeyState::Ready,
                _ => match &self.key_fallback {
                    Some(fb) if !fb.trim().is_empty() => KeyState::UsingFallback(fb.clone()),
                    _ => KeyState::KeyNeeded,
                },
            },
        }
    }

    /// The key value to actually send, or `None` when the stream is KEY NEEDED (and must not be
    /// fetched). `NotNeeded` also answers `None` — there is no key to send — so callers gate on
    /// [`StreamDef::key_state`], not on this, to decide fetchability.
    #[must_use]
    pub fn resolved_key<F>(&self, env: F) -> Option<String>
    where
        F: Fn(&str) -> Option<String>,
    {
        match self.key_state(&env) {
            KeyState::Ready => self.key_env.as_ref().and_then(|v| env(v)),
            KeyState::UsingFallback(fb) => Some(fb),
            KeyState::NotNeeded | KeyState::KeyNeeded => None,
        }
    }

    /// Whether the stream can be fetched right now: everything except KEY NEEDED. A keyless NASA
    /// stream on its `DEMO_KEY` fallback is fetchable; a keyless FIRMS stream is not.
    #[must_use]
    pub fn is_fetchable<F>(&self, env: F) -> bool
    where
        F: Fn(&str) -> Option<String>,
    {
        !matches!(self.key_state(&env), KeyState::KeyNeeded)
    }
}

/// The key situation for a stream, resolved against the environment (plan D14, §11.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyState {
    /// The stream needs no key.
    NotNeeded,
    /// The key env var is set to a real value.
    Ready,
    /// The key env var is unset but a fallback applies (NASA's `DEMO_KEY`), carrying the fallback.
    UsingFallback(String),
    /// The key env var is unset and there is no fallback: the stream is not fetched and says so in
    /// words. This is the honest refusal, never a silent hole.
    KeyNeeded,
}

impl KeyState {
    /// The picker/LED words for this state (§5.3's LED vocabulary includes KEY NEEDED). Only
    /// KEY NEEDED is a data-affecting state (the stream is not fetched); the others are fetchable.
    #[must_use]
    pub fn as_words(&self) -> &'static str {
        match self {
            Self::NotNeeded => "NO KEY",
            Self::Ready | Self::UsingFallback(_) => "KEYED",
            Self::KeyNeeded => "KEY NEEDED",
        }
    }

    /// The label suffix the dropdown appends for a KEY NEEDED stream (`" (KEY NEEDED)"`), else "".
    #[must_use]
    pub fn label_suffix(&self) -> &'static str {
        match self {
            Self::KeyNeeded => " (KEY NEEDED)",
            _ => "",
        }
    }
}

/// A registry parse failure, carrying the row and field so the message is actionable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryError {
    /// The stream id (or row index when the id itself is what's missing) the failure is about.
    pub where_: String,
    /// What went wrong, in words naming the field and the fix.
    pub message: String,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "streams.toml [{}]: {}", self.where_, self.message)
    }
}

impl std::error::Error for RegistryError {}

/// The parsed registry: every stream row, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    streams: Vec<StreamDef>,
}

/// The checked-in registry text, embedded at compile time so the broker, the validator and the
/// tests all read the *same* bytes as the file (a drift gate in INC2 pins the two together).
pub const EMBEDDED_REGISTRY: &str = include_str!("../streams.toml");

impl Registry {
    /// Parses the checked-in `streams.toml` (embedded). This is the registry the broker and the CLI
    /// use; a parse failure here is a broken check-in and surfaces as a `RegistryError`.
    ///
    /// # Errors
    /// A [`RegistryError`] naming the malformed row/field, or the underlying TOML syntax error.
    pub fn load() -> Result<Self, RegistryError> {
        Self::from_toml(EMBEDDED_REGISTRY)
    }

    /// Parses registry text (the embedded file, or a test fixture). Kept separate from [`Self::load`]
    /// so the parse rules are testable against hostile inputs without touching the real file.
    ///
    /// # Errors
    /// A [`RegistryError`] for a missing/invalid required field or an unknown schema; a TOML syntax
    /// error is mapped into one carrying the parser's line.
    pub fn from_toml(text: &str) -> Result<Self, RegistryError> {
        let root = toml::parse(text).map_err(|e| RegistryError {
            where_: "file".to_string(),
            message: format!("TOML syntax error: {e}"),
        })?;
        let rows = root.tables("stream").ok_or_else(|| RegistryError {
            where_: "file".to_string(),
            message:
                "no `[[stream]]` rows — a registry with no streams is a mistake, not an empty set"
                    .to_string(),
        })?;
        let mut streams = Vec::with_capacity(rows.len());
        for (i, row) in rows.into_iter().enumerate() {
            streams.push(parse_row(row, i)?);
        }
        // Duplicate ids are a registry bug: two rows claiming one id means a binding is ambiguous.
        for a in 0..streams.len() {
            for b in (a + 1)..streams.len() {
                if streams[a].id == streams[b].id {
                    return Err(RegistryError {
                        where_: streams[a].id.clone(),
                        message: format!(
                            "duplicate stream id (rows {a} and {b}) — ids must be unique"
                        ),
                    });
                }
            }
        }
        Ok(Self { streams })
    }

    /// The number of streams.
    #[must_use]
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    /// Whether the registry is empty (a loaded registry never is; `from_toml` refuses empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    /// All streams, in file order.
    #[must_use]
    pub fn streams(&self) -> &[StreamDef] {
        &self.streams
    }

    /// Looks a stream up by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&StreamDef> {
        self.streams.iter().find(|s| s.id == id)
    }

    /// True when `id` names a registered stream — the predicate the INC2 validator's stream-binding
    /// check calls (a `stream = "…"` or a `param:` enum option must satisfy it).
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// Every stream id, in file order — the "values allowed" list for refusal words.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.streams.iter().map(|s| s.id.as_str()).collect()
    }

    /// The distinct domains present, in [`DOMAIN_ORDER`] (unknown domains sort last, file order
    /// within). The picker's group headers read this.
    #[must_use]
    pub fn domains(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for d in DOMAIN_ORDER.iter() {
            if self.streams.iter().any(|s| s.domain == *d) && !out.contains(d) {
                out.push(d);
            }
        }
        for s in &self.streams {
            if !DOMAIN_ORDER.contains(&s.domain.as_str()) && !out.contains(&s.domain.as_str()) {
                out.push(&s.domain);
            }
        }
        out
    }

    /// The streams in one domain, in file order.
    #[must_use]
    pub fn by_domain(&self, domain: &str) -> Vec<&StreamDef> {
        self.streams.iter().filter(|s| s.domain == domain).collect()
    }

    /// The actionable refusal for an unknown stream id — the words behind the pre-registered
    /// `E-STREAM-UNKNOWN` code (plan §7.2). Names the field, the value found, the values allowed
    /// (every registry id) and the fix, exactly like the manifest catalogue's other codes.
    #[must_use]
    pub fn unknown_id_message(&self, found: &str) -> String {
        let allowed: Vec<&str> = self.ids();
        format!(
            "unknown stream id `{found}' (field: stream)\n  \
             allowed: {allowed}\n  \
             fix: bind one of the {} registry ids above, or add the stream to \
             crates/sparq-streams/streams.toml first (the registry is the single source of truth).",
            allowed.len(),
            allowed = allowed.join(", ")
        )
    }
}

/// Reads one `[[stream]]` row into a [`StreamDef`], refusing a missing/invalid required field in
/// words that name the row and the field.
fn parse_row(row: &Table, index: usize) -> Result<StreamDef, RegistryError> {
    // The id first, so every later error can name the stream rather than just its row number.
    let id = req_str(row, "id")
        .map_err(|m| RegistryError { where_: format!("row {index}"), message: m })?
        .to_string();
    let where_ = id.clone();
    let err = |message: String| RegistryError { where_: where_.clone(), message };

    let domain = req_str(row, "domain").map_err(err)?.to_string();
    let label = req_str(row, "label").map_err(err)?.to_string();
    let endpoint = req_str(row, "endpoint").map_err(err)?.to_string();
    let view = req_str(row, "view").map_err(err)?.to_string();
    let schema_str = req_str(row, "schema").map_err(err)?.to_string();
    let attribution = req_str(row, "attribution").map_err(err)?.to_string();
    let units = opt_str(row, "units").unwrap_or_default().to_string();

    let cadence_s = row
        .get("cadence_s")
        .and_then(Value::as_u32)
        .filter(|&c| c > 0)
        .ok_or_else(|| err("`cadence_s` must be a positive integer (seconds)".to_string()))?;

    // The schema must be one of the five frozen observatory shapes, or a normalizer would have no
    // channel order to trust. Caught here, at load, never at draw.
    let (schema_id, _ver) = schema::parse_schema(&schema_str)
        .ok_or_else(|| err(format!("`schema` version is not a number: `{schema_str}`")))?;
    if !schema::is_observatory_schema(schema_id) {
        return Err(err(format!(
            "`schema` = `{schema_str}` is not one of the five observatory schemas \
             (observatory/timeseries|events|grid|position|text@1)"
        )));
    }

    Ok(StreamDef {
        id,
        domain,
        label,
        endpoint,
        cadence_s,
        view,
        schema: schema_str,
        units,
        attribution,
        key_env: opt_str(row, "key_env").map(str::to_string),
        key_fallback: opt_str(row, "key_fallback").map(str::to_string),
        transform: opt_str(row, "transform").map(str::to_string),
        uses_location: row.get("uses_location").and_then(Value::as_bool).unwrap_or(false),
        default_tide_station: opt_str(row, "default_tide_station").map(str::to_string),
        default_country: opt_str(row, "default_country").map(str::to_string),
        accumulate: row.get("accumulate").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A required string field, or a words refusal naming it.
fn req_str<'a>(row: &'a Table, key: &str) -> Result<&'a str, String> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required field `{key}` (a string)"))
}

/// An optional string field (`None` when absent).
fn opt_str<'a>(row: &'a Table, key: &str) -> Option<&'a str> {
    row.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn no_env(_k: &str) -> Option<String> {
        None
    }
    fn env_with<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).to_string())
    }

    #[test]
    fn the_checked_in_registry_loads_with_23_streams() {
        let reg = Registry::load().expect("the checked-in streams.toml must parse");
        assert_eq!(reg.len(), 23, "the v1 registry is 23 streams (§3.1)");
        assert!(!reg.is_empty());
    }

    #[test]
    fn every_domain_is_present_and_ordered() {
        let reg = Registry::load().unwrap();
        assert_eq!(reg.domains(), vec!["SW", "SAT", "GEO", "WX", "SP"]);
        assert_eq!(reg.by_domain("SW").len(), 8, "SWPC's full live set is 8 streams (§3.2)");
    }

    #[test]
    fn ids_are_unique_and_queryable() {
        let reg = Registry::load().unwrap();
        let ids = reg.ids();
        assert!(reg.contains("swpc.aurora"));
        assert!(reg.contains("space.power"));
        assert!(!reg.contains("nope.nope"));
        assert_eq!(ids.len(), 23);
        assert_eq!(reg.get("sat.iss").map(|s| s.cadence_s), Some(5));
    }

    #[test]
    fn accumulate_defaults_false_and_the_summary_trio_declares_it() {
        // The registry column is data: absent means REPLACE (a whole-picture payload is the
        // window). Exactly the three SWPC `summary/*` feeds — one record per poll, history lives
        // in the broker — declare `accumulate = true` (INC3 finding #7's remedy, INC5).
        let reg = Registry::load().unwrap();
        let accum: Vec<&str> =
            reg.streams().iter().filter(|d| d.accumulate).map(|d| d.id.as_str()).collect();
        assert_eq!(accum, vec!["swpc.solar-wind", "swpc.bz", "swpc.flux10cm"]);
        assert!(!reg.get("swpc.kp").unwrap().accumulate, "a 3-day series payload REPLACES");
        assert!(!reg.get("geo.quakes-hour").unwrap().accumulate, "an event set REPLACES");
        // A hand-written row without the column parses to the default.
        let one = Registry::from_toml(
            r#"
[[stream]]
id = "test.plain"
domain = "SW"
label = "Plain"
endpoint = "https://example.test/x"
cadence_s = 60
view = "text"
schema = "observatory/text@1"
units = ""
attribution = "test"
"#,
        )
        .unwrap();
        assert!(!one.get("test.plain").unwrap().accumulate);
    }

    #[test]
    fn aurora_carries_its_declared_regrid_transform() {
        let reg = Registry::load().unwrap();
        let a = reg.get("swpc.aurora").unwrap();
        assert_eq!(a.transform.as_deref(), Some("regrid:2deg"));
        assert_eq!(a.schema_parts(), Some(("observatory/grid", 1)));
        assert_eq!(a.view, "map-heat");
    }

    #[test]
    fn nasa_streams_fall_back_to_demo_key_and_stay_fetchable() {
        let reg = Registry::load().unwrap();
        let neo = reg.get("space.neo").unwrap();
        assert!(neo.requires_key());
        assert_eq!(neo.key_state(no_env), KeyState::UsingFallback("DEMO_KEY".into()));
        assert_eq!(neo.resolved_key(no_env).as_deref(), Some("DEMO_KEY"));
        assert!(neo.is_fetchable(no_env), "DEMO_KEY makes NASA fetchable without a personal key");
        // With a real key set, it is used instead of the fallback.
        let keyed = neo.key_state(env_with(&[("SPARQ_NASA_API_KEY", "MYKEY")]));
        assert_eq!(keyed, KeyState::Ready);
        assert_eq!(
            neo.resolved_key(env_with(&[("SPARQ_NASA_API_KEY", "MYKEY")])).as_deref(),
            Some("MYKEY")
        );
    }

    #[test]
    fn firms_without_a_key_is_key_needed_and_not_fetchable() {
        let reg = Registry::load().unwrap();
        let firms = reg.get("geo.firms").unwrap();
        assert!(firms.requires_key());
        assert_eq!(firms.key_state(no_env), KeyState::KeyNeeded);
        assert_eq!(firms.key_state(no_env).as_words(), "KEY NEEDED");
        assert_eq!(firms.key_state(no_env).label_suffix(), " (KEY NEEDED)");
        assert!(
            !firms.is_fetchable(no_env),
            "a keyless FIRMS stream is never fetched — no silent hole"
        );
        assert!(firms.is_fetchable(env_with(&[("SPARQ_FIRMS_KEY", "abc")])));
        assert_eq!(firms.default_country.as_deref(), Some("USA"));
    }

    #[test]
    fn keyless_streams_need_no_key() {
        let reg = Registry::load().unwrap();
        assert_eq!(reg.get("swpc.kp").unwrap().key_state(no_env), KeyState::NotNeeded);
        assert!(!reg.get("swpc.kp").unwrap().requires_key());
    }

    #[test]
    fn unknown_id_refusal_names_field_value_allowed_and_fix() {
        let reg = Registry::load().unwrap();
        let msg = reg.unknown_id_message("foo.bar");
        assert!(msg.contains("foo.bar"), "names the value found");
        assert!(msg.contains("field: stream"), "names the field");
        assert!(msg.contains("swpc.aurora") && msg.contains("space.power"), "lists allowed ids");
        assert!(msg.contains("streams.toml"), "points at the fix");
    }

    #[test]
    fn a_row_missing_a_required_field_is_refused_in_words() {
        // `view` is required; dropping it must name the stream and the field.
        let bad = "[[stream]]\nid = \"x.y\"\ndomain = \"SW\"\nlabel = \"L\"\nendpoint = \"http://e\"\ncadence_s = 60\nschema = \"observatory/text@1\"\nattribution = \"A\"\n";
        let e = Registry::from_toml(bad).expect_err("a row without `view` must be refused");
        assert_eq!(e.where_, "x.y");
        assert!(e.message.contains("`view`"), "the refusal names the missing field: {e}");
    }

    #[test]
    fn an_unknown_schema_is_refused() {
        let bad = "[[stream]]\nid = \"x.y\"\ndomain = \"SW\"\nlabel = \"L\"\nendpoint = \"http://e\"\ncadence_s = 60\nview = \"text\"\nschema = \"observatory/nope@1\"\nattribution = \"A\"\n";
        let e = Registry::from_toml(bad).expect_err("an unregistered schema must be refused");
        assert!(e.message.contains("not one of the five observatory schemas"), "{e}");
    }

    #[test]
    fn a_zero_cadence_is_refused() {
        let bad = "[[stream]]\nid = \"x.y\"\ndomain = \"SW\"\nlabel = \"L\"\nendpoint = \"http://e\"\ncadence_s = 0\nview = \"text\"\nschema = \"observatory/text@1\"\nattribution = \"A\"\n";
        assert!(Registry::from_toml(bad).is_err());
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let dup = "[[stream]]\nid = \"a.b\"\ndomain = \"SW\"\nlabel = \"L\"\nendpoint = \"e\"\ncadence_s = 60\nview = \"text\"\nschema = \"observatory/text@1\"\nattribution = \"A\"\n\n[[stream]]\nid = \"a.b\"\ndomain = \"WX\"\nlabel = \"L2\"\nendpoint = \"e2\"\ncadence_s = 60\nview = \"text\"\nschema = \"observatory/text@1\"\nattribution = \"A\"\n";
        let e = Registry::from_toml(dup).expect_err("duplicate ids must be refused");
        assert!(e.message.contains("duplicate stream id"), "{e}");
    }

    #[test]
    fn empty_registry_is_refused_not_silently_empty() {
        assert!(Registry::from_toml("# nothing here\n").is_err());
    }
}
