//! `sparq streams …` — the CLI voice over the stream plane's broker (WO-020 INC1).
//!
//! Two hermetic verbs work in any `--features streams` build and touch no network:
//! * `list` — the registry as data: all 23 streams with domain, cadence, view, schema and key state
//!   (KEY NEEDED in words, plan D14 — never a silent hole).
//! * `cache path|clear` — the disk cache location and its contents (plan §6.2).
//!
//! The live verbs (`probe --live`, `fetch`, `tail`) ride the transport half — `ureq` + TLS +
//! `fetch.rs`, behind `streams-net` (landed INC5). Without the feature they **refuse in words and
//! exit non-zero**, exactly like `sparq mod validate`'s runtime stages and `sparq ui` without the
//! windowing stack: a refusal that names what would make it run, never a pretend-fetch and never
//! a silent hole. With it, they really reach the network — one request per stream, the recorder's
//! politeness (400 ms apart), one retry on 429/5xx, every status recorded as data (the DONKI
//! lesson: a dead endpoint is a fact, not a crash). `record-fixtures` stays a refusal even with
//! the transport on: the fixture format (`_meta.json` + raw payloads + PROBE-LOG + the flagged
//! synthetic FIRMS sample) has ONE owner — `tools/streams_record.py` — and a second owner in Rust
//! would be exactly the drift the house forbids.

use std::process::ExitCode;

#[cfg(feature = "streams")]
use sparq_streams::cache::Cache;
#[cfg(feature = "streams")]
use sparq_streams::registry::Registry;

/// A command's failure is a message shown verbatim to the operator, so it is a `String` (the CLI's
/// uniform dispatch contract; refusals are printed words + exit codes, not Rust errors).
type Result<T> = std::result::Result<T, String>;

/// What `sparq streams cache` was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheOp {
    /// Print the cache root path.
    Path,
    /// Remove every cached payload.
    Clear,
    /// Print the root and the cached stream ids.
    Show,
}

/// What `sparq streams` was asked to do. The subcommand vocabulary is closed, like every other door.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamsCommand {
    /// Print the registry (hermetic).
    List,
    /// Probe endpoints — `--live` hits the network (refused without `streams-net`).
    Probe { live: bool },
    /// Fetch one stream to a file (network; refused without `streams-net`).
    Fetch { id: String, out: Option<String> },
    /// Tail one stream's records (network; refused without `streams-net`).
    Tail { id: String },
    /// Record fixtures for every stream (network; refused without `streams-net`).
    RecordFixtures { out: String },
    /// Inspect or clear the disk cache (hermetic).
    Cache { op: CacheOp },
}

/// Parses `sparq streams` arguments.
///
/// # Errors
/// A message naming the unrecognised shape — the subcommand vocabulary is closed.
pub fn parse(args: &[String]) -> Result<StreamsCommand> {
    let (sub, rest) = args.split_first().ok_or_else(|| {
        "`sparq streams` needs a subcommand: `list`, `cache [path|clear]`, `probe [--live]`, \
         `fetch ID [--out FILE]`, `tail ID`, or `record-fixtures --out DIR`"
            .to_string()
    })?;
    match sub.as_str() {
        "list" => Ok(StreamsCommand::List),
        "cache" => Ok(StreamsCommand::Cache { op: parse_cache_op(rest)? }),
        "probe" => {
            reject_unknown(rest, &["--live"], "sparq streams probe")?;
            Ok(StreamsCommand::Probe { live: rest.iter().any(|a| a == "--live") })
        },
        "fetch" => {
            let mut id = None;
            let mut out = None;
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--out" => {
                        i += 1;
                        out = Some(rest.get(i).ok_or_else(|| "`--out` needs a FILE".to_string())?.clone());
                    },
                    other if other.starts_with("--") => return Err(format!("unknown option `{other}` for `sparq streams fetch`")),
                    other => {
                        if id.is_some() {
                            return Err(format!("`sparq streams fetch` takes one stream id, got `{other}` too"));
                        }
                        id = Some(other.to_string());
                    },
                }
                i += 1;
            }
            Ok(StreamsCommand::Fetch { id: id.ok_or_else(|| "`sparq streams fetch` needs a stream ID".to_string())?, out })
        },
        "tail" => {
            let id = rest.first().ok_or_else(|| "`sparq streams tail` needs a stream ID".to_string())?;
            if rest.len() > 1 {
                return Err(format!("`sparq streams tail` takes exactly one id, got {} more", rest.len() - 1));
            }
            Ok(StreamsCommand::Tail { id: id.clone() })
        },
        "record-fixtures" => {
            let mut out = None;
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--out" => {
                        i += 1;
                        out = Some(rest.get(i).ok_or_else(|| "`--out` needs a DIR".to_string())?.clone());
                    },
                    other => return Err(format!("unknown argument `{other}` for `sparq streams record-fixtures`")),
                }
                i += 1;
            }
            Ok(StreamsCommand::RecordFixtures {
                out: out.ok_or_else(|| "`sparq streams record-fixtures` needs `--out DIR`".to_string())?,
            })
        },
        other => Err(format!(
            "unknown subcommand `{other}` for `sparq streams`; try `list`, `cache`, `probe`, `fetch`, `tail`, or `record-fixtures`"
        )),
    }
}

