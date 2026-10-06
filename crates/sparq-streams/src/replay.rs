//! Fixture replay — the hermetic door (plan D11, §6.1).
//!
//! Goldens, tests and the at-rest shell all need live-shaped data *without* a socket. This module
//! reads the recorded fixtures (`reference/fixtures/observatory/`, captured by
//! `tools/streams_record.py`), runs each body through [`crate::normalize`], and fills a
//! [`crate::window::Window`] — so a golden render is bit-exact and reproducible, and a network
//! outage changes what is displayed but never whether the render is deterministic.
//!
//! The fixture is the recorder's raw payload plus a `_meta.json` sidecar (status, fetch time);
//! replay uses the recorded
//! `fetched_unix` as the fetch fact and an injected `now_unix` (defaulting to it) as the evaluation
//! clock, so the same fixture can be replayed LIVE (now = fetched) or STALE/OFFLINE (now later)
//! without touching a clock — the determinism firewall, applied.

use std::path::Path;

use crate::normalize;
use crate::registry::Registry;
use crate::window::Window;

/// One recorded fixture: the fetch metadata from the `_meta.json` sidecar plus the raw payload body,
/// as written by `tools/streams_record.py`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    /// The stream id the fixture belongs to.
    pub stream_id: String,
    /// The resolved endpoint that was fetched.
    pub endpoint: String,
    /// The ISO fetch stamp (informational).
    pub fetched_utc: String,
    /// Unix seconds of the fetch — replay's default evaluation clock and the record `now`.
    pub fetched_unix: i64,
    /// The HTTP status recorded (200 for a good payload; 0 for a transport error).
    pub status: u16,
    /// The response content type.
    pub content_type: String,
    /// The recorded byte length.
    pub byte_length: usize,
    /// Which key source was used (`none` / `fallback:…` / `env:…`) — never a secret.
    pub key_source: String,
    /// Whether the body is a flagged synthetic sample (the keyless-FIRMS case).
    pub synthetic: bool,
    /// Transport-error or synthetic-sample note.
    pub note: String,
    /// The raw payload body, verbatim.
    pub body: String,
}

impl Fixture {
    /// Builds a fixture from a `_meta.json` entry plus the raw payload body. Malformed or missing
    /// metadata reads as a replay error (in words), never a panic.
    fn from_meta(id: &str, entry: &serde_json::Value, body: String) -> Option<Self> {
        let s =
            |k: &str| entry.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        Some(Self {
            stream_id: id.to_string(),
            endpoint: s("endpoint"),
            fetched_utc: s("fetched_utc"),
            fetched_unix: entry
                .get("fetched_unix")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0),
            status: entry.get("status").and_then(serde_json::Value::as_u64).map_or(0, |n| n as u16),
            content_type: s("content_type"),
            byte_length: entry.get("byte_length").and_then(serde_json::Value::as_u64).unwrap_or(0)
                as usize,
            key_source: s("key_source"),
            synthetic: entry.get("synthetic").and_then(serde_json::Value::as_bool).unwrap_or(false),
            note: s("note"),
            body,
        })
    }
}

/// The `_meta.json` sidecar the recorder writes beside the raw payload files.
const META_FILE: &str = "_meta.json";

/// Reads the `_meta.json` sidecar as a JSON object mapping stream id → fetch metadata.
fn load_meta(dir: &Path) -> Result<serde_json::Value, ReplayError> {
    let path = dir.join(META_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| ReplayError {
        stream_id: META_FILE.to_string(),
        message: format!(
            "no fixture metadata at {} ({e}) — record fixtures first: python3 tools/streams_record.py --all",
            path.display()
        ),
    })?;
    serde_json::from_str(&text).map_err(|e| ReplayError {
        stream_id: META_FILE.to_string(),
        message: format!("fixture metadata `{META_FILE}` is malformed: {e}"),
    })
}

