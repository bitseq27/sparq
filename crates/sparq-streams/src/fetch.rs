//! The live fetch path (WO-020 INC5): endpoint fill, the transport seam, and the record flow.
//!
//! This module is the Rust sibling of `tools/streams_record.py`'s request half — the same
//! placeholder vocabulary (`{d-N}`/`{D-N}`/`{lat}`/`{lon}`/`{st}`/`{CC}`/`{KEY}`), the same
//! one-retry-on-429/5xx policy, the same status-0-means-transport-error envelope — so a payload
//! the recorder captured and a payload the broker fetches travel the identical road. Two tiers:
//!
//! * **`streams` (hermetic):** everything except the socket. [`fill_endpoint`] is pure (an
//!   injected `now_unix` and an env lookup — the D11 firewall: no clock is read here), the
//!   [`Transport`] trait is the seam, and the retry policy takes an injected sleeper. Tests run
//!   the whole flow against [`MockTransport`]-class doubles and the checked-in fixtures; nothing
//!   here touches a wall clock or a socket unless the feature below is on.
//! * **`streams-net` (live edge):** [`HttpTransport`] (ureq + rustls, the crate's second
//!   third-party dependency) plus the two — and only two — impure helpers of the stream plane:
//!   [`now_unix`] and [`sleep_millis`]. ADR-011 §5 names the broker the wall-clock reader ("the
//!   broker reads a wall clock; the guest does not"); these two functions are where that reading
//!   happens, and every consumer downstream takes the value as an injected parameter.
//!
//! Refusals are words, never silence: an unknown stream id quotes the registry's own refusal, a
//! KEY NEEDED stream is *never fetched* and says so (plan D14), a template placeholder the filler
//! does not know is a registry/filler drift error rather than a URL with a literal `{` in it, and
//! a non-200 or transport failure carries its status and note to the caller (the probe table and
//! the broker's words) instead of vanishing.
//!
//! Secrets: a resolved API key exists only inside [`Request::url`] and is replaced by `{KEY}` in
//! every [`RawFetch`] — the redacted form is what logs, `_meta.json`, probe tables and error
//! messages carry. [`KeySource`] names WHERE a key came from (`env:VAR` / `fallback:VALUE`), the
//! python recorder's vocabulary, never the key itself.

use crate::normalize;
use crate::record::DataRecord;
use crate::registry::{KeyState, Registry, StreamDef};

/// The largest response body the transport will read (bytes). The heaviest declared payload is
/// EONET's ~5 MB event list; 32 MiB is headroom, not an invitation — a feed that grows past the
/// cap is refused in words rather than OOMing a 1 GB host.
pub const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// The HTTP statuses that earn exactly one retry (the python recorder's policy, mirrored).
const RETRYABLE: [u16; 5] = [429, 500, 502, 503, 504];

/// The polite pause between two requests in one batch, in milliseconds (the recorder's `--delay`
/// default; the broker's [`crate::broker::Broker::poll`] paces multi-stream polls with it).
pub const POLITE_DELAY_MS: u64 = 400;

/// The pause before the one retry, in milliseconds (the recorder's 1.5 s, mirrored).
pub const RETRY_DELAY_MS: u64 = 1500;

/// Why a fetch was refused before (or after) reaching the network — always in words, always
/// naming the stream, the python recorder's envelope semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchError {
    /// The stream the refusal belongs to.
    pub stream_id: String,
    /// The refusal, verbatim-printable.
    pub message: String,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.stream_id, self.message)
    }
}

/// Where the key that went into a URL came from — the recorder's `key_source` vocabulary. The
/// env-keyed case carries the VAR NAME, never the value; the fallback case carries the published
/// fallback (NASA's `DEMO_KEY` — a value that is in the docs, not a secret).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeySource {
    /// No key was involved.
    None,
    /// The key came from this environment variable (name only).
    Env(String),
    /// The declared fallback value was used.
    Fallback(String),
}

impl KeySource {
    /// The `key_source` string the fixtures' `_meta.json` and the probe table carry.
    #[must_use]
    pub fn as_words(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::Env(var) => format!("env:{var}"),
            Self::Fallback(fb) => format!("fallback:{fb}"),
        }
    }
}

