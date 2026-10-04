//! `sparq-module-guest` — the guest SDK for sparq **instrument** components (T2 tier).
//!
//! **Status: v1.1 — the contract is FROZEN (WO-017 close, 2026-10-05).** This crate is the
//! friendly face of the WIT contract `sparq:instrument@1.1.0`: it embeds the frozen contract
//! snapshot at compile time (`wit_bindgen::generate!` over the VENDORED `wit/` beside this
//! crate), re-exports every contract type under flat names, and provides the
//! [`InstrumentModule`] trait + [`sparq_instrument!`] macro an author implements against.
//! It is published separately from sparq and the core workspace does not depend on it (ADR-010);
//! it lives under `tools/` on the `dispatch-bench` precedent — its own workspace, excluded from
//! the root.
//!
//! **The crate version CARRIES THE CONTRACT VERSION** (freeze packaging rule): `1.1.x` speaks
//! `sparq:instrument@1.1.*`; SDK-only fixes move the patch, a contract minor moves the minor.
//!
//! # What v1.1 is
//!
//! * The WIT binding, embedded from the vendored snapshot (`./wit`) of the frozen contract —
//!   self-contained for publishing, kept honest in a checkout by
//!   `the_vendored_wit_is_the_repo_contract_byte_for_byte` (below), with the repo copy itself
//!   hash-pinned by the root workspace (`crates/sparq-module-api/tests/wit_snapshot.rs`).
//! * [`InstrumentModule`]: the trait, shaped like the native `Module` trait of
//!   `docs/module-author-guide-v0.md` §3 (required: `id`/`prepare`/`process`; defaulted:
//!   `activate`/`deactivate`/`configure`/`save_state`/`message`/`draw`), because the two-hour
//!   test applies to instruments too.
//! * [`sparq_instrument!`]: one macro call turns your type into a component. The instance lives
//!   in a lazily-initialised `thread_local` of YOUR concrete type — no `dyn`, no constructor
//!   registration, no wasm init-order folklore. A component instance is single-threaded, so
//!   `thread_local` is the whole story.
//!
//! # What v1.1 is NOT (honest scope, post-freeze)
//!
//! * No display-list builder ergonomics beyond type aliases — those are designed AFTER the
//!   vocabulary froze, i.e. now-ish, shaped by WO-019's reference instruments; they will be
//!   SDK additions (minor bumps), never contract changes.
//! * No packaging tooling (`sparq mod package`/`validate`/`dev` are host-side, WO-018).
//! * `unsafe`: forbidden in this crate. The generated bindings are safe Rust by construction;
//!   if a future wit-bindgen emits `unsafe`, that gets a reviewed, scoped `allow` with a
//!   SAFETY comment — not a blanket lift.
//!
//! # Author shape (the whole program)
//!
//! ```ignore
//! use sparq_module_guest as sparq;
//!
//! #[derive(Default)]
//! struct MyInstrument { /* ... */ }
//!
//! impl sparq::InstrumentModule for MyInstrument { /* id, prepare, process, ... */ }
//!
//! sparq::sparq_instrument!(MyInstrument);
//! ```
//!
//! Build it as a component (`cargo component build --release`, or plain
//! `cargo build --target wasm32-wasip1` + `wasm-tools component new`), package it into the
//! five-file layout (`MODULE-BUILD-GUIDE.md` §3), drop it in `instruments/`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// The contract, embedded at compile time from the VENDORED snapshot (`./wit`) of the frozen
// v1.1 files. Vendoring is the freeze's packaging step (WIT README §5): a published crate must
// be self-contained. The copy stays honest by test — `the_vendored_wit_is_the_repo_contract_
// byte_for_byte` below fails on any difference against `docs/api/instrument-wit/wit/`, and the
// root workspace pins those bytes by hash (`crates/sparq-module-api/tests/wit_snapshot.rs`).
// A deliberate contract amendment therefore moves the docs, the root pin AND this vendor in one
// reviewable commit; an accidental edit fails one of the two gates.
pub mod bindings {
    #![allow(missing_docs)] // generated code is documented by the WIT comments it was made from
    wit_bindgen::generate!({
        world: "instrument",
        path: "wit",
        // Cross-crate component export: the generated macro is `pub`, uniquely named (no
        // generic `export!` colliding in author crates), and hardwired to THIS crate's
        // bindings module so it resolves from any consumer (wit-bindgen's own options for
        // exactly this case).
        pub_export_macro: true,
        export_macro_name: "sparq_export_component",
        default_bindings_module: "::sparq_module_guest::bindings",
    });
}