/// A replay failure, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayError {
    /// The stream id that could not be replayed.
    pub stream_id: String,
    /// What went wrong (a missing fixture, an unknown id, a normalizer refusal).
    pub message: String,
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "replay `{}`: {}", self.stream_id, self.message)
    }
}

impl std::error::Error for ReplayError {}

/// Reads one stream's fixture from `dir`: its metadata from `_meta.json` and its raw payload from
/// the file the metadata names. The raw file IS the recorded bytes (no envelope), so a fixture is
/// byte-faithful to the live payload — which is what makes the ≤ 1.2 × recorded-size acceptance hold
/// at ratio ~1.0 and re-recording diff cleanly.
///
/// # Errors
/// A [`ReplayError`] when the metadata, the entry, or the raw payload file is absent or malformed.
pub fn load_fixture(dir: &Path, stream_id: &str) -> Result<Fixture, ReplayError> {
    let meta = load_meta(dir)?;
    let entry = meta.get(stream_id).ok_or_else(|| ReplayError {
        stream_id: stream_id.to_string(),
        message: format!("no `{stream_id}` entry in {META_FILE} (was it recorded?)"),
    })?;
    let file =
        entry.get("file").and_then(serde_json::Value::as_str).ok_or_else(|| ReplayError {
            stream_id: stream_id.to_string(),
            message: "metadata entry has no `file` field".to_string(),
        })?;
    let body = std::fs::read_to_string(dir.join(file)).map_err(|e| ReplayError {
        stream_id: stream_id.to_string(),
        message: format!("cannot read raw payload `{file}` ({e})"),
    })?;
    Fixture::from_meta(stream_id, entry, body).ok_or_else(|| ReplayError {
        stream_id: stream_id.to_string(),
        message: "fixture metadata is malformed (expected the streams_record.py shape)".to_string(),
    })
}

/// The stream ids that have a fixture in `dir` (sorted), read from the `_meta.json` sidecar. Used by
/// tests and the `sparq streams` diagnostics to report what is replayable.
#[must_use]
pub fn fixture_ids(dir: &Path) -> Vec<String> {
    let Ok(meta) = load_meta(dir) else { return Vec::new() };
    let Some(obj) = meta.as_object() else { return Vec::new() };
    let mut ids: Vec<String> = obj.keys().cloned().collect();
    ids.sort();
    ids
}

/// Replays one stream's fixture into a [`Window`]. `now_unix` (defaulting to the fixture's fetch
/// time) is the evaluation clock: at the fetch time the window reads LIVE; a later `now` reads STALE
/// then OFFLINE per the window's policy, with the records' `age_s` grown to match.
///
/// A fixture whose recorded `status` is not 200 replays as an **empty, un-fetched** window — the
/// honest OFFLINE state, never a normalizer run over an error page.
///
/// # Errors
/// A [`ReplayError`] for a missing/malformed fixture, an id absent from the registry, or a
/// normalizer refusal (all in words).
pub fn replay_window(
    reg: &Registry,
    dir: &Path,
    stream_id: &str,
    now_unix: Option<i64>,
) -> Result<Window, ReplayError> {
    let fix = load_fixture(dir, stream_id)?;
    let def = reg.get(stream_id).ok_or_else(|| ReplayError {
        stream_id: stream_id.to_string(),
        message: "not in the registry (a fixture for an unknown stream is a stale artefact)"
            .to_string(),
    })?;
    let mut window = Window::for_stream(def);
    if fix.status != 200 {
        // A failed fetch has no good payload: an empty window the caller reads as OFFLINE.
        return Ok(window);
    }
    let now = now_unix.unwrap_or(fix.fetched_unix);
    let records = normalize::normalize(def, &fix.body, now)
        .map_err(|e| ReplayError { stream_id: stream_id.to_string(), message: e.message })?;
    window.extend(records);
    window.mark_fetched(fix.fetched_unix);
    Ok(window)
}

