//! The disk cache: last-good payloads + fetch metadata, so a restart shows the wall immediately.
//!
//! The broker is cache-first (plan §11.1): on boot it serves the last-good payload with STALE flags
//! while the first live round runs, so a cold start is never a blank wall and a restart storm never
//! hammers a rate-limited feed (the DEMO_KEY budget arithmetic depends on it). The cache also
//! absorbs the "network is down" case honestly — the wall shows the last good data, flagged STALE
//! then OFFLINE in words, rather than a hole.
//!
//! # Location (plan §6.2)
//!
//! `SPARQ_STREAMS_CACHE` overrides; otherwise the OS user-data dir + `/sparq/streams-cache`
//! (resolved from environment only — no platform crate, keeping the zero-dep promise). Each entry is
//! `<root>/<stream-id>.json`, the same envelope shape the fixtures use, so a cached payload and a
//! recorded fixture are interchangeable to [`crate::replay`].
//!
//! # File I/O discipline
//!
//! `clippy.toml` bans `std::fs::read` and `std::fs::write` workspace-wide (they are the audio-path
//! hazards). This module is a control-thread concern, but it honours the ban literally: reads go
//! through `read_to_string`, writes through `File::create` + `write_all`. No clock is read here
//! either — `fetched_unix` is passed in by the caller (the transport half stamps it, INC5).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// One cached stream payload with its fetch metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheEntry {
    /// The stream id this entry belongs to.
    pub stream_id: String,
    /// Unix seconds of the fetch that produced this payload (stamped by the transport, not here).
    pub fetched_unix: i64,
    /// The HTTP status of that fetch (200 for a good payload).
    pub status: u16,
    /// The raw payload body, verbatim.
    pub body: String,
}

impl CacheEntry {
    /// Serialises to the on-disk envelope JSON. Built by hand (no serde derive — the crate's one
    /// dependency is `serde_json`, not `serde`), which is also why the shape is stable and inspectable.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut m = Map::new();
        m.insert("stream_id".to_string(), Value::String(self.stream_id.clone()));
        m.insert("fetched_unix".to_string(), Value::from(self.fetched_unix));
        m.insert("status".to_string(), Value::from(u64::from(self.status)));
        m.insert("body".to_string(), Value::String(self.body.clone()));
        // serde_json's to_string on a small map cannot fail in practice; if it somehow does, an
        // empty envelope is the honest fallback (a cache miss), never a panic.
        serde_json::to_string(&Value::Object(m)).unwrap_or_else(|_| "{}".to_string())
    }

    /// Parses the on-disk envelope. `None` when the JSON is malformed or missing a field — a corrupt
    /// cache entry reads as a miss, never as a crash.
    #[must_use]
    pub fn from_json(text: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(text).ok()?;
        Some(Self {
            stream_id: v.get("stream_id")?.as_str()?.to_string(),
            fetched_unix: v.get("fetched_unix")?.as_i64()?,
            status: u16::try_from(v.get("status")?.as_u64()?).ok()?,
            body: v.get("body")?.as_str()?.to_string(),
        })
    }
}

/// A directory-backed payload cache.
#[derive(Clone, Debug)]
pub struct Cache {
    root: PathBuf,
}

/// The cache's own failure, in words (a miss is `None`, not an error; this is for I/O that failed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheError {
    /// What operation failed.
    pub op: String,
    /// The OS message, in words.
    pub message: String,
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cache {}: {}", self.op, self.message)
    }
}

impl std::error::Error for CacheError {}