fn parse_cache_op(rest: &[String]) -> Result<CacheOp> {
    match rest.first().map(String::as_str) {
        None => Ok(CacheOp::Show),
        Some("path") => Ok(CacheOp::Path),
        Some("clear") => Ok(CacheOp::Clear),
        Some(other) => {
            Err(format!("unknown `sparq streams cache` op `{other}`; try `path` or `clear`"))
        },
    }
}

fn reject_unknown(rest: &[String], allowed: &[&str], ctx: &str) -> Result<()> {
    for a in rest {
        if !allowed.contains(&a.as_str()) {
            return Err(format!("unknown argument `{a}` for `{ctx}`"));
        }
    }
    Ok(())
}

/// Runs the parsed subcommand.
///
/// # Errors
/// Never — refusals are printed words and exit codes, not Rust errors (the `Result` shape is the
/// CLI's uniform dispatch contract; this door always answers `Ok`).
pub fn run(cmd: StreamsCommand) -> Result<ExitCode> {
    Ok(ExitCode::from(run_code(cmd)))
}

/// The exit code for a parsed command: `0` = success, `1` = a refusal or failure. Kept separate from
/// [`run`] so the refusal contract ("live verbs exit non-zero without `streams-net`") is *testable* —
/// `std::process::ExitCode` is opaque and cannot be compared, but its `u8` can. Refusals print words
/// and return `1`; they are never a Rust `Err` (the CLI's uniform dispatch contract).
fn run_code(cmd: StreamsCommand) -> u8 {
    #[cfg(feature = "streams")]
    {
        match cmd {
            StreamsCommand::List => run_list(),
            StreamsCommand::Cache { op } => run_cache(op),
            StreamsCommand::Probe { live } => probe_cmd(live),
            StreamsCommand::Fetch { id, out } => fetch_cmd(&id, out.as_deref()),
            StreamsCommand::Tail { id } => tail_cmd(&id),
            StreamsCommand::RecordFixtures { out } => record_fixtures_cmd(&out),
        }
    }
    #[cfg(not(feature = "streams"))]
    {
        let _ = cmd;
        eprintln!("sparq streams: this binary was built without the streams feature.");
        eprintln!("  build it with:  cargo build --release -p sparq-app --features streams");
        eprintln!(
            "  (the default build is deliberately dependency-free: ADR-008 / the zero-dep promise)"
        );
        1
    }
}