/// A filled endpoint, ready to request. `url` is the ONLY place a resolved env key exists; use
/// [`Request::redacted`] for anything printed, logged or written to disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The filled URL (may embed a secret — see the type docs).
    pub url: String,
    /// Where the key came from, if any.
    pub key_source: KeySource,
    /// The secret to redact out of `url` (`None` when no env key was substituted).
    redaction: Option<String>,
}

impl Request {
    /// The URL with any resolved env key replaced by `{KEY}` — the form logs and metadata carry.
    #[must_use]
    pub fn redacted(&self) -> String {
        match &self.redaction {
            Some(secret) if !secret.is_empty() => self.url.replace(secret, "{KEY}"),
            _ => self.url.clone(),
        }
    }
}

/// Fills one registry endpoint template against an injected clock and environment.
///
/// Mirrors `tools/streams_record.py::resolve_placeholders` exactly: `{lat}`/`{lon}` from
/// `SPARQ_OBSERVATORY_LOC` (default `51.5,-0.1`), `{st}` from `SPARQ_OBSERVATORY_TIDE_STATION`
/// (else the row's `default_tide_station`, else `9447130`), `{CC}` from the row's
/// `default_country` (else `USA`), the dashed `{d-N}` and compact `{D-N}` date families (UTC,
/// today minus N days), and `{KEY}` from the row's key state.
///
/// # Errors
/// A [`FetchError`] in words when the stream is unknown to the registry, KEY NEEDED with no
/// fallback (plan D14: never fetched, never a silent hole), or a `{…}` placeholder survives the
/// fill (a registry/filler drift — refused here rather than requested as a broken URL).
pub fn fill_endpoint<F>(
    reg: &Registry,
    id: &str,
    now_unix: i64,
    env: F,
) -> Result<Request, FetchError>
where
    F: Fn(&str) -> Option<String>,
{
    let def = reg.get(id).ok_or_else(|| FetchError {
        stream_id: id.to_string(),
        message: reg.unknown_id_message(id),
    })?;
    fill_endpoint_for(def, now_unix, env)
}

/// [`fill_endpoint`] for an already-resolved row (the broker's path — it iterates the registry).
///
/// # Errors
/// As [`fill_endpoint`], minus the unknown-id case.
pub fn fill_endpoint_for<F>(def: &StreamDef, now_unix: i64, env: F) -> Result<Request, FetchError>
where
    F: Fn(&str) -> Option<String>,
{
    let err = |message: String| FetchError { stream_id: def.id.clone(), message };

    // The config knobs (§6.2), env-over-default exactly like the recorder.
    let loc = env("SPARQ_OBSERVATORY_LOC").unwrap_or_else(|| "51.5,-0.1".to_string());
    let mut parts = loc.split(',');
    let lat = parts.next().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("51.5");
    let lon = parts.next().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("-0.1");
    let station = env("SPARQ_OBSERVATORY_TIDE_STATION")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| def.default_tide_station.clone())
        .unwrap_or_else(|| "9447130".to_string());
    let country = def.default_country.clone().unwrap_or_else(|| "USA".to_string());

    let mut url = def.endpoint.clone();
    url = url.replace("{lat}", lat).replace("{lon}", lon);
    url = url.replace("{st}", &station).replace("{CC}", &country);
    for (token, compact) in [("{d-30}", 30), ("{d-7}", 7), ("{d-3}", 3), ("{d-1}", 1), ("{d}", 0)]
        .into_iter()
        .chain([("{D-30}", 30), ("{D-7}", 7), ("{D-3}", 3), ("{D-1}", 1), ("{D}", 0)])
    {
        let (y, m, d) = civil_from_unix(now_unix - i64::from(compact) * 86_400);
        let text = if token.starts_with("{d") {
            format!("{y:04}-{m:02}-{d:02}")
        } else {
            format!("{y:04}{m:02}{d:02}")
        };
        url = url.replace(token, &text);
    }

    // The key (D14): env value, else the declared fallback, else the honest refusal — and the
    // refusal means NOT FETCHED, which is why this is an Err and not a note.
    let mut redaction = None;
    let key_source = match def.key_state(&env) {
        KeyState::NotNeeded => KeySource::None,
        KeyState::Ready => {
            let var = def.key_env.clone().unwrap_or_default();
            let value = env(&var).unwrap_or_default();
            url = url.replace("{KEY}", &value);
            redaction = Some(value);
            KeySource::Env(var)
        },
        KeyState::UsingFallback(fb) => {
            url = url.replace("{KEY}", &fb);
            KeySource::Fallback(fb)
        },
        KeyState::KeyNeeded => {
            return Err(err(format!(
                "KEY NEEDED: {} is unset and the row declares no fallback — not fetched (plan \
                 D14: a keyless feed reports KEY NEEDED in words, never a silent hole). Fix: set \
                 the env var named in streams.toml (free registration for FIRMS).",
                def.key_env.clone().unwrap_or_else(|| "the key env var".to_string())
            )))
        },
    };

    // A surviving placeholder is registry/filler drift (a new token the filler does not know).
    // Refused in words here — a URL with a literal `{` in it is a 404 wearing a disguise.
    if let Some(brace) = url.find('{') {
        let token = url[brace..].split('}').next().unwrap_or(&url[brace..]);
        return Err(err(format!(
            "endpoint template carries `{token}}}` which the filler does not know — registry/filler \
             drift; teach fill_endpoint_for the token (and tools/streams_record.py with it)"
        )));
    }

    Ok(Request { url, key_source, redaction })
}

