//! The hand-in gate's runtime stages (guide §7 stages 3/4/6 + stage 5's measurement half) —
//! instrument-host.md §7's test plan as gate code.
//!
//! These run ONLY with the `instrument-host` feature AND a [`RuntimeContext`] (the caller names
//! the fixture set, the cache dir and the at-rest door — the stage never guesses a path). Without
//! them the stages refuse in words exactly as before: a partial gate is not a hand-in.
//!
//! The division of labour with the tests is deliberate: a STAGE measures and reports (the golden
//! hash is recorded in words, twice-run determinism is asserted); the cross-boundary PIN (the
//! wasm path's IR must equal the harness's golden sha) lives in `tests/instrument_runtime.rs` —
//! the stage tells the operator the truth about THIS package, the test guards the tree's anchor.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sparq_module_api::error::{CodeKind, ValidationError, ValidationReport};

use super::bindings::sparq::instrument::{display, types};
use super::convert;
use super::load::{InstrumentRuntime, LoadContext, PREPARE_FUEL_MULTIPLIER};
use crate::sources::ReplayProvider;
use crate::validate::{Stage, StageOutcome, Verdict};

/// What the runtime stages need from the world — every path named by the caller (the CLI knows
/// the repo; a stage that guesses where fixtures live is a stage that silently validates against
/// nothing).
#[derive(Clone, Debug)]
pub struct RuntimeContext {
    /// The recorded fixture set the golden render replays (`reference/fixtures/observatory`).
    pub fixtures_dir: PathBuf,
    /// The wasmtime compile cache dir (`None` = off).
    pub cache_dir: Option<PathBuf>,
    /// Where stage 6 publishes the at-rest IR (`dat_<id>.ir.json`) — the app's at-rest door,
    /// computed by the caller (one owner of the path rule). `None` = do not publish.
    pub atrest_path: Option<PathBuf>,
    /// Whether stage 6 may WRITE `preview.svg` into the package dir (the hand-in gate does;
    /// a read-only check does not).
    pub write_previews: bool,
    /// The negotiation shape for the smoke lifecycle.
    pub sample_rate: u32,
    /// The negotiation shape for the smoke lifecycle.
    pub block_frames: u32,
}

/// The draw measurement budget: the declared per-call budget would trap a genuinely
/// under-declared guest before the gate could MEASURE it — and an unmeasured overrun cannot be
/// re-baselined. The stages therefore draw under a diagnostic budget (prepare's multiplier) and
/// stage 5 judges the measured number against the declared one. Production draw enforcement is
/// the watchdog's row (§3): the declared budget, frame-skip semantics, words in the panel.
fn draw_diagnostic_budget(max_fuel: u64) -> u64 {
    max_fuel.saturating_mul(super::load::PREPARE_FUEL_MULTIPLIER)
}

/// One display's declared rect (the manifest's `min_size` — the render case's canvas).
struct DisplayCase {
    id: String,
    w: f32,
    h: f32,
}

fn outcome(
    stage: Stage,
    verdict: Verdict,
    report: ValidationReport,
    words: String,
) -> StageOutcome {
    StageOutcome { stage, verdict, report, words }
}

fn err(kind: CodeKind, path: &str, found: impl Into<String>, fix: &str) -> ValidationError {
    ValidationError::new(kind, path, fix).with_found(found)
}

/// Builds the runtime + the replay-fed load context, or the honest refusal words.
fn runtime_and_ctx(ctx: &RuntimeContext) -> Result<(InstrumentRuntime, LoadContext, i64), String> {
    let runtime = InstrumentRuntime::new(ctx.cache_dir.clone())?;
    let replay = ReplayProvider::load(&ctx.fixtures_dir)?;
    let now = replay.now();
    let provider: Arc<dyn crate::sources::StreamProvider + Send + Sync> = Arc::new(replay);
    let load = LoadContext {
        provider,
        now_unix: now,
        seed_root: 0, // the gate's session seed: determinism wants the root pinned, not random
        node_path: vec!["validate".to_string()],
        sample_rate: ctx.sample_rate,
        block_frames: ctx.block_frames,
    };
    Ok((runtime, load, now))
}