/// `sparq streams list` — the registry, as data, in words.
#[cfg(feature = "streams")]
fn run_list() -> u8 {
    let reg = match Registry::load() {
        Ok(r) => r,
        Err(e) => {
            println!("sparq streams list: the checked-in registry failed to load: {e}");
            return 1;
        },
    };
    // The overrides store (WO-020 INC6 D18): the list reads THROUGH it — the cadence shown is
    // the one a poll would actually pace on, and the KEY column names the governing source
    // (env wins over the file, and the words say which — the precedence is visible). A
    // malformed store is words on stdout and an empty store, never a crash (§6.3).
    let (store, store_words) = sparq_streams::store::Overrides::load_default();
    if let Some(w) = store_words {
        println!("sparq streams list: {w}");
    }
    let env = |k: &str| std::env::var(k).ok();
    println!(
        "sparq streams — the registry ({} streams, crates/sparq-streams/streams.toml)",
        reg.len()
    );
    println!("  the single source of truth for stream ids (broker + validator + manifest generator read it)");
    println!();
    println!("STREAM ID            DOM   CAD  VIEW             SCHEMA                   KEY");
    for def in reg.streams() {
        let key = sparq_streams::store::key_words(def, &reg, env, &store).to_string();
        let cadence = match store.cadence_s(&def.id) {
            Some(o) => format!("{o}s*"),
            None => format!("{}s", def.cadence_s),
        };
        println!(
            "{:<20} {:<4} {:>4}  {:<16} {:<24} {}",
            def.id, def.domain, cadence, def.view, def.schema, key
        );
    }
    println!();
    for d in reg.domains() {
        println!("  {d}: {} stream(s)", reg.by_domain(d).len());
    }
    let overridden: Vec<String> = reg
        .streams()
        .iter()
        .filter_map(|s| {
            store.cadence_s(&s.id).map(|o| format!("{} {o}s (registry {}s)", s.id, s.cadence_s))
        })
        .collect();
    if !overridden.is_empty() {
        println!("\n  * cadence overrides from the user-data store: {}", overridden.join("; "));
    }
    println!(
        "\n  overrides store: {} ({} row(s)) — keys persist there in plain text (ruling O-4);",
        store.path().display(),
        store.len()
    );
    println!(
        "  env vars WIN over it; UNSET = running on the row's declared fallback (NASA's DEMO_KEY)."
    );
    let needed: Vec<&str> = reg
        .streams()
        .iter()
        .filter(|s| sparq_streams::store::key_words(s, &reg, env, &store) == "KEY NEEDED")
        .map(|s| s.id.as_str())
        .collect();
    if needed.is_empty() {
        println!("\n  every stream is fetchable (keys present or not needed).");
    } else {
        println!("\n  KEY NEEDED (not fetched, in words — plan D14): {}", needed.join(", "));
        println!("  fix: set the env var named in streams.toml (e.g. SPARQ_FIRMS_KEY, free registration),");
        println!("       or type the key into the shell's STREAMS tab (it persists to the store).");
    }
    0
}

/// `sparq streams cache …` — the disk cache, hermetically.
#[cfg(feature = "streams")]
fn run_cache(op: CacheOp) -> u8 {
    let cache = Cache::open_default();
    match op {
        CacheOp::Path => println!("{}", cache.root().display()),
        CacheOp::Show => {
            println!("cache root: {}", cache.root().display());
            let ids = cache.ids();
            if ids.is_empty() {
                println!("  (empty — no last-good payloads cached yet)");
            } else {
                println!("  {} cached stream(s): {}", ids.len(), ids.join(", "));
            }
        },
        CacheOp::Clear => match cache.clear() {
            Ok(n) => println!("cleared {n} cached payload(s) from {}", cache.root().display()),
            Err(e) => {
                println!("sparq streams cache clear: {e}");
                return 1;
            },
        },
    }
    0
}

/// The refusal for a live verb when the transport half is not in the build (plan INC5 / D2).
/// Names what would make it run and the hermetic alternatives, in words; returns `1` (a refusal
/// is not a success).
#[cfg(all(feature = "streams", not(feature = "streams-net")))]
fn refuse_network(verb: &str, _live: bool) -> u8 {
    eprintln!("sparq streams {verb}: the live transport half is not in this build.");
    eprintln!(
        "  build it with:  cargo build --release -p sparq-app --features streams,streams-net"
    );
    eprintln!(
        "  (ureq + rustls ride `streams-net`; the default build is deliberately socket-free.)"
    );
    eprintln!("  hermetic now:   sparq streams list    |    sparq streams cache path|clear");
    eprintln!("  record live payloads with python (it has network):  python3 tools/streams_record.py --all");
    1
}

#[cfg(all(feature = "streams", not(feature = "streams-net")))]
fn probe_cmd(live: bool) -> u8 {
    refuse_network("probe", live)
}
#[cfg(all(feature = "streams", not(feature = "streams-net")))]
fn fetch_cmd(_id: &str, _out: Option<&str>) -> u8 {
    refuse_network("fetch", true)
}
#[cfg(all(feature = "streams", not(feature = "streams-net")))]
fn tail_cmd(_id: &str) -> u8 {
    refuse_network("tail", true)
}

// ── the live verbs (streams-net) ────────────────────────────────────────────────────────────────

