//! The broker's scheduling core (WO-020 INC5): cadence-floored polling, the windows, the
//! last-good cache and the words a failed feed leaves behind.
//!
//! This is the half of ADR-011's "host-side broker" that decides WHEN and WHAT to fetch and how
//! a fetched payload meets its window. It rides the `streams` feature and stays hermetic: the
//! socket is the injected [`Transport`] (the live `HttpTransport` is `streams-net`), the clock is
//! the injected `now_unix` (D11 — nothing here reads a wall clock), and the sleeper is injected
//! (tests never wait). The app's driver thread (or the CLI's verb) owns the impure edge and calls
//! [`Broker::poll`]/[`Broker::poll_due`]; a live shell wraps the broker in the control-plane's
//! usual `Arc<Mutex<…>>` (the defect-#66 pattern — never an audio-path lock; this plane has no
//! audio path at all, plan D5).
//!
//! # The three rules this module owns
//!
//! 1. **The cadence is a floor, never a target (§11.1).** [`Broker::due`] answers only streams
//!    whose last ATTEMPT is at least `cadence_s` old — attempts, not successes, so a failing feed
//!    retries on cadence and never tight-loops a dying server.
//! 2. **Replace or accumulate, as the registry says.** A successful fetch of a whole-picture
//!    payload (a 3-day series, an event set, a grid) REPLACES the window — the payload IS the
//!    current picture, and appending it would double every record it repeats. A `summary/*` poll
//!    (one record) APPENDS, monotonically by the record's own stamp, so history accumulates
//!    across polls (INC3's finding #7 — a dot becomes a trace) and a re-poll of an unchanged
//!    minute is deduplicated, never doubled. The rule is data: the registry's `accumulate` column.
//! 3. **A failure changes the words, not the data (§6.3: "the app never dies because a feed
//!    died").** A non-200, a transport error or a normalizer refusal keeps the last-good window
//!    in place; the stale policy (3× cadence → STALE, 10× → OFFLINE, `window.rs`) then ages it in
//!    words on its own. The disk cache carries the last good payload across restarts, so an
//!    offline launch still shows the last wall — aged, in words, honestly.
//!
//! KEY NEEDED streams are never attempted and never due (plan D14); their outcome names the env
//! var, in words.

use std::collections::HashMap;

use crate::cache::{Cache, CacheEntry};
use crate::fetch::{self, FetchError, RawFetch, Transport, POLITE_DELAY_MS};
use crate::record::DataRecord;
use crate::registry::{KeyState, Registry, StreamDef};
use crate::window::{StreamStatus, Window};

/// One stream's poll result — the probe table's row and the broker's diagnostic face. `words` is
/// empty on a clean success; on any refusal or failure it is the verbatim sentence the UI, the
/// CLI and the log show (never a bare colour, never a silent hole).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollOutcome {
    /// The stream this outcome belongs to.
    pub stream_id: String,
    /// Whether the window absorbed a payload.
    pub ok: bool,
    /// The HTTP status (0 = refused pre-request or transport failure).
    pub status: u16,
    /// How many records the window took (0 on failure; for `accumulate` rows, how many were NEW).
    pub records: usize,
    /// The words — empty on success, the verbatim refusal/failure otherwise.
    pub words: String,
}

/// The broker: registry + windows + attempt stamps + the last-good cache.
///
/// Not `Sync` by design — the driver owns it (or an `Arc<Mutex<Broker>>` does); every method is a
/// plain `&mut self`/`&self` call with no interior locks of its own.
#[derive(Debug)]
pub struct Broker {
    registry: Registry,
    windows: HashMap<String, Window>,
    last_attempt: HashMap<String, i64>,
    words: HashMap<String, String>,
    cache: Option<Cache>,
    /// Per-stream cadence overrides, seconds (WO-020 INC6 D18): `due` and the stale horizon
    /// read them before the registry's cadence. Fed from the overrides store at launch and on
    /// every edit (control thread); every value is ≥ the O-3 floor — the setter refuses below
    /// it in words, and the store refuses before that.
    cadence_overrides: HashMap<String, u32>,
}

