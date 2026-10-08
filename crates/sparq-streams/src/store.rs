//! The key/cadence overrides store (WO-020 INC6 S3, ruling O-2/O-3/O-4, plan D18): one small
//! TOML beside the streams cache in the OS user-data dir — `sparq/streams.d/overrides.toml` —
//! carrying, per stream id, an optional API `key` and an optional `cadence_s`.
//!
//! The rules this module owns, in the plan's words:
//!
//! * **Env WINS over the file** (O-4): a machine policy cannot be edited by the instrument.
//!   [`Overrides::env_closure_over`] layers the store UNDER the real environment, so the
//!   broker's key resolution, the fetch's `{KEY}` fill and its existing redaction all ride
//!   along unchanged — a resolved key exists only inside `Request::url`, wherever it came from.
//! * **The floor refuses in words** (O-3): [`Overrides::set_cadence`] answers `Err(words)` and
//!   KEEPS THE OLD VALUE below the 10 s floor — a 5 s entry never becomes a 5 s cadence, and
//!   never a silent 10 s one either. The registry cadences (60–1800 s) stay the polite
//!   defaults; the floor exists because the operator's 1000-calls/h NASA key covers 10 s polls
//!   on the three NASA feeds even together (3 × 360/h = 1080/h at the worst alignment, and the
//!   one-retry-on-429 policy plus the attempt-counting cadence floors absorb the overlap).
//! * **Control-thread-only I/O** (D18): loaded at launch, written on edit, never read on an
//!   audio path; nothing here reads a clock (the plane's D11 firewall holds).
//! * **The file is user data with the env var's own threat model** (O-4): the key sits in it in
//!   plain text — and appears NOWHERE else. This type's `Debug` masks every key (`••••last4`,
//!   the D20 UI form), the UI and the log print the key's STATE words
//!   ([`key_words`]: `SET (env)` / `SET (file)` / `UNSET` / `KEY NEEDED`), and the store never
//!   rides a pack, a patch, a journal or an at-rest artefact.
//!
//! The file's shape is an array of tables (the house TOML subset's strongest shape — quoted
//! dotted keys are NOT in the subset, and stream ids are dotted):
//!
//! ```toml
//! [[override]]
//! id = "nasa.neo"
//! key = "…"          # optional; plain text, this file only (O-4)
//! cadence_s = 30     # optional; floor 10 (O-3)
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::registry::{Registry, StreamDef};

/// The cadence floor (ruling O-3), seconds: a panel or the tab may override a stream anywhere
/// from here upward; below it the field refuses in words and keeps the old value.
pub const CADENCE_FLOOR_S: u32 = 10;

/// The store file's name, inside `sparq/streams.d/` in the user-data dir (D18).
pub const STORE_FILE: &str = "overrides.toml";

/// One stream's override row. The key is PRIVATE to this module by design: the only doors the
/// value walks out of are the env closure (into `Request::url`, the one legal place) and the
/// file itself; every reading surface gets [`StreamOverride::masked_key`] or the state words.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct StreamOverride {
    key: Option<String>,
    /// The cadence override in seconds (`None` = the registry's cadence governs). Always
    /// ≥ [`CADENCE_FLOOR_S`] when set — the setter refuses below it.
    pub cadence_s: Option<u32>,
}

impl StreamOverride {
    /// Whether a file key is set for this row.
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.key.as_ref().is_some_and(|k| !k.is_empty())
    }

    /// The key's masked form (D20: `••••last4`) — what a UI may show. `None` when unset.
    #[must_use]
    pub fn masked_key(&self) -> Option<String> {
        self.key.as_ref().filter(|k| !k.is_empty()).map(|k| mask(k))
    }
}

impl fmt::Debug for StreamOverride {
    /// The Debug that never leaks: the key prints masked (the store's redaction rule — a
    /// `{:?}` in a log line is exactly the leak D18 forbids).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamOverride")
            .field("key", &self.masked_key())
            .field("cadence_s", &self.cadence_s)
            .finish()
    }
}

