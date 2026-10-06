//! From `sparqmod.toml` text to a [`ValidatedManifest`].
//!
//! Three stages, and each has its own failure mode:
//!
//! 1. **parse** ([`crate::toml`]) — is this TOML at all? Failures become `E-VALUE-MALFORMED` with the
//!    line number the parser reported.
//! 2. **decode** — does each key have the type the schema declares? An unknown key is
//!    `E-UNKNOWN-KEY` (typo protection), a wrong type is `E-VALUE-MALFORMED:<path>`.
//! 3. **validate** ([`Manifest::validate`]) — is the manifest *loadable*? Missing required fields,
//!    illegal enum values, duplicate ids, defaults outside their range.
//!
//! Decode never stops at the first problem: it records what it can and leaves the rest `None`, so
//! stage 3 reports the omission. An author therefore gets the whole list in one pass, which is the
//! point of validating at discovery rather than at first use.
//!
//! # No I/O, again
//!
//! [`decode`] takes `&str`. Reading the file belongs to the caller (discovery is a control-thread
//! activity in `sparq-app`), so this crate holds no filesystem access at all and cannot violate the
//! `clippy.toml` ban on I/O by accident.

use crate::error::{CodeKind, ValidationError, ValidationReport};
use crate::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
    ValidatedManifest,
};
use crate::toml::{self, Table, Value};

/// The tables a manifest may contain. Anything else is `E-UNKNOWN-KEY`.
pub const TOP_LEVEL: [&str; 13] = [
    "identity",
    "classification",
    "ports",
    "params",
    "data_schemas",
    "voices",
    "state",
    "resources",
    "ui",
    "lifecycle",
    "capabilities",
    "distribution",
    "signature",
];

/// The tables this decoder actually reads in v0. The rest of [`TOP_LEVEL`] is *accepted but not yet
/// modelled*: accepted so a real manifest loads, and published here so the gap is visible to a
/// caller rather than silent. `ui`, `lifecycle`, `capabilities`, `distribution`, `signature` and
/// `data_schemas` are parsed by [`crate::toml`] and checked for unknown keys, but their contents are
/// not yet decoded into types — that lands with the modules that need them (WO-014's panels, the
/// T2/T3 tiers, Perform-mode signature enforcement).
pub const MODELLED: [&str; 7] =
    ["identity", "classification", "ports", "params", "voices", "state", "resources"];

const IDENTITY_KEYS: [&str; 13] = [
    "id",
    "version",
    "host_api",
    "display_name",
    "summary",
    "description",
    "authors",
    "license",
    "repository",
    "docs_url",
    "issues_url",
    "tags",
    "signature",
];
const CLASSIFICATION_KEYS: [&str; 6] = ["category", "top", "kind", "tier", "layer", "stability"];
const PORT_KEYS: [&str; 16] = [
    "id",
    "name",
    "direction",
    "type",
    "required",
    "channel_set",
    "rate",
    "range",
    "event_kinds",
    "data_schema",
    "read_policy",
    "stale_policy",
    "multiplicity",
    "latency_contribution",
    "default_connected",
    "decay_ms",
];
/// The two fields decisions G3 and G4 added. They live here rather than in [`PORT_KEYS`]'s schema
/// order because `manifest-schema.md` §3 lists them after `range`, and this list is only used for
/// unknown-key detection, where order does not matter.
const PORT_KEYS_V2: [&str; 2] = ["cv_reduce", "cv_interp"];
const PARAM_KEYS: [&str; 20] = [
    "id",
    "name",
    "type",
    "unit",
    "min",
    "max",
    "default",
    "steps",
    "curve",
    "smoothing",
    "self_smoothed",
    "mod_depth",
    "automatable",
    "randomisable",
    "lockable",
    "hidden",
    "per_voice",
    "morph",
    "group",
    "aliases",
];
const PARAM_KEYS_V2: [&str; 1] = ["options"];
const STATE_KEYS: [&str; 5] = ["schema_id", "schema_version", "migrations", "size_class", "assets"];
const RESOURCE_KEYS: [&str; 8] = [
    "latency",
    "tail_samples",
    "cpu_class",
    "mem_class",
    "gpu_class",
    "oversampling",
    "requires",
    "internal_rate",
];
const VOICES_KEYS: [&str; 5] =
    ["policy", "per_voice_params", "allocation", "unison", "note_events"];

// Key sets for the sections v0 accepts but does not decode into types. They are here so that a typo
// inside an unmodelled section is still refused: accepting the *section* while ignoring its keys
// would let `colour_klass` through silently, and an invisible failure is the one thing plan §4.3
// forbids. Sourced from `docs/api/manifest-schema.md` §9–§13 and §5.
const UI_KEYS: [&str; 5] = ["panel", "display_slots", "custom_draw", "colour_class", "displays"];
/// `ui.displays[]` entries — the frozen v1.1 shape (ADR-010; manifest-schema §9). Displays are
/// the first `ui` content this decoder validates below the section's own keys: their vocabularies
/// are part of the frozen instrument contract (WO-017), so a misspelled `kind` must fail at
/// discovery with the closed set named, not at the first draw call.
const DISPLAY_KEYS: [&str; 6] = ["id", "kind", "sources", "colormap", "min_size", "lod"];
/// One `ui.displays[].sources` binding: the guest fetches snapshots by (display-id, source-id),
/// and the host resolves EITHER `port` (that port's analysis ring / scalar / data window,
/// sources.wit) OR `stream` (a broker stream binding — a registry id or a `param:` indirection,
/// ADR-011 / WO-020). The two are mutually exclusive; exactly one is required.
const DISPLAY_SOURCE_KEYS: [&str; 3] = ["id", "port", "stream"];
/// The keys a `ui.panel.widgets[].visible_if` predicate carries (v1: one equality on one param —
/// plan D7). Anything richer is a later minor; the shape is pinned here so a typo is a word, not a
/// silently-ignored widget that never shows.
const VISIBLE_IF_KEYS: [&str; 2] = ["param", "equals"];
/// `ui.displays[].kind` — the two host-rendered vocabularies (module-api §10): a 2D display list
/// or a 3D scene descriptor.
const DISPLAY_KINDS: [&str; 2] = ["display_list", "scene"];
/// `ui.displays[].lod` — the display's LOD policy; `auto` follows the host's zoom/breakpoint
/// rules, the others pin a level.
const DISPLAY_LODS: [&str; 4] = ["auto", "full", "reduced", "minimal"];
const LIFECYCLE_KEYS: [&str; 5] =
    ["thread_affinity", "init_cost", "supports_hot_reload", "reset_semantics", "suspendable"];