impl Cache {
    /// A cache rooted at `root` (created lazily on first write).
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The default cache root: `SPARQ_STREAMS_CACHE` if set, else the OS user-data dir +
    /// `/sparq/streams-cache`, resolved from environment only.
    #[must_use]
    pub fn default_root() -> PathBuf {
        if let Ok(dir) = std::env::var("SPARQ_STREAMS_CACHE") {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir);
            }
        }
        let base = user_data_dir();
        base.join("sparq").join("streams-cache")
    }

    /// Opens the default cache.
    #[must_use]
    pub fn open_default() -> Self {
        Self::new(Self::default_root())
    }

    /// The cache root path (for `sparq streams cache path`).
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of one stream's entry.
    #[must_use]
    pub fn entry_path(&self, stream_id: &str) -> PathBuf {
        // Stream ids are dotted names (swpc.kp); they are safe file stems, but guard the separator
        // anyway so a hostile id can never escape the cache root.
        let safe: String =
            stream_id.chars().map(|c| if c == '/' || c == '\\' { '_' } else { c }).collect();
        self.root.join(format!("{safe}.json"))
    }

    /// Writes (or overwrites) one stream's cached payload, creating the root if needed.
    ///
    /// # Errors
    /// A [`CacheError`] naming the operation when the directory or file write fails.
    pub fn put(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        std::fs::create_dir_all(&self.root).map_err(|e| CacheError {
            op: format!("create_dir {}", self.root.display()),
            message: e.to_string(),
        })?;
        let path = self.entry_path(&entry.stream_id);
        let mut f = std::fs::File::create(&path).map_err(|e| CacheError {
            op: format!("create {}", path.display()),
            message: e.to_string(),
        })?;
        f.write_all(entry.to_json().as_bytes()).map_err(|e| CacheError {
            op: format!("write {}", path.display()),
            message: e.to_string(),
        })
    }

    /// Reads one stream's cached payload. `None` on a miss or a corrupt entry (both read as "no
    /// last-good", which the broker turns into STALE/OFFLINE words, never a crash).
    #[must_use]
    pub fn get(&self, stream_id: &str) -> Option<CacheEntry> {
        let text = std::fs::read_to_string(self.entry_path(stream_id)).ok()?;
        CacheEntry::from_json(&text)
    }

    /// Removes every cached entry, leaving the (now-empty) root. Returns how many entries were
    /// removed (for `sparq streams cache clear`).
    ///
    /// # Errors
    /// A [`CacheError`] if the directory cannot be read.
    pub fn clear(&self) -> Result<usize, CacheError> {
        if !self.root.exists() {
            return Ok(0);
        }
        let mut n = 0;
        let rd = std::fs::read_dir(&self.root).map_err(|e| CacheError {
            op: format!("read_dir {}", self.root.display()),
            message: e.to_string(),
        })?;
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json")
                && std::fs::remove_file(&p).is_ok()
            {
                n += 1;
            }
        }
        Ok(n)
    }

    /// The stream ids currently cached (for `sparq streams cache` diagnostics).
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(&self.root) else { return Vec::new() };
        let mut ids: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
                } else {
                    None
                }
            })
            .collect();
        ids.sort();
        ids
    }
}

/// The OS user-data directory, resolved from environment only (no platform crate): `%APPDATA%` on
/// Windows, `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
/// Falls back to `.` when no home is discoverable — a cache with no home is a cache miss, not a crash.
fn user_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(d) = std::env::var("APPDATA") {
            if !d.is_empty() {
                return PathBuf::from(d);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(h) = std::env::var("HOME") {
            if !h.is_empty() {
                return PathBuf::from(h).join("Library").join("Application Support");
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(d) = std::env::var("XDG_DATA_HOME") {
            if !d.is_empty() {
                return PathBuf::from(d);
            }
        }
        if let Ok(h) = std::env::var("HOME") {
            if !h.is_empty() {
                return PathBuf::from(h).join(".local").join("share");
            }
        }
    }
    PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        // A per-test unique dir under the system temp dir; the test cleans it up.
        let mut p = std::env::temp_dir();
        p.push(format!("sparq-cache-{tag}-{}", std::process::id()));
        p
    }

    #[test]
    fn envelope_round_trips_through_json() {
        let e = CacheEntry {
            stream_id: "swpc.kp".into(),
            fetched_unix: 123,
            status: 200,
            body: "[{\"Kp\":2}]".into(),
        };
        let back = CacheEntry::from_json(&e.to_json()).expect("a written envelope must parse");
        assert_eq!(e, back);
    }

    #[test]
    fn corrupt_envelope_reads_as_a_miss_not_a_crash() {
        assert_eq!(CacheEntry::from_json("not json"), None);
        assert_eq!(CacheEntry::from_json("{}"), None); // missing fields
    }

    #[test]
    fn put_get_round_trips_on_disk() {
        let root = temp_root("roundtrip");
        let c = Cache::new(root.clone());
        let e = CacheEntry {
            stream_id: "sat.iss".into(),
            fetched_unix: 999,
            status: 200,
            body: "{\"latitude\":1}".into(),
        };
        c.put(&e).expect("put must succeed in a temp dir");
        assert_eq!(c.get("sat.iss"), Some(e));
        assert_eq!(c.ids(), vec!["sat.iss".to_string()]);
        assert_eq!(c.clear().unwrap(), 1);
        assert_eq!(c.get("sat.iss"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_entry_is_none() {
        let c = Cache::new(temp_root("missing"));
        assert_eq!(c.get("nope.nope"), None);
        assert!(c.ids().is_empty());
        assert_eq!(c.clear().unwrap(), 0); // clearing a non-existent root is a no-op, not an error
    }

    #[test]
    fn a_hostile_id_cannot_escape_the_root() {
        let c = Cache::new(PathBuf::from("/tmp/sparq-cache-escape"));
        let p = c.entry_path("../../etc/passwd");
        assert!(p.starts_with("/tmp/sparq-cache-escape"), "separators are neutralised: {p:?}");
    }

    #[test]
    fn default_root_honours_the_env_override() {
        // Save and restore the env var so the test is order-independent.
        let prev = std::env::var("SPARQ_STREAMS_CACHE").ok();
        std::env::set_var("SPARQ_STREAMS_CACHE", "/tmp/sparq-cache-env-test");
        assert_eq!(Cache::default_root(), PathBuf::from("/tmp/sparq-cache-env-test"));
        match prev {
            Some(v) => std::env::set_var("SPARQ_STREAMS_CACHE", v),
            None => std::env::remove_var("SPARQ_STREAMS_CACHE"),
        }
    }
}