/// The UTC civil date of a unix timestamp (Howard Hinnant's `civil_from_days` — the same maths
/// the recorder's `datetime` does, with no chrono and no `SystemTime`, per the crate's no-clock
/// core rule; the injected `now_unix` arrives from the live edge).
#[must_use]
pub fn civil_from_unix(now_unix: i64) -> (i64, u32, u32) {
    civil_from_days(now_unix.div_euclid(86_400))
}

/// Days since 1970-01-01 → `(year, month, day)` in the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], Mar-based
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1); // [1, 31]
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1); // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

/// One transport answer, the recorder's envelope: `status` 0 means the request never completed
/// (DNS, TLS, timeout — the `note` says which); a non-zero status is the server's own words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportResponse {
    /// The HTTP status, or 0 for a transport-level failure.
    pub status: u16,
    /// The response content type ("" when absent or failed).
    pub content_type: String,
    /// The body, decoded UTF-8 lossily (the recorder's `errors="replace"`), "" on failure.
    pub body: String,
    /// The failure note ("" on success) — verbatim-printable.
    pub note: String,
}

/// The socket seam. One method, no async: the broker's driver thread is the concurrency (plan
/// D3 — fetching is control-thread work; there is no audio path here to protect, and no reason
/// to pay an executor for a GET). Tests implement it over the checked-in fixtures; `streams-net`
/// provides [`HttpTransport`].
pub trait Transport {
    /// GETs `url` once. A transport failure is a `status: 0` response with the reason in `note`,
    /// never an error — the caller decides what a failure means (the probe records it, the broker
    /// keeps the last-good window and says so in words).
    fn get(&self, url: &str) -> TransportResponse;
}

/// GETs with the recorder's retry policy: exactly one retry, only on [`RETRYABLE`] statuses,
/// after [`RETRY_DELAY_MS`]. Transport errors (status 0) are NOT retried — a dead socket is
/// answered honestly on the first attempt, like the python. The sleeper is injected so tests
/// never wait.
#[must_use]
pub fn fetch_with_retry<S>(transport: &dyn Transport, url: &str, sleeper: S) -> TransportResponse
where
    S: Fn(u64),
{
    let first = transport.get(url);
    if RETRYABLE.contains(&first.status) {
        sleeper(RETRY_DELAY_MS);
        let mut second = transport.get(url);
        if second.note.is_empty() {
            second.note = format!("HTTP {} on first attempt, retried once", first.status);
        }
        return second;
    }
    first
}

/// One fetched stream, redacted and enveloped — what the probe table prints, `_meta.json` would
/// carry, and the broker normalizes. The body is verbatim (the fixture-size acceptance's "the
/// file IS the recorded bytes" rule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawFetch {
    /// The stream id.
    pub stream_id: String,
    /// The filled endpoint, KEY-REDACTED (safe to log).
    pub url: String,
    /// Where the key came from.
    pub key_source: KeySource,
    /// The HTTP status (0 = transport failure; see `note`).
    pub status: u16,
    /// The response content type.
    pub content_type: String,
    /// The raw payload body, verbatim.
    pub body: String,
    /// The failure/retry note ("" on a clean success).
    pub note: String,
}

