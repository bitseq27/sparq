//! `sparq-streams` — the stream plane's broker (WO-020, ADR-011).
//!
//! This crate is the host-side half of "live data reaches an instrument": it owns the checked-in
//! stream [`registry`], turns each feed's raw payload into the frozen [`record::DataRecord`]
//! vocabulary ([`normalize`]), keeps bounded rolling [`window`]s with a stale/offline policy, caches
//! last-good payloads to disk ([`cache`]), and replays recorded fixtures into windows ([`replay`])
//! so goldens and tests never touch a socket (the determinism firewall, plan D11).
//!
//! # The guest never sees this crate
//!
//! The instrument consumes the broker's records through the contract's own door —
//! `sources.snapshot(display, source)` → `window(list<data-record>)` (sources.wit) — resolved
//! host-side by the stream-source bindings (plan D4). The T2 world has no network door, so live
//! fetching *cannot* live in the guest; it lives here, on a control thread (plan D3). Nothing in
//! this crate is on the audio path, and v1 has no audio path at all (plan D5): it is display-only.
//!
//! # Two features, and a zero-dependency default
//!
//! * **default (no features):** the hermetic, dependency-free core — the data model ([`record`],
//!   [`schema`]), the [`registry`] (parsed with sparq-module-api's TOML subset, no serde), and the
//!   [`window`]s + stale policy. This is real, tested content, not an empty crate (ADR-000).
//! * **`streams`:** adds the one third-party dependency (`serde_json`) and with it the JSON/GeoJSON/
//!   CSV [`normalize`]rs, the disk [`cache`] and fixture [`replay`]. Everything the sandbox can
//!   prove lives here; the app's `sparq streams` verbs and the 23 per-stream normalizer tests ride
//!   this feature.
//! * **`streams-net`:** the live transport (`ureq` + TLS + `fetch.rs`). Declared so the feature
//!   surface and the CLI's refusals are stable, but the transport itself is the *device* increment
//!   (plan INC5) — it is not in this crate yet, so until it lands every network verb refuses in
//!   words rather than pretending to reach the network.
//!
//! # No clocks, no sockets, no panics in the core
//!
//! Freshness is a pure function of an injected `now_unix` (never `SystemTime::now`, which the
//! workspace forbids on principle and the determinism firewall forbids here). Parsing and normalizing
//! return `Result`/`Option` and refuse in words; the core never panics on external data, because a
//! broker that dies on a malformed feed is a broker that takes the app with it (plan §6.3: "the app
//! never dies because a feed died").

pub mod record;
pub mod registry;
pub mod schema;
pub mod time;
pub mod window;

#[cfg(feature = "streams")]
pub mod cache;
#[cfg(feature = "streams")]
pub mod normalize;
#[cfg(feature = "streams")]
pub mod replay;

pub use record::{DataFlags, DataRecord, DataValue};
pub use registry::{KeyState, Registry, RegistryError, StreamDef};
pub use window::{StreamStatus, Window, WindowKind};

/// The crate version, for the descriptive `User-Agent` every request must carry (§11.2). The live
/// transport (INC5) formats `sparq-observatory/<version> (+https://github.com/bitseq27/sparq; WO-020)`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `User-Agent` string every broker request carries (§11.2: NWS *requires* a descriptive one,
/// and the others ask for attribution). Declared here so the transport and the recorder agree on it.
#[must_use]
pub fn user_agent() -> String {
    format!("sparq-observatory/{VERSION} (+https://github.com/bitseq27/sparq; WO-020)")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_user_agent_is_descriptive_and_carries_the_version() {
        let ua = user_agent();
        assert!(ua.starts_with("sparq-observatory/"), "{ua}");
        assert!(ua.contains(VERSION), "the UA carries the crate version: {ua}");
        assert!(
            ua.contains("github.com/bitseq27/sparq"),
            "the UA points at the repo for attribution: {ua}"
        );
        assert!(ua.contains("WO-020"), "{ua}");
    }

    #[test]
    fn the_core_types_are_re_exported() {
        // A smoke test that the public face resolves: build one record through the re-exports.
        let r =
            DataRecord::new(schema::TEXT_ID, schema::VERSION, vec![DataValue::Text("hi".into())]);
        assert_eq!(r.record_flags, DataFlags::Ok);
        let reg = Registry::load().unwrap();
        assert!(reg.contains("swpc.wwv"));
        let w = Window::for_stream(reg.get("swpc.wwv").unwrap());
        assert_eq!(w.kind(), WindowKind::Text);
        assert_eq!(KeyState::KeyNeeded.as_words(), "KEY NEEDED");
        assert_eq!(StreamStatus::Live.as_words(), "LIVE");
    }
}