#[cfg(all(feature = "streams", feature = "streams-net"))]
fn probe_cmd(live: bool) -> u8 {
    if live {
        run_probe_live()
    } else {
        run_probe_static()
    }
}

/// `sparq streams probe` — the fetch plan as data: what WOULD be requested, with which key state,
/// on which cadence. Hermetic (no socket); `--live` is the real probe.
#[cfg(all(feature = "streams", feature = "streams-net"))]
fn run_probe_static() -> u8 {
    let reg = match Registry::load() {
        Ok(r) => r,
        Err(e) => {
            println!("sparq streams probe: the checked-in registry failed to load: {e}");
            return 1;
        },
    };
    // The env door is the STORE-composed closure (D18): a file key makes a stream fetchable
    // exactly like an env key — one store, every door.
    let (store, store_words) = sparq_streams::store::Overrides::load_default();
    if let Some(w) = store_words {
        println!("sparq streams probe: {w}");
    }
    let env = store.env_closure(&reg);
    println!(
        "sparq streams probe — the fetch plan ({} streams; add --live to hit the network)",
        reg.len()
    );
    println!("STREAM ID            DOM  CADENCE  KEY          ENDPOINT (template)");
    let mut needed = 0usize;
    for def in reg.streams() {
        let key = match def.key_state(&env) {
            sparq_streams::KeyState::NotNeeded => "—".to_string(),
            sparq_streams::KeyState::Ready => "key set".to_string(),
            sparq_streams::KeyState::UsingFallback(fb) => format!("{fb} (fallback)"),
            sparq_streams::KeyState::KeyNeeded => {
                needed += 1;
                "KEY NEEDED".to_string()
            },
        };
        println!(
            "{:<20} {:<4} {:>5}s  {:<12} {}",
            def.id, def.domain, def.cadence_s, key, def.endpoint
        );
    }
    if needed > 0 {
        println!("\n  {needed} stream(s) KEY NEEDED — not fetched, in words (plan D14); set the env var named in streams.toml.");
    }
    0
}

/// `sparq streams probe --live` — every endpoint, once, for real: the run-sheet's step B
/// (test007 §15.2). Statuses are DATA (a 404 is recorded, not a crash — the DONKI lesson), so the
/// exit code stays 0; the summary line names every kind of failure in words.
#[cfg(all(feature = "streams", feature = "streams-net"))]
fn run_probe_live() -> u8 {
    use sparq_streams::fetch::{self, POLITE_DELAY_MS};
    let reg = match Registry::load() {
        Ok(r) => r,
        Err(e) => {
            println!("sparq streams probe --live: the checked-in registry failed to load: {e}");
            return 1;
        },
    };
    let transport = fetch::HttpTransport::new(std::time::Duration::from_secs(20));
    let now = fetch::now_unix();
    let (store, store_words) = sparq_streams::store::Overrides::load_default();
    if let Some(w) = store_words {
        println!("sparq streams probe --live: {w}");
    }
    let env = store.env_closure(&reg);
    println!("sparq streams probe --live — {} endpoints at {}", reg.len(), utc_stamp_i64(now));
    println!("  one request per stream, {POLITE_DELAY_MS} ms apart; 429/5xx retried once (the recorder's policy)");
    println!();
    println!(" #  STREAM ID            DOM   STATUS      BYTES  KEY SOURCE           NOTE");
    let (mut ok, mut other, mut dead, mut refused) = (0usize, 0usize, 0usize, 0usize);
    for (i, def) in reg.streams().iter().enumerate() {
        if i > 0 {
            fetch::sleep_millis(POLITE_DELAY_MS);
        }
        match fetch::fetch_raw(&reg, &def.id, &transport, now, &env, fetch::sleep_millis) {
            Err(e) => {
                refused += 1;
                println!(
                    "{:>2}  {:<20} {:<5} {:>6}  {:>9}  {:<20} {}",
                    i + 1,
                    def.id,
                    def.domain,
                    "—",
                    "—",
                    "—",
                    e.message
                );
            },
            Ok(raw) => {
                match raw.status {
                    200 => ok += 1,
                    0 => dead += 1,
                    _ => other += 1,
                }
                let status =
                    if raw.status == 0 { "ERR".to_string() } else { raw.status.to_string() };
                println!(
                    "{:>2}  {:<20} {:<5} {:>6}  {:>9}  {:<20} {}",
                    i + 1,
                    def.id,
                    def.domain,
                    status,
                    raw.body.len(),
                    raw.key_source.as_words(),
                    if raw.note.is_empty() { "—".to_string() } else { raw.note.clone() }
                );
            },
        }
    }
    println!(
        "\n  {ok}/{} HTTP 200 · {other} other status · {dead} transport failure(s) · {refused} refused pre-fetch (KEY NEEDED, in words — D14)",
        reg.len()
    );
    println!("  statuses are data: record them (run-sheet test007 §B); a dead endpoint is a fact, not a crash.");
    0
}

