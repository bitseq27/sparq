//! sparq CLI (WO-005).
//!
//! Three commands:
//!
//! * `render` — offline, deterministic render to WAV. Works with **no** audio device, no features
//!   and no third-party crates. This is the CI path and the golden-reference generator.
//! * `selftest` — runs the Phase 0 gates in-process and prints a report (determinism, allocation
//!   discipline, underrun counting, golden hash).
//! * `play` — live output through the **disposable bootstrap** device path (ADR-008). Requires
//!   `--features bootstrap-audio`; refuses to run without it, with a pointer to WO-006.
//!
//! Scope note (WO-005): the work order asked for "a minimal window". The egui shell is WO-012, so
//! Phase 0 uses a terminal control surface for gain and diagnostics. Everything the window would
//! have driven — the control ring, the diagnostics counters, the render path — is real and tested.

// The CLI legitimately reads the wall clock (to report render throughput and self-test timings).
// `clippy::disallowed_methods` denies `Instant::now` workspace-wide because it is forbidden on the
// audio path; the audio path lives in sparq-kernel/sparq-audio, where the denial stays in force.
// The one exception inside the audio crates is the kernel's null-device *test harness*, which is
// allowed at its call site with its own comment.
#![allow(clippy::disallowed_methods, clippy::print_stdout)]

mod bridge;
mod cli;
mod demo;
mod devices;
mod exec;
mod instruments;
mod modules;
mod play;

/// The counting allocator is installed for the whole binary, not just tests.
///
/// This is deliberate and it is the difference between a diagnostic and a decoration: `sparq render`
/// and `sparq soak` print `alloc 0`, and that number is only *true* if something is counting. Cost
/// is one relaxed atomic load per allocation when auditing is disarmed (the normal case), which is
/// why it is acceptable in a release build. If it ever becomes measurable, gate it behind a feature
/// — but do not remove it, because then every allocation claim in the CLI becomes unverifiable.
#[global_allocator]
static ALLOC: sparq_kernel::alloc::CountingAllocator<std::alloc::System> =
    sparq_kernel::alloc::CountingAllocator::new(std::alloc::System);
mod render;
mod selftest;
mod soak;
mod stamp;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("sparq: {e}");
            ExitCode::from(2)
        },
    }
}
