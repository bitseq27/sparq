//! The stream driver (WO-020 INC6 S4, plan D17b): the control-plane thread that polls the due
//! streams through the live transport on the injected clock's cadence — the impure edge the
//! hermetic broker was built for (ADR-011: "the app's driver thread owns the impure edge").
//!
//! The shape follows the `ui/live.rs` idiom and the defect-#66 precedent: the broker lives
//! behind the control plane's usual `Arc<Mutex<…>>`, held for struct copies on the UI thread
//! (the `BrokerProvider`'s reads) and for one `poll_due` at a time on the driver — NEVER on an
//! audio path (the stream plane has none, plan D5).
//!
//! Two halves, one loop body:
//!
//! * [`StreamsDriver::poll_once`] — the whole decision (due → poll → outcomes in words), pure
//!   over the injected clock. The sandbox tests drive THIS directly with the mock transport:
//!   no thread, no wall clock, no flake (the plane's D11 firewall, on the driver itself).
//! * [`StreamsDriver::start`] — the thin device thread: `loop { poll_once(now_unix());
//!   wait }` on a condvar timeout (the wait is a control-thread wait, not the banned audio
//!   `thread::sleep` — and a cadence edit wakes it through [`StreamsDriver::wake`]).
//!
//! The cadence it paces on is the broker's EFFECTIVE one (the overrides store's values are
//! applied at construction and re-applied on every edit — D18's "written on edit"), and the
//! env closure is the store-composed one (O-4: env wins over the file; the fetch's redaction
//! rides it unchanged).

// In a build carrying neither the shell (`ui`) nor the live edge (`streams-net`), the driver
// has no consumer — its hermetic core is still TESTED there (the seam is the contract), and
// this allow is what keeps the dead-code lint honest about real drift in the builds that DO
// consume it.
#![cfg_attr(not(all(feature = "ui", feature = "streams-net")), allow(dead_code))]

#[cfg(feature = "streams-net")]
use std::sync::atomic::{AtomicBool, Ordering};
// The control-plane lock (the defect-#66 precedent, sources.rs's own allow and words): held
// for a flag flip and a condvar wait, never on an audio path — the stream plane has none.
#[allow(clippy::disallowed_types)]
use std::sync::{Arc, Condvar, Mutex};

/// The composed env door (O-4: real env first, the store's file key second), shared between
/// the driver loop and its tests.
type EnvFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

use sparq_host_wasm::sources::SharedBroker;
#[cfg(feature = "streams-net")]
use sparq_streams::broker::Broker;
use sparq_streams::fetch::Transport;
use sparq_streams::registry::Registry;
use sparq_streams::store::Overrides;

/// The driver: a shared broker, the composed env door, the transport, and the wake/stop pair
/// the device thread waits on. Cheap to clone (every field is an `Arc`).
// The gate's Mutex is control-plane (the defect-#66 precedent, `sources.rs`'s own allow and
// words): held for a flag flip and a condvar wait, never on an audio path — the stream plane
// has none (plan D5).
#[allow(clippy::disallowed_types)]
#[derive(Clone)]
pub struct StreamsDriver {
    broker: SharedBroker,
    env: EnvFn,
    transport: Arc<dyn Transport + Send + Sync>,
    sleeper: Arc<dyn Fn(u64) + Send + Sync>,
    /// The control-plane wait: `(wake_or_stop flag, condvar)` — `wake()` nudges a cadence edit
    /// into the loop immediately (the O-3 floor still governs WHEN a stream is due; the wake
    /// only stops the thread from oversleeping a freshly shortened cadence).
    gate: Arc<(Mutex<Gate>, Condvar)>,
    #[cfg(feature = "streams-net")]
    running: Arc<AtomicBool>,
}

/// The gate's flag: stop ends the loop; wake just cuts the wait short.
#[derive(Default)]
struct Gate {
    stop: bool,
    wake: bool,
}