/// Stage 3 — smoke: the full lifecycle over the frozen WIT, refusals in words (the Rust mirror
/// of `harness/smoke.mjs`'s 11 checks, instrument-host §7.1 — same assertions, no node).
#[must_use]
pub fn smoke_stage(pkg: &crate::package::Package, ctx: &RuntimeContext) -> StageOutcome {
    let stage = Stage::Smoke;
    let mut report = ValidationReport::new();
    let (runtime, load_ctx, _now) = match runtime_and_ctx(ctx) {
        Ok(r) => r,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Refused,
                report,
                format!("the runtime could not start on this host: {e}"),
            )
        },
    };
    let mut loaded = match runtime.load_package(&pkg.dir, &load_ctx) {
        Ok(l) => l,
        Err(e) => {
            report.push(err(
                CodeKind::CapabilityUndeclared,
                "distribution.entrypoint",
                e.clone(),
                "the component must compile and instantiate against the four doors only — a \
                 component importing anything else (WASI included) cannot link, and that is the \
                 capability honour proof; fix the import list or the manifest",
            ));
            return outcome(stage, Verdict::Fail, report, format!("load refused: {e}"));
        },
    };
    // Reaching here already proved: compile ✓, exports match the frozen WIT ✓, capability honour
    // (instantiation against the four-door linker) ✓, identity cross-check ✓, prepare ✓ (§2.3/2.4
    // are load-order guarantees). The checks below are the lifecycle the JS smoke drove.
    let mut checks: Vec<(bool, String)> = Vec::new();
    let caps = loaded.caps;

    // prepare(0 frames) must refuse IN WORDS (resources), then re-prepare cleanly.
    let zero = {
        let mut r =
            portless_wit_resources(ctx, caps.max_fuel, &loaded.audio.store.data().bundle.version);
        r.block_frames = 0;
        r
    };
    let refused_zero = loaded
        .audio
        .budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |store, guest| {
            guest.call_prepare(store, &zero)
        });
    match refused_zero {
        Ok((Err(e), _)) => checks.push((
            matches!(e, types::ModuleError::Resources(_)),
            format!("prepare(0 frames) refuses in words ({})", module_error_tag(&e)),
        )),
        Ok((Ok(()), _)) => checks.push((
            false,
            "prepare(0 frames) was ACCEPTED — a zero block is a resources refusal".into(),
        )),
        Err(words) => {
            checks.push((false, format!("prepare(0 frames) trapped instead of refusing: {words}")))
        },
    }
    // re-prime (the smoke's own step)
    let good =
        portless_wit_resources(ctx, caps.max_fuel, &loaded.audio.store.data().bundle.version);
    match loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| {
        g.call_prepare(s, &good)
    }) {
        Ok((Ok(()), _)) => {
            checks.push((true, "re-prepare after a refusal succeeds (idempotent)".into()))
        },
        Ok((Err(e), _)) => checks.push((false, format!("re-prepare refused: {}", guest_words(&e)))),
        Err(w) => checks.push((false, format!("re-prepare trapped: {w}"))),
    }

    // process → the exact negotiated shapes (port-less: empty everything), status Ok/Silenced.
    let input = portless_block_input(ctx);
    match loaded.audio.budget_call(caps.max_fuel, |s, g| g.call_process(s, &input)) {
        Ok((out, fuel)) => {
            let shapes_empty = out.audio_out.is_empty()
                && out.cv_out.is_empty()
                && out.events_out.is_empty()
                && out.data_out.is_empty();
            let status_legal =
                matches!(out.status, types::BlockStatus::Ok | types::BlockStatus::Silenced);
            checks.push((
                shapes_empty && status_legal,
                format!(
                    "process → status {:?}, the exact empty negotiated shapes ({} fuel/block)",
                    out.status, fuel
                ),
            ));
        },
        Err(w) => checks.push((false, format!("process trapped: {w}"))),
    }

    // configure: an empty blob is always legal; a foreign blob must be Ok-or-refused-in-words,
    // never a trap and never a silent lie (the smoke asserts noop's refusal; a guest that
    // ACCEPTS unknown bytes is the defect — but the contract allows acceptance, so the check is
    // "no trap + save-state stays deterministic").
    match loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| {
        g.call_configure(s, &[1, 2, 3])
    }) {
        Ok((res, _)) => checks.push((
            true,
            match res {
                Ok(()) => {
                    "configure(foreign blob) accepted (the guest's contract allows it)".to_string()
                },
                Err(e) => {
                    format!("configure(foreign blob) refuses in words ({})", module_error_tag(&e))
                },
            },
        )),
        Err(w) => checks.push((false, format!("configure(foreign blob) trapped: {w}"))),
    }
    match loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| {
        g.call_configure(s, &[])
    }) {
        Ok((Ok(()), _)) => checks.push((true, "configure(empty) ok".into())),
        Ok((Err(e), _)) => {
            checks.push((false, format!("configure(empty) refused: {}", guest_words(&e))))
        },
        Err(w) => checks.push((false, format!("configure(empty) trapped: {w}"))),
    }
    // save-state twice → identical bytes (the journal hashes it — determinism is the contract).
    let s1 =
        loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| {
            g.call_save_state(s)
        });
    let s2 =
        loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| {
            g.call_save_state(s)
        });
    match (s1, s2) {
        (Ok((a, _)), Ok((b, _))) => checks.push((
            a == b,
            format!("save-state twice → {} bytes, bit-identical: {}", a.len(), a == b),
        )),
        (Err(w), _) | (_, Err(w)) => checks.push((false, format!("save-state trapped: {w}"))),
    }
    // message(unknown) → Ok-or-refused-in-words, never a trap (a silent no-op is indistinguishable
    // from acceptance here; the SDK's default refuses, and the author-facing rule is the guide's).
    match loaded.audio.budget_call(caps.max_fuel.saturating_mul(PREPARE_FUEL_MULTIPLIER), |s, g| g.call_message(s, &[9])) {
        Ok((res, _)) => checks.push((
            true,
            match res {
                Ok(()) => "message(unknown) accepted (a silent no-op is the author's defect, not a trap — guide §6.7)".to_string(),
                Err(e) => format!("message(unknown) refuses in words ({})", module_error_tag(&e)),
            },
        )),
        Err(w) => checks.push((false, format!("message(unknown) trapped: {w}"))),
    }
    // draw on every declared display → a surface, no trap.
    for d in declared_displays(&loaded.manifest_text) {
        let frame = display::FrameContext {
            display_id: d.id.clone(),
            width_px: d.w,
            height_px: d.h,
            lod: display::Lod::Full,
            time_sec: 0.0,
        };
        match loaded
            .display
            .budget_call(draw_diagnostic_budget(caps.max_fuel), |s, g| g.call_draw(s, &frame))
        {
            Ok((surf, fuel)) => checks.push((
                true,
                format!("draw({}) → {} item(s), {} fuel", d.id, surface_len(&surf), fuel),
            )),
            Err(w) => checks.push((false, format!("draw({}) trapped: {w}", d.id))),
        }
    }
    // The RT-violation capture must be EMPTY across a clean lifecycle (the doors watched).
    let diags = loaded.audio.take_diagnostics();
    let diags2 = loaded.display.take_diagnostics();
    checks.push((
        diags.is_empty() && diags2.is_empty(),
        format!(
            "no RT violations or door refusals across the lifecycle ({} captured)",
            diags.len() + diags2.len()
        ),
    ));

    let failed: Vec<&String> = checks.iter().filter(|(ok, _)| !ok).map(|(_, w)| w).collect();
    for w in &failed {
        report.push(err(
            CodeKind::CapabilityUndeclared,
            "runtime.smoke",
            (*w).clone(),
            "the smoke lifecycle must complete over the frozen WIT with refusals in words — \
             fix the guest (guide §6.7) and re-validate",
        ));
    }
    let n = checks.len();
    let words = checks
        .iter()
        .map(|(ok, w)| format!("  [{}] {w}", if *ok { "PASS" } else { "FAIL" }))
        .collect::<Vec<_>>()
        .join("\n");
    outcome(
        stage,
        if failed.is_empty() { Verdict::Pass } else { Verdict::Fail },
        report,
        format!(
            "smoke {}/{} checks pass (component {}):\n{words}",
            n - failed.len(),
            n,
            loaded.component_sha
        ),
    )
}

