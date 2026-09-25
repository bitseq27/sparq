//! `sparq play` — live output.
//!
//! ADR-008: the live path in Phase 0 is the **disposable bootstrap**. It is compiled only with
//! `--features bootstrap-audio`, it is not part of the default build, and it is scheduled for
//! deletion when the real HAL (WO-006) lands. Without the feature this module compiles to a
//! refusal that explains exactly why and points at the work order.

use std::process::ExitCode;

use sparq_kernel::hal::BackendKind;

use crate::cli::PlayOpts;

pub mod hal;

/// Backend names that route to the real HAL (WO-006) instead of the bootstrap.
///
/// Deliberately disjoint from the cpal host names (`wasapi`, `directsound`, `mme`, ...): a bare
/// `--backend wasapi` keeps meaning the bootstrap path until the HAL is *proven on hardware* —
/// a proven path is never rerouted by a sync (defect #29's lesson: silent behaviour changes on
/// the user's machine are how trust dies). `null` moves to the HAL because the HAL null is
/// strictly more capable (paced pump, diagnostics, capture) and nothing depends on cpal's.
fn hal_backend(name: Option<&str>) -> Option<BackendKind> {
    use BackendKind as K;
    Some(match name?.to_ascii_lowercase().as_str() {
        "null" | "hal-null" => K::Null,
        "wasapi-exclusive" | "wasapi-x" | "exclusive" => K::WasapiExclusive,
        "wasapi-shared" | "wasapi-s" => K::WasapiShared,
        "asio" => K::Asio, // routes to the registry's honest refusal until increment 2
        _ => return None,
    })
}

/// The HAL routes exist in every build (null works everywhere; device backends refuse with
/// build guidance when the feature is off). Only the *rest* of `play` is feature-gated.
pub fn run(o: PlayOpts) -> Result<ExitCode, String> {
    if !o.list_devices {
        if let Some(kind) = hal_backend(o.backend.as_deref()) {
            return hal::run(o, kind);
        }
    }
    run_routed(o)
}

#[cfg(not(feature = "bootstrap-audio"))]
fn run_routed(o: PlayOpts) -> Result<ExitCode, String> {
    // `--list-devices` is handled here rather than in a separate function so the refusal path has a
    // single shape and no cfg-dependent dead code.
    if o.list_devices {
        return Err(String::from(
            "device enumeration needs an audio backend: build with --features bootstrap-audio\n\
             (scripts\\build.bat does this for you) — or use the HAL: `sparq devices`",
        ));
    }
    Err(String::from(
        "this build has no device backend compiled in.\n\
         \n\
         The real HAL (WO-006) is in sparq-kernel; the WASAPI backends need --features hal-wasapi\n\
         on Windows, and the disposable bootstrap (ADR-008) needs --features bootstrap-audio.\n\
         scripts\\build.bat enables both.\n\
         \n\
         To hear sound now:   cargo run -p sparq-app --features hal-wasapi -- play --backend wasapi-exclusive\n\
         To verify without a device:  cargo run -p sparq-app -- play --backend null --seconds 5\n\
         To render offline:   cargo run -p sparq-app -- render --out out.wav",
    ))
}

#[cfg(feature = "bootstrap-audio")]
fn run_routed(o: PlayOpts) -> Result<ExitCode, String> {
    if o.list_devices {
        return bootstrap::list_devices();
    }
    bootstrap::run(o)
}

#[cfg(feature = "bootstrap-audio")]
mod bootstrap;
