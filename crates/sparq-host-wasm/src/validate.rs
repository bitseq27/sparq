//! The hand-in gate — MODULE-BUILD-GUIDE §7's chain, WO-018's zero-dependency half.
//!
//! The chain runs in the guide's order, and the split is honesty, not convenience: what can be
//! proven without a wasm runtime is **run**; what needs one **refuses in words**
//! ([`Verdict::Refused`]) — never a silent skip, never a pretend-pass. [`GateReport::complete`]
//! says whether the whole chain ran; a PARTIAL report still gates on what ran ([`GateReport::passed`]
//! is false when any stage failed), and the refusals print verbatim so nobody mistakes the static
//! half for the whole gate. "A package that passes validate IS a package that loads at launch"
//! becomes literally true when the runtime half lands; until then the report says which half of
//! that sentence it proved.
//!
//! # Stage-to-code wiring (the design record is `docs/instrument-host.md` §6)
//!
//! | stage | runs here | what |
//! |---|---|---|
//! | 1 schema | **yes** | the contract crate's full decode, plus the static instrument rules: placement (instruments/ carries `layer = instrument`), the `host_api` pin vs [`HOST_MODULE_API`], the capability-honour list (guide §6's sentence, made mechanical), the data-port routing refusal (v1.1 status), scene⇒`gpu_class` |
//! | 2 package | **yes** | [`crate::package::check`] at the strict profile: cap, roles, manifest, component, README, example |
//! | 3 smoke | refused | instantiation, exports vs the frozen WIT, no ambient authority — needs the runtime |
//! | 4 golden render | refused | null device, twice, bit-exact — needs the runtime |
//! | 5 budgets | **static half** | declared `max_fuel`/`max_memory_mb` present and sane; the MEASUREMENT needs the runtime |
//! | 6 visual conformance | refused | breakpoints, look-board audit, literal-appearance scan, touch ≥ 44 px, `preview.svg` regeneration — needs the runtime + the headless painter |
//! | 7 signature/badge | **yes** | presence and shape run; the badge words follow plan §15. Cryptographic verification lands with the runtime's keyring, and the stage says so |

use std::path::Path;

use sparq_module_api::error::{CodeKind, ValidationError, ValidationReport};
use sparq_module_api::manifest::Layer;
use sparq_module_api::toml::{Table, Value};
use sparq_module_api::{decode, toml};

use crate::package::{self, Profile};

/// The module-API major this host speaks.
///
/// A manifest's `identity.host_api = {min, max}` pin that excludes this value is refused with
/// words (`E-HOST-API-INCOMPATIBLE`) — an incompatible guest never loads half-working. Lives here
/// until load-time negotiation lands with the runtime half, then moves to `sparq-module-api`: the
/// contract crate should own the number both sides negotiate against, and moving it is a
/// one-line diff plus a pin update in `tests/api_snapshot.rs`.
pub const HOST_MODULE_API: u32 = 1;

/// One stage's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The stage ran and the package passed it.
    Pass,
    /// The stage ran and the package failed it — every failure actionable, verbatim.
    Fail,
    /// The stage could not run in this build, IN WORDS: what it would check and what would make
    /// it run. A refusal is not a pass and not a fail; `complete()` counts them.
    Refused,
}

/// The guide's seven stages, in the guide's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// 1 — the manifest validates; `layer = instrument`; `tier = t2`; unknown keys rejected;
    /// `host_api` compatible.
    Schema,
    /// 2 — ≤ 5 files; file roles recognised; asset hashes present and matching (the hash half
    /// needs the content-addressed library — Phase 3; until then the declared hashes are
    /// shape-checked by the decoder and named here as unverified).
    Package,
    /// 3 — the component instantiates in the sandbox; exports match the WIT contract;
    /// capabilities honoured; no ambient authority.
    Smoke,
    /// 4 — the declared render case through the null device, twice, bit-exact; hash recorded.
    GoldenRender,
    /// 5 — fuel per block, memory, `prepare` cost within declared classes.
    Budgets,
    /// 6 — panel + displays rendered headless at the three breakpoints; look-board audit;
    /// literal-appearance scan; touch ≥ 44 px; `preview.svg` regenerated.
    VisualConformance,
    /// 7 — signed → clean load; unsigned → badged, Perform-disabled.
    SignatureBadge,
}