impl Broker {
    /// Builds a broker over `registry`, seeding every window from `cache`'s last-good payloads
    /// (an offline launch shows the last wall, aged by the stale policy against the injected
    /// clock — words, not holes). A cached payload that no longer normalizes reads as a miss
    /// with words, never as a crash.
    #[must_use]
    pub fn new(registry: Registry, cache: Option<Cache>) -> Self {
        let mut b = Self {
            registry,
            windows: HashMap::new(),
            last_attempt: HashMap::new(),
            words: HashMap::new(),
            cache,
            cadence_overrides: HashMap::new(),
        };
        b.load_last_good();
        b
    }

    /// The registry this broker polls against.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The window for a stream (`None` until its first successful fetch or cache seed).
    #[must_use]
    pub fn window(&self, stream_id: &str) -> Option<&Window> {
        self.windows.get(stream_id)
    }

    /// The freshness of a stream at the injected clock (an absent window is OFFLINE — the
    /// provider contract's answer for "nothing has ever arrived").
    #[must_use]
    pub fn status(&self, stream_id: &str, now_unix: i64) -> StreamStatus {
        self.windows.get(stream_id).map_or(StreamStatus::Offline, |w| w.status(now_unix))
    }

    /// The standing words for a stream: the last failure/refusal, or a corrupt-cache note.
    /// Empty (`None`) when the stream is healthy or untouched.
    #[must_use]
    pub fn words(&self, stream_id: &str) -> Option<&str> {
        self.words.get(stream_id).map(String::as_str).filter(|s| !s.is_empty())
    }

    /// When a stream was last ATTEMPTED (success or failure — the cadence floor counts attempts).
    #[must_use]
    pub fn last_attempt(&self, stream_id: &str) -> Option<i64> {
        self.last_attempt.get(stream_id).copied()
    }

    /// The streams due for a poll at `now_unix`, in registry order (the file order is the
    /// picker's canonical order): fetchable (not KEY NEEDED) and last attempted at least one
    /// EFFECTIVE cadence ago — the override when one is set, else the registry's (D18:
    /// "`Broker::due` reads the override-then-registry cadence"). The floor still counts
    /// ATTEMPTS, not successes: the anti-thundering-herd rule is untouched.
    #[must_use]
    pub fn due<F>(&self, now_unix: i64, env: F) -> Vec<String>
    where
        F: Fn(&str) -> Option<String>,
    {
        self.registry
            .streams()
            .iter()
            .filter(|def| def.is_fetchable(&env))
            .filter(|def| match self.last_attempt.get(&def.id) {
                None => true,
                Some(last) => {
                    now_unix.saturating_sub(*last) >= i64::from(self.effective_cadence(def))
                },
            })
            .map(|def| def.id.clone())
            .collect()
    }

    /// The cadence a stream is actually polled on: the override when set, else the registry's
    /// (D18). The stale horizon reads the same number ([`Self::window_for`]), so the freshness
    /// words age against the cadence that governs.
    #[must_use]
    pub fn effective_cadence(&self, def: &StreamDef) -> u32 {
        self.cadence_overrides.get(&def.id).copied().unwrap_or(def.cadence_s)
    }

    /// Sets or clears one stream's cadence override — or REFUSES in words below the O-3 floor,
    /// changing nothing (the store refuses first; this is the second door, and no door is
    /// silent). An accepted override re-tunes the live window's stale horizon immediately.
    ///
    /// # Errors
    /// The refusal sentence, when `secs` is below [`crate::store::CADENCE_FLOOR_S`].
    pub fn set_cadence_override(
        &mut self,
        stream_id: &str,
        secs: Option<u32>,
    ) -> Result<(), String> {
        if let Some(s) = secs {
            if s < crate::store::CADENCE_FLOOR_S {
                return Err(format!(
                    "{s} s is below the {} s floor (ruling O-3) — the cadence keeps its old value",
                    crate::store::CADENCE_FLOOR_S
                ));
            }
        }
        match secs {
            Some(s) => {
                self.cadence_overrides.insert(stream_id.to_string(), s);
            },
            None => {
                self.cadence_overrides.remove(stream_id);
            },
        }
        // Re-tune the live window (if one exists) so the freshness words follow at once —
        // the effective cadence is computed BEFORE the mutable borrow (one truth, no clash).
        let tuned = self.registry.get(stream_id).map(|def| self.effective_cadence(def));
        if let (Some(w), Some(cad)) = (self.windows.get_mut(stream_id), tuned) {
            w.set_cadence(cad);
        }
        Ok(())
    }