/// Stage 4 — the golden render: the declared render case through the fixture-fed sources door,
/// TWICE, bit-exact, the hash recorded in words (the instrument's regression anchor forever).
#[must_use]
pub fn golden_stage(pkg: &crate::package::Package, ctx: &RuntimeContext) -> StageOutcome {
    let stage = Stage::GoldenRender;
    let mut report = ValidationReport::new();
    let (runtime, load_ctx, _now) = match runtime_and_ctx(ctx) {
        Ok(r) => r,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Refused,
                report,
                format!("the runtime could not start on this host: {e}"),
            )
        },
    };
    let mut loaded = match runtime.load_package(&pkg.dir, &load_ctx) {
        Ok(l) => l,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Fail,
                report,
                format!("load refused (stage 3 names the fix): {e}"),
            );
        },
    };
    let displays = declared_displays(&loaded.manifest_text);
    let Some(d) = displays.first() else {
        return outcome(
            stage,
            Verdict::Fail,
            report,
            "the manifest declares no ui.displays — an instrument with nothing to render has no \
             golden (stage 1 polices the shape; this names the consequence)"
                .to_string(),
        );
    };
    let frame = display::FrameContext {
        display_id: d.id.clone(),
        width_px: d.w,
        height_px: d.h,
        lod: display::Lod::Full,
        time_sec: 0.0, // the pinned animation phase — a golden with a moving ticker is no golden
    };
    // The warm-up draw (defect #99): wasmtime meters `memory.grow` per page, so the FIRST draw
    // of a fresh load pays the cold heap — the guest's allocator grows linear memory for the
    // items/strings of a full wall, and every later draw reuses that warm shape. Both numbers
    // are deterministic (the device measured 12 707 802 for the first draw of TWO independent
    // loads — smoke's and the golden's — and 12 401 514 for the second), but cold-against-warm
    // is not "two identical runs": the fuel moved between run 1 and run 2 and failed the stage
    // with the words for unseeded randomness, which was not the defect. Warm the heap once,
    // discard the surface, then measure the pair — the steady state a frame loop actually lives in.
    let _ = loaded
        .display
        .budget_call(draw_diagnostic_budget(loaded.caps.max_fuel), |s, g| g.call_draw(s, &frame));
    let mut hashes = Vec::new();
    let mut fuels = Vec::new();
    let mut items = 0usize;
    for run in 0..2 {
        match loaded.display.budget_call(draw_diagnostic_budget(loaded.caps.max_fuel), |s, g| {
            g.call_draw(s, &frame)
        }) {
            Ok((surf, fuel)) => {
                let v = convert::surface_to_ir_json(&surf, d.w, d.h);
                let pretty = match convert::ir_json_pretty(&v) {
                    Ok(p) => p,
                    Err(e) => {
                        return outcome(stage, Verdict::Fail, report, format!("IR serialise: {e}"));
                    },
                };
                hashes.push(convert::sha256_hex(pretty.as_bytes()));
                fuels.push(fuel);
                if run == 0 {
                    items = surface_len(&surf);
                }
            },
            Err(w) => {
                return outcome(
                    stage,
                    Verdict::Fail,
                    report,
                    format!("golden render trapped on run {}: {w}", run + 1),
                );
            },
        }
    }
    let deterministic = hashes[0] == hashes[1];
    let fuel_deterministic = fuels[0] == fuels[1];
    if !deterministic || !fuel_deterministic {
        report.push(err(
            CodeKind::DisplayPrimitiveUnknown,
            "runtime.golden",
            format!("{} / fuel {} vs {}", hashes[0], fuels[0], fuels[1]),
            "a golden render that moves between two identical runs is a determinism defect, not \
             a statistic (instrument-host §3) — find the unseeded randomness or the clock read; \
             if ONLY the fuel moved (the hashes agree), it is a first-call cost inside the guest — \
             defect #99's class: the heap the first draw grows, which wasmtime meters per page — \
             not randomness",
        ));
    }
    outcome(
        stage,
        if deterministic && fuel_deterministic { Verdict::Pass } else { Verdict::Fail },
        report,
        format!(
            "golden render ×2 bit-exact: {} — {} item(s) at {}×{} Full, draw fuel {} (prepare \
             burned {}; component {})",
            hashes[0], items, d.w, d.h, fuels[0], loaded.prepare_fuel, loaded.component_sha
        ),
    )
}