const CAPABILITY_KEYS: [&str; 9] = [
    "fs_read",
    "fs_write",
    "network",
    "device",
    "gpu",
    "process_spawn",
    "max_fuel",
    "max_memory_mb",
    "max_cpu_ms_per_block",
];
const DISTRIBUTION_KEYS: [&str; 7] = [
    "package_format",
    "entrypoint",
    "platforms",
    "arch",
    "min_host_version",
    "registry_namespace",
    "channel",
];
const SIGNATURE_KEYS: [&str; 5] = ["key_id", "algorithm", "value", "signed_at", "attestation_url"];
const DATA_SCHEMA_KEYS: [&str; 5] = ["id", "version", "rate_class", "clock_domain", "channels"];
const DATA_CHANNEL_KEYS: [&str; 5] = ["id", "dtype", "unit", "range", "interpolation"];

/// Parses and validates a manifest from its TOML text.
///
/// # Errors
/// A [`ValidationReport`] holding every failure — syntax, type and semantic — at once.
pub fn decode(text: &str) -> Result<ValidatedManifest, ValidationReport> {
    let table = match toml::parse(text) {
        Ok(t) => t,
        Err(e) => {
            let mut r = ValidationReport::new();
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    "sparqmod.toml",
                    "fix the TOML syntax; the parser names the line",
                )
                .with_found(e.to_string()),
            );
            return Err(r);
        },
    };
    let mut r = ValidationReport::new();
    let m = manifest_from(&table, &mut r);
    // Validate even if decoding recorded problems: a manifest can be both malformed and missing
    // fields, and the author should see all of it in one pass.
    match m.validate() {
        Ok(v) if r.is_empty() => Ok(v),
        Ok(_) => Err(r),
        Err(vr) => {
            // Decode failures first, then semantic ones: a report is read top-down, and "this is
            // not the type the schema declares" is the thing to fix before "this field is missing".
            for e in vr.into_errors() {
                r.push(e);
            }
            Err(r)
        },
    }
}

/// Decodes a manifest that has already been parsed, for callers that read many files.
#[must_use]
pub fn manifest_from(table: &Table, r: &mut ValidationReport) -> Manifest {
    check_keys(table, &TOP_LEVEL, "", r);
    check_unmodelled(table, r);
    Manifest {
        identity: identity(table, r),
        classification: classification(table, r),
        ports: ports(table, r),
        params: params(table, r),
        state: state(table, r),
        resources: resources(table, r),
        voices_policy: voices_policy(table, r),
    }
}

/// Key-checks the sections v0 accepts but does not decode. A typo in one of these is a typo, and
/// silently ignoring it would let a module load without the panel, capability or signature its
/// author meant to declare.
fn check_unmodelled(root: &Table, r: &mut ValidationReport) {
    for (name, allowed) in [
        ("ui", UI_KEYS.as_slice()),
        ("lifecycle", LIFECYCLE_KEYS.as_slice()),
        ("capabilities", CAPABILITY_KEYS.as_slice()),
        ("distribution", DISTRIBUTION_KEYS.as_slice()),
        ("signature", SIGNATURE_KEYS.as_slice()),
    ] {
        if let Some(t) = sub_table(root, name, name, r) {
            check_keys(t, allowed, name, r);
            if name == "ui" {
                // v1.1: `displays[]` entries are content-checked against the frozen shape
                // (ADR-010 / WO-017). The rest of `ui` stays key-checked-only until its typed
                // decode lands with the painters that need it (WO-014) — EXCEPT the two shapes
                // WO-020 specifies and needs now: display `stream` bindings (ADR-011) and the
                // panel widgets' `visible_if` predicate (plan D7).
                check_displays(t, r);
                check_widget_visible_ifs(t, r);
            }
        }
    }
    for (n, t) in root.tables("data_schemas").unwrap_or_default().iter().enumerate() {
        let base = format!("data_schemas[{n}]");
        check_keys(t, &DATA_SCHEMA_KEYS, &base, r);
        for (c, ch) in t.tables("channels").unwrap_or_default().iter().enumerate() {
            check_keys(ch, &DATA_CHANNEL_KEYS, &format!("{base}.channels[{c}]"), r);
        }
    }
}