/// Re-exported so author crates never need a direct `wit-bindgen` dependency (version-matched
/// by construction).
pub use wit_bindgen;

// ─────────────────────────── contract re-exports (flat names) ───────────────────────────

pub use bindings::sparq::instrument::types::{
    BlockStatus, DataFlags, DataRecord, DataValue, Event, EventKind, LogLevel, ModuleError,
    Oversampling, ParamSet, Resources, Semver,
};
pub use bindings::sparq::instrument::audio::{AudioBuf, BlockInput, BlockOutput, CvInBuf, CvOutBuf};
pub use bindings::sparq::instrument::display::{
    ArcItem, Camera, Corner, Dash, Item, FrameContext, GlyphRunItem, HeatCellsItem,
    Lod, Material, PathItem, PathSegment, Point, PointsItem, PolylineItem, RectItem, Scene,
    SceneElement, SceneHeightfield, SceneLines, SceneMesh, ScenePoints, Shading, StrokeWidth,
    Style, Surface, TraceItem,
};
pub use bindings::sparq::instrument::host::TimeInfo;
pub use bindings::sparq::instrument::sources::StreamSnapshot;

/// A display list: the 2D surface vocabulary (ADR-010 decision 3). Append [`Item`]s; the host
/// renders them with the current tokens. Never pixels.
pub type DisplayList = Vec<Item>;

// ─────────────────────────── the author-facing trait ───────────────────────────

/// What an instrument is, in Rust. Mirrors the native `Module` trait's discipline
/// (module-author-guide §3) across the sandbox boundary; the WIT contract
/// (`docs/api/instrument-wit/`) is the normative text, this trait is its face.
///
/// Threading: `process` runs on the audio instance (real-time — no growth, no I/O, no host
/// calls; see `audio.wit`), `draw` on the display instance (UI thread), everything else on the
/// control thread. The host manages both instances of your one component (README decision D);
/// your type only ever sees `&mut self`, one call at a time.
pub trait InstrumentModule: Default {
    /// The module id — MUST equal `identity.id` in your manifest. The host cross-checks at
    /// registration and refuses on disagreement (a module that lies about who it is breaks
    /// patch provenance silently).
    fn id(&self) -> String;

    /// All allocation happens here. Called again whenever any [`Resources`] field changes —
    /// write it idempotent, release what you previously took.
    fn prepare(&mut self, resources: Resources) -> Result<(), ModuleError>;

    /// One block. Real-time rules in force; fuel-metered (blowing `resources.fuel_per_block`
    /// traps and the host records [`BlockStatus::Overrun`] — auto-bypass ladder, ADR-009 d6).
    /// Output shapes must match the negotiated contract exactly (`audio.wit`).
    fn process(&mut self, input: BlockInput) -> BlockOutput;

    /// Entering the live state (default: nothing to do).
    fn activate(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Leaving the live state (default: nothing to do).
    fn deactivate(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Receive a state blob at your declared schema version (the host runs the manifest's
    /// migration chain BEFORE this call). Default: honest refusal — refusing with words is
    /// valid; silently ignoring is the bug.
    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            Ok(())
        } else {
            Err(ModuleError::State("this instrument keeps no state".into()))
        }
    }

    /// Emit state: deterministic bytes (two calls with unchanged state produce identical
    /// output — the journal hashes it). Default: the empty blob, honestly empty.
    fn save_state(&mut self) -> Vec<u8> {
        Vec::new()
    }

    /// An `atom` delivery. Default: honest refusal (never a silent no-op).
    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("this instrument takes no messages".into()))
    }

    /// One frame of one declared display: emit STRUCTURE ([`Surface`]), never appearance —
    /// every style is a semantic token id resolved by the host against the CURRENT token
    /// bundle, which is what makes app design changes re-theme you with zero intervention.
    /// Default: the empty list (an at-rest display shows words or nothing, never garbage).
    fn draw(&mut self, _frame: FrameContext) -> Surface {
        Surface::Items(Vec::new())
    }
}