/// Stage 5's measurement half — declared budgets vs MEASURED fuel/memory (the static half stays
/// in `validate::budgets_stage`; this is the half that refused until now).
#[must_use]
pub fn measured_budgets(pkg: &crate::package::Package, ctx: &RuntimeContext) -> StageOutcome {
    let stage = Stage::Budgets;
    let mut report = ValidationReport::new();
    let (runtime, load_ctx, _now) = match runtime_and_ctx(ctx) {
        Ok(r) => r,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Refused,
                report,
                format!("the runtime could not start on this host: {e}"),
            )
        },
    };
    let mut loaded = match runtime.load_package(&pkg.dir, &load_ctx) {
        Ok(l) => l,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Fail,
                report,
                format!("load refused (stage 3 names the fix): {e}"),
            )
        },
    };
    // A few blocks and a frame — the measurement, not a soak (the device's frame-time histogram
    // is the run-sheet's acceptance line; the gate measures the budget class).
    let mut max_block_fuel = 0u64;
    for b in 0..4u64 {
        let mut input = portless_block_input(ctx);
        input.block_id = b;
        match loaded.audio.budget_call(loaded.caps.max_fuel, |s, g| g.call_process(s, &input)) {
            Ok((_, fuel)) => max_block_fuel = max_block_fuel.max(fuel),
            Err(w) => {
                return outcome(
                    stage,
                    Verdict::Fail,
                    report,
                    format!("process trapped under measurement: {w}"),
                );
            },
        }
    }
    let displays = declared_displays(&loaded.manifest_text);
    let mut draw_fuel = 0u64;
    if let Some(d) = displays.first() {
        let frame = display::FrameContext {
            display_id: d.id.clone(),
            width_px: d.w,
            height_px: d.h,
            lod: display::Lod::Full,
            time_sec: 0.0,
        };
        match loaded.display.budget_call(draw_diagnostic_budget(loaded.caps.max_fuel), |s, g| {
            g.call_draw(s, &frame)
        }) {
            Ok((_, fuel)) => draw_fuel = fuel,
            Err(w) => {
                return outcome(
                    stage,
                    Verdict::Fail,
                    report,
                    format!("draw trapped under measurement: {w}"),
                )
            },
        }
    }
    let mem_draw = loaded.display.store.data().limits.high_water;
    let mem_proc = loaded.audio.store.data().limits.high_water;
    let within = max_block_fuel <= loaded.caps.max_fuel && draw_fuel <= loaded.caps.max_fuel;
    if !within {
        report.push(err(
            CodeKind::CrossField,
            "capabilities.max_fuel",
            format!(
                "measured process {max_block_fuel} / draw {draw_fuel} vs declared {}",
                loaded.caps.max_fuel
            ),
            "the measured budget exceeds the declared class — re-baseline max_fuel WITH an \
             operator ruling recorded (the ceilings move up only with a measurement, never \
             silently), or make the guest cheaper",
        ));
    }
    outcome(
        stage,
        if within { Verdict::Pass } else { Verdict::Fail },
        report,
        format!(
            "measured: instantiate {} fuel (audio) / {} (display); prepare {} / {}; worst process block {max_block_fuel} \
             of declared {}; one full draw {draw_fuel}; memory high-water {} B (audio) / {} B \
             (display) against the {} MB the limiter enforces",
            loaded.instantiate_fuel,
            loaded.display_instantiate_fuel,
            loaded.prepare_fuel,
            loaded.display_prepare_fuel,
            loaded.caps.max_fuel,
            mem_proc,
            mem_draw,
            loaded.caps.max_memory_mb
        ),
    )
}