#[cfg(all(feature = "streams", feature = "streams-net"))]
fn fetch_cmd(id: &str, out: Option<&str>) -> u8 {
    use sparq_streams::fetch;
    let reg = match Registry::load() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sparq streams fetch: the checked-in registry failed to load: {e}");
            return 1;
        },
    };
    let transport = fetch::HttpTransport::new(std::time::Duration::from_secs(20));
    let now = fetch::now_unix();
    // The store-composed env door (D18): a file key rides the fetch exactly like an env key,
    // and the fetch's own redaction covers it (the value lives in Request::url alone).
    let (store, store_words) = sparq_streams::store::Overrides::load_default();
    if let Some(w) = store_words {
        eprintln!("sparq streams: {w}");
    }
    let env = store.env_closure(&reg);
    let raw = match fetch::fetch_raw(&reg, id, &transport, now, env, fetch::sleep_millis) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sparq streams fetch {id}: {}", e.message);
            return 1;
        },
    };
    if raw.status != 200 {
        eprintln!(
            "sparq streams fetch {id}: HTTP {} — payload NOT written{}.",
            raw.status,
            if raw.note.is_empty() { String::new() } else { format!(" ({})", raw.note) }
        );
        return 1;
    }
    match out {
        Some(path) => {
            // File::create + write_all, the cache.rs idiom — the banned `std::fs::write`
            // convenience is the audio-path rule; this is the CLI's control path.
            use std::io::Write;
            let mut f = match std::fs::File::create(path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("sparq streams fetch {id}: cannot create {path}: {e}");
                    return 1;
                },
            };
            if let Err(e) = f.write_all(raw.body.as_bytes()) {
                eprintln!("sparq streams fetch {id}: write to {path} failed: {e}");
                return 1;
            }
            eprintln!(
                "wrote {} bytes to {path} — {} (key: {})",
                raw.body.len(),
                raw.url,
                raw.key_source.as_words()
            );
        },
        None => print!("{}", raw.body),
    }
    0
}

#[cfg(all(feature = "streams", feature = "streams-net"))]
fn tail_cmd(id: &str) -> u8 {
    use sparq_streams::fetch;
    let reg = match Registry::load() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sparq streams tail: the checked-in registry failed to load: {e}");
            return 1;
        },
    };
    let Some(def) = reg.get(id) else {
        eprintln!("sparq streams tail: {}", reg.unknown_id_message(id));
        return 1;
    };
    let transport = fetch::HttpTransport::new(std::time::Duration::from_secs(20));
    let now = fetch::now_unix();
    // The store-composed env door (D18): a file key rides the fetch exactly like an env key,
    // and the fetch's own redaction covers it (the value lives in Request::url alone).
    let (store, store_words) = sparq_streams::store::Overrides::load_default();
    if let Some(w) = store_words {
        eprintln!("sparq streams: {w}");
    }
    let env = store.env_closure(&reg);
    let raw = match fetch::fetch_raw(&reg, id, &transport, now, env, fetch::sleep_millis) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sparq streams tail {id}: {}", e.message);
            return 1;
        },
    };
    let records = match fetch::records_of(def, &raw, now) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sparq streams tail {id}: {}", e.message);
            return 1;
        },
    };
    let units = if def.units.is_empty() { "—".to_string() } else { def.units.clone() };
    println!(
        "sparq streams tail {id} — {} record(s), oldest → newest, schema {} (units: {units}); one fetch, no follow (the live wall owns cadence)",
        records.len(),
        def.schema
    );
    // The newest 20, oldest → newest (a tail shows the end, not the whole 3-day series).
    let start = records.len().saturating_sub(20);
    if start > 0 {
        println!("  … {} older record(s) not shown", start);
    }
    for r in &records[start..] {
        println!("  {}  {}", utc_stamp_ns(r.t_wall_ns), channel_words(&r.channels));
    }
    0
}