/// The mask (D20's `••••last4`): the last four characters at most, never more.
fn mask(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    if tail.len() == key.chars().count() {
        "••••".to_string()
    } else {
        format!("••••{tail}")
    }
}

/// The overrides store: the rows plus the path they were loaded from / will be saved to.
///
/// Cheap to clone (the UI thread may hold a copy); the file is written only by [`Self::save`],
/// on an edit, on the control thread (D18).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    rows: BTreeMap<String, StreamOverride>,
    path: PathBuf,
}

impl fmt::Debug for Overrides {
    /// The Debug that never leaks: rows print through [`StreamOverride`]'s masked form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Overrides").field("path", &self.path).field("rows", &self.rows).finish()
    }
}

/// The key's STATE words (D18/D20) — what the UI shows and the log prints, never the value.
/// `env` must be the REAL environment (not the composed closure): the precedence is visible,
/// not a mystery — an env-set key shows `SET (env)` and the file's field is disabled with
/// those words.
///
/// The vocabulary, exactly: `SET (env)` · `SET (file)` · `UNSET` (the row declares a fallback —
/// NASA's `DEMO_KEY` — and runs on it) · `KEY NEEDED` (no key, no fallback: the stream is not
/// fetched and says so) · `—` (the stream needs no key).
///
/// File keys resolve PER ENV VAR: the three NASA streams share `SPARQ_NASA_API_KEY`, so a key
/// typed on ANY of their rows serves all of them (the first row with a key, in registry order,
/// governs — the closure's own rule) and every sibling row shows the same honest state. One
/// variable, one effective key; the store stays keyed per stream id (D18) because that is what
/// the row you typed on shows.
#[must_use]
pub fn key_words<F>(def: &StreamDef, registry: &Registry, env: F, store: &Overrides) -> &'static str
where
    F: Fn(&str) -> Option<String>,
{
    let Some(var) = &def.key_env else { return "—" };
    if env(var).is_some_and(|v| !v.trim().is_empty()) {
        return "SET (env)";
    }
    if store.key_by_env(registry, var).is_some() {
        return "SET (file)";
    }
    match &def.key_fallback {
        Some(fb) if !fb.trim().is_empty() => "UNSET",
        _ => "KEY NEEDED",
    }
}

impl Overrides {
    /// The default store path: `SPARQ_STREAMS_OVERRIDES` if set (the cache's env-door idiom —
    /// tests and review builds point it at a temp dir), else the OS user-data dir +
    /// `/sparq/streams.d/overrides.toml` — beside the streams cache, as D18 rules.
    #[must_use]
    pub fn default_path() -> PathBuf {
        Self::default_path_with(|k| std::env::var(k).ok())
    }