    /// The window a fetch builds for a stream: the registry's kind, the EFFECTIVE cadence
    /// (D18 — the override rides into the stale horizon the same step it rides into `due`).
    fn window_for(&self, def: &StreamDef) -> Window {
        let mut w = Window::for_stream(def);
        w.set_cadence(self.effective_cadence(def));
        w
    }

    /// Polls exactly `ids` (unknown ids answer with the registry's refusal in words, never a
    /// panic), pacing requests [`POLITE_DELAY_MS`] apart with the injected sleeper. Every id gets
    /// an outcome; one feed's failure never stops the batch (§6.3).
    #[must_use]
    pub fn poll<F, S>(
        &mut self,
        ids: &[String],
        transport: &dyn Transport,
        now_unix: i64,
        env: F,
        sleeper: S,
    ) -> Vec<PollOutcome>
    where
        F: Fn(&str) -> Option<String>,
        S: Fn(u64),
    {
        let mut outcomes = Vec::with_capacity(ids.len());
        let mut attempted = 0usize;
        for id in ids {
            let Some(def) = self.registry.get(id).cloned() else {
                outcomes.push(PollOutcome {
                    stream_id: id.clone(),
                    ok: false,
                    status: 0,
                    records: 0,
                    words: self.registry.unknown_id_message(id),
                });
                continue;
            };
            // D14: a KEY NEEDED stream is not attempted at all.
            if let KeyState::KeyNeeded = def.key_state(&env) {
                let words = format!(
                    "KEY NEEDED — {} is unset and the row declares no fallback; not fetched \
                     (plan D14). Fix: set the env var (free registration for FIRMS).",
                    def.key_env.clone().unwrap_or_else(|| "the key env var".to_string())
                );
                self.words.insert(def.id.clone(), words.clone());
                outcomes.push(PollOutcome {
                    stream_id: def.id.clone(),
                    ok: false,
                    status: 0,
                    records: 0,
                    words,
                });
                continue;
            }
            if attempted > 0 {
                sleeper(POLITE_DELAY_MS); // the recorder's politeness, between requests only
            }
            attempted += 1;
            outcomes.push(self.poll_one(&def, transport, now_unix, &env, &sleeper));
        }
        outcomes
    }

    /// [`poll`] over [`due`] — the driver thread's one call per tick.
    #[must_use]
    pub fn poll_due<F, S>(
        &mut self,
        transport: &dyn Transport,
        now_unix: i64,
        env: F,
        sleeper: S,
    ) -> Vec<PollOutcome>
    where
        F: Fn(&str) -> Option<String>,
        S: Fn(u64),
    {
        let due = self.due(now_unix, &env);
        self.poll(&due, transport, now_unix, env, sleeper)
    }

    fn poll_one<F, S>(
        &mut self,
        def: &StreamDef,
        transport: &dyn Transport,
        now_unix: i64,
        env: &F,
        sleeper: &S,
    ) -> PollOutcome
    where
        F: Fn(&str) -> Option<String>,
        S: Fn(u64),
    {
        // The attempt stamp moves even on failure: the cadence floor paces retries (§11.1), so a
        // dead feed is re-tried on its cadence and never tight-loops.
        self.last_attempt.insert(def.id.clone(), now_unix);
        let outcome =
            match fetch::fetch_raw(&self.registry, &def.id, transport, now_unix, env, sleeper) {
                Err(e) => PollOutcome {
                    stream_id: def.id.clone(),
                    ok: false,
                    status: 0,
                    records: 0,
                    words: e.message.clone(),
                },
                Ok(raw) => match fetch::records_of(def, &raw, now_unix) {
                    Err(e) => PollOutcome {
                        stream_id: def.id.clone(),
                        ok: false,
                        status: raw.status,
                        records: 0,
                        words: e.message.clone(),
                    },
                    Ok(records) => {
                        let n = self.absorb(def, &raw, records, now_unix);
                        PollOutcome {
                            stream_id: def.id.clone(),
                            ok: true,
                            status: 200,
                            records: n,
                            words: String::new(),
                        }
                    },
                },
            };
        if outcome.ok {
            self.words.remove(&def.id);
        } else {
            self.words.insert(def.id.clone(), outcome.words.clone());
        }
        outcome
    }