/// Stage 6 — visual conformance: the three breakpoints' LOD renders against the declared
/// `gpu_class` ceilings, the unknown-token scan (`E-LITERAL-APPEARANCE`'s runtime face), the
/// `preview.svg` regeneration (the package's file 4, validator-generated), and the at-rest
/// publish (the LATER door: a device that has ever validated shows its last wall at rest).
#[must_use]
pub fn visual_stage(pkg: &crate::package::Package, ctx: &RuntimeContext) -> StageOutcome {
    let stage = Stage::VisualConformance;
    let mut report = ValidationReport::new();
    let (runtime, load_ctx, _now) = match runtime_and_ctx(ctx) {
        Ok(r) => r,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Refused,
                report,
                format!("the runtime could not start on this host: {e}"),
            )
        },
    };
    let mut loaded = match runtime.load_package(&pkg.dir, &load_ctx) {
        Ok(l) => l,
        Err(e) => {
            return outcome(
                stage,
                Verdict::Fail,
                report,
                format!("load refused (stage 3 names the fix): {e}"),
            )
        },
    };
    let gpu_class = gpu_class_of(&loaded.manifest_text);
    let Some(ceilings) = crate::ceilings::ceilings_for(&gpu_class) else {
        report.push(err(
            CodeKind::EnumUnknown,
            "resources.gpu_class",
            gpu_class.clone(),
            "gpu_class must be one of the frozen spellings (stage 1 polices this; named here \
             because the measurement could not run)",
        ));
        return outcome(
            stage,
            Verdict::Fail,
            report,
            "gpu_class is outside the frozen vocabulary".into(),
        );
    };
    let displays = declared_displays(&loaded.manifest_text);
    let Some(d) = displays.first() else {
        return outcome(
            stage,
            Verdict::Fail,
            report,
            "no declared display to conform (stage 4 names it too)".into(),
        );
    };
    // The LOD ladder (the harness's breakpoints): Full/Reduced/Minimal over the same rect.
    let lods = [
        ("wall-large", display::Lod::Full),
        ("wall-laptop", display::Lod::Reduced),
        ("wall-tablet", display::Lod::Minimal),
    ];
    let mut lines = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for (name, lod) in lods {
        let frame = display::FrameContext {
            display_id: d.id.clone(),
            width_px: d.w,
            height_px: d.h,
            lod,
            time_sec: 0.0,
        };
        let surf = match loaded
            .display
            .budget_call(draw_diagnostic_budget(loaded.caps.max_fuel), |s, g| {
                g.call_draw(s, &frame)
            }) {
            Ok((surf, _)) => surf,
            Err(w) => {
                return outcome(stage, Verdict::Fail, report, format!("draw({name}) trapped: {w}"))
            },
        };
        let b = budget_of(&surf);
        let n = surface_len(&surf);
        let over = b.vertices > ceilings.max_vertices
            || b.instances > ceilings.max_instances
            || b.heat_cells > ceilings.max_heat_cells;
        if over {
            report.push(err(
                CodeKind::CrossField,
                "resources.gpu_class",
                format!(
                    "{name}: vertices {} / instances {} / heat_cells {} vs {} ceilings {}/{}/{}",
                    b.vertices,
                    b.instances,
                    b.heat_cells,
                    gpu_class,
                    ceilings.max_vertices,
                    ceilings.max_instances,
                    ceilings.max_heat_cells
                ),
                "the render exceeds its declared gpu_class — degrade LOD further, or re-declare \
                 the class WITH an operator ruling (the ceilings table moves up only with a \
                 recorded re-baselining)",
            ));
        }
        // The 500-primitive rule at Minimal (§5.7's ladder).
        if matches!(lod, display::Lod::Minimal) && n > 500 {
            report.push(err(
                CodeKind::CrossField,
                "ui.displays[0].lod",
                format!("{n} items at Minimal LOD"),
                "the Minimal breakpoint keeps the renderers under the 500-primitive rule (§5.7) — \
                 drop chrome harder at Minimal",
            ));
        }
        lines.push(format!(
            "  {name} ({:?}): {n} item(s), budget v{}/i{}/h{} vs {gpu_class} {}/{}/{}",
            lod,
            b.vertices,
            b.instances,
            b.heat_cells,
            ceilings.max_vertices,
            ceilings.max_instances,
            ceilings.max_heat_cells
        ));
        if matches!(lod, display::Lod::Full) {
            let v = convert::surface_to_ir_json(&surf, d.w, d.h);
            let full_json = convert::ir_json_pretty(&v).ok();
            // The unknown-token scan: the painter's own diagnostics ARE the E-LITERAL-APPEARANCE
            // runtime face (an id that does not resolve is an appearance value the bundle does
            // not carry — the show goes on, the hand-in does not).
            let parsed = match full_json.as_deref() {
                Some(text) => Some(sparq_ui::json::parse(text)),
                None => None,
            };
            match parsed {
                None => failures.push("no Full-LOD render to paint (the draw above failed)".into()),
                Some(Ok(j)) => match sparq_ui::displaylist::parse_surface(&j) {
                    Ok(items) => {
                        let mut painter = sparq_ui::displaylist::Painter::new();
                        let _ = painter.resolve(&items);
                        for diag in painter.diagnostics() {
                            report.push(err(
                                CodeKind::LiteralAppearance,
                                "ui.displays[0].items",
                                diag.clone(),
                                "every appearance value must be a token id the checked-in bundle \
                                 resolves — the painter skipped this primitive; fix the id (or \
                                 add the token through token_gen, never a literal)",
                            ));
                        }
                        if ctx.write_previews {
                            // preview.svg — file 4 of 5, validator-GENERATED (never hand-drawn),
                            // through the SAME painter the shell uses: a preview that disagrees
                            // with the app is worse than no preview.
                            let painted = {
                                let mut p2 = sparq_ui::displaylist::Painter::new();
                                p2.resolve(&items)
                            };
                            let bg = sparq_ui::tokens::color_hex("color.ground.panel")
                                .and_then(|h| sparq_ui::displaylist::Rgba::from_hex(h, 1.0));
                            let svg = sparq_ui::displaylist::svg(&painted, d.w, d.h, bg);
                            let path = pkg.dir.join(crate::package::PREVIEW_FILE);
                            match write_text_file(&path, &svg) {
                                Ok(()) => {
                                    lines.push(format!(
                                        "  preview.svg regenerated ({} B)",
                                        svg.len()
                                    ));
                                },
                                Err(e) => {
                                    // I/O has no catalogue code — the sentence prints verbatim
                                    // and the stage fails on it (a preview that cannot be
                                    // written is a failed hand-in, in words).
                                    failures.push(format!("preview.svg write failed: {e}"));
                                },
                            }
                        }
                    },
                    Err(e) => report.push(err(
                        CodeKind::DisplayPrimitiveUnknown,
                        "ui.displays[0].items",
                        e.clone(),
                        "the emitted display list did not parse against the interchange — an \
                         unknown primitive kind is E-DISPLAY-PRIMITIVE-UNKNOWN; fix the guest's \
                         emitted vocabulary",
                    )),
                },
                Some(Err(e)) => report.push(err(
                    CodeKind::DisplayPrimitiveUnknown,
                    "ui.displays[0].items",
                    format!("{e}"),
                    "the emitted display list is not valid interchange JSON — fix the guest",
                )),
            }
            // The at-rest publish (the LATER door): the Full-LOD IR to the app's at-rest path.
            if let Some(atrest) = &ctx.atrest_path {
                if let Some(json) = &full_json {
                    match write_text_file(atrest, json) {
                        Ok(()) => {
                            lines.push(format!("  at-rest published to {}", atrest.display()))
                        },
                        Err(e) => lines.push(format!("  at-rest publish FAILED: {e}")),
                    }
                }
            }
        }
    }
    for f in &failures {
        lines.push(format!("  FAIL: {f}"));
    }
    let failed = report.len() + failures.len();
    outcome(
        stage,
        if failed == 0 { Verdict::Pass } else { Verdict::Fail },
        report,
        format!(
            "visual conformance at the three breakpoints (display `{}`, {}×{}, gpu_class {gpu_class}):\n{}",
            d.id,
            d.w,
            d.h,
            lines.join("\n")
        ),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────────────────────────

/// The budget of one rendered surface — the harness's `count_budget` mapping, mirrored over the
/// WIT items (same policy, second implementation, drift-gated by the golden test that runs both).
#[must_use]
pub fn budget_of(surf: &display::Surface) -> Budget {
    let mut b = Budget::default();
    if let display::Surface::Items(items) = surf {
        for it in items {
            match it {
                display::Item::Polyline(p) => b.vertices += p.points.len() as u32,
                display::Item::Path(p) => b.vertices += p.segments.len() as u32,
                display::Item::Trace(t) => b.vertices += t.values.len() as u32,
                display::Item::Rect(_) => b.vertices += 4,
                display::Item::Arc(_) => b.vertices += 32,
                display::Item::GlyphRun(_) => b.instances += 1,
                display::Item::Points(p) => b.instances += p.positions.len() as u32,
                display::Item::HeatCells(h) => b.heat_cells += h.cols.saturating_mul(h.rows),
            }
        }
    }
    b
}

/// The §5.8 budget columns (the ceilings table's vocabulary).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Budget {
    /// Total vertices.
    pub vertices: u32,
    /// Total instances.
    pub instances: u32,
    /// Total heat cells.
    pub heat_cells: u32,
}