/// Content-checks `ui.displays[]` against the frozen v1.1 shape (manifest-schema §9; ADR-010).
///
/// What is checked here, and what is deliberately NOT: the closed vocabularies (`kind`, `lod`),
/// the required unique `id` (the draw/sources ABI is keyed by it), the `min_size` shape and the
/// source-binding shape are discovery-time properties of the MANIFEST, so they fail here with the
/// same report discipline as every other field. The emitted-list vocabularies (the primitives
/// inside a display list / scene descriptor) and the literal-appearance prohibition are properties
/// of what the module EMITS at runtime — those gates live in `sparq mod validate` (WO-018:
/// `E-DISPLAY-PRIMITIVE-UNKNOWN`, `E-LITERAL-APPEARANCE`), not in the manifest decoder. The
/// `colormap` token is prefix-checked here; the full domain check needs the live token bundle and
/// is the validator's job too.
fn check_displays(ui: &Table, r: &mut ValidationReport) {
    let Some(items) = ui.tables("displays") else {
        if let Some(v) = ui.get("displays") {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    "ui.displays",
                    "displays is a list of tables — write [[ui.displays]] entries",
                )
                .with_found(v.type_name()),
            );
        }
        return;
    };
    let mut seen_ids: Vec<&str> = Vec::new();
    for (n, d) in items.iter().enumerate() {
        let base = format!("ui.displays[{n}]");
        check_keys(d, &DISPLAY_KEYS, &base, r);
        // id: required, unique within the module.
        match d.get("id").and_then(Value::as_str) {
            None => r.push(ValidationError::new(
                CodeKind::KeyMissing,
                format!("{base}.id"),
                "name the display — draw calls and source bindings are keyed by this id",
            )),
            Some(id) => {
                if seen_ids.contains(&id) {
                    r.push(
                        ValidationError::new(
                            CodeKind::IdDup,
                            format!("{base}.id"),
                            "two displays share an id — rename one",
                        )
                        .with_found(id),
                    );
                }
                seen_ids.push(id);
            },
        }
        // kind: required, one of the two host-rendered vocabularies.
        match d.get("kind").and_then(Value::as_str) {
            None => r.push(ValidationError::new(
                CodeKind::KeyMissing,
                format!("{base}.kind"),
                "declare which vocabulary this display emits: display_list (2D) or scene (3D)",
            )),
            Some(k) if !DISPLAY_KINDS.contains(&k) => r.push(
                ValidationError::new(
                    CodeKind::EnumUnknown,
                    format!("{base}.kind"),
                    "use one of the two display vocabularies (module-api §10)",
                )
                .with_found(k)
                .with_allowed(DISPLAY_KINDS.join(" ")),
            ),
            Some(_) => {},
        }
        // lod: optional (absent = auto), closed policy vocabulary.
        if let Some(l) = d.get("lod").and_then(Value::as_str) {
            if !DISPLAY_LODS.contains(&l) {
                r.push(
                    ValidationError::new(
                        CodeKind::EnumUnknown,
                        format!("{base}.lod"),
                        "use one of the LOD policies",
                    )
                    .with_found(l)
                    .with_allowed(DISPLAY_LODS.join(" ")),
                );
            }
        }
        // min_size: two positive integers [width, height], logical px.
        if let Some(v) = d.get("min_size") {
            let ok = v.as_array().is_some_and(|a| {
                a.len() == 2 && a.iter().all(|x| x.as_i64().is_some_and(|i| i > 0))
            });
            if !ok {
                r.push(
                    ValidationError::new(
                        CodeKind::ValueMalformed,
                        format!("{base}.min_size"),
                        "two positive integers [width, height], in logical px",
                    )
                    .with_found(v.type_name()),
                );
            }
        }
        // colormap: a colormap token id (prefix rule; the full domain is the token bundle's and
        // belongs to `sparq mod validate`, WO-018).
        if let Some(c) = d.get("colormap").and_then(Value::as_str) {
            if !c.starts_with("colormap.") {
                r.push(
                    ValidationError::new(
                        CodeKind::ValueMalformed,
                        format!("{base}.colormap"),
                        "name a colormap token id, e.g. colormap.spectrum.sonogram",
                    )
                    .with_found(c),
                );
            }
        }
        // sources: optional list of {id, port|stream} bindings; ids unique within this display.
        if let Some(sources) = d.tables("sources") {
            let mut seen: Vec<&str> = Vec::new();
            for (s, src) in sources.iter().enumerate() {
                let sb = format!("{base}.sources[{s}]");
                check_keys(src, &DISPLAY_SOURCE_KEYS, &sb, r);
                match src.get("id").and_then(Value::as_str) {
                    None => r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{sb}.id"),
                        "name this binding — the guest fetches snapshots by (display-id, source-id)",
                    )),
                    Some(id) => {
                        if seen.contains(&id) {
                            r.push(
                                ValidationError::new(
                                    CodeKind::IdDup,
                                    format!("{sb}.id"),
                                    "two bindings on this display share an id — rename one",
                                )
                                .with_found(id),
                            );
                        }
                        seen.push(id);
                    }
                }
                // Exactly one of `port` or `stream` (ADR-011 / WO-020 plan D4): a binding reads
                // EITHER a port/analysis source OR a broker stream, never both, never neither.
                match (src.get("port").and_then(Value::as_str), src.get("stream")) {
                    (Some(_), Some(_)) => r.push(
                        ValidationError::new(
                            CodeKind::CrossField,
                            sb.clone(),
                            "a source binding reads EITHER a port/analysis source OR a broker \
                             stream, not both (ADR-011) — remove one",
                        )
                        .with_found("both `port` and `stream`")
                        .with_allowed(
                            "port = \"<port/analysis id>\"  |  stream = \"<registry-id>\" or \
                             \"param:<param-id>\"",
                        ),
                    ),
                    (None, None) => r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{sb}.port"),
                        "say what this binding reads: `port = \"<port/analysis id>\"` or \
                         `stream = \"<registry-id>\" | \"param:<param-id>\"` (ADR-011)",
                    )),
                    (Some(_), None) => {}, // a port binding — the port-id check is the validator's
                    (None, Some(sv)) => check_stream_binding(sv, &sb, r),
                }
            }
        } else if let Some(v) = d.get("sources") {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    format!("{base}.sources"),
                    "sources is a list of {id, port|stream} tables",
                )
                .with_found(v.type_name()),
            );
        }
    }
}

/// Shape-checks a `ui.displays[].sources[].stream` binding (ADR-011 / WO-020 plan D4). The decoder
/// checks the SHAPE only — a non-empty string, either a bare registry id or a `param:<id>`
/// indirection with a non-empty param id. Whether the registry id EXISTS in `streams.toml`, and
/// whether a `param:` target exists / is an enum / has options inside the registry, is resolved by
/// `sparq mod validate` (it owns the registry and the decoded params — sparq-host-wasm), because the
/// contract crate must not depend on the broker crate.
fn check_stream_binding(v: &Value, sb: &str, r: &mut ValidationReport) {
    let Some(s) = v.as_str() else {
        r.push(
            ValidationError::new(
                CodeKind::ValueMalformed,
                format!("{sb}.stream"),
                "stream is a string: a registry id (`swpc.kp`) or a param indirection \
                 (`param:cell_01`) — ADR-011",
            )
            .with_found(v.type_name()),
        );
        return;
    };
    if s.is_empty() {
        r.push(ValidationError::new(
            CodeKind::ValueMalformed,
            format!("{sb}.stream"),
            "stream must name a registry id or `param:<id>`; an empty string binds nothing",
        ));
        return;
    }
    if let Some(param) = s.strip_prefix("param:") {
        if param.trim().is_empty() {
            r.push(ValidationError::new(
                CodeKind::ValueMalformed,
                format!("{sb}.stream"),
                "`param:` needs a param id after the colon (e.g. `param:cell_01`) — the host \
                 resolves the indirection to whatever enum stream that param selects",
            ));
        }
    }
}

/// Shape-checks `ui.panel.widgets[].visible_if` (WO-020 plan D7 — the semantics this specifies: v1
/// is ONE equality predicate on ONE param value). The widget vocabulary itself stays key-checked-only
/// until WO-014 types the panel; this walks just far enough to pin `visible_if`'s shape, so a widget
/// that means to be conditional cannot silently never show. The referenced param's EXISTENCE is the
/// validator's cross-check (sparq-host-wasm), not the decoder's.
fn check_widget_visible_ifs(ui: &Table, r: &mut ValidationReport) {
    let Some(panel) = ui.get("panel").and_then(Value::as_table) else {
        return;
    };
    for (w, widget) in panel.tables("widgets").unwrap_or_default().iter().enumerate() {
        let Some(vif) = widget.get("visible_if") else {
            continue;
        };
        let wb = format!("ui.panel.widgets[{w}].visible_if");
        let Some(t) = vif.as_table() else {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    wb.clone(),
                    "visible_if is a table { param = \"<id>\", equals = <value> } — one equality \
                     predicate on one param (plan D7)",
                )
                .with_found(vif.type_name()),
            );
            continue;
        };
        check_keys(t, &VISIBLE_IF_KEYS, &wb, r);
        match t.get("param").and_then(Value::as_str) {
            Some(p) if !p.is_empty() => {},
            _ => r.push(ValidationError::new(
                CodeKind::KeyMissing,
                format!("{wb}.param"),
                "name the param whose value gates this widget's visibility",
            )),
        }
        if t.get("equals").is_none() {
            r.push(ValidationError::new(
                CodeKind::KeyMissing,
                format!("{wb}.equals"),
                "the value the param must equal for the widget to show (an enum option index, a \
                 float within 1e-6, or a bool as 0.0/1.0 — plan D7)",
            ));
        }
    }
}