impl Stage {
    /// Every stage, in chain order.
    pub const ALL: [Self; 7] = [
        Self::Schema,
        Self::Package,
        Self::Smoke,
        Self::GoldenRender,
        Self::Budgets,
        Self::VisualConformance,
        Self::SignatureBadge,
    ];

    /// The guide's numbering (1-based — the report quotes the guide).
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Self::Schema => 1,
            Self::Package => 2,
            Self::Smoke => 3,
            Self::GoldenRender => 4,
            Self::Budgets => 5,
            Self::VisualConformance => 6,
            Self::SignatureBadge => 7,
        }
    }

    /// The guide's name for the stage.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::Package => "package",
            Self::Smoke => "smoke",
            Self::GoldenRender => "golden render",
            Self::Budgets => "budgets",
            Self::VisualConformance => "visual conformance",
            Self::SignatureBadge => "signature/badge",
        }
    }
}

/// One stage's outcome: the verdict, the failures (empty unless [`Verdict::Fail`]), and the line
/// of words the report prints.
#[derive(Clone, Debug)]
pub struct StageOutcome {
    /// Which stage.
    pub stage: Stage,
    /// How it went.
    pub verdict: Verdict,
    /// The failures, in the house verbatim style (field, found, allowed, fix).
    pub report: ValidationReport,
    /// The words: what passed, or what is refused and what would make it run.
    pub words: String,
}

/// The whole chain's outcome.
#[derive(Clone, Debug)]
pub struct GateReport {
    /// One outcome per stage, in chain order — always all seven.
    pub outcomes: Vec<StageOutcome>,
}

impl GateReport {
    /// Whether no stage failed. Refusals do not fail the gate — they are printed, and
    /// [`complete`](Self::complete) is the flag that says the gate was partial.
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.outcomes.iter().any(|o| o.verdict == Verdict::Fail)
    }

    /// Whether every stage actually ran — false while any stage refuses in words (the runtime
    /// half, WO-018).
    #[must_use]
    pub fn complete(&self) -> bool {
        !self.outcomes.iter().any(|o| o.verdict == Verdict::Refused)
    }

    /// The stages that failed, in chain order.
    #[must_use]
    pub fn failed_stages(&self) -> Vec<Stage> {
        self.outcomes.iter().filter(|o| o.verdict == Verdict::Fail).map(|o| o.stage).collect()
    }

    /// The stages that refused, in chain order.
    #[must_use]
    pub fn refused_stages(&self) -> Vec<Stage> {
        self.outcomes.iter().filter(|o| o.verdict == Verdict::Refused).map(|o| o.stage).collect()
    }

    /// The report as the CLI and the browser show it: one bracketed verdict per stage, the
    /// failures verbatim under their stage, and a summary line that never lets a PARTIAL gate
    /// read as a full one.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for o in &self.outcomes {
            let tag = match o.verdict {
                Verdict::Pass => "[PASS]   ",
                Verdict::Fail => "[FAIL]   ",
                Verdict::Refused => "[REFUSED]",
            };
            out.push_str(&format!("{tag} {} {} — {}\n", o.stage.number(), o.stage.name(), o.words));
            for e in o.report.iter() {
                out.push_str(&format!("    {e}\n"));
            }
        }
        let failed = self.failed_stages();
        let refused = self.refused_stages();
        if !failed.is_empty() {
            let names: Vec<String> =
                failed.iter().map(|s| format!("{} {}", s.number(), s.name())).collect();
            out.push_str(&format!(
                "GATE: FAIL — {} stage(s) failed ({}); every failure above names its fix.\n",
                failed.len(),
                names.join(", ")
            ));
        } else if !refused.is_empty() {
            out.push_str(&format!(
                "GATE: PARTIAL — the {} stage(s) that ran passed; {} stage(s) refused in words \
                 ({}). A partial gate is not a hand-in: the runtime stages land with WO-018's \
                 `instrument-host` increment.\n",
                Stage::ALL.len() - refused.len(),
                refused.len(),
                refused.iter().map(|s| s.name()).collect::<Vec<_>>().join(", ")
            ));
        } else {
            out.push_str("GATE: PASS — the whole chain ran; this package loads at launch.\n");
        }
        out
    }
}