/// `record-fixtures` refuses even with the transport on: ONE owner of the fixture format.
#[cfg(feature = "streams")]
fn record_fixtures_cmd(out: &str) -> u8 {
    eprintln!("sparq streams record-fixtures: the fixture recorder is tools/streams_record.py — one owner, not two.");
    eprintln!("  the fixture format (_meta.json + raw payloads + PROBE-LOG + the flagged synthetic FIRMS sample)");
    eprintln!("  is that tool's artefact; the normalizer tests and the golden pin read what it writes, and");
    eprintln!("  re-recording MOVES the goldens — run it deliberately, never as a side effect of a CLI verb:");
    eprintln!("    python3 tools/streams_record.py --all --out {out}");
    eprintln!(
        "  the Rust live verbs cover probe/fetch/tail; recording stays with the python owner."
    );
    1
}

/// A unix-seconds stamp as `YYYY-MM-DD HH:MM:SS UTC` (the probe header's clock words).
#[cfg(all(feature = "streams", feature = "streams-net"))]
fn utc_stamp_i64(now_unix: i64) -> String {
    let (y, m, d) = sparq_streams::fetch::civil_from_unix(now_unix);
    let sod = now_unix.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", sod / 3600, (sod % 3600) / 60, sod % 60)
}

/// A record's nanosecond stamp as `YYYY-MM-DDTHH:MM:SSZ` (the tail's row head).
#[cfg(all(feature = "streams", feature = "streams-net"))]
fn utc_stamp_ns(t_wall_ns: u64) -> String {
    let secs = i64::try_from(t_wall_ns / 1_000_000_000).unwrap_or(0);
    let (y, m, d) = sparq_streams::fetch::civil_from_unix(secs);
    let sod = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", sod / 3600, (sod % 3600) / 60, sod % 60)
}