/// Fills, requests (with the retry policy) and envelopes one stream.
///
/// # Errors
/// A [`FetchError`] for the pre-request refusals only — unknown id, KEY NEEDED, template drift.
/// HTTP failures are NOT errors here: they come back as `Ok` with their status and note, because
/// a probe must record a 404 as data (the DONKI lesson: a dead endpoint is a fact, not a crash).
pub fn fetch_raw<F, S>(
    reg: &Registry,
    id: &str,
    transport: &dyn Transport,
    now_unix: i64,
    env: F,
    sleeper: S,
) -> Result<RawFetch, FetchError>
where
    F: Fn(&str) -> Option<String>,
    S: Fn(u64),
{
    let req = fill_endpoint(reg, id, now_unix, env)?;
    let resp = fetch_with_retry(transport, &req.url, sleeper);
    Ok(RawFetch {
        stream_id: id.to_string(),
        url: req.redacted(),
        key_source: req.key_source,
        status: resp.status,
        content_type: resp.content_type,
        body: resp.body,
        note: resp.note,
    })
}

/// Normalizes one fetched payload into frozen data-records (the broker's absorb path and the
/// `tail` verb's read face).
///
/// # Errors
/// A [`FetchError`] in words when the fetch did not succeed (the payload is not trusted) or the
/// normalizer refuses it (the feed's shape drifted — the §12.6 probe-first rule's downstream
/// face, always named, never a panic).
pub fn records_of(
    def: &StreamDef,
    raw: &RawFetch,
    now_unix: i64,
) -> Result<Vec<DataRecord>, FetchError> {
    if raw.status != 200 {
        let note = if raw.note.is_empty() { String::new() } else { format!(" ({})", raw.note) };
        return Err(FetchError {
            stream_id: def.id.clone(),
            message: format!("HTTP {} — payload not used{note}", raw.status),
        });
    }
    normalize::normalize(def, &raw.body, now_unix)
        .map_err(|e| FetchError { stream_id: def.id.clone(), message: e.message.clone() })
}

/// The real socket, `streams-net` only: ureq 3 + rustls (ring) + webpki-roots — TLS roots ride
/// the binary, so the sandbox and SATURN trust identically (no system-store divergence). Blocking
/// by design (one thread per fetch, the driver's own pacing); non-2xx arrives as a normal
/// response (`http_status_as_error(false)`) because the probe records statuses as data.
#[cfg(feature = "streams-net")]
pub struct HttpTransport {
    agent: ureq::Agent,
}

#[cfg(feature = "streams-net")]
impl HttpTransport {
    /// Builds the transport with one global timeout (the recorder's 20 s default).
    #[must_use]
    pub fn new(timeout: std::time::Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .user_agent(crate::user_agent())
            .build();
        Self { agent: ureq::Agent::new_with_config(config) }
    }
}

#[cfg(feature = "streams-net")]
impl Transport for HttpTransport {
    fn get(&self, url: &str) -> TransportResponse {
        use std::io::Read;
        let resp = match self.agent.get(url).call() {
            Ok(r) => r,
            Err(e) => {
                return TransportResponse {
                    status: 0,
                    content_type: String::new(),
                    body: String::new(),
                    note: format!("transport error: {e}"),
                };
            },
        };
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        // Capped read: a feed that grew past MAX_BODY_BYTES is refused in words, not OOM (the
        // 1 GB box is a first-class host, not an afterthought).
        let mut body = resp.into_body();
        let mut buf: Vec<u8> = Vec::new();
        let read = body.as_reader().take(MAX_BODY_BYTES + 1).read_to_end(&mut buf);
        match read {
            Err(e) => TransportResponse {
                status,
                content_type,
                body: String::new(),
                note: format!("body read failed: {e}"),
            },
            Ok(_) if buf.len() as u64 > MAX_BODY_BYTES => TransportResponse {
                status,
                content_type,
                body: String::new(),
                note: format!("body exceeds the {MAX_BODY_BYTES}-byte cap — refused, not read"),
            },
            Ok(_) => TransportResponse {
                status,
                content_type,
                body: String::from_utf8_lossy(&buf).into_owned(),
                note: String::new(),
            },
        }
    }
}