/// Replays every stream in the registry that has a fixture, returning `(id, result)` pairs so a
/// caller (the shell's `ReplayProvider`, INC4) can build the whole wall and report per-cell failures
/// in words rather than aborting on the first.
#[must_use]
pub fn replay_all(
    reg: &Registry,
    dir: &Path,
    now_unix: Option<i64>,
) -> Vec<(String, Result<Window, ReplayError>)> {
    reg.ids()
        .into_iter()
        .map(|id| (id.to_string(), replay_window(reg, dir, id, now_unix)))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::record::DataValue;
    use crate::window::StreamStatus;

    fn fixture_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/fixtures/observatory")
    }

    #[test]
    fn the_recorded_fixture_set_is_present() {
        let dir = fixture_dir();
        assert!(dir.is_dir(), "fixtures must be checked in at {}", dir.display());
        let ids = fixture_ids(&dir);
        assert!(ids.len() >= 23, "expected the 23 recorded fixtures, found {}", ids.len());
        assert!(ids.contains(&"swpc.wwv".to_string()));
    }

    #[test]
    fn a_text_fixture_replays_to_a_live_window_at_its_fetch_time() {
        let reg = Registry::load().unwrap();
        let w =
            replay_window(&reg, &fixture_dir(), "swpc.wwv", None).expect("wwv fixture must replay");
        assert_eq!(w.len(), 1, "a raw-text bulletin is one record");
        let r = w.latest().unwrap();
        assert_eq!(r.schema_id, crate::schema::TEXT_ID);
        // The record's own fetch time is the default clock, so it reads LIVE with age 0-ish.
        assert_eq!(w.status(w.last_fetch_unix().unwrap()), StreamStatus::Live);
    }

    #[test]
    fn replaying_at_a_later_clock_goes_stale_then_offline() {
        let reg = Registry::load().unwrap();
        let w = replay_window(&reg, &fixture_dir(), "swpc.kp", None).unwrap();
        let fetched = w.last_fetch_unix().unwrap();
        // Kp cadence is 300 s: +900 s (3×) is STALE, +3000 s (10×) is OFFLINE.
        assert_eq!(w.status(fetched + 899), StreamStatus::Live);
        assert_eq!(w.status(fetched + 900), StreamStatus::Stale);
        assert_eq!(w.status(fetched + 3000), StreamStatus::Offline);
    }

    #[test]
    fn a_position_fixture_replays_with_the_unit_conversion_applied() {
        let reg = Registry::load().unwrap();
        let w = replay_window(&reg, &fixture_dir(), "sat.iss", None).unwrap();
        let r = w.latest().unwrap();
        assert_eq!(r.schema_id, crate::schema::POSITION_ID);
        // Channel 3 is vel_kms; the ISS orbits at ~7.66 km/s (the API's km/h ÷ 3600).
        if let Some(DataValue::Double(v)) = r.channel(3) {
            assert!((7.0..8.5).contains(v), "ISS vel_kms should be ~7.66, got {v}");
        } else {
            panic!("position channel 3 must be a Double vel_kms");
        }
    }

    #[test]
    fn replay_all_covers_the_whole_registry() {
        let reg = Registry::load().unwrap();
        let all = replay_all(&reg, &fixture_dir(), None);
        assert_eq!(all.len(), reg.len());
        // Every recorded stream (all 200) replays without error; a failure here is a real drift.
        for (id, res) in &all {
            assert!(res.is_ok(), "replay failed for {id}: {:?}", res.as_ref().err());
        }
    }

    #[test]
    fn an_unknown_stream_refuses_in_words() {
        let reg = Registry::load().unwrap();
        let e = replay_window(&reg, &fixture_dir(), "nope.nope", None)
            .expect_err("unknown id must refuse");
        assert!(
            e.message.contains("nope.nope")
                && (e.message.contains("_meta.json") || e.message.contains("registry")),
            "{e}"
        );
    }
}