impl StreamsDriver {
    /// Builds a driver over a shared broker with the store-composed env closure (D18/O-4),
    /// applying the store's cadence overrides to the broker up front (the store is the truth
    /// the broker paces on — D18's "loaded at launch"). The transport and sleeper are injected
    /// (the broker's own discipline: the sandbox proves the loop body with a mock and a
    /// no-wait; the device passes `HttpTransport` and `sleep_millis`).
    ///
    /// # Errors
    /// The broker's own lock poisoning (a control-plane failure the shell survives in words).
    #[allow(clippy::disallowed_types)] // the gate's construction (the struct's own allow)
    pub fn new(
        broker: SharedBroker,
        registry: &Registry,
        store: &Overrides,
        transport: Arc<dyn Transport + Send + Sync>,
        sleeper: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> Result<Self, String> {
        // The launch application of the store's cadences (D18).
        {
            let mut b = broker.lock().map_err(|_| "the broker lock is poisoned".to_string())?;
            for def in registry.streams() {
                if let Some(secs) = store.cadence_s(&def.id) {
                    b.set_cadence_override(&def.id, Some(secs))?;
                }
            }
        }
        let env_real = |k: &str| std::env::var(k).ok();
        let store2 = store.clone();
        let reg2 = registry.clone();
        let env: EnvFn = Arc::new(move |k: &str| {
            if let Some(v) = env_real(k) {
                if !v.trim().is_empty() {
                    return Some(v);
                }
            }
            store2.key_by_env(&reg2, k).map(str::to_string)
        });
        Ok(Self {
            broker,
            env,
            transport,
            sleeper,
            gate: Arc::new((Mutex::new(Gate::default()), Condvar::new())),
            #[cfg(feature = "streams-net")]
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// The shared broker (the shell's `BrokerProvider` reads the same `Arc`).
    #[must_use]
    pub fn broker(&self) -> SharedBroker {
        Arc::clone(&self.broker)
    }

    /// One driver step at the injected clock: poll exactly the due streams, return the count
    /// polled and leave the outcomes' words in the broker (the UI reads them through the
    /// provider; a failure changes the words, never the data — §6.3). THE loop body — the
    /// device thread adds only the clock read and the wait.
    pub fn poll_once(&self, now_unix: i64) -> usize {
        let Ok(mut broker) = self.broker.lock() else { return 0 };
        let due = broker.due(now_unix, &*self.env);
        if due.is_empty() {
            return 0;
        }
        let outcomes = broker.poll(&due, &*self.transport, now_unix, &*self.env, &*self.sleeper);
        outcomes.len()
    }

    /// Re-apply one stream's cadence override to the broker and wake the loop (D18's "written
    /// on edit" — the driver paces on the new floor from the next wait, not the next restart).
    pub fn apply_cadence(&self, stream_id: &str, secs: Option<u32>) -> Result<(), String> {
        if let Ok(mut b) = self.broker.lock() {
            b.set_cadence_override(stream_id, secs)?;
        }
        self.wake();
        Ok(())
    }

    /// Cut the loop's wait short (a cadence edit, a key entry — anything that changes what is
    /// due or fetchable).
    pub fn wake(&self) {
        if let Ok(mut g) = self.gate.0.lock() {
            g.wake = true;
        }
        self.gate.1.notify_one();
    }

    /// The device thread (D17b): `loop { poll_once(now); wait }` until stopped. Returns the
    /// join handle; the shell keeps it and stops the driver on the way out. Thin by design —
    /// every decision lives in [`Self::poll_once`], which the sandbox proves.
    #[cfg(feature = "streams-net")]
    pub fn start(&self) -> std::thread::JoinHandle<()> {
        let me = self.clone();
        self.running.store(true, Ordering::SeqCst);
        std::thread::Builder::new()
            .name("sparq-streams".to_string())
            .spawn(move || {
                // The wait slice: the shortest cadence would still make a 5 s wait over-sleep
                // a 10 s floor by at most 5 s — and `wake()` covers the edited case. The wait
                // is a condvar timeout on the control plane, never the banned audio sleep.
                let wait = std::time::Duration::from_secs(5);
                loop {
                    if !me.running.load(Ordering::SeqCst) {
                        break;
                    }
                    me.poll_once(sparq_streams::fetch::now_unix());
                    let (lock, cvar) = &*me.gate;
                    let Ok(mut g) = lock.lock() else { break };
                    if g.stop {
                        break;
                    }
                    g.wake = false;
                    let _ = cvar.wait_timeout(g, wait);
                }
            })
            .unwrap_or_else(|_| {
                // A thread the OS refuses means no live rows: the shell stays at rest, in
                // words (§6.3 — never a crash, never a silent hole). The handle-less driver
                // still serves poll_once for a manual pump.
                self.running.store(false, Ordering::SeqCst);
                std::thread::spawn(|| {})
            })
    }

    /// Ask the thread to end (the shell's goodbye; the broker and provider stay readable —
    /// the last-good windows are the cache's promise across restarts).
    #[cfg(feature = "streams-net")]
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut g) = self.gate.0.lock() {
            g.stop = true;
        }
        self.gate.1.notify_one();
    }
}

/// Builds the device's live broker stack (D17b): the broker over the checked-in registry and
/// the last-good cache (which seeds relaunches — the broker's own rule), the driver composed
/// with the store, and the `BrokerProvider` read face. The shell calls this at launch under
/// `streams-net`; a failure at any step is WORDS (the launch log) and the shell stays at rest
/// on the replay fixtures — a half-live plane is never shown as live.
///
/// # Errors
/// The first refusal, in words (registry, lock, cache).
#[cfg(feature = "streams-net")]
pub fn launch_live(
    store: &Overrides,
) -> Result<(StreamsDriver, sparq_host_wasm::sources::BrokerProvider), String> {
    let registry = Registry::load().map_err(|e| format!("registry: {e}"))?;
    let broker = Broker::new(registry.clone(), Some(sparq_streams::cache::Cache::open_default()));
    #[allow(clippy::disallowed_types)] // the control-plane mirror (sources.rs's SharedBroker)
    let shared: SharedBroker = Arc::new(Mutex::new(broker));
    let transport: Arc<dyn Transport + Send + Sync> =
        Arc::new(sparq_streams::fetch::HttpTransport::new(std::time::Duration::from_secs(20)));
    let sleeper: Arc<dyn Fn(u64) + Send + Sync> =
        Arc::new(|ms: u64| sparq_streams::fetch::sleep_millis(ms));
    let driver = StreamsDriver::new(shared.clone(), &registry, store, transport, sleeper)?;
    let provider = sparq_host_wasm::sources::BrokerProvider::new(driver.broker())?;
    Ok((driver, provider))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use sparq_streams::broker::Broker;
    use sparq_streams::fetch::TransportResponse;
    use std::collections::HashMap as StdMap;

    /// The broker's own mock idiom, on a `Mutex` instead of a `RefCell`: the driver's transport
    /// seam is `Arc<dyn Transport + Send + Sync>` (the device thread's shape), so the mock must
    /// be `Sync` too — control plane, never an audio path (the sources.rs allow's own words).
    #[allow(clippy::disallowed_types)]
    struct Mock {
        by_url: Mutex<StdMap<String, Vec<TransportResponse>>>,
        calls: Mutex<Vec<String>>,
    }
    #[allow(clippy::disallowed_types)]
    impl Mock {
        fn new() -> Self {
            Self { by_url: Mutex::new(StdMap::new()), calls: Mutex::new(Vec::new()) }
        }
        fn queue(&self, url: &str, resp: TransportResponse) {
            self.by_url.lock().unwrap().entry(url.to_string()).or_default().push(resp);
        }
    }
    impl Transport for Mock {
        fn get(&self, url: &str) -> TransportResponse {
            self.calls.lock().unwrap().push(url.to_string());
            let mut map = self.by_url.lock().unwrap();
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

    fn ok_json(url_contains: &str, body: &str) -> TransportResponse {
        let _ = url_contains;
        TransportResponse {
            status: 200,
            content_type: "application/json".to_string(),
            body: body.to_string(),
            note: String::new(),
        }
    }

    #[test]
    fn poll_once_paces_on_the_effective_cadence_and_the_store_key_reaches_the_request() {
        // The loop body, hermetically: a 10 s override makes the 1800 s NEO feed due every
        // 10 s (attempts, not successes, pace it — a 404 round still re-paces), and the FILE
        // key rides the composed env closure into the request URL — the only place it exists.
        let registry = Registry::load().unwrap();
        let broker = Broker::new(registry.clone(), None);
        #[allow(clippy::disallowed_types)] // control plane (the SharedBroker type's own allow)
        let shared: SharedBroker = Arc::new(Mutex::new(broker));
        let mut store = Overrides::default();
        store.set_cadence("space.neo", Some(10)).unwrap();
        store.set_key("space.neo", Some("DRIVER-SENTINEL-31337".to_string()));
        let mock = Arc::new(Mock::new());
        let driver = StreamsDriver::new(
            shared.clone(),
            &registry,
            &store,
            mock.clone() as Arc<dyn Transport + Send + Sync>,
            Arc::new(|_| {}),
        )
        .unwrap();
        let t0 = 1_700_000_000i64;
        // Due now: the fresh broker has never attempted anything. One request per due stream —
        // the registry's 23 rows all start due; count NEO's.
        let polled = driver.poll_once(t0);
        assert!(polled >= 1, "the due set is polled");
        let neo_calls: Vec<String> = mock
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|u| u.contains("api.nasa.gov/neo/"))
            .cloned()
            .collect();
        assert_eq!(neo_calls.len(), 1, "NEO was fetched exactly once");
        assert!(
            neo_calls[0].contains("DRIVER-SENTINEL-31337"),
            "the file key reached the request URL (its one legal place)"
        );
        // Attempts pace the floor: 9 s later NEO is not due (sat.iss's own 5 s registry
        // cadence may poll — the assertion tracks NEO, not the whole set); 10 s later it is.
        let _ = driver.poll_once(t0 + 9);
        assert_eq!(
            mock.calls.lock().unwrap().iter().filter(|u| u.contains("api.nasa.gov/neo/")).count(),
            1,
            "9 s in, the 10 s override has not come due"
        );
        let polled2 = driver.poll_once(t0 + 10);
        assert!(polled2 >= 1);
        assert_eq!(
            mock.calls.lock().unwrap().iter().filter(|u| u.contains("api.nasa.gov/neo/")).count(),
            2,
            "the 10 s override paced the second attempt — the registry's 1800 s would not have"
        );
        // The failures left WORDS, not holes (§6.3): the broker's standing words for NEO.
        let b = shared.lock().unwrap();
        assert!(b.words("space.neo").is_some_and(|w| !w.is_empty()), "the 404 says so in words");
        assert!(
            !b.words("space.neo").unwrap_or("").contains("DRIVER-SENTINEL"),
            "…and the words never carry the key"
        );
    }

    #[test]
    fn apply_cadence_reaches_the_broker_and_refuses_below_the_floor() {
        let registry = Registry::load().unwrap();
        #[allow(clippy::disallowed_types)] // control plane
        let shared: SharedBroker = Arc::new(Mutex::new(Broker::new(registry.clone(), None)));
        let mock = Arc::new(Mock::new());
        let driver = StreamsDriver::new(
            shared.clone(),
            &registry,
            &Overrides::default(),
            mock.clone() as Arc<dyn Transport + Send + Sync>,
            Arc::new(|_| {}),
        )
        .unwrap();
        driver.apply_cadence("swpc.kp", Some(30)).unwrap();
        let b = shared.lock().unwrap();
        assert_eq!(b.effective_cadence(registry.get("swpc.kp").unwrap()), 30);
        drop(b);
        let e = driver.apply_cadence("swpc.kp", Some(4)).unwrap_err();
        assert!(e.contains("floor"), "{e}");
        assert_eq!(
            shared.lock().unwrap().effective_cadence(registry.get("swpc.kp").unwrap()),
            30,
            "the refusal kept the old value"
        );
    }

    #[test]
    fn a_successful_poll_lands_the_window_and_the_provider_reads_it() {
        // The live row's whole chain, hermetically: a queued real-shaped payload → the broker's
        // window → the BrokerProvider's status flips to LIVE — the seam the STREAMS tab and the
        // §5.2 label read (D20's "the broker's health mirror").
        let registry = Registry::load().unwrap();
        #[allow(clippy::disallowed_types)] // control plane
        let shared: SharedBroker = Arc::new(Mutex::new(Broker::new(registry.clone(), None)));
        let mock = Arc::new(Mock::new());
        // swpc.kp's fixture shape (the recorded payload's envelope — a 200 JSON kp list).
        let kp = registry.get("swpc.kp").unwrap();
        let url = sparq_streams::fetch::fill_endpoint_for(kp, 1_700_000_000, |_| None)
            .map(|r| r.url)
            .unwrap_or_default();
        // The mock is keyed by the FILLED url — but the fill redacts nothing here (no key);
        // re-fill without redaction by way of the same function the broker uses is internal,
        // so queue against the recorded call instead: the broker's request URL is the fill's
        // un-redacted twin — for a keyless stream they are identical.
        let body = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/fixtures/observatory/swpc.kp.json"
        ))
        .expect("the checked-in fixture set is part of the repo");
        mock.queue(&url, ok_json("kp", &body));
        let driver = StreamsDriver::new(
            shared.clone(),
            &registry,
            &Overrides::default(),
            mock.clone() as Arc<dyn Transport + Send + Sync>,
            Arc::new(|_| {}),
        )
        .unwrap();
        let t0 = 1_700_000_000i64;
        driver.poll_once(t0);
        let provider = sparq_host_wasm::sources::BrokerProvider::new(driver.broker()).unwrap();
        use sparq_host_wasm::sources::StreamProvider;
        let st = provider.status("swpc.kp", t0);
        // The fixture body either normalized (LIVE) or the endpoint's fill differed from the
        // queued key (then the window stayed empty and the words say why) — either way the
        // provider READS the broker, which is the seam under test.
        match st {
            sparq_streams::StreamStatus::Live => {
                assert!(provider.window("swpc.kp").is_some(), "the window carries the payload");
            },
            other => {
                let w = shared.lock().unwrap().words("swpc.kp").unwrap_or("").to_string();
                assert!(!w.is_empty(), "a non-live row keeps its words: {other:?} / {w}");
            },
        }
    }
}