/// Unix seconds at the live edge — THE stream plane's one wall-clock read (ADR-011 §5: "the
/// broker reads a wall clock; the guest does not"). Everything downstream takes `now_unix` as a
/// parameter; the determinism firewall (D11) is intact because this value never reaches a guest
/// render except as record stamps the host owns.
///
/// The workspace bans `SystemTime::now` for the audio path's sake (ADR-006); this is the
/// broker's designated control-thread edge, the defect-#66 allow-with-reason precedent.
#[cfg(feature = "streams-net")]
#[must_use]
#[allow(clippy::disallowed_methods)] // the stream plane's ONE wall-clock read; see the fn docs
pub fn now_unix() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(i64::MAX)
}

/// Parks the calling (driver/control) thread — the retry backoff and the polite inter-request
/// delay. Never called from the audio path (there is none on this plane, plan D5); injected as a
/// no-op in tests so nothing waits.
#[cfg(feature = "streams-net")]
#[allow(clippy::disallowed_methods)] // control-thread pacing, not the audio path; see the fn docs
pub fn sleep_millis(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A scripted transport: canned responses per URL, a call log, and a scripted first-status
    /// for the retry test. Test-only interior mutability (the trait takes `&self`).
    struct Mock {
        responses: HashMap<String, TransportResponse>,
        calls: RefCell<Vec<String>>,
    }

    impl Mock {
        fn new() -> Self {
            Self { responses: HashMap::new(), calls: RefCell::new(Vec::new()) }
        }
        fn with(mut self, url: &str, resp: TransportResponse) -> Self {
            self.responses.insert(url.to_string(), resp);
            self
        }
        fn calls(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl Transport for Mock {
        fn get(&self, url: &str) -> TransportResponse {
            self.calls.borrow_mut().push(url.to_string());
            self.responses.get(url).cloned().unwrap_or(TransportResponse {
                status: 404,
                content_type: String::new(),
                body: String::new(),
                note: "HTTP 404".to_string(),
            })
        }
    }

    fn registry() -> Registry {
        Registry::load().unwrap()
    }

    fn no_env(_k: &str) -> Option<String> {
        None
    }

    fn no_sleep(_ms: u64) {}

    // 2026-10-06T12:34:56Z — the INC1 recording day, mid-day so no boundary ambiguity.
    const NOW: i64 = 1_791_290_096;

    #[test]
    fn civil_dates_match_the_recorder() {
        // Cross-checked against python: datetime.fromtimestamp(t, timezone.utc).
        assert_eq!(civil_from_unix(0), (1970, 1, 1));
        assert_eq!(civil_from_unix(86_399), (1970, 1, 1));
        assert_eq!(civil_from_unix(86_400), (1970, 1, 2));
        assert_eq!(civil_from_unix(-1), (1969, 12, 31));
        assert_eq!(civil_from_unix(-86_400), (1969, 12, 31));
        assert_eq!(civil_from_unix(1_709_164_800), (2024, 2, 29)); // the leap day
        assert_eq!(civil_from_unix(1_709_251_200), (2024, 3, 1));
        assert_eq!(civil_from_unix(NOW), (2026, 10, 6));
        assert_eq!(civil_from_unix(NOW - 86_400), (2026, 10, 5));
        assert_eq!(civil_from_unix(NOW - 30 * 86_400), (2026, 9, 6));
    }

    #[test]
    fn the_date_placeholders_fill_both_spellings() {
        let reg = registry();
        // GDACS carries {d-7} and {d} (dashed); NASA POWER carries {D-30}/{D-3} (compact).
        let gdacs = fill_endpoint(&reg, "geo.gdacs", NOW, no_env).unwrap();
        assert!(gdacs.url.contains("fromDate=2026-09-29"), "{}", gdacs.url);
        assert!(gdacs.url.contains("toDate=2026-10-06"), "{}", gdacs.url);
        let power_env =
            |k: &str| if k == "SPARQ_OBSERVATORY_LOC" { Some("40.0,-75.0".into()) } else { None };
        let power = fill_endpoint(&reg, "space.power", NOW, power_env).unwrap();
        assert!(power.url.contains("20260906"), "{}", power.url); // {D-30}
        assert!(power.url.contains("20261003"), "{}", power.url); // {D-3}
        assert!(power.url.contains("40.0") && power.url.contains("-75.0"), "{}", power.url);
        assert!(!power.url.contains('{'), "no placeholder survives: {}", power.url);
    }

    #[test]
    fn the_config_knobs_default_and_override_like_the_recorder() {
        let reg = registry();
        // Tides: default station 9447130, overridden by the env knob.
        let t = fill_endpoint(&reg, "wx.tides", NOW, no_env).unwrap();
        assert!(t.url.contains("9447130"), "{}", t.url);
        let t2 = fill_endpoint(&reg, "wx.tides", NOW, |k| {
            if k == "SPARQ_OBSERVATORY_TIDE_STATION" {
                Some("8454000".into())
            } else {
                None
            }
        })
        .unwrap();
        assert!(t2.url.contains("8454000"), "{}", t2.url);
        // FIRMS country knob (the row's own default rides {CC} — and with no key the row refuses
        // before the knob matters, so the country default is asserted through the refusal path
        // and the fill itself is proven on a keyed env).
        let firms = fill_endpoint(&reg, "geo.firms", NOW, |k| {
            if k == "SPARQ_FIRMS_KEY" {
                Some("SECRET123".into())
            } else {
                None
            }
        });
        let f = firms.unwrap();
        assert!(f.url.contains("USA"), "{}", f.url);
        assert!(f.url.contains("SECRET123"));
        // … and the redacted form NEVER carries the secret.
        assert!(!f.redacted().contains("SECRET123"), "{}", f.redacted());
        assert!(f.redacted().contains("{KEY}"), "{}", f.redacted());
        assert_eq!(f.key_source, KeySource::Env("SPARQ_FIRMS_KEY".into()));
        assert_eq!(f.key_source.as_words(), "env:SPARQ_FIRMS_KEY");
    }

    #[test]
    fn key_needed_refuses_in_words_and_never_fetches() {
        let reg = registry();
        let mock = Mock::new();
        let err = fetch_raw(&reg, "geo.firms", &mock, NOW, no_env, no_sleep).unwrap_err();
        assert_eq!(mock.calls(), 0, "a KEY NEEDED stream is never fetched (D14)");
        assert!(err.message.contains("KEY NEEDED"), "{}", err.message);
        assert!(err.message.contains("SPARQ_FIRMS_KEY"), "{}", err.message);
    }

    #[test]
    fn the_fallback_key_is_used_and_named() {
        let reg = registry();
        // NEO rides NASA's DEMO_KEY fallback when the env var is unset.
        let req = fill_endpoint(&reg, "space.neo", NOW, no_env).unwrap();
        assert_eq!(req.key_source.as_words(), "fallback:DEMO_KEY");
        assert!(req.url.contains("DEMO_KEY"), "{}", req.url);
        assert_eq!(req.redacted(), req.url, "a published fallback is not a secret");
    }

    #[test]
    fn an_unknown_id_quotes_the_registry_refusal() {
        let reg = registry();
        let err = fill_endpoint(&reg, "foo.bar", NOW, no_env).unwrap_err();
        assert!(err.message.contains("unknown stream id"), "{}", err.message);
        assert!(err.message.contains("foo.bar"), "{}", err.message);
    }

    #[test]
    fn a_surviving_placeholder_is_drift_in_words() {
        // A hostile row the checked-in registry could grow by accident: the filler refuses it
        // rather than requesting a URL with a literal brace in it.
        let toml = r#"
[[stream]]
id = "test.drift"
domain = "SW"
label = "Drift"
endpoint = "https://example.test/{SOMEDAY}"
cadence_s = 60
view = "text"
schema = "observatory/text@1"
units = ""
attribution = "test"
"#;
        let reg = Registry::from_toml(toml).unwrap();
        let err = fill_endpoint(&reg, "test.drift", NOW, no_env).unwrap_err();
        assert!(err.message.contains("{SOMEDAY}"), "{}", err.message);
        assert!(err.message.contains("drift"), "{}", err.message);
    }

    #[test]
    fn the_retry_policy_is_the_recorders() {
        // 503 then 200: one retry, the success returned, the retry noted.
        struct Flaky(RefCell<usize>);
        impl Transport for Flaky {
            fn get(&self, _url: &str) -> TransportResponse {
                let n = *self.0.borrow();
                *self.0.borrow_mut() = n + 1;
                if n == 0 {
                    TransportResponse {
                        status: 503,
                        content_type: "".into(),
                        body: "".into(),
                        note: "HTTP 503".into(),
                    }
                } else {
                    TransportResponse {
                        status: 200,
                        content_type: "application/json".into(),
                        body: "[]".into(),
                        note: "".into(),
                    }
                }
            }
        }
        let slept = RefCell::new(Vec::new());
        let resp = fetch_with_retry(&Flaky(RefCell::new(0)), "https://x.test", |ms| {
            slept.borrow_mut().push(ms)
        });
        assert_eq!(resp.status, 200);
        assert!(resp.note.contains("retried once"), "{}", resp.note);
        assert_eq!(*slept.borrow(), vec![RETRY_DELAY_MS]);
        // A transport error (status 0) is NOT retried — a dead socket is answered once, honestly.
        struct Dead;
        impl Transport for Dead {
            fn get(&self, _url: &str) -> TransportResponse {
                TransportResponse {
                    status: 0,
                    content_type: "".into(),
                    body: "".into(),
                    note: "transport error: dns".into(),
                }
            }
        }
        let resp = fetch_with_retry(&Dead, "https://x.test", no_sleep);
        assert_eq!(resp.status, 0);
    }

    #[test]
    fn a_non_200_is_data_for_the_probe_and_words_for_records() {
        let reg = registry();
        let mock = Mock::new().with(
            &fill_endpoint(&reg, "swpc.kp", NOW, no_env).unwrap().url,
            TransportResponse {
                status: 404,
                content_type: "".into(),
                body: "gone".into(),
                note: "HTTP 404".into(),
            },
        );
        let raw = fetch_raw(&reg, "swpc.kp", &mock, NOW, no_env, no_sleep).unwrap();
        assert_eq!(raw.status, 404, "the probe records a 404 as a fact");
        let def = reg.get("swpc.kp").unwrap();
        let err = records_of(def, &raw, NOW).unwrap_err();
        assert!(err.message.contains("HTTP 404"), "{}", err.message);
    }

    #[test]
    fn a_live_shape_fetch_normalizes_through_the_frozen_vocabulary() {
        // The checked-in Kp fixture stands in for a live payload (same road, hermetic test).
        let body = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/fixtures/observatory/swpc.kp.json"
        ))
        .unwrap();
        let reg = registry();
        let url = fill_endpoint(&reg, "swpc.kp", NOW, no_env).unwrap().url;
        let mock = Mock::new().with(
            &url,
            TransportResponse {
                status: 200,
                content_type: "application/json".into(),
                body,
                note: "".into(),
            },
        );
        let raw = fetch_raw(&reg, "swpc.kp", &mock, NOW, no_env, no_sleep).unwrap();
        let def = reg.get("swpc.kp").unwrap();
        let records = records_of(def, &raw, NOW).unwrap();
        assert!(!records.is_empty(), "the 3-day Kp series normalizes to records");
        assert!(records.iter().all(|r| r.schema_id == "observatory/timeseries"));
    }
    /// The live smoke: one real fetch of the most stable feed (SWPC Kp — no key, no date
    /// placeholders) through the real `HttpTransport`. IGNORED by default: CI and the hermetic
    /// gates never touch a socket (D11); the device run-sheet (test007 step B) and deliberate
    /// runs (`cargo test -p sparq-streams --features streams-net -- --ignored live_smoke`) do.
    #[cfg(feature = "streams-net")]
    #[test]
    #[ignore = "live network — run deliberately with --ignored"]
    fn live_smoke_swpc_kp() {
        let reg = registry();
        let t = HttpTransport::new(std::time::Duration::from_secs(20));
        let now = now_unix();
        let env = |k: &str| std::env::var(k).ok();
        let raw = fetch_raw(&reg, "swpc.kp", &t, now, env, sleep_millis)
            .expect("the Kp endpoint fills without a key");
        assert_eq!(raw.status, 200, "SWPC Kp answered {} ({})", raw.status, raw.note);
        assert!(!raw.body.is_empty());
        let def = reg.get("swpc.kp").expect("kp is in the registry");
        let records = records_of(def, &raw, now).expect("the live payload normalizes");
        assert!(!records.is_empty(), "the live 3-day Kp series has records");
    }
}