    /// [`Self::default_path`] over an injected environment (the hermetic door — the plane
    /// never reads a wall clock, and its tests never read the real env either).
    #[must_use]
    pub fn default_path_with<F>(env: F) -> PathBuf
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(dir) = env("SPARQ_STREAMS_OVERRIDES") {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir);
            }
        }
        crate::cache::user_data_dir_with(env).join("sparq").join("streams.d").join(STORE_FILE)
    }

    /// Loads the store from `path`. A MISSING file is an empty store with no words (a fresh
    /// machine has no overrides — that is not a defect). A MALFORMED file is an empty store
    /// WITH words (the §6.3 rule: the app never dies because a data file died; the launch log
    /// carries the sentence and the fields show their states honestly).
    #[must_use]
    pub fn load(path: PathBuf) -> (Self, Option<String>) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (Self { rows: BTreeMap::new(), path }, None);
        };
        Self::parse(&text, path)
    }

    /// Loads from [`Self::default_path`].
    #[must_use]
    pub fn load_default() -> (Self, Option<String>) {
        let path = Self::default_path();
        Self::load(path)
    }

    /// Parses store text (the testable half of [`Self::load`], against hostile inputs).
    #[must_use]
    pub fn parse(text: &str, path: PathBuf) -> (Self, Option<String>) {
        let parsed = match sparq_module_api::toml::parse(text) {
            Ok(t) => t,
            Err(e) => {
                return (
                    Self { rows: BTreeMap::new(), path },
                    Some(format!("overrides store refused (TOML syntax): {e} — starting empty")),
                );
            },
        };
        let mut rows = BTreeMap::new();
        let mut notes = Vec::new();
        for row in parsed.tables("override").unwrap_or_default() {
            let Some(id) = row.get("id").and_then(|v| v.as_str()) else {
                notes.push("a row without `id` was skipped".to_string());
                continue;
            };
            let key = row
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .filter(|k| !k.is_empty());
            let cadence_s = row
                .get("cadence_s")
                .and_then(|v| v.as_i64())
                .and_then(|v| u32::try_from(v).ok().filter(|c| *c >= CADENCE_FLOOR_S));
            if rows.contains_key(id) {
                notes.push(format!("duplicate id `{id}` — the first row governs"));
                continue;
            }
            rows.insert(id.to_string(), StreamOverride { key, cadence_s });
        }
        let words = (!notes.is_empty()).then(|| {
            format!("overrides store: {} — starting without the noted rows", notes.join("; "))
        });
        (Self { rows, path }, words)
    }

    /// The path this store loads from / saves to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The row count (the tab's header diagnostic, tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The cadence override for a stream (`None` = the registry's cadence governs).
    #[must_use]
    pub fn cadence_s(&self, stream_id: &str) -> Option<u32> {
        self.rows.get(stream_id).and_then(|r| r.cadence_s)
    }

    /// Whether a FILE key is set for a stream (the state words' `SET (file)` input).
    #[must_use]
    pub fn has_key(&self, stream_id: &str) -> bool {
        self.rows.get(stream_id).is_some_and(|r| r.has_key())
    }

    /// The masked form of a stream's file key (D20's `••••last4`), `None` when unset. The
    /// VALUE has no reading door — the env closure and the file are its only exits.
    #[must_use]
    pub fn masked_key(&self, stream_id: &str) -> Option<String> {
        self.rows.get(stream_id).and_then(StreamOverride::masked_key)
    }

    /// Sets or clears a stream's file key (`None`/empty clears). Writing is the caller's next
    /// step ([`Self::save`]) — the store is memory until the control thread says otherwise.
    pub fn set_key(&mut self, stream_id: &str, key: Option<String>) {
        let key = key.filter(|k| !k.trim().is_empty());
        let row = self.rows.entry(stream_id.to_string()).or_default();
        row.key = key;
        if *row == StreamOverride::default() {
            self.rows.remove(stream_id); // an all-empty row is no row (the file stays honest)
        }
    }

    /// Sets or clears a stream's cadence override — or REFUSES in words below the O-3 floor,
    /// keeping the old value (the field's behaviour, exactly: "a 5 s entry refuses in words").
    ///
    /// # Errors
    /// The refusal sentence, when `secs` is below [`CADENCE_FLOOR_S`].
    pub fn set_cadence(&mut self, stream_id: &str, secs: Option<u32>) -> Result<(), String> {
        if let Some(s) = secs {
            if s < CADENCE_FLOOR_S {
                return Err(format!(
                    "{s} s is below the {CADENCE_FLOOR_S} s floor (ruling O-3) — the cadence keeps its old value"
                ));
            }
        }
        let row = self.rows.entry(stream_id.to_string()).or_default();
        row.cadence_s = secs;
        if *row == StreamOverride::default() {
            self.rows.remove(stream_id);
        }
        Ok(())
    }

    /// Writes the store to its path (tmp + rename — a partially written store is never
    /// observable, the project format's atomicity rule). Creates the directory.
    ///
    /// # Errors
    /// The I/O failure, in words (a store that cannot save says so; the edit stays in memory
    /// for this session and the log carries the sentence).
    pub fn save(&self) -> Result<(), String> {
        use std::io::Write;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let tmp = self.path.with_extension("toml.tmp");
        // File::create + write_all — the cache's own idiom under the clippy.toml ban on the
        // fs::write convenience (control-thread I/O, stated where it happens).
        let mut f = std::fs::File::create(&tmp)
            .map_err(|e| format!("cannot write the store ({}): {e}", tmp.display()))?;
        f.write_all(self.to_toml().as_bytes())
            .map_err(|e| format!("cannot write the store: {e}"))?;
        f.sync_all().map_err(|e| format!("cannot flush the store: {e}"))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("cannot move the store into place: {e}"))
    }

    /// The store as TOML text (the [`Self::parse`] round-trip's other half). Rows in id order
    /// (the `BTreeMap`'s), fields in a fixed order — the file diffs like the graph part of a
    /// project (deterministic, reviewable).
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut out = String::from(
            "# sparq stream overrides (WO-020 INC6 D18) — keys and cadences, per stream id.\n\
             # Written by the shell's STREAMS tab and the card's RATE field; env vars WIN over\n\
             # the keys here (ruling O-4). USER DATA with the env var's own threat model: the\n\
             # key lives in this file and nowhere else — it never rides a pack, a patch, a\n\
             # journal, a log line or an at-rest artefact.\n",
        );
        for (id, row) in &self.rows {
            out.push_str("\n[[override]]\n");
            out.push_str(&format!("id = {}\n", quote(id)));
            if let Some(k) = &row.key {
                out.push_str(&format!("key = {}\n", quote(k)));
            }
            if let Some(c) = row.cadence_s {
                out.push_str(&format!("cadence_s = {c}\n"));
            }
        }
        out
    }

    /// The broker's env closure (D18: "the broker's env closure reads key-store-then-env",
    /// with env WINNING per O-4): the real environment first, then this store's key for any
    /// stream whose `key_env` names the variable. Composed once by the driver (control thread);
    /// the fetch path's `{KEY}` fill and its redaction ride it unchanged.
    pub fn env_closure_over<'a, F>(
        &'a self,
        registry: &'a Registry,
        real_env: F,
    ) -> impl Fn(&str) -> Option<String> + 'a
    where
        F: Fn(&str) -> Option<String> + 'a,
    {
        move |name: &str| {
            if let Some(v) = real_env(name) {
                if !v.trim().is_empty() {
                    return Some(v);
                }
            }
            self.key_by_env(registry, name).map(str::to_string)
        }
    }

    /// [`Self::env_closure_over`] over the real process environment (the driver's door).
    pub fn env_closure<'a>(
        &'a self,
        registry: &'a Registry,
    ) -> impl Fn(&str) -> Option<String> + 'a {
        self.env_closure_over(registry, |k| std::env::var(k).ok())
    }

    /// The file key behind an env-var NAME: the first registry stream declaring that variable
    /// whose row carries a key (streams sharing one variable share one key — the registry's
    /// own vocabulary; the closure's rule, public so the state words and the tab's reading
    /// resolve exactly what a fetch would).
    #[must_use]
    pub fn key_by_env<'a>(&'a self, registry: &'a Registry, name: &str) -> Option<&'a str> {
        registry.streams().iter().find_map(|def| {
            if def.key_env.as_deref() != Some(name) {
                return None;
            }
            self.rows.get(&def.id)?.key.as_deref().filter(|k| !k.is_empty())
        })
    }

    /// The masked form of the EFFECTIVE file key behind a stream's env var (what the tab's
    /// field shows — the sibling-sharing rule of [`Self::key_by_env`], masked).
    #[must_use]
    pub fn masked_key_for_env(&self, registry: &Registry, var: &str) -> Option<String> {
        self.key_by_env(registry, var).map(mask)
    }
}