    /// Meets a good payload to its window (replace or accumulate per the registry column),
    /// stamps the fetch and writes the last-good cache. Returns how many records the window took.
    fn absorb(
        &mut self,
        def: &StreamDef,
        raw: &RawFetch,
        records: Vec<DataRecord>,
        now_unix: i64,
    ) -> usize {
        let taken = if def.accumulate {
            // APPEND, monotonically by the record's own stamp: a summary poll contributes its one
            // record; a re-poll of an unchanged minute contributes nothing (deduplicated, never
            // doubled). Out-of-order older records are dropped — the window is a history, not a
            // sorting problem.
            let mut taken = 0usize;
            if let Some(w) = self.windows.get_mut(&def.id) {
                for r in records {
                    let dominated = w.latest().is_some_and(|l| r.t_wall_ns <= l.t_wall_ns);
                    if !dominated {
                        w.push(r);
                        taken += 1;
                    }
                }
                w.mark_fetched(now_unix);
            } else {
                let mut w = self.window_for(def);
                taken = records.len();
                w.extend(records);
                w.mark_fetched(now_unix);
                self.windows.insert(def.id.clone(), w);
            }
            taken
        } else {
            // REPLACE: the payload is the whole current picture.
            let mut w = self.window_for(def);
            let taken = records.len();
            w.extend(records);
            w.mark_fetched(now_unix);
            self.windows.insert(def.id.clone(), w);
            taken
        };
        if let Some(cache) = self.cache.clone() {
            let entry = CacheEntry {
                stream_id: def.id.clone(),
                fetched_unix: now_unix,
                status: 200,
                body: raw.body.clone(),
            };
            if let Err(e) = cache.put(&entry) {
                // A failed cache write does not undo a good fetch — the window is live; only the
                // next offline launch loses this payload. Words, not silence.
                let note = format!(
                    "last-good cache write failed ({}: {}) — the live window is \
                                    unaffected, but an offline relaunch will not see this payload",
                    e.op, e.message
                );
                self.words.insert(def.id.clone(), note);
            }
        }
        taken
    }

    /// Seeds the windows from the disk cache (constructor path).
    fn load_last_good(&mut self) {
        let Some(cache) = self.cache.clone() else { return };
        for def in self.registry.streams() {
            let Some(entry) = cache.get(&def.id) else { continue };
            if entry.status != 200 {
                continue; // a failed fetch was never a last-good
            }
            match crate::normalize::normalize(def, &entry.body, entry.fetched_unix) {
                Ok(records) => {
                    let mut w = self.window_for(def);
                    w.extend(records);
                    w.mark_fetched(entry.fetched_unix);
                    self.windows.insert(def.id.clone(), w);
                },
                Err(e) => {
                    self.words.insert(
                        def.id.clone(),
                        format!(
                            "the cached last-good payload no longer normalizes ({}) — treated as \
                             no last-good; the next successful fetch replaces it",
                            e.message
                        ),
                    );
                },
            }
        }
    }
}

/// A no-op sleeper for tests and for callers that pace themselves.
pub fn no_sleep(_ms: u64) {}