fn surface_len(surf: &display::Surface) -> usize {
    match surf {
        display::Surface::Items(items) => items.len(),
        display::Surface::Scene(_) => 0,
    }
}

fn module_error_tag(e: &types::ModuleError) -> &'static str {
    match e {
        types::ModuleError::Unsupported(_) => "unsupported",
        types::ModuleError::State(_) => "state",
        types::ModuleError::Resources(_) => "resources",
        types::ModuleError::Message(_) => "message",
    }
}

fn guest_words(e: &types::ModuleError) -> String {
    super::load::guest_error_words("the call", e)
}

fn portless_wit_resources(
    ctx: &RuntimeContext,
    max_fuel: u64,
    bundle: &types::Semver,
) -> types::Resources {
    convert::resources_to_wit(
        &sparq_module_api::module::Resources {
            sample_rate: ctx.sample_rate,
            block_frames: ctx.block_frames as usize,
            input_channels: 0,
            output_channels: 0,
            audio_in_channels: [0; sparq_module_api::module::MAX_PORTS_PER_CLASS],
            audio_in_count: 0,
            audio_out_channels: [0; sparq_module_api::module::MAX_PORTS_PER_CLASS],
            audio_out_count: 0,
            oversampling: sparq_module_api::module::Oversampling::None,
            voices: 0,
            arena_bytes: 0,
        },
        max_fuel,
        bundle,
    )
}