/// A TOML basic-string quoting for the store's two secret-bearing lines (ids carry dots and
/// dashes only; keys can carry anything the provider issues).
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("sparq-store-test-{}-{name}.toml", std::process::id()));
        p
    }

    #[test]
    fn the_round_trip_is_exact_and_the_file_carries_the_provenance_words() {
        let path = temp_store("roundtrip");
        let _ = std::fs::remove_file(&path);
        let mut s = Overrides::load(path.clone()).0;
        assert!(s.is_empty(), "a missing file is an empty store, no words");
        s.set_key("space.neo", Some("SENTINEL-KEY-9876".to_string()));
        s.set_cadence("space.neo", Some(30)).unwrap();
        s.set_cadence("swpc.kp", Some(120)).unwrap();
        s.save().unwrap();
        let (back, words) = Overrides::load(path.clone());
        assert!(words.is_none(), "a well-formed store loads without words");
        assert_eq!(back, s, "the round trip is exact");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("SENTINEL-KEY-9876"), "the FILE carries the key — that is O-4");
        assert!(text.contains("ruling O-4"), "…and its provenance words");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_floor_refuses_in_words_and_keeps_the_old_value() {
        let mut s = Overrides::load(temp_store("floor")).0;
        s.set_cadence("space.neo", Some(60)).unwrap();
        let e = s.set_cadence("space.neo", Some(5)).unwrap_err();
        assert!(e.contains("10 s floor") && e.contains("old value"), "{e}");
        assert_eq!(s.cadence_s("space.neo"), Some(60), "the 5 s entry changed nothing");
        assert_eq!(s.set_cadence("space.neo", Some(10)), Ok(()), "the floor itself is legal (O-3)");
        assert_eq!(s.cadence_s("space.neo"), Some(10));
        s.set_cadence("space.neo", None).unwrap();
        assert_eq!(s.cadence_s("space.neo"), None, "clearing returns the registry cadence");
    }

    #[test]
    fn env_wins_over_the_file_and_the_words_say_which_governs() {
        let reg = Registry::load().unwrap();
        let neo = reg.get("space.neo").expect("the registry carries space.neo");
        let var = neo.key_env.as_deref().expect("space.neo declares SPARQ_NASA_API_KEY");
        let mut s = Overrides::load(temp_store("precedence")).0;
        s.set_key("space.neo", Some("FILE-KEY-abcdef".to_string()));
        // fn items, not closures: the plane's env door is higher-ranked over the name's
        // lifetime, and a closure's inferred signature would pin it (the broker tests' idiom).
        fn no_env(_k: &str) -> Option<String> {
            None
        }
        fn with_env(_k: &str) -> Option<String> {
            Some("ENV-KEY-xyz".to_string())
        }
        // The composed closure: env wins (O-4); the file serves when the env is unset.
        let composed_empty = s.env_closure_over(&reg, no_env);
        assert_eq!(composed_empty(var).as_deref(), Some("FILE-KEY-abcdef"));
        let composed_env = s.env_closure_over(&reg, with_env);
        assert_eq!(composed_env(var).as_deref(), Some("ENV-KEY-xyz"));
        // The state words name the governing source — the precedence is visible, not a mystery.
        assert_eq!(key_words(neo, &reg, with_env, &s), "SET (env)");
        assert_eq!(key_words(neo, &reg, no_env, &s), "SET (file)");
        let empty = Overrides::default();
        assert_eq!(key_words(neo, &reg, no_env, &empty), "UNSET", "NASA's DEMO_KEY fallback");
        let keyless = reg.streams().iter().find(|d| d.key_env.is_none()).unwrap();
        assert_eq!(key_words(keyless, &reg, no_env, &empty), "—", "no key needed, no key words");
        // FIRMS declares its key with NO fallback: the D14 KEY NEEDED refusal, in words.
        let firms = reg.get("geo.firms").expect("the registry carries geo.firms");
        assert_eq!(key_words(firms, &reg, no_env, &empty), "KEY NEEDED");
        // The sibling-sharing rule: a key typed on the neo row serves epic and power too —
        // one variable, one effective key — and their rows show the same honest state.
        let epic = reg.get("space.epic").unwrap();
        assert_eq!(key_words(epic, &reg, no_env, &s), "SET (file)", "the sibling's key governs");
        assert_eq!(s.masked_key_for_env(&reg, var).as_deref(), Some("••••cdef"));
        assert_eq!(composed_empty(var).as_deref(), Some("FILE-KEY-abcdef"));
    }

    #[test]
    fn the_mask_shows_at_most_the_last_four() {
        let mut s = Overrides::default();
        s.set_key("a.b", Some("ABCDEFGH".to_string()));
        assert_eq!(s.masked_key("a.b").as_deref(), Some("••••EFGH"));
        s.set_key("c.d", Some("xy".to_string()));
        assert_eq!(s.masked_key("c.d").as_deref(), Some("••••"), "a short key shows no tail");
        s.set_key("e.f", Some("   ".to_string()));
        assert!(!s.has_key("e.f"), "a whitespace key is no key");
    }

    #[test]
    fn debug_never_carries_the_value() {
        // The redaction rule's first line of defence: a {:?} in ANY log line must not leak.
        let mut s = Overrides::default();
        s.set_key("space.neo", Some("SENTINEL-do-not-log-1234".to_string()));
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("SENTINEL-do-not-log-1234"), "{dbg}");
        assert!(dbg.contains("••••1234"), "the masked form is what Debug shows: {dbg}");
        let row_dbg = format!("{:?}", s.rows.get("space.neo").unwrap());
        assert!(!row_dbg.contains("SENTINEL"), "{row_dbg}");
    }

    #[test]
    fn a_malformed_store_is_an_empty_store_with_words_never_a_crash() {
        let (s, words) = Overrides::parse("this is not [toml", PathBuf::from("x.toml"));
        assert!(s.is_empty());
        assert!(
            words.as_ref().is_some_and(|w| w.contains("refused") && w.contains("starting empty")),
            "{words:?}"
        );
        // A row without an id and a duplicate id are noted, not fatal.
        let (s2, w2) = Overrides::parse(
            "[[override]]\ncadence_s = 30\n\n[[override]]\nid = \"a.b\"\ncadence_s = 15\n\n[[override]]\nid = \"a.b\"\ncadence_s = 90\n",
            PathBuf::from("y.toml"),
        );
        assert_eq!(s2.cadence_s("a.b"), Some(15), "the first row governs");
        assert!(
            w2.as_ref().is_some_and(|w| w.contains("without `id`") && w.contains("duplicate")),
            "{w2:?}"
        );
        // A below-floor cadence in the FILE is dropped at load (the floor governs the file too —
        // a hand-edited 5 never becomes a live 5).
        let (s3, _) = Overrides::parse(
            "[[override]]\nid = \"a.b\"\ncadence_s = 5\n",
            PathBuf::from("z.toml"),
        );
        assert_eq!(s3.cadence_s("a.b"), None, "a below-floor file row loads as no override");
    }

    #[test]
    fn the_default_path_honours_the_env_door_and_sits_beside_the_cache() {
        let p = Overrides::default_path_with(|k| {
            (k == "SPARQ_STREAMS_OVERRIDES").then(|| "/tmp/custom-store.toml".to_string())
        });
        assert_eq!(p, PathBuf::from("/tmp/custom-store.toml"));
        let p2 =
            Overrides::default_path_with(|k| (k == "HOME").then(|| "/home/tester".to_string()));
        assert!(
            p2.ends_with("sparq/streams.d/overrides.toml"),
            "the user-data location D18 rules: {p2:?}"
        );
    }

    #[test]
    fn the_file_key_reaches_the_request_and_no_host_surface_carries_it() {
        // The redaction acceptance (O-4/D18), grepped not hoped: the key exists ONLY inside the
        // request URL — the fetch metadata, the broker's outcome words, the broker's own Debug
        // and the store's Debug are all searched for the sentinel and must not carry it.
        use crate::broker::Broker;
        use crate::fetch::{self, Transport, TransportResponse};
        use std::cell::RefCell;

        struct Capture {
            urls: RefCell<Vec<String>>,
        }
        impl Transport for Capture {
            fn get(&self, url: &str) -> TransportResponse {
                self.urls.borrow_mut().push(url.to_string());
                TransportResponse {
                    status: 404,
                    content_type: String::new(),
                    body: String::new(),
                    note: "HTTP 404".to_string(),
                }
            }
        }

        const SENTINEL: &str = "SENTINEL-do-not-log-9876";
        let reg = Registry::load().unwrap();
        let mut s = Overrides::default();
        s.set_key("space.neo", Some(SENTINEL.to_string()));
        fn no_env(_k: &str) -> Option<String> {
            None
        }
        let env = s.env_closure_over(&reg, no_env);
        let cap = Capture { urls: RefCell::new(Vec::new()) };
        let no_sleep = |_: u64| {};
        // The fetch layer: the real request carries the key (its ONE legal place); the
        // RawFetch the host keeps is `{KEY}`-redacted (fetch.rs's existing rule, now riding
        // a FILE key through the composed closure).
        let raw = fetch::fetch_raw(&reg, "space.neo", &cap, 1_700_000_000, &env, no_sleep).unwrap();
        assert!(
            cap.urls.borrow().iter().any(|u| u.contains(SENTINEL)),
            "the request URL is the one legal place for the resolved key"
        );
        assert!(
            !raw.url.contains(SENTINEL) && raw.url.contains("{KEY}"),
            "the fetch metadata is redacted: {}",
            raw.url
        );
        let raw_dbg = format!("{raw:?}");
        assert!(!raw_dbg.contains(SENTINEL), "…and so is its Debug");
        // The broker layer: the failure words and the broker's state never carry the value.
        let mut b = Broker::new(reg.clone(), None);
        let out = b.poll(&["space.neo".into()], &cap, 1_700_000_000, &env, no_sleep);
        assert!(
            !out.iter().any(|o| o.words.contains(SENTINEL)),
            "the outcome words are clean: {out:?}"
        );
        assert!(
            b.words("space.neo").map_or(true, |w| !w.contains(SENTINEL)),
            "the standing words are clean"
        );
        let broker_dbg = format!("{b:?}");
        assert!(!broker_dbg.contains(SENTINEL), "the broker holds no key");
    }

    #[test]
    fn the_override_paces_due_and_the_second_door_refuses_below_the_floor() {
        // D18: `Broker::due` reads the override-then-registry cadence, and the floor still
        // counts ATTEMPTS — a 10 s override makes a 1800 s feed due every 10 s, and the
        // broker's own setter refuses below the floor in words (no silent door).
        use crate::broker::Broker;
        use crate::fetch::{Transport, TransportResponse};
        use std::cell::RefCell;

        struct Fail404 {
            calls: RefCell<usize>,
        }
        impl Transport for Fail404 {
            fn get(&self, _url: &str) -> TransportResponse {
                *self.calls.borrow_mut() += 1;
                TransportResponse {
                    status: 404,
                    content_type: String::new(),
                    body: String::new(),
                    note: "HTTP 404".to_string(),
                }
            }
        }

        let reg = Registry::load().unwrap();
        assert_eq!(reg.get("space.neo").unwrap().cadence_s, 1800, "the registry default");
        let mut b = Broker::new(reg.clone(), None);
        let e = b.set_cadence_override("space.neo", Some(5)).unwrap_err();
        assert!(e.contains("floor") && e.contains("old value"), "{e}");
        b.set_cadence_override("space.neo", Some(10)).unwrap();
        assert_eq!(b.effective_cadence(reg.get("space.neo").unwrap()), 10);
        fn no_env(_k: &str) -> Option<String> {
            None
        }
        let t = 1_700_000_000i64;
        let cap = Fail404 { calls: RefCell::new(0) };
        let no_sleep = |_: u64| {};
        // One attempt at t (a failure — attempts, not successes, pace the floor).
        let out = b.poll(&["space.neo".into()], &cap, t, no_env, no_sleep);
        assert!(!out.is_empty() && !out[0].ok, "the 404 attempt is recorded as a failure");
        assert_eq!(*cap.calls.borrow(), 1);
        assert_eq!(b.last_attempt("space.neo"), Some(t));
        // Due at t+10, NOT at t+1800: the override paces.
        assert!(!b.due(t + 9, no_env).contains(&"space.neo".to_string()));
        assert!(b.due(t + 10, no_env).contains(&"space.neo".to_string()));
        // Clearing returns the registry's polite default.
        b.set_cadence_override("space.neo", None).unwrap();
        assert!(!b.due(t + 10, no_env).contains(&"space.neo".to_string()));
        assert!(b.due(t + 1800, no_env).contains(&"space.neo".to_string()));
    }
}