/// Maps a [`FetchError`] to the broker's outcome words (kept public so the CLI's verbs and the
/// broker refuse with the same sentences).
#[must_use]
pub fn outcome_of_error(e: &FetchError) -> PollOutcome {
    PollOutcome {
        stream_id: e.stream_id.clone(),
        ok: false,
        status: 0,
        records: 0,
        words: e.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::fetch::TransportResponse;
    use std::cell::RefCell;
    use std::collections::HashMap as StdMap;
    use std::path::PathBuf;

    /// Canned-response transport keyed by stream id (the URLs are filled deterministically, so
    /// keying the mock by the id keeps the tests readable).
    struct Mock {
        by_url: RefCell<StdMap<String, Vec<TransportResponse>>>,
        calls: RefCell<Vec<String>>,
    }

    impl Mock {
        fn new() -> Self {
            Self { by_url: RefCell::new(StdMap::new()), calls: RefCell::new(Vec::new()) }
        }
        fn queue(&self, url: &str, resp: TransportResponse) {
            self.by_url.borrow_mut().entry(url.to_string()).or_default().push(resp);
        }
        fn calls(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl Transport for Mock {
        fn get(&self, url: &str) -> TransportResponse {
            self.calls.borrow_mut().push(url.to_string());
            let mut map = self.by_url.borrow_mut();
            match map.get_mut(url) {
                Some(q) if !q.is_empty() => q.remove(0),
                _ => TransportResponse {
                    status: 404,
                    content_type: String::new(),
                    body: String::new(),
                    note: "HTTP 404".to_string(),
                },
            }
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/../../reference/fixtures/observatory/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn json(body: String) -> TransportResponse {
        TransportResponse {
            status: 200,
            content_type: "application/json".into(),
            body,
            note: "".into(),
        }
    }

    fn url_for(reg: &Registry, id: &str, now: i64) -> String {
        fetch::fill_endpoint(reg, id, now, |_| None).unwrap().url
    }

    fn temp_cache(tag: &str) -> Cache {
        let mut p: PathBuf = std::env::temp_dir();
        p.push(format!("sparq-broker-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Cache::new(p)
    }

    fn no_env(_k: &str) -> Option<String> {
        None
    }

    const NOW: i64 = 1_791_290_096; // 2026-10-06T12:34:56Z

    #[test]
    fn due_respects_the_cadence_floor_and_registry_order() {
        let reg = Registry::load().unwrap();
        let b = Broker::new(reg.clone(), None);
        let due = b.due(NOW, no_env);
        // Every fetchable stream is due on a cold broker, in registry (file) order; the keyless
        // FIRMS row is NOT due — it is never attempted (D14).
        let fetchable: Vec<String> =
            reg.streams().iter().filter(|d| d.is_fetchable(no_env)).map(|d| d.id.clone()).collect();
        assert_eq!(due, fetchable);
        assert!(!due.iter().any(|id| id == "geo.firms"));
    }

    #[test]
    fn a_successful_replace_poll_owns_the_window() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        mock.queue(&url_for(&reg, "swpc.kp", NOW), json(fixture("swpc.kp.json")));
        let out = b.poll(&["swpc.kp".into()], &mock, NOW, no_env, no_sleep);
        assert_eq!(out.len(), 1);
        assert!(out[0].ok, "{}", out[0].words);
        assert_eq!(out[0].status, 200);
        assert!(out[0].records > 10, "the 3-day Kp series is many records: {}", out[0].records);
        let w = b.window("swpc.kp").unwrap();
        assert_eq!(w.len(), out[0].records);
        assert_eq!(b.status("swpc.kp", NOW), StreamStatus::Live);
        // Not due again until the cadence has passed — attempts are the floor.
        assert!(
            b.due(NOW + 299, no_env).is_empty()
                || !b.due(NOW + 299, no_env).contains(&"swpc.kp".to_string())
        );
        assert!(b.due(NOW + 300, no_env).contains(&"swpc.kp".to_string()));
        // A second poll REPLACES: the window is the new picture, not old + new.
        let later = NOW + 300;
        mock.queue(&url_for(&reg, "swpc.kp", later), json(fixture("swpc.kp.json")));
        let out2 = b.poll(&["swpc.kp".into()], &mock, later, no_env, no_sleep);
        assert!(out2[0].ok);
        assert_eq!(b.window("swpc.kp").unwrap().len(), out2[0].records, "replace, not append");
    }

    #[test]
    fn an_accumulate_poll_builds_history_and_deduplicates() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        let def = reg.get("swpc.solar-wind").unwrap();
        assert!(def.accumulate, "the summary feed is an accumulate row");
        // Poll 1: the recorded summary minute.
        mock.queue(&url_for(&reg, "swpc.solar-wind", NOW), json(fixture("swpc.solar-wind.json")));
        let out = b.poll(&["swpc.solar-wind".into()], &mock, NOW, no_env, no_sleep);
        assert!(out[0].ok, "{}", out[0].words);
        assert_eq!(out[0].records, 1, "one summary record per poll");
        // Poll 2 (a minute later, a NEW payload): history grows — the dot becomes a trace.
        let t2 = NOW + 60;
        let next = r#"[{"proton_speed": 412, "time_tag": "2026-10-06T01:56:00Z"}]"#;
        mock.queue(&url_for(&reg, "swpc.solar-wind", t2), json(next.to_string()));
        let out2 = b.poll(&["swpc.solar-wind".into()], &mock, t2, no_env, no_sleep);
        assert_eq!(out2[0].records, 1, "the newer minute appends");
        assert_eq!(b.window("swpc.solar-wind").unwrap().len(), 2);
        // Poll 3 (the server has not advanced — the SAME minute again): nothing doubles.
        let t3 = t2 + 60;
        mock.queue(&url_for(&reg, "swpc.solar-wind", t3), json(next.to_string()));
        let out3 = b.poll(&["swpc.solar-wind".into()], &mock, t3, no_env, no_sleep);
        assert!(out3[0].ok);
        assert_eq!(out3[0].records, 0, "a re-poll of an unchanged minute adds nothing");
        assert_eq!(b.window("swpc.solar-wind").unwrap().len(), 2);
        // … but the freshness stamp DID move (the feed is reachable — LIVE, not STALE).
        assert_eq!(b.status("swpc.solar-wind", t3), StreamStatus::Live);
    }

    #[test]
    fn a_failure_keeps_the_last_good_and_leaves_words() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        mock.queue(&url_for(&reg, "swpc.kp", NOW), json(fixture("swpc.kp.json")));
        let _ = b.poll(&["swpc.kp".into()], &mock, NOW, no_env, no_sleep);
        let good_len = b.window("swpc.kp").unwrap().len();
        // Two 503s: the retry policy fires once, the poll fails, the window SURVIVES.
        let t = NOW + 300;
        let down = || TransportResponse {
            status: 503,
            content_type: "".into(),
            body: "".into(),
            note: "".into(),
        };
        mock.queue(&url_for(&reg, "swpc.kp", t), down());
        mock.queue(&url_for(&reg, "swpc.kp", t), down());
        let out = b.poll(&["swpc.kp".into()], &mock, t, no_env, no_sleep);
        assert!(!out[0].ok);
        assert_eq!(out[0].status, 503);
        assert!(out[0].words.contains("HTTP 503"), "{}", out[0].words);
        assert_eq!(b.words("swpc.kp"), Some(out[0].words.as_str()));
        assert_eq!(b.window("swpc.kp").unwrap().len(), good_len, "the last-good window stands");
        // The stale policy ages it in words on its own: STALE at 3× cadence, OFFLINE at 10×.
        assert_eq!(b.status("swpc.kp", NOW + 2 * 300), StreamStatus::Live);
        assert_eq!(b.status("swpc.kp", NOW + 3 * 300), StreamStatus::Stale);
        assert_eq!(b.status("swpc.kp", NOW + 10 * 300), StreamStatus::Offline);
        // A recovery clears the words.
        let t2 = t + 300;
        mock.queue(&url_for(&reg, "swpc.kp", t2), json(fixture("swpc.kp.json")));
        let out2 = b.poll(&["swpc.kp".into()], &mock, t2, no_env, no_sleep);
        assert!(out2[0].ok);
        assert_eq!(b.words("swpc.kp"), None);
    }

    #[test]
    fn key_needed_is_never_attempted_and_says_so() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        let out = b.poll(&["geo.firms".into()], &mock, NOW, no_env, no_sleep);
        assert_eq!(mock.calls(), 0, "D14: a keyless feed is never fetched");
        assert!(!out[0].ok);
        assert!(out[0].words.contains("KEY NEEDED"), "{}", out[0].words);
        assert!(out[0].words.contains("SPARQ_FIRMS_KEY"), "{}", out[0].words);
        // With the key set, the same stream becomes due and fetchable.
        let keyed = |k: &str| if k == "SPARQ_FIRMS_KEY" { Some("K".into()) } else { None };
        assert!(b.due(NOW, keyed).contains(&"geo.firms".to_string()));
    }

    #[test]
    fn an_unknown_id_answers_with_the_registry_refusal() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        let out = b.poll(&["foo.bar".into()], &mock, NOW, no_env, no_sleep);
        assert!(!out[0].ok);
        assert!(out[0].words.contains("unknown stream id"), "{}", out[0].words);
        assert_eq!(mock.calls(), 0);
    }

    #[test]
    fn the_last_good_cache_survives_a_relaunch() {
        let reg = Registry::load().unwrap();
        let cache = temp_cache("relaunch");
        let mut b = Broker::new(reg.clone(), Some(cache.clone()));
        let mock = Mock::new();
        mock.queue(&url_for(&reg, "swpc.kp", NOW), json(fixture("swpc.kp.json")));
        let out = b.poll(&["swpc.kp".into()], &mock, NOW, no_env, no_sleep);
        assert!(out[0].ok, "{}", out[0].words);
        let len = out[0].records;
        drop(b);
        // Relaunch OFFLINE (no transport calls will succeed): the window is seeded from the
        // cache, stamped with the cached fetch time, and ages in words against the new clock.
        let b2 = Broker::new(reg.clone(), Some(cache.clone()));
        assert_eq!(b2.window("swpc.kp").unwrap().len(), len);
        assert_eq!(b2.status("swpc.kp", NOW + 60), StreamStatus::Live);
        assert_eq!(b2.status("swpc.kp", NOW + 3 * 300), StreamStatus::Stale);
        assert_eq!(b2.status("swpc.kp", NOW + 10 * 300), StreamStatus::Offline);
        let _ = std::fs::remove_dir_all(cache.root());
    }

    #[test]
    fn a_corrupt_cache_entry_is_words_not_a_crash() {
        let reg = Registry::load().unwrap();
        let cache = temp_cache("corrupt");
        cache
            .put(&CacheEntry {
                stream_id: "swpc.kp".into(),
                fetched_unix: NOW,
                status: 200,
                body: "this is not the Kp JSON shape".into(),
            })
            .unwrap();
        let b = Broker::new(reg.clone(), Some(cache.clone()));
        assert!(b.window("swpc.kp").is_none(), "a payload that will not normalize is a miss");
        let w = b.words("swpc.kp").unwrap();
        assert!(w.contains("no longer normalizes"), "{w}");
        let _ = std::fs::remove_dir_all(cache.root());
    }

    #[test]
    fn the_batch_is_paced_between_requests_only() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        for id in ["swpc.kp", "swpc.wwv", "sat.iss"] {
            let ext = if id == "swpc.wwv" { "txt" } else { "json" };
            mock.queue(&url_for(&reg, id, NOW), json(fixture(format!("{id}.{ext}").as_str())));
        }
        let slept = RefCell::new(Vec::new());
        let out = b.poll(
            &["swpc.kp".into(), "swpc.wwv".into(), "sat.iss".into()],
            &mock,
            NOW,
            no_env,
            |ms| slept.borrow_mut().push(ms),
        );
        assert!(out.iter().all(|o| o.ok), "{out:?}");
        // Two gaps between three requests — polite, but never after the last one.
        assert_eq!(*slept.borrow(), vec![POLITE_DELAY_MS, POLITE_DELAY_MS]);
    }

    #[test]
    fn poll_due_polls_exactly_the_due_set() {
        let reg = Registry::load().unwrap();
        let mut b = Broker::new(reg.clone(), None);
        let mock = Mock::new();
        mock.queue(&url_for(&reg, "swpc.kp", NOW), json(fixture("swpc.kp.json")));
        // Only kp is queued; polling the DUE set would hit every fetchable stream — so narrow
        // the broker to one stream by advancing the others out of existence is not possible
        // (the registry is data). Instead: assert poll_due attempts == due count via the mock.
        let due_before = b.due(NOW, no_env).len();
        let out = b.poll_due(&mock, NOW, no_env, no_sleep);
        assert_eq!(out.len(), due_before);
        assert!(mock.calls() >= due_before, "one request per due stream (retries add calls)");
        let kp = out.iter().find(|o| o.stream_id == "swpc.kp").unwrap();
        assert!(kp.ok, "{}", kp.words);
        // Immediately after, nothing is due (every attempt stamp moved).
        assert!(b.due(NOW, no_env).is_empty());
    }
}