// ─────────────────────────── helpers ───────────────────────────

/// Parameter `index` from a snapshot, native `ParamSet::at` semantics: 0.0 past the declared
/// count (the neutral value for every modulation target in the closed unit vocabulary — the
/// audio path pays no presence branch).
#[must_use]
pub fn param(set: &ParamSet, index: usize) -> f32 {
    set.values.get(index).copied().unwrap_or(0.0)
}

/// Take a deterministic PRNG root for a named stream from the host seed tree (ADR-007).
/// CONTROL-THREAD ONLY — call it in `prepare`/`configure`, never in `process`; then run your
/// own deterministic PRNG from the root so golden renders reproduce bit-exactly.
#[must_use]
pub fn seed(stream_name: &str) -> u64 {
    bindings::sparq::instrument::host::random_seed(stream_name)
}

/// A diagnostic to the operator (intent log + validator report). CONTROL-THREAD ONLY: calls
/// from `process`/`draw` work but are reported as RT violations at hand-in.
pub fn log(level: LogLevel, message: &str) {
    bindings::sparq::instrument::host::log(level, message);
}

/// The token bundle in force (version + serialised payload — JSON of `tokens.json` at v1.1).
/// CONTROL-THREAD ONLY: fetch in `prepare`, cache in your state, use it for LAYOUT maths;
/// for appearance emit token ids and the host resolves them.
#[must_use]
pub fn token_bundle() -> bindings::sparq::instrument::tokens::Bundle {
    bindings::sparq::instrument::tokens::get_bundle()
}

/// A bound stream snapshot for a display, at draw time (`sources` import). `None` = draw the
/// at-rest state.
#[must_use]
pub fn source(display_id: &str, source_id: &str) -> Option<StreamSnapshot> {
    bindings::sparq::instrument::sources::snapshot(display_id, source_id)
}

/// Open a capability-scoped asset by its manifest hash (`blake3:<hex>`). CONTROL-THREAD ONLY —
/// read and cache in `prepare`; there is no ambient filesystem in the sandbox.
pub fn open_asset(
    hash: &str,
) -> Result<bindings::sparq::instrument::assets::Asset, bindings::sparq::instrument::assets::AssetError> {
    bindings::sparq::instrument::assets::open(hash)
}

// ─────────────────────────── the component macro ───────────────────────────