/// Runs the gate over one package directory. Never panics; I/O problems arrive as words inside
/// the relevant stage's report.
#[must_use]
pub fn run(dir: &Path) -> GateReport {
    let pkg = package::open(dir);
    let outcomes = vec![
        schema_stage(&pkg),
        package_stage(&pkg),
        refused(
            Stage::Smoke,
            "the component's instantiation needs the instrument-host runtime (wasmtime behind \
             the `instrument-host` feature): exports checked against the frozen WIT, capability \
             honour at the door, no ambient authority (guide §7 stage 3). The zero-dependency \
             build refuses in words rather than pretend — WO-018's runtime increment.",
        ),
        refused(
            Stage::GoldenRender,
            "the golden render needs the runtime half: the declared render case through the null \
             device, twice, bit-exact, hash recorded — the instrument's regression anchor \
             forever (guide §7 stage 4).",
        ),
        budgets_stage(&pkg),
        refused(
            Stage::VisualConformance,
            "visual conformance needs the runtime half plus the headless painter: panel and \
             displays at the three breakpoints, the look-board audit, the literal-appearance \
             scan (E-LITERAL-APPEARANCE), the 44 px touch floor, and preview.svg regeneration \
             (guide §7 stage 6). The manifest-side vocabularies ARE checked — stage 1 polices \
             the display shapes; what refuses here is the emitted-frame half.",
        ),
        signature_stage(&pkg),
    ];
    GateReport { outcomes }
}

/// A stage that refuses in words, with an empty report.
fn refused(stage: Stage, words: &str) -> StageOutcome {
    StageOutcome {
        stage,
        verdict: Verdict::Refused,
        report: ValidationReport::new(),
        words: words.to_string(),
    }
}

/// Stage 1 — schema: the contract crate's full decode plus the static instrument rules.
fn schema_stage(pkg: &package::Package) -> StageOutcome {
    let stage = Stage::Schema;
    if pkg.manifest_text.is_none() {
        return StageOutcome {
            stage,
            verdict: Verdict::Fail,
            report: ValidationReport::new(),
            words: "no readable sparqmod.toml — nothing to schema-check; stage 2 names the file \
                    problem with its fix"
                .to_string(),
        };
    }
    let r = schema_check(pkg);
    let verdict = if r.is_empty() { Verdict::Pass } else { Verdict::Fail };
    let words = if r.is_empty() {
        format!(
            "the manifest decodes and the instrument rules hold — placement, host_api pin \
             covers {HOST_MODULE_API}, capability honour, no unrouted data ports, scene budgets \
             declared"
        )
    } else {
        format!("{} schema failure(s), verbatim below", r.len())
    };
    StageOutcome { stage, verdict, report: r, words }
}