fn portless_block_input(
    ctx: &RuntimeContext,
) -> super::bindings::sparq::instrument::audio::BlockInput {
    use super::bindings::sparq::instrument::{audio, types as t};
    audio::BlockInput {
        block_id: 0,
        frames: ctx.block_frames,
        t_sample: 0,
        tick: 0,
        ppqn: 960,
        params: t::ParamSet { version: 1, values: Vec::new() },
        audio_in: Vec::new(),
        cv_in: Vec::new(),
        events_in: Vec::new(),
        data_in: Vec::new(),
    }
}

/// The manifest's declared displays (`id` + `min_size`) — the render cases. Parsed from the text
/// (the core decoder does not model ui.displays; validate.rs's stage 1 already policed the shape).
fn declared_displays(manifest_text: &str) -> Vec<DisplayCase> {
    let Ok(root) = sparq_module_api::toml::parse(manifest_text) else { return Vec::new() };
    let Some(ui) = root.get("ui").and_then(sparq_module_api::toml::Value::as_table) else {
        return Vec::new();
    };
    let Some(displays) = ui.get("displays").and_then(sparq_module_api::toml::Value::as_array)
    else {
        return Vec::new();
    };
    displays
        .iter()
        .filter_map(|d| {
            let t = d.as_table()?;
            let id = t.get("id")?.as_str()?.to_string();
            let size = t.get("min_size")?.as_array()?;
            let w = size.first()?.as_f64()? as f32;
            let h = size.get(1)?.as_f64()? as f32;
            Some(DisplayCase { id, w, h })
        })
        .collect()
}

fn gpu_class_of(manifest_text: &str) -> String {
    sparq_module_api::toml::parse(manifest_text)
        .ok()
        .and_then(|root| {
            root.get("resources")
                .and_then(sparq_module_api::toml::Value::as_table)
                .and_then(|r| r.get("gpu_class"))
                .and_then(sparq_module_api::toml::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "none".to_string())
}

/// Writes a text file (LF, UTF-8) the house way: `File` + `write_all`, never the banned
/// conveniences (the `clippy.toml` fs rule is the audio path's, but the house helper discipline
/// is everywhere — `check_text_io` watches tools/, this is the Rust sibling).
fn write_text_file(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}