/// One record's channels, positionally (the schema declares the order; the tail is a diagnostic,
/// not a renderer — numbers WITH their units ride the header line, token rule 9).
#[cfg(all(feature = "streams", feature = "streams-net"))]
fn channel_words(channels: &[sparq_streams::record::DataValue]) -> String {
    use sparq_streams::record::DataValue;
    let parts: Vec<String> = channels
        .iter()
        .map(|v| match v {
            DataValue::Single(f) => format!("{f}"),
            DataValue::Double(f) => format!("{f}"),
            DataValue::Int32(i) => format!("{i}"),
            DataValue::Int64(i) => format!("{i}"),
            DataValue::Boolean(b) => format!("{b}"),
            DataValue::Vec(xs) => format!("[{} value(s)]", xs.len()),
            DataValue::Enumerated(i) => format!("#{i}"),
            DataValue::Text(t) => {
                let flat: String = t.chars().take(60).collect();
                if flat.len() < t.len() {
                    format!("“{flat}…”")
                } else {
                    format!("“{flat}”")
                }
            },
            DataValue::Blob(b) => format!("<{} byte(s)>", b.len()),
        })
        .collect();
    parts.join("  ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parses_the_closed_subcommand_vocabulary() {
        assert_eq!(parse(&args("list")).unwrap(), StreamsCommand::List);
        assert_eq!(
            parse(&args("cache path")).unwrap(),
            StreamsCommand::Cache { op: CacheOp::Path }
        );
        assert_eq!(
            parse(&args("cache clear")).unwrap(),
            StreamsCommand::Cache { op: CacheOp::Clear }
        );
        assert_eq!(parse(&args("cache")).unwrap(), StreamsCommand::Cache { op: CacheOp::Show });
        assert_eq!(parse(&args("probe")).unwrap(), StreamsCommand::Probe { live: false });
        assert_eq!(parse(&args("probe --live")).unwrap(), StreamsCommand::Probe { live: true });
        assert_eq!(
            parse(&args("tail swpc.kp")).unwrap(),
            StreamsCommand::Tail { id: "swpc.kp".into() }
        );
        assert_eq!(
            parse(&args("fetch sat.iss --out /tmp/iss.json")).unwrap(),
            StreamsCommand::Fetch { id: "sat.iss".into(), out: Some("/tmp/iss.json".into()) }
        );
        assert_eq!(
            parse(&args("record-fixtures --out reference/fixtures/observatory")).unwrap(),
            StreamsCommand::RecordFixtures { out: "reference/fixtures/observatory".into() }
        );
    }

    #[test]
    fn refuses_bad_shapes_in_words() {
        assert!(parse(&args("")).is_err(), "no subcommand is an error");
        assert!(parse(&args("nope")).is_err(), "unknown subcommand is an error");
        assert!(parse(&args("fetch")).is_err(), "fetch without an id is an error");
        assert!(
            parse(&args("record-fixtures")).is_err(),
            "record-fixtures without --out is an error"
        );
        let e = parse(&args("cache nope")).unwrap_err();
        assert!(e.contains("cache"), "the refusal names the subcommand: {e}");
    }

    // The refusal contract, tested via the u8 exit code (ExitCode itself is opaque). Without the
    // `streams` feature EVERY verb refuses (exit 1); with `streams` but without `streams-net`, the
    // hermetic verbs succeed (exit 0) and the live verbs still refuse (exit 1) — the INC1
    // contract, unchanged. With `streams-net` the live verbs are real (the tests below stay
    // hermetic: they exercise the pre-network refusals and the static faces only).
    #[cfg(all(feature = "streams", not(feature = "streams-net")))]
    #[test]
    fn live_verbs_exit_nonzero_without_streams_net() {
        for cmd in [
            StreamsCommand::Probe { live: false },
            StreamsCommand::Probe { live: true },
            StreamsCommand::Fetch { id: "swpc.kp".into(), out: None },
            StreamsCommand::Tail { id: "swpc.kp".into() },
            StreamsCommand::RecordFixtures { out: "/tmp/sparq-rec".into() },
        ] {
            assert_eq!(run_code(cmd), 1, "a live verb must refuse (exit 1) without streams-net");
        }
    }

    #[cfg(feature = "streams")]
    #[test]
    fn hermetic_verbs_succeed_with_the_streams_feature() {
        assert_eq!(run_code(StreamsCommand::List), 0, "list is hermetic (registry as data)");
        assert_eq!(
            run_code(StreamsCommand::Cache { op: CacheOp::Path }),
            0,
            "cache path is hermetic"
        );
    }

    // With the transport on, the refusal contract narrows to the shapes that refuse BEFORE any
    // socket: unknown ids (the registry's own words), record-fixtures (the python recorder owns
    // the fixture format — one owner, not two), and the static probe still exits 0 hermetically.
    #[cfg(all(feature = "streams", feature = "streams-net"))]
    #[test]
    fn streams_net_verbs_refuse_the_right_shapes_hermetically() {
        assert_eq!(
            run_code(StreamsCommand::Fetch { id: "foo.bar".into(), out: None }),
            1,
            "an unknown id refuses pre-network with the registry's words"
        );
        assert_eq!(run_code(StreamsCommand::Tail { id: "foo.bar".into() }), 1);
        assert_eq!(
            run_code(StreamsCommand::RecordFixtures { out: "/tmp/sparq-rec".into() }),
            1,
            "record-fixtures stays a refusal — tools/streams_record.py owns the format"
        );
        assert_eq!(
            run_code(StreamsCommand::Probe { live: false }),
            0,
            "the static probe is hermetic (the fetch plan as data)"
        );
    }

    #[cfg(all(feature = "streams", feature = "streams-net"))]
    #[test]
    fn the_stamp_and_channel_formatters_print_words() {
        // 2026-10-06T12:34:56Z = 1791290096 (cross-checked against python datetime).
        assert_eq!(utc_stamp_i64(1_791_290_096), "2026-10-06 12:34:56 UTC");
        assert_eq!(utc_stamp_ns(1_791_290_096_000_000_000), "2026-10-06T12:34:56Z");
        let words = channel_words(&[
            sparq_streams::record::DataValue::Double(3.5),
            sparq_streams::record::DataValue::Text("hello".into()),
        ]);
        assert!(words.contains("3.5") && words.contains("hello"), "{words}");
    }

    #[test]
    fn run_wraps_the_code_as_an_exitcode_without_erroring() {
        // A refusal is Ok(exit-code), never a Rust Err — the CLI's uniform dispatch contract.
        assert!(run(StreamsCommand::Probe { live: true }).is_ok());
    }
}