/// The static schema checks as a bare report — stage 1 without its stage furniture.
///
/// This is the launch-time door for a package whose SHAPE already passed
/// [`crate::package::is_loadable`]: module-api §11's "a bad package never loads" is these rules
/// plus the decoder, and `sparq mod list` shows exactly what the browser would. A `None`
/// manifest yields an empty report because the missing file is the package stage's refusal,
/// not a schema line — callers check shape first.
#[must_use]
pub fn schema_check(pkg: &package::Package) -> ValidationReport {
    let Some(text) = pkg.manifest_text.as_deref() else {
        return ValidationReport::new();
    };
    let validated = match decode(text) {
        Ok(v) => v,
        Err(report) => return report, // the decoder's own full pass, verbatim
    };

    // The decode passed; now the instrument-door rules on top of it. The raw table is re-parsed
    // for the fields the core decoder accepts but does not model (capabilities, ui.displays,
    // resources.gpu_class, signature) — parse failures are impossible here (decode just parsed
    // the same text), so `ok()` losing one is a refusal-free no-op, not a silent pass: every
    // rule below degrades to "field absent", which has its own error from the decoder when the
    // field was required.
    let parsed = toml::parse(text).ok();
    let mut r = ValidationReport::new();

    // Placement: instruments/ carries instruments (ADR-010 decision 5). A backbone manifest here
    // is refused in words — the layers do not mix, and module-api §11 forbids the same id in both.
    if validated.layer() != Layer::Instrument {
        r.push(
            ValidationError::new(
                CodeKind::CrossField,
                "classification.layer",
                "instruments/ carries layer = \"instrument\" packages (ADR-010 decision 5); a \
                 backbone module is first-party T1 and belongs in modules/ — the same id may \
                 not exist in both layers (module-api §11)",
            )
            .with_found(format!("layer = \"{}\"", validated.layer().as_str()))
            .with_allowed("layer = \"instrument\" with tier = \"t2\""),
        );
    }

    // The host_api pin must include the host's number, or the load is refused with words.
    let (min, max) =
        (validated.manifest().identity.host_api_min, validated.manifest().identity.host_api_max);
    let pinned = matches!(
        (min, max),
        (Some(lo), Some(hi)) if lo <= HOST_MODULE_API && HOST_MODULE_API <= hi
    );
    if !pinned {
        r.push(
            ValidationError::new(
                CodeKind::HostApiIncompatible,
                "identity.host_api",
                "widen the pin to include the host's module-API version, or rebuild against the \
                 contract version this host speaks — an incompatible guest is refused with \
                 words, never loaded half-working",
            )
            .with_found(format!(
                "host speaks {HOST_MODULE_API}, manifest pins {}..={}",
                min.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string()),
                max.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string()),
            )),
        );
    }

    // Data ports: the vocabulary is frozen (v1.1, ratified 2026-10-05) but the host does not
    // ROUTE them yet — refused in words until the native arbiter lands (Phase 5), never loaded
    // half-working (guide §6's honest status).
    for (n, p) in validated.manifest().ports.iter().enumerate() {
        if p.port_type.as_deref() == Some("data") {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    format!("ports[{n}].type"),
                    "the host does not route data ports yet (contract v1.1 status; recheck at \
                     the Phase-5 native landing) — remove the port; until then, parse files \
                     from hash-declared assets (guide §6)",
                )
                .with_found("type = \"data\""),
            );
        }
    }

    // Capability honour (guide §6's sentence, made mechanical): the host validates that a T2
    // package declares no capability it cannot honour. `fs_read` (asset scopes) and the numeric
    // knobs ARE honourable; the five doors below do not exist in the T2 world — capability by
    // absence (instrument-wit decision E) — and each fix names the door that CAN serve.
    if let Some(caps) = sub_table(parsed.as_ref(), "capabilities") {
        check_capability_honour(caps, &mut r);
    }

    // Scene ⇒ gpu_class above none: the zero ceiling row cannot be honoured by a scene display,
    // so the combination is refused HERE, at validation — not at first draw.
    let gpu_class = sub_table(parsed.as_ref(), "resources")
        .and_then(|t| t.get("gpu_class"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    if gpu_class == "none" {
        if let Some(ui) = sub_table(parsed.as_ref(), "ui") {
            for (n, d) in ui.tables("displays").unwrap_or_default().iter().enumerate() {
                if d.get("kind").and_then(Value::as_str) == Some("scene") {
                    r.push(
                        ValidationError::new(
                            CodeKind::CrossField,
                            format!("ui.displays[{n}].kind"),
                            "a scene display draws against the gpu_class ceiling table \
                             (sparq-host-wasm::ceilings) — declare resources.gpu_class above \
                             \"none\"; the zero row cannot be honoured, and this door refuses \
                             at validation rather than at first draw",
                        )
                        .with_found("kind = \"scene\", gpu_class = \"none\"")
                        .with_allowed("light medium heavy very_heavy"),
                    );
                }
            }
        }
    }

    r
}

/// The capability-honour list: what a T2 package may not declare ON, and the door that can serve
/// instead. One error per violation, each fix naming the alternative — a refusal that does not
/// say where the capability lives is half a message.
fn check_capability_honour(caps: &Table, r: &mut ValidationReport) {
    // network: only "none" is honourable in T2.
    if let Some(v) = caps.get("network").and_then(Value::as_str) {
        if v != "none" {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "capabilities.network",
                    "the T2 world has no network door (capability by absence, instrument-wit \
                     decision E) — fetching belongs to the T3 bridged-process plane, which an \
                     instrument may DRIVE through data streams (guide §6)",
                )
                .with_found(format!("network = \"{v}\""))
                .with_allowed("network = \"none\""),
            );
        }
    }
    // device: no midi/camera/serial/hid/ble in the sandbox — the host owns devices, T3 bridges them.
    if let Some(list) = caps.get("device").and_then(Value::as_array) {
        if !list.is_empty() {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "capabilities.device",
                    "the sandbox has no device door — devices belong to the host (midi arrives \
                     as events) or the T3 plane (camera, serial, hid, ble); an instrument \
                     consumes their data streams (guide §6)",
                )
                .with_found(format!("device = [{} entries]", list.len()))
                .with_allowed("device = [] or absent"),
            );
        }
    }
    // process_spawn: T3-only by schema §11, and impossible in a wasm world regardless.
    if let Some(v) = caps.get("process_spawn") {
        let spawned = v.as_bool() == Some(true) || v.as_array().is_some_and(|a| !a.is_empty());
        if spawned {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "capabilities.process_spawn",
                    "spawning is T3-only (schema §11) and the T2 world has no process door — \
                     heavy or non-real-time work belongs to the T3 plane (guide §6)",
                )
                .with_found(v.type_name()),
            );
        }
    }
    // gpu: instruments emit scene DATA; the host renders. gpu compute is the T3/host pipeline's.
    if let Some(v) = caps.get("gpu").and_then(Value::as_str) {
        if v != "none" {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "capabilities.gpu",
                    "instruments emit display lists and scene descriptors and the HOST renders \
                     them (guide §5) — there is no guest GPU door; compute belongs to the host \
                     pipeline or the T3 plane",
                )
                .with_found(format!("gpu = \"{v}\""))
                .with_allowed("gpu = \"none\" or absent"),
            );
        }
    }
    // fs_write: a guest's durable output is its state blob — the host writes, the guest declares
    // nothing to write. (fs_read — asset scopes — and the numeric knobs ARE honourable.)
    if let Some(list) = caps.get("fs_write").and_then(Value::as_array) {
        if !list.is_empty() {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "capabilities.fs_write",
                    "a guest's durable output is its state blob (save-state/configure, \
                     instrument-wit decision D) — the host writes it; there is no guest \
                     filesystem to write to",
                )
                .with_found(format!("fs_write = [{} entries]", list.len()))
                .with_allowed("fs_write = [] or absent"),
            );
        }
    }
}