/// Reports `E-UNKNOWN-KEY` for every key outside `allowed`.
fn check_keys(table: &Table, allowed: &[&str], path: &str, r: &mut ValidationReport) {
    for key in table.keys() {
        if !allowed.contains(&key) {
            let p = if path.is_empty() { key.to_string() } else { format!("{path}.{key}") };
            r.push(
                ValidationError::new(
                    CodeKind::UnknownKey,
                    p,
                    "no such key in the manifest schema; a typo here would otherwise be ignored silently",
                )
                .with_found(key)
                .with_allowed(allowed.join(" ")),
            );
        }
    }
}

/// Reports `E-VALUE-MALFORMED:<path>` and yields `None`.
fn wrong_type(
    t: &Table,
    key: &str,
    path: &str,
    want: &str,
    r: &mut ValidationReport,
) -> Option<String> {
    let found = t.get(key).map(Value::type_name).unwrap_or("absent");
    let line = t.line_of(key);
    r.push(
        ValidationError::new(
            CodeKind::ValueMalformed,
            path.to_string(),
            format!("this field must be {want}"),
        )
        .with_found(format!("{found}{}", line.map_or(String::new(), |l| format!(" on line {l}")))),
    );
    None
}

fn opt_str(t: &Table, key: &str, path: &str, r: &mut ValidationReport) -> Option<String> {
    match t.get(key) {
        None => None,
        Some(Value::Str(s)) => Some(s.clone()),
        Some(_) => wrong_type(t, key, path, "a string", r).map(|_| String::new()),
    }
}

fn opt_bool(t: &Table, key: &str, path: &str, r: &mut ValidationReport) -> Option<bool> {
    match t.get(key) {
        None => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => {
            wrong_type(t, key, path, "a boolean", r);
            None
        },
    }
}

fn opt_u32(t: &Table, key: &str, path: &str, r: &mut ValidationReport) -> Option<u32> {
    match t.get(key) {
        None => None,
        Some(v) => match v.as_u32() {
            Some(n) => Some(n),
            None => {
                wrong_type(t, key, path, "a non-negative integer", r);
                None
            },
        },
    }
}

fn opt_f64(t: &Table, key: &str, path: &str, r: &mut ValidationReport) -> Option<f64> {
    match t.get(key) {
        None => None,
        Some(v) => match v.as_f64() {
            Some(x) => Some(x),
            None => {
                wrong_type(t, key, path, "a number", r);
                None
            },
        },
    }
}

fn str_vec(t: &Table, key: &str, path: &str, r: &mut ValidationReport) -> Vec<String> {
    match t.get(key) {
        None => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| match v {
                Value::Str(s) => Some(s.clone()),
                other => {
                    r.push(
                        ValidationError::new(
                            CodeKind::ValueMalformed,
                            path.to_string(),
                            "every element of this array must be a string",
                        )
                        .with_found(other.type_name()),
                    );
                    None
                },
            })
            .collect(),
        Some(_) => {
            wrong_type(t, key, path, "an array of strings", r);
            Vec::new()
        },
    }
}

fn sub_table<'a>(
    t: &'a Table,
    key: &str,
    path: &str,
    r: &mut ValidationReport,
) -> Option<&'a Table> {
    match t.get(key) {
        None => None,
        Some(Value::Table(st)) => Some(st),
        Some(other) => {
            r.push(
                ValidationError::new(CodeKind::ValueMalformed, path, "this must be a table")
                    .with_found(other.type_name()),
            );
            None
        },
    }
}

fn identity(root: &Table, r: &mut ValidationReport) -> Identity {
    let Some(t) = sub_table(root, "identity", "identity", r) else {
        return Identity::default();
    };
    check_keys(t, &IDENTITY_KEYS, "identity", r);
    let host = sub_table(t, "host_api", "identity.host_api", r);
    Identity {
        id: opt_str(t, "id", "identity.id", r),
        version: opt_str(t, "version", "identity.version", r),
        host_api_min: host.and_then(|h| opt_u32(h, "min", "identity.host_api.min", r)),
        host_api_max: host.and_then(|h| opt_u32(h, "max", "identity.host_api.max", r)),
        display_name: opt_str(t, "display_name", "identity.display_name", r),
        summary: opt_str(t, "summary", "identity.summary", r),
        authors: str_vec(t, "authors", "identity.authors", r),
        license: opt_str(t, "license", "identity.license", r),
    }
}

fn classification(root: &Table, r: &mut ValidationReport) -> Classification {
    let Some(t) = sub_table(root, "classification", "classification", r) else {
        return Classification::default();
    };
    check_keys(t, &CLASSIFICATION_KEYS, "classification", r);
    Classification {
        category: opt_str(t, "category", "classification.category", r),
        top: opt_str(t, "top", "classification.top", r),
        kind: opt_str(t, "kind", "classification.kind", r),
        tier: opt_str(t, "tier", "classification.tier", r),
        layer: opt_str(t, "layer", "classification.layer", r),
        stability: opt_str(t, "stability", "classification.stability", r),
    }
}