/// Turn your [`InstrumentModule`] type into a sparq instrument component. One call, crate root:
///
/// ```ignore
/// sparq_module_guest::sparq_instrument!(MyInstrument);
/// ```
///
/// What it expands to: a `thread_local` holding `Option<YourType>` (lazily built via
/// [`Default`]), and the generated `Guest` implementation delegating every WIT export to it —
/// your concrete type, no dynamic dispatch at the boundary, no constructor-registration or
/// wasm init-order folklore. The instance lives for the component instance's lifetime, which
/// is exactly the module instance's lifetime (the host owns hot reload: a new component
/// instance is a new `Default`, state crossing via `configure`/`save_state`).
#[macro_export]
macro_rules! sparq_instrument {
    ($ty:ty) => {
        ::std::thread_local! {
            static __SPARQ_INSTANCE: ::std::cell::RefCell<::core::option::Option<$ty>> =
                const { ::std::cell::RefCell::new(::core::option::Option::None) };
        }

        #[allow(dead_code)]
        fn __sparq_with<T>(f: impl FnOnce(&mut $ty) -> T) -> T {
            __SPARQ_INSTANCE.with(|cell| {
                let mut borrowed = cell.borrow_mut();
                let module = borrowed.get_or_insert_with(<$ty>::default);
                f(module)
            })
        }

        struct __SparqComponent;

        impl $crate::bindings::exports::sparq::instrument::guest::Guest for __SparqComponent {
            fn id() -> ::std::string::String {
                __sparq_with(|m| $crate::InstrumentModule::id(m))
            }
            fn prepare(res: $crate::Resources) -> ::core::result::Result<(), $crate::ModuleError> {
                __sparq_with(|m| $crate::InstrumentModule::prepare(m, res))
            }
            fn activate() -> ::core::result::Result<(), $crate::ModuleError> {
                __sparq_with(|m| $crate::InstrumentModule::activate(m))
            }
            fn deactivate() -> ::core::result::Result<(), $crate::ModuleError> {
                __sparq_with(|m| $crate::InstrumentModule::deactivate(m))
            }
            fn configure(state: ::std::vec::Vec<u8>) -> ::core::result::Result<(), $crate::ModuleError> {
                __sparq_with(|m| $crate::InstrumentModule::configure(m, &state))
            }
            fn save_state() -> ::std::vec::Vec<u8> {
                __sparq_with(|m| $crate::InstrumentModule::save_state(m))
            }
            fn message(payload: ::std::vec::Vec<u8>) -> ::core::result::Result<(), $crate::ModuleError> {
                __sparq_with(|m| $crate::InstrumentModule::message(m, &payload))
            }
            fn process(input: $crate::BlockInput) -> $crate::BlockOutput {
                __sparq_with(|m| $crate::InstrumentModule::process(m, input))
            }
            fn draw(frame: $crate::FrameContext) -> $crate::Surface {
                __sparq_with(|m| $crate::InstrumentModule::draw(m, frame))
            }
        }

        // Component exports only exist on the wasm face: native builds are for the logic
        // tests (the dev-harness split — `cargo test` natively, `cargo build --target
        // wasm32-wasip1` for the component). The canonical ABI symbol names contain `:`/`#`/`@`,
        // which native linkers reject in version scripts, and the wit-bindgen metadata section
        // is itself wasm-gated — so this gate mirrors the generator's own.
        #[cfg(target_arch = "wasm32")]
        $crate::bindings::sparq_export_component!(__SparqComponent);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_past_the_end_is_the_neutral_zero() {
        let set = ParamSet { version: 7, values: vec![0.5, -1.0] };
        assert_eq!(param(&set, 0), 0.5);
        assert_eq!(param(&set, 1), -1.0);
        assert_eq!(param(&set, 2), 0.0, "0.0 past the end — native ParamSet::at semantics");
    }

    #[test]
    fn module_error_carries_words() {
        // The refusal style the contract demands: every failure says what and why.
        let e = ModuleError::Unsupported("no such query".into());
        assert!(matches!(e, ModuleError::Unsupported(ref w) if w == "no such query"));
    }

    #[test]
    fn the_vendored_wit_is_the_repo_contract_byte_for_byte() {
        // The freeze's packaging step (WIT README §5): the published crate is self-contained,
        // and this test is what keeps the vendored snapshot honest INSIDE a checkout. The repo
        // copy is itself hash-pinned by the root workspace (wit_snapshot.rs), so a deliberate
        // contract amendment moves the docs, the root pin and this vendor in one commit — and
        // an accidental edit fails one of the two gates.
        let vendored = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("wit");
        let canonical =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/api/instrument-wit/wit");
        if !canonical.is_dir() {
            // Outside a sparq checkout (e.g. a published crate on its own): nothing to compare
            // against, and that is exactly what vendoring is for. Say so, pass, move on.
            println!("note: repo contract directory absent — vendored copy stands alone");
            return;
        }
        let list = |dir: &std::path::Path| {
            let mut v: Vec<String> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
                .map(|x| x.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            list(&vendored),
            list(&canonical),
            "the vendored file set moved apart from the contract — re-copy the whole directory"
        );
        for name in list(&canonical) {
            let a = std::fs::read_to_string(vendored.join(&name)).unwrap();
            let b = std::fs::read_to_string(canonical.join(&name)).unwrap();
            assert_eq!(
                a, b,
                "{name}: the vendored snapshot drifted from docs/api/instrument-wit/wit/ — \
                 re-copy it in the same commit as the contract amendment (and repin the root \
                 workspace's wit_snapshot.rs in that same commit)"
            );
        }
    }
}