/// Stage 2 — package: the five-file model at the strict (hand-in) profile.
fn package_stage(pkg: &package::Package) -> StageOutcome {
    let stage = Stage::Package;
    let r = package::check(pkg, Profile::Strict);
    let verdict = if r.is_empty() { Verdict::Pass } else { Verdict::Fail };
    let words = if r.is_empty() {
        let advisory = package::advisories(pkg);
        if advisory.is_empty() {
            "the five-file model holds: cap, roles, manifest, component, README, example"
                .to_string()
        } else {
            // Strict passed, so README/example exist; the only possible advisory is the preview.
            format!("the five-file model holds; note: {}", advisory.join("; "))
        }
    } else {
        format!("{} package failure(s), verbatim below", r.len())
    };
    StageOutcome { stage, verdict, report: r, words }
}

/// Stage 5 — budgets: the declared half RUNS (the watchdog's inputs must exist and be sane
/// before any measurement can mean anything); the measured half refuses until the runtime lands.
fn budgets_stage(pkg: &package::Package) -> StageOutcome {
    let stage = Stage::Budgets;
    let mut r = ValidationReport::new();
    let parsed = pkg.manifest_text.as_deref().and_then(|t| toml::parse(t).ok());
    let caps = sub_table(parsed.as_ref(), "capabilities");

    let fuel = positive_knob(
        caps,
        "max_fuel",
        &mut r,
        "declare the per-block fuel budget — the watchdog auto-bypasses against it (guide §4: \
         the watchdog's budget, not a suggestion)",
        true,
    );
    let mem = positive_knob(
        caps,
        "max_memory_mb",
        &mut r,
        "declare the memory ceiling in MB — the sandbox instantiation is capped by it",
        true,
    );
    positive_knob(
        caps,
        "max_cpu_ms_per_block",
        &mut r,
        "max_cpu_ms_per_block, when declared, is a positive number of milliseconds",
        false,
    );

    if !r.is_empty() {
        let n = r.len();
        return StageOutcome {
            stage,
            verdict: Verdict::Fail,
            report: r,
            words: format!(
                "{n} declared-budget failure(s); measurement refused until these are fixed and \
                 the runtime half lands"
            ),
        };
    }
    StageOutcome {
        stage,
        verdict: Verdict::Refused,
        report: r,
        words: format!(
            "declared budgets present and sane (max_fuel {fuel}/block, max_memory_mb {mem}) — \
             the MEASUREMENT (fuel per block, memory, prepare cost, the real-time property test) \
             needs the runtime half (guide §7 stage 5)"
        ),
    }
}