fn ports(root: &Table, r: &mut ValidationReport) -> Vec<PortSpec> {
    let Some(items) = root.tables("ports") else {
        if let Some(v) = root.get("ports") {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    "ports",
                    "ports must be an array of tables ([[ports]])",
                )
                .with_found(v.type_name()),
            );
        }
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .map(|(n, t)| {
            let base = format!("ports[{n}]");
            let mut allowed: Vec<&str> = PORT_KEYS.to_vec();
            allowed.extend_from_slice(&PORT_KEYS_V2);
            check_keys(t, &allowed, &base, r);
            PortSpec {
                id: opt_str(t, "id", &format!("{base}.id"), r),
                name: opt_str(t, "name", &format!("{base}.name"), r),
                direction: opt_str(t, "direction", &format!("{base}.direction"), r),
                port_type: opt_str(t, "type", &format!("{base}.type"), r),
                required: opt_bool(t, "required", &format!("{base}.required"), r),
                channel_set: opt_str(t, "channel_set", &format!("{base}.channel_set"), r),
                rate: opt_str(t, "rate", &format!("{base}.rate"), r),
                range: opt_str(t, "range", &format!("{base}.range"), r),
                multiplicity: opt_str(t, "multiplicity", &format!("{base}.multiplicity"), r),
                latency_contribution: opt_u32(
                    t,
                    "latency_contribution",
                    &format!("{base}.latency_contribution"),
                    r,
                ),
                cv_reduce: opt_str(t, "cv_reduce", &format!("{base}.cv_reduce"), r),
                cv_interp: opt_str(t, "cv_interp", &format!("{base}.cv_interp"), r),
                event_kinds: str_vec(t, "event_kinds", &format!("{base}.event_kinds"), r),
            }
        })
        .collect()
}

fn params(root: &Table, r: &mut ValidationReport) -> Vec<ParamSpec> {
    let Some(items) = root.tables("params") else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .map(|(n, t)| {
            let base = format!("params[{n}]");
            let mut allowed: Vec<&str> = PARAM_KEYS.to_vec();
            allowed.extend_from_slice(&PARAM_KEYS_V2);
            check_keys(t, &allowed, &base, r);
            ParamSpec {
                id: opt_str(t, "id", &format!("{base}.id"), r),
                name: opt_str(t, "name", &format!("{base}.name"), r),
                kind: opt_str(t, "type", &format!("{base}.type"), r),
                unit: opt_str(t, "unit", &format!("{base}.unit"), r),
                min: opt_f64(t, "min", &format!("{base}.min"), r),
                max: opt_f64(t, "max", &format!("{base}.max"), r),
                default: opt_f64(t, "default", &format!("{base}.default"), r),
                per_voice: opt_bool(t, "per_voice", &format!("{base}.per_voice"), r),
                morph: opt_str(t, "morph", &format!("{base}.morph"), r),
                options: param_options(t, &base, r),
            }
        })
        .collect()
}

/// Parses an enum param's `options[]` (an array of inline tables with `value` + `label`). A
/// malformed entry is a words refusal naming the index — an option the picker cannot label is an
/// option the operator cannot choose.
fn param_options(
    t: &Table,
    base: &str,
    r: &mut ValidationReport,
) -> Vec<crate::manifest::ParamOption> {
    let Some(items) = t.get("options").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let ob = format!("{base}.options[{i}]");
            let ot = match o.as_table() {
                Some(ot) => ot,
                None => {
                    r.push(
                        ValidationError::new(
                            CodeKind::ValueMalformed,
                            ob.clone(),
                            "an option is an inline table { value = \"…\", label = \"…\" }",
                        )
                        .with_found(o.type_name()),
                    );
                    return crate::manifest::ParamOption::default();
                },
            };
            crate::manifest::ParamOption {
                value: opt_str(ot, "value", &format!("{ob}.value"), r)
                    .unwrap_or_default()
                    .to_string(),
                label: opt_str(ot, "label", &format!("{ob}.label"), r)
                    .unwrap_or_default()
                    .to_string(),
            }
        })
        .collect()
}

fn state(root: &Table, r: &mut ValidationReport) -> StateDecl {
    let Some(t) = sub_table(root, "state", "state", r) else {
        return StateDecl::default();
    };
    check_keys(t, &STATE_KEYS, "state", r);
    StateDecl {
        schema_id: opt_str(t, "schema_id", "state.schema_id", r),
        schema_version: opt_u32(t, "schema_version", "state.schema_version", r),
    }
}

fn resources(root: &Table, r: &mut ValidationReport) -> ResourceDecl {
    let Some(t) = sub_table(root, "resources", "resources", r) else {
        return ResourceDecl::default();
    };
    check_keys(t, &RESOURCE_KEYS, "resources", r);
    check_gpu_class(t, r);
    // `latency` is `int | "param:<id>"` in the schema: a constant, or a promise that it follows a
    // parameter (convolution, granular). Both forms are accepted; neither may be absent.
    let (latency_samples, latency_param) = match t.get("latency") {
        None => (None, None),
        Some(Value::Int(_)) => (opt_u32(t, "latency", "resources.latency", r), None),
        Some(Value::Str(s)) if s.starts_with("param:") => (None, Some(s.clone())),
        Some(other) => {
            r.push(
                ValidationError::new(
                    CodeKind::ValueMalformed,
                    "resources.latency",
                    "latency is an integer number of samples, or the string `param:<id>`",
                )
                .with_found(other.type_name()),
            );
            (None, None)
        },
    };
    ResourceDecl {
        latency_samples,
        latency_param,
        tail_samples: opt_u32(t, "tail_samples", "resources.tail_samples", r),
        cpu_class: opt_str(t, "cpu_class", "resources.cpu_class", r),
        oversampling: opt_str(t, "oversampling", "resources.oversampling", r),
        requires: str_vec(t, "requires", "resources.requires", r),
    }
}

/// Domain-checks `resources.gpu_class` at decode — the `displays` precedent, deliberately NOT
/// modelled into [`ResourceDecl`]. The class is a closed v1.1 vocabulary (contract freeze,
/// WO-017), but what the classes MEAN — the vertex/instance/heat-cell ceiling table — lives in
/// `sparq-host-wasm::ceilings` (WO-018), and this crate does not depend on that one. A misspelled
/// class must fail at discovery with the closed set named, not at the first draw call; the
/// ceiling arithmetic stays on the host side. `cpu_class`/`mem_class` are diagnostics-only
/// strings today (schema §8: "a module that lies shows up in diagnostics") and stay unmodelled
/// until a scheduler consumes them.
fn check_gpu_class(t: &Table, r: &mut ValidationReport) {
    let Some(v) = t.get("gpu_class") else { return };
    match v.as_str() {
        Some(s) if crate::manifest::GPU_CLASSES.contains(&s) => {},
        Some(s) => r.push(
            ValidationError::new(
                CodeKind::EnumUnknown,
                "resources.gpu_class",
                "use one of the renderer budget classes (manifest-schema §8) — the ceiling table is sparq-host-wasm::ceilings",
            )
            .with_found(s)
            .with_allowed(crate::manifest::GPU_CLASSES.join(" ")),
        ),
        None => r.push(
            ValidationError::new(
                CodeKind::ValueMalformed,
                "resources.gpu_class",
                "gpu_class is a string from the closed vocabulary",
            )
            .with_found(v.type_name()),
        ),
    }
}

