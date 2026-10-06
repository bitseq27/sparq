//! `sparq streams …` — the CLI voice over the stream plane's broker (WO-020 INC1).
//!
//! Two hermetic verbs work in any `--features streams` build and touch no network:
//! * `list` — the registry as data: all 23 streams with domain, cadence, view, schema and key state
//!   (KEY NEEDED in words, plan D14 — never a silent hole).
//! * `cache path|clear` — the disk cache location and its contents (plan §6.2).
//!
//! The live verbs (`probe`, `fetch`, `tail`, `record-fixtures`) need the transport half — `ureq` +
//! TLS + `fetch.rs`, behind `streams-net` — which is the DEVICE increment (plan INC5, D2/§12.1: the
//! ~1 GB sandbox builds the hermetic half; the socket lands on a ≥ 2 GB host). Until it does, they
//! **refuse in words and exit non-zero**, exactly like `sparq mod validate`'s runtime stages and
//! `sparq ui` without the windowing stack: a refusal that names what would make it run, never a
//! pretend-fetch and never a silent hole. (Fixtures are recorded with `tools/streams_record.py`,
//! which has python + network and is the sandbox's legal way to capture live payloads.)

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
            StreamsCommand::Probe { live } => refuse_network("probe", live),
            StreamsCommand::Fetch { .. } => refuse_network("fetch", true),
            StreamsCommand::Tail { .. } => refuse_network("tail", true),
            StreamsCommand::RecordFixtures { .. } => refuse_network("record-fixtures", true),
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
    let env = |k: &str| std::env::var(k).ok();
    println!(
        "sparq streams — the registry ({} streams, crates/sparq-streams/streams.toml)",
        reg.len()
    );
    println!("  the single source of truth for stream ids (broker + validator + manifest generator read it)");
    println!();
    println!("STREAM ID            DOM   CAD  VIEW             SCHEMA                   KEY");
    for def in reg.streams() {
        let key = match def.key_state(env) {
            sparq_streams::KeyState::NotNeeded => "—".to_string(),
            sparq_streams::KeyState::Ready => "key set".to_string(),
            sparq_streams::KeyState::UsingFallback(fb) => format!("{fb} (fallback)"),
            sparq_streams::KeyState::KeyNeeded => "KEY NEEDED".to_string(),
        };
        println!(
            "{:<20} {:<4} {:>4}s  {:<16} {:<24} {}",
            def.id, def.domain, def.cadence_s, def.view, def.schema, key
        );
    }
    println!();
    for d in reg.domains() {
        println!("  {d}: {} stream(s)", reg.by_domain(d).len());
    }
    let needed: Vec<&str> = reg
        .streams()
        .iter()
        .filter(|s| matches!(s.key_state(env), sparq_streams::KeyState::KeyNeeded))
        .map(|s| s.id.as_str())
        .collect();
    if needed.is_empty() {
        println!("\n  every stream is fetchable (keys present or not needed).");
    } else {
        println!("\n  KEY NEEDED (not fetched, in words — plan D14): {}", needed.join(", "));
        println!("  fix: set the env var named in streams.toml (e.g. SPARQ_FIRMS_KEY, free registration).");
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

/// The refusal for a live verb when the transport half is not in the build (plan INC5 / D2). Names
/// what would make it run and the hermetic alternatives, in words; returns `1` (a refusal is not a
/// success). When INC5 lands `fetch.rs`, the real transport path replaces this behind
/// `#[cfg(feature = "streams-net")]`.
#[cfg(feature = "streams")]
fn refuse_network(verb: &str, _live: bool) -> u8 {
    eprintln!("sparq streams {verb}: the live transport half is not in this build.");
    eprintln!("  `streams-net` (ureq + TLS + fetch.rs) is the DEVICE increment — WO-020 INC5 (plan D2, §12.1):");
    eprintln!(
        "  the ~1 GB sandbox builds the hermetic half only; the socket lands on a >= 2 GB host."
    );
    eprintln!("  hermetic now:   sparq streams list    |    sparq streams cache path|clear");
    eprintln!("  record live payloads with python (it has network):  python3 tools/streams_record.py --all");
    1
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
    // `streams` feature EVERY verb refuses (exit 1); with it, the hermetic verbs succeed (exit 0)
    // and the live verbs still refuse (exit 1) until the transport half lands (INC5).
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

    #[test]
    fn run_wraps_the_code_as_an_exitcode_without_erroring() {
        // A refusal is Ok(exit-code), never a Rust Err — the CLI's uniform dispatch contract.
        assert!(run(StreamsCommand::Probe { live: true }).is_ok());
    }
}
