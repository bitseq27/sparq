//! `sparq devices` — HAL enumeration, capabilities and conformance (WO-006).
//!
//! The first thing to run on a new machine and the first thing to paste back when nothing works
//! (same job `play --list-devices` did for the bootstrap, but through the real HAL trait): every
//! backend compiled into this binary, every device each backend sees, what each device *verifiably*
//! supports (probed, never guessed), and — with `--conformance` — whether each backend keeps the
//! behavioural promises the layers above rely on.
//!
//! ASIO is deliberately visible-by-absence: until increment 2 lands, requesting it prints the
//! honest refusal from the registry rather than an empty list that looks like "no ASIO drivers".

use std::process::ExitCode;

use sparq_kernel::hal::conformance::{
    conformance_offline, format_report, reopen_cycles_do_not_leak,
};
use sparq_kernel::hal::{available_backends, backend_by_name, BackendKind};

#[derive(Default)]
pub struct DevicesOpts {
    /// Restrict to one backend by CLI name (`null`, `wasapi-exclusive`, `wasapi-shared`, `asio`).
    pub backend: Option<String>,
    /// Print probed capabilities per device (rates, channels, exclusive, periods).
    pub caps: bool,
    /// Run the offline conformance suite against each backend and print the gate table.
    /// On Windows with `hal-wasapi` this briefly opens the default device (paced fallback):
    /// the conformance signal is deliberately near-silent (-60 dB) because it may reach speakers.
    pub conformance: bool,
}

pub fn run(o: DevicesOpts) -> Result<ExitCode, String> {
    let backends: Vec<Box<dyn sparq_kernel::hal::HalBackend>> = match &o.backend {
        Some(name) => vec![backend_by_name(name).map_err(|e| e.with_hint())?],
        None => available_backends(),
    };

    println!(
        "sparq devices — HAL backends in this build: {}",
        sparq_kernel::hal::compiled_backends()
            .iter()
            .map(|b| b.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !sparq_kernel::hal::compiled_backends().contains(&BackendKind::Asio) {
        println!("  (asio: not compiled in — WO-006 increment 2; requesting it will say so, not fake it)");
    }
    println!();

    let mut failures = 0usize;
    for backend in &backends {
        println!("[{}] {}", backend.kind(), backend.display_name());
        match backend.enumerate() {
            Ok(devices) => {
                if devices.is_empty() {
                    println!("  (no devices)");
                    if backend.kind() == BackendKind::WasapiShared
                        || backend.kind() == BackendKind::WasapiExclusive
                    {
                        println!(
                            "  -> no render endpoints at all is unusual on Windows: check the"
                        );
                        println!("     Windows Audio service, or whether this is a session that");
                        println!("     hides the hardware (see WINDOWS.md on RDP).");
                    }
                }
                for (i, d) in devices.iter().enumerate() {
                    let marker = if d.is_default_output { "  <- default" } else { "" };
                    println!("  out [{i}] {}{}", d.name, marker);
                    if let Some(ep) = &d.id.endpoint {
                        println!("        id: {ep}");
                    }
                    if o.caps {
                        match backend.capabilities(&d.id) {
                            Ok(c) => {
                                for line in c.summary().lines() {
                                    println!("        {line}");
                                }
                            },
                            Err(e) => println!("        capabilities FAILED: {}", e.with_hint()),
                        }
                    }
                }
            },
            Err(e) => {
                println!("  enumeration FAILED: {}", e.with_hint());
                failures += 1;
            },
        }
        println!();
    }

    if o.conformance {
        println!("conformance (offline suite; device backends use a short paced fallback run)");
        println!("  NOTE: the paced fallback briefly opens the default device with a -60 dB ramp.");
        println!();
        for backend in &backends {
            // Device backends have no manual mode: the suite falls back to a short paced run,
            // clearly labelled in its own output. Null runs the full deterministic suite.
            let paced_fallback = backend.kind() != BackendKind::Null;
            let report = conformance_offline(backend.as_ref(), paced_fallback);
            print!("{}", format_report(backend.kind(), &report));
            failures += report.iter().filter(|c| !c.passed).count();
            let leak = reopen_cycles_do_not_leak(backend.as_ref(), 8);
            println!(
                "  [{}] {} — {}",
                if leak.passed { "PASS" } else { "FAIL" },
                leak.name,
                leak.detail
            );
            failures += usize::from(!leak.passed);
            println!();
        }
    }

    if failures > 0 {
        Err(format!("{failures} failure(s) — the output above says which"))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