fn voices_policy(root: &Table, r: &mut ValidationReport) -> Option<String> {
    let t = sub_table(root, "voices", "voices", r)?;
    check_keys(t, &VOICES_KEYS, "voices", r);
    opt_str(t, "policy", "voices.policy", r)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::error::CodeKind;

    /// A complete, valid `sparqmod.toml` for `util/gain`, as an author would write it.
    const GAIN: &str = r#"
[identity]
id = "sparq/util/gain"
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "Gain"
summary = "Gain and trim"
authors = ["sparq"]
license = "MIT"

[classification]
category = "utility/gain"
top = "util"
kind = "processor"
tier = "t1"
stability = "stable"

[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"
channel_set = "stereo"

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "stereo"

[[params]]
id = "gain"
name = "Gain"
type = "float"
unit = "ratio"
min = 0.0
max = 2.0
default = 1.0

[state]
schema_id = "sparq/util/gain/state"
schema_version = 1

[resources]
latency = 0
cpu_class = "trivial"
"#;

    /// The same manifest with a parametric latency, an unmodelled section, and a block-rate cv port
    /// carrying both of the policies decisions G3 and G4 added.
    const RICH: &str = r#"
[identity]
id = "sparq/spc/convolve"
version = "1.2.0"
host_api = { min = 1, max = 2 }
display_name = "Partitioned convolution"
summary = "Convolution reverb with a declared, parameter-dependent latency"
authors = ["sparq", "a collaborator"]
license = "MIT"
tags = ["reverb", "spatial"]

[classification]
category = "spectral/convolution"
top = "fx"
kind = "processor"
tier = "t1"
stability = "experimental"

[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"
channel_set = "variable"

[[ports]]
id = "mod"
name = "Mix"
direction = "in"
type = "cv"
rate = "block"
range = "unipolar"
cv_reduce = "mean"

[[params]]
id = "size"
name = "Size"
type = "float"
unit = "ms"
min = 1.0
max = 8000.0
default = 1200.0
smoothing = "one_pole:20"
morph = "continuous"

[state]
schema_id = "sparq/spc/convolve/state"
schema_version = 3
size_class = "large"

[resources]
latency = "param:size"
tail_samples = 96000
cpu_class = "very_heavy"
requires = ["fft"]
oversampling = "none"

[voices]
policy = "none"

[ui]
colour_class = "fx"

[lifecycle]
supports_hot_reload = true
"#;

    #[test]
    fn a_real_manifest_decodes_and_validates() {
        let v = decode(GAIN).unwrap_or_else(|r| panic!("{r}"));
        assert_eq!(v.id(), "sparq/util/gain");
        assert_eq!(v.ports().len(), 2);
        assert_eq!(v.latency_samples(), 0);
        assert!(!v.latency_is_parametric());
    }

    #[test]
    fn a_parametric_latency_decodes_as_parametric() {
        let v = decode(RICH).unwrap_or_else(|r| panic!("{r}"));
        assert!(v.latency_is_parametric(), "latency = \"param:size\" is the parametric form");
        assert_eq!(v.manifest().resources.latency_param.as_deref(), Some("param:size"));
        assert_eq!(v.latency_samples(), 0, "no static sample count to report");
        // A parametric latency cannot be checked against a measurement statically.
        assert!(v.check_declared_latency(4096).is_none());
        assert_eq!(v.ports().len(), 2);
        assert!(v.ports()[0].channel_set_variable, "`variable` resolves at prepare");
    }

    #[test]
    fn unmodelled_sections_are_accepted_and_their_keys_are_still_checked() {
        // `ui`, `lifecycle` and `tags` are not decoded into types in v0, but they must not be
        // rejected either — a real manifest has them.
        assert!(decode(RICH).is_ok(), "unmodelled sections must not fail a valid manifest");
        let broken = RICH.replace("[ui]\ncolour_class = \"fx\"", "[ui]\ncolour_klass = \"fx\"");
        let r = decode(&broken).unwrap_err();
        assert!(
            r.has(CodeKind::UnknownKey),
            "an unknown key inside an unmodelled section is still a typo: {r}"
        );
    }

    #[test]
    fn an_unknown_top_level_key_is_a_typo_and_is_reported() {
        let text = format!("{GAIN}\n[identitiy]\nid = \"sparq/util/other\"\n");
        let r = decode(&text).unwrap_err();
        assert!(r.has(CodeKind::UnknownKey), "{r}");
        let e = r.iter().find(|e| e.kind == CodeKind::UnknownKey).unwrap();
        assert!(e.to_string().contains("identitiy"), "{e}");
        assert!(
            e.to_string().contains("identity"),
            "the message lists the legal keys, so the typo is obvious: {e}"
        );
    }

    #[test]
    fn a_wrong_type_is_reported_with_the_line_it_was_on() {
        let text = GAIN.replace("schema_version = 1", "schema_version = \"one\"");
        let r = decode(&text).unwrap_err();
        let e = r.first_at("state.schema_version").unwrap();
        assert_eq!(e.kind, CodeKind::ValueMalformed);
        assert!(e.to_string().contains("string"), "names the type it found: {e}");
        assert!(
            e.to_string().contains("line"),
            "and the line, which is what makes it actionable: {e}"
        );
        assert!(e.to_string().contains("non-negative integer"), "and what was wanted: {e}");
    }

    #[test]
    fn all_five_acceptance_rejections_are_reachable_from_real_toml_text() {
        // The criterion is about manifests authors write, not about Rust structs, so it is tested
        // from text.
        let cases: [(&str, &str, CodeKind); 5] = [
            ("type = \"audio\"", "type = \"midi\"", CodeKind::PortTypeUnknown),
            ("id = \"out\"", "id = \"in\"", CodeKind::IdDup),
            (
                "type = \"audio\"\nchannel_set = \"stereo\"",
                "channel_set = \"stereo\"",
                CodeKind::KeyMissing,
            ),
            ("latency = 0", "tail_samples = 0", CodeKind::KeyMissing),
            ("unit = \"ratio\"", "unit = \"dBFS\"", CodeKind::UnitUnknown),
        ];
        for (from, to, kind) in cases {
            let text = GAIN.replace(from, to);
            assert_ne!(text, GAIN, "the substitution `{from}` did not apply");
            let r = decode(&text).unwrap_err();
            assert!(r.has(kind), "replacing `{from}` with `{to}` should give {kind:?}: {r}");
        }
    }

    #[test]
    fn broken_toml_reports_the_line_and_still_counts_as_a_validation_failure() {
        let r = decode("[identity\nid = 1\n").unwrap_err();
        let e = r.iter().next().unwrap();
        assert_eq!(e.kind, CodeKind::ValueMalformed);
        assert_eq!(e.path, "sparqmod.toml");
        assert!(e.to_string().contains("line 1"), "{e}");
    }

    #[test]
    fn a_manifest_missing_everything_reports_the_whole_list_at_once() {
        let r = decode("[identity]\n").unwrap_err();
        assert!(
            r.len() >= 15,
            "an author should see every omission in one pass, got {}: {r}",
            r.len()
        );
        assert!(r.has(CodeKind::StateSchemaMissing), "{r}");
    }

    #[test]
    fn every_modelled_table_is_a_legal_top_level_key() {
        // The invariant that keeps `MODELLED` honest: a decoder cannot read a section the schema
        // does not allow, or it would accept a key that `check_keys` simultaneously rejects.
        for m in MODELLED {
            assert!(TOP_LEVEL.contains(&m), "{m} is decoded but not allowed at the top level");
        }
        assert!(
            MODELLED.len() < TOP_LEVEL.len(),
            "if these are equal, this list has stopped meaning anything"
        );
    }

    #[test]
    fn ports_and_params_must_be_arrays_of_tables() {
        // Both [[ports]] blocks have to go: replacing only the first leaves a table and an array
        // with the same name, which the parser refuses as a conflict — a different error from the
        // one this test is about, and the reason its first version failed.
        let text = GAIN.replace(
            "[[ports]]\nid = \"in\"\nname = \"Input\"\ndirection = \"in\"\ntype = \"audio\"\nchannel_set = \"stereo\"\n\n[[ports]]\nid = \"out\"\nname = \"Output\"\ndirection = \"out\"\ntype = \"audio\"\nchannel_set = \"stereo\"\n",
            "[ports]\nid = \"in\"\nname = \"Input\"\ndirection = \"in\"\ntype = \"audio\"\nchannel_set = \"stereo\"\n",
        );
        assert_ne!(text, GAIN, "the substitution did not apply");
        let r = decode(&text).unwrap_err();
        assert!(r.has(CodeKind::ValueMalformed), "a single table where an array was declared: {r}");
        assert!(r.first_at("ports").unwrap().to_string().contains("[[ports]]"), "{r}");
    }

    /// The skeleton package's manifest, byte-pinned into this test at compile time.
    const TEMPLATE: &str = include_str!("../../../reference/instrument-template/sparqmod.toml");

    #[test]
    fn the_reference_instrument_template_validates_as_is() {
        // WO-017 acceptance criterion: "The skeleton template's manifest passes schema validation
        // as-is." Not a paraphrase of the template — the checked-in file itself, so the template
        // and the validator can never drift apart silently. What it exercises end to end:
        // classification.layer = instrument with tier = t2, the [[ui.displays]] frozen shape,
        // and the capabilities/distribution keys an instrument package declares.
        let v = match decode(TEMPLATE) {
            Ok(v) => v,
            Err(r) => panic!("the reference template no longer validates:\n{r}"),
        };
        assert_eq!(v.id(), "you/syn/template");
        assert_eq!(v.layer(), crate::manifest::Layer::Instrument);
    }

    #[test]
    fn gpu_class_is_domain_checked_at_decode() {
        // WO-018: the renderer budget class is a closed v1.1 vocabulary. The check lives at
        // decode (the `displays` precedent) because the ceiling TABLE lives in
        // sparq-host-wasm::ceilings and this crate must not depend on it — but a misspelled
        // class still has to fail at discovery, with the closed set named in the fix.
        fn with_gpu(v: &str) -> String {
            GAIN.replace(
                "cpu_class = \"trivial\"",
                &format!("cpu_class = \"trivial\"\ngpu_class = {v}"),
            )
        }
        for ok in ["\"none\"", "\"light\"", "\"medium\"", "\"heavy\"", "\"very_heavy\""] {
            let text = with_gpu(ok);
            decode(&text).unwrap_or_else(|r| panic!("gpu_class = {ok} should decode: {r}"));
        }
        let bad = with_gpu("\"ultra\"");
        let r = decode(&bad).unwrap_err();
        let e = r.first_at("resources.gpu_class").unwrap();
        assert_eq!(e.code(), "E-ENUM-UNKNOWN:resources.gpu_class");
        assert!(e.to_string().contains("none light medium heavy very_heavy"), "{e}");
        let wrong_type = with_gpu("3");
        let r = decode(&wrong_type).unwrap_err();
        assert_eq!(
            r.first_at("resources.gpu_class").unwrap().code(),
            "E-VALUE-MALFORMED:resources.gpu_class"
        );
    }

    #[test]
    fn displays_entries_are_content_checked_against_the_frozen_shape() {
        // The frozen v1.1 shape (manifest-schema §9): {id, kind, sources[]{id, port}, colormap?,
        // min_size, lod}. Each break below must produce ONE actionable line naming the field.
        const DISPLAY: &str = "\n[ui]\ncolour_class = \"util\"\n\n[[ui.displays]]\nid = \"view\"\nkind = \"display_list\"\nmin_size = [160, 96]\nlod = \"auto\"\n\n[[ui.displays.sources]]\nid = \"scope\"\nport = \"out\"\n";
        let ok = format!("{GAIN}{DISPLAY}");
        let v = decode(&ok).unwrap_or_else(|r| panic!("the legal display declaration failed: {r}"));
        assert_eq!(v.layer(), crate::manifest::Layer::Backbone); // GAIN declares none

        // A kind outside the two vocabularies names the closed set.
        let r = decode(&ok.replace("kind = \"display_list\"", "kind = \"pixels\"")).unwrap_err();
        let e = r.first_at("ui.displays[0].kind").unwrap();
        assert_eq!(e.code(), "E-ENUM-UNKNOWN:ui.displays[0].kind");
        assert!(e.to_string().contains("display_list scene"), "{e}");

        // A missing id is refused: the draw/sources ABI is keyed by it.
        let r = decode(&ok.replace("id = \"view\"\n", "")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].id").unwrap().code(),
            "E-KEY-MISSING:ui.displays[0].id"
        );

        // Two displays sharing an id.
        let r = decode(&format!("{ok}\n[[ui.displays]]\nid = \"view\"\nkind = \"scene\"\n"))
            .unwrap_err();
        assert!(r.has(CodeKind::IdDup), "{r}");

        // An LOD outside the policy vocabulary.
        let r = decode(&ok.replace("lod = \"auto\"", "lod = \"sometimes\"")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].lod").unwrap().code(),
            "E-ENUM-UNKNOWN:ui.displays[0].lod"
        );

        // min_size must be two positive integers.
        let r = decode(&ok.replace("min_size = [160, 96]", "min_size = [160]")).unwrap_err();
        assert!(r.has_path("ui.displays[0].min_size"), "{r}");
        let r = decode(&ok.replace("min_size = [160, 96]", "min_size = [0, 96]")).unwrap_err();
        assert!(r.has_path("ui.displays[0].min_size"), "zero is not a size: {r}");

        // A source binding without its port says nothing to resolve.
        let r = decode(&ok.replace("port = \"out\"\n", "")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].sources[0].port").unwrap().code(),
            "E-KEY-MISSING:ui.displays[0].sources[0].port"
        );

        // A colormap must name a colormap token, never a bare value.
        let r = decode(&ok.replace("lod = \"auto\"", "lod = \"auto\"\ncolormap = \"#ff0000\""))
            .unwrap_err();
        let e = r.first_at("ui.displays[0].colormap").unwrap();
        assert_eq!(e.code(), "E-VALUE-MALFORMED:ui.displays[0].colormap");
        assert!(
            e.to_string().contains("colormap.spectrum.sonogram"),
            "the fix names an example: {e}"
        );

        // And a key nobody declared is still a typo, inside displays too. (E-UNKNOWN-KEY is a
        // catalogue code: the path rides in the message, not in a `:` suffix.)
        let r =
            decode(&ok.replace("lod = \"auto\"", "lod = \"auto\"\nbrightness = 3")).unwrap_err();
        let e = r.first_at("ui.displays[0].brightness").unwrap();
        assert_eq!(e.code(), "E-UNKNOWN-KEY");
        assert!(e.to_string().contains("brightness"), "{e}");
    }

    #[test]
    fn stream_source_bindings_are_shape_checked() {
        // ADR-011 / WO-020 plan D4: a display source binds EITHER a port OR a broker `stream`. The
        // decoder checks the SHAPE; registry membership and `param:` resolution are the validator's
        // (sparq-host-wasm), because the contract crate must not depend on the broker crate.
        const DISP: &str = "\n[ui]\ncolour_class = \"dat\"\n\n[[ui.displays]]\nid = \"wall\"\nkind = \"display_list\"\nmin_size = [2176, 1120]\nlod = \"auto\"\n\n[[ui.displays.sources]]\nid = \"cell-01\"\nstream = \"param:cell_01\"\n";
        // A param: indirection and a bare registry id are both shape-legal.
        decode(&format!("{GAIN}{DISP}"))
            .unwrap_or_else(|r| panic!("param: stream binding should decode: {r}"));
        let bare = DISP.replace("stream = \"param:cell_01\"", "stream = \"swpc.kp\"");
        decode(&format!("{GAIN}{bare}"))
            .unwrap_or_else(|r| panic!("bare registry id should decode: {r}"));

        // Both port and stream on one binding => E-CROSS-FIELD (mutually exclusive).
        let both =
            DISP.replace("stream = \"param:cell_01\"", "port = \"out\"\nstream = \"swpc.kp\"");
        let r = decode(&format!("{GAIN}{both}")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].sources[0]").unwrap().code(),
            "E-CROSS-FIELD:ui.displays[0].sources[0]"
        );

        // An empty stream binds nothing => E-VALUE-MALFORMED.
        let empty = DISP.replace("stream = \"param:cell_01\"", "stream = \"\"");
        let r = decode(&format!("{GAIN}{empty}")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].sources[0].stream").unwrap().code(),
            "E-VALUE-MALFORMED:ui.displays[0].sources[0].stream"
        );

        // `param:` with no id after the colon => E-VALUE-MALFORMED.
        let noparam = DISP.replace("stream = \"param:cell_01\"", "stream = \"param:\"");
        let r = decode(&format!("{GAIN}{noparam}")).unwrap_err();
        assert!(r.has_path("ui.displays[0].sources[0].stream"), "{r}");

        // A non-string stream => E-VALUE-MALFORMED.
        let nonstr = DISP.replace("stream = \"param:cell_01\"", "stream = 42");
        let r = decode(&format!("{GAIN}{nonstr}")).unwrap_err();
        assert_eq!(
            r.first_at("ui.displays[0].sources[0].stream").unwrap().code(),
            "E-VALUE-MALFORMED:ui.displays[0].sources[0].stream"
        );
    }

    #[test]
    fn visible_if_is_shape_checked() {
        // WO-020 plan D7: visible_if is { param = "<id>", equals = <value> } — one equality on one
        // param. Shape is the decoder's; the param's existence is the validator's cross-check.
        const PANEL: &str = "\n[ui]\ncolour_class = \"dat\"\n\n[[ui.panel.widgets]]\nkind = \"enum_select\"\nparam = \"selected_cell\"\nvisible_if = { param = \"selected_cell\", equals = 2 }\n";
        decode(&format!("{GAIN}{PANEL}"))
            .unwrap_or_else(|r| panic!("a legal visible_if should decode: {r}"));

        // visible_if must be a table.
        let bad = PANEL.replace(
            "visible_if = { param = \"selected_cell\", equals = 2 }",
            "visible_if = \"selected_cell\"",
        );
        let r = decode(&format!("{GAIN}{bad}")).unwrap_err();
        assert_eq!(
            r.first_at("ui.panel.widgets[0].visible_if").unwrap().code(),
            "E-VALUE-MALFORMED:ui.panel.widgets[0].visible_if"
        );

        // Missing `equals`.
        let noeq = PANEL.replace(
            "visible_if = { param = \"selected_cell\", equals = 2 }",
            "visible_if = { param = \"selected_cell\" }",
        );
        let r = decode(&format!("{GAIN}{noeq}")).unwrap_err();
        assert!(r.has_path("ui.panel.widgets[0].visible_if.equals"), "{r}");

        // Missing `param`.
        let noparam = PANEL.replace(
            "visible_if = { param = \"selected_cell\", equals = 2 }",
            "visible_if = { equals = 2 }",
        );
        let r = decode(&format!("{GAIN}{noparam}")).unwrap_err();
        assert!(r.has_path("ui.panel.widgets[0].visible_if.param"), "{r}");

        // An unknown key inside visible_if (v1 is exactly {param, equals}).
        let extra = PANEL.replace(
            "visible_if = { param = \"selected_cell\", equals = 2 }",
            "visible_if = { param = \"selected_cell\", equals = 2, unless = 3 }",
        );
        let r = decode(&format!("{GAIN}{extra}")).unwrap_err();
        assert_eq!(
            r.first_at("ui.panel.widgets[0].visible_if.unless").unwrap().code(),
            "E-UNKNOWN-KEY"
        );
    }
}