/// Reads one numeric capability knob. `required` decides whether absence is an error. Returns
/// the value's rendering for the stage words ("?" when absent-and-optional).
fn positive_knob(
    caps: Option<&Table>,
    key: &str,
    r: &mut ValidationReport,
    fix: &str,
    required: bool,
) -> String {
    let Some(v) = caps.and_then(|t| t.get(key)) else {
        if required {
            r.push(ValidationError::new(CodeKind::KeyMissing, format!("capabilities.{key}"), fix));
        }
        return "?".to_string();
    };
    // Integers or floats both parse; the knobs are u32-class in the schema but a float spelling
    // is a value error, not a rounding surprise.
    match v.as_i64() {
        Some(n) if n > 0 => n.to_string(),
        Some(n) => {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    format!("capabilities.{key}"),
                    "a budget is a POSITIVE integer — zero or negative budgets cannot be honoured",
                )
                .with_found(n.to_string()),
            );
            n.to_string()
        },
        None => {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    format!("capabilities.{key}"),
                    "a budget is a positive integer",
                )
                .with_found(v.type_name()),
            );
            "?".to_string()
        },
    }
}

/// Stage 7 — signature/badge: presence and shape RUN (plan §15's badge decision is static);
/// cryptographic verification is the runtime keyring's, and the words say so.
fn signature_stage(pkg: &package::Package) -> StageOutcome {
    let stage = Stage::SignatureBadge;
    let mut r = ValidationReport::new();
    let parsed = pkg.manifest_text.as_deref().and_then(|t| toml::parse(t).ok());
    let words = match sub_table(parsed.as_ref(), "signature") {
        None => "unsigned — loads BADGED and disabled by default in Perform mode (plan §15); \
                 usable everywhere else"
            .to_string(),
        Some(sig) => {
            // Shape: a signature object carries the four required members (schema §13). An
            // incomplete one is worse than none — drop the table and load badged.
            let mut missing = Vec::new();
            for key in ["key_id", "algorithm", "value", "signed_at"] {
                match sig.get(key).and_then(Value::as_str) {
                    Some(s) if !s.is_empty() => {},
                    _ => missing.push(key),
                }
            }
            if missing.is_empty() {
                let key_id = sig.get("key_id").and_then(Value::as_str).unwrap_or("?");
                format!(
                    "signed (key_id {key_id}) — the badge is clean; the VALUE's cryptographic \
                     verification lands with the runtime keyring (WO-018 runtime half), and this \
                     stage says so rather than implying it checked"
                )
            } else {
                r.push(
                    ValidationError::new(
                        CodeKind::ValueMalformed,
                        "signature",
                        "a signature object carries key_id, algorithm, value and signed_at \
                         (schema §13) — an incomplete one is worse than none: drop the table \
                         and the package loads badged instead",
                    )
                    .with_found(format!("missing or empty: {}", missing.join(", "))),
                );
                "incomplete signature object — refused below".to_string()
            }
        },
    };
    let verdict = if r.is_empty() { Verdict::Pass } else { Verdict::Fail };
    StageOutcome { stage, verdict, report: r, words }
}

/// A named sub-table of the raw parse, if present and a table. The borrow shape stays flat: one
/// `Option<&Table>` in, one out, no chains through `Value` clones (desk-audit risk #1's lesson).
fn sub_table<'a>(parsed: Option<&'a Table>, key: &str) -> Option<&'a Table> {
    parsed.and_then(|t| t.get(key)).and_then(Value::as_table)
}
