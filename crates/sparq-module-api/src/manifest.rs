//! The manifest, and validation at discovery.
//!
//! A manifest is validated **before** anything is instantiated, and a bad manifest never loads:
//! `validate` returns either a [`ValidatedManifest`] the executor is allowed to use, or a
//! [`ValidationReport`] holding *every* failure at once, so an author fixes a module in one pass
//! instead of one error per run.
//!
//! Every required field is an `Option` in [`Manifest`]. That is not defensive styling — it is the
//! only way absence can be *reported*. Defect #55 was exactly this: the schema marked 20 fields
//! required and the catalogue had no code for one of them being missing, so the acceptance criterion
//! "rejects missing required ports" was unmeetable. Here, `None` becomes `E-KEY-MISSING:<path>`.
//!
//! The field list, the closed domains and the code each violation produces are
//! `docs/api/manifest-fields.toml`. This file is the compiled form of that table; where they
//! disagree, the table wins and this file is wrong.

use crate::error::{CodeKind, ValidationError, ValidationReport};
use crate::module::Oversampling;
use crate::params::MAX_PARAMS;
use crate::port::{
    ChannelSet, ChannelSetError, CvRange, CvRate, Direction, Multiplicity, PortType,
};

/// The closed unit vocabulary (module-api §5). No invented units.
pub const UNITS: [&str; 16] = [
    "Hz", "s", "ms", "samples", "dB", "%", "st", "cents", "ratio", "ch", "LUFS", "deg", "m", "bpm",
    "ticks", "x",
];
/// `classification.top` — the taxonomy's first segment.
pub const TOPS: [&str; 16] = [
    "syn", "smp", "flt", "fx", "dyn", "ana", "spa", "seq", "gen", "harm", "dat", "io", "dsp",
    "util", "ml", "out",
];
/// `classification.kind` — what a module *is*, as against where it sits in the browser.
pub const KINDS: [&str; 9] = [
    "source",
    "processor",
    "utility",
    "analysis",
    "spatial",
    "display",
    "data",
    "generative",
    "io",
];
/// `classification.tier` (ADR-002).
pub const TIERS: [&str; 3] = ["t1", "t2", "t3"];
/// `classification.stability`.
pub const STABILITIES: [&str; 3] = ["experimental", "stable", "deprecated"];
/// `params[].type`.
pub const PARAM_TYPES: [&str; 6] = ["float", "int", "bool", "enum", "text", "blob"];
/// `params[].morph`.
pub const MORPHS: [&str; 2] = ["continuous", "discrete"];
/// `identity.summary` is one line, used in tooltips and search.
pub const SUMMARY_MAX_CHARS: usize = 120;
/// The most ports a module may declare in v0.
pub const MAX_PORTS: usize = 64;

/// A parameter's value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamKind {
    /// A continuous value.
    Float,
    /// An integer, with `steps`.
    Int,
    /// A switch.
    Bool,
    /// One of a declared `options[]`.
    Enum,
    /// Free text.
    Text,
    /// An opaque blob.
    Blob,
}

impl ParamKind {
    /// Parses a manifest spelling.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "float" => Some(Self::Float),
            "int" => Some(Self::Int),
            "bool" => Some(Self::Bool),
            "enum" => Some(Self::Enum),
            "text" => Some(Self::Text),
            "blob" => Some(Self::Blob),
            _ => None,
        }
    }
}

/// `identity` — who made it, and which host versions it speaks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// `namespace/category/name`, e.g. `sparq/syn/wavetable`. A stable id.
    pub id: Option<String>,
    /// The module's own semver.
    pub version: Option<String>,
    /// The lowest host module-API version this module speaks.
    pub host_api_min: Option<u32>,
    /// The highest host module-API version this module speaks.
    pub host_api_max: Option<u32>,
    /// What the browser shows.
    pub display_name: Option<String>,
    /// One line, at most [`SUMMARY_MAX_CHARS`] characters.
    pub summary: Option<String>,
    /// At least one author.
    pub authors: Vec<String>,
    /// An SPDX identifier.
    pub license: Option<String>,
}

/// `classification` — where it sits and what it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Classification {
    /// Taxonomy path, e.g. `synth/oscillator/wavetable`.
    pub category: Option<String>,
    /// One of [`TOPS`].
    pub top: Option<String>,
    /// One of [`KINDS`].
    pub kind: Option<String>,
    /// One of [`TIERS`].
    pub tier: Option<String>,
    /// One of [`STABILITIES`].
    pub stability: Option<String>,
}

/// One `ports[]` entry, as written. Closed vocabularies stay strings here so that an illegal value
/// can be reported rather than failing to parse into a type that cannot represent it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PortSpec {
    /// Stable id, unique within the module.
    pub id: Option<String>,
    /// Human label.
    pub name: Option<String>,
    /// `in` | `out`.
    pub direction: Option<String>,
    /// One of the six (ADR-005).
    pub port_type: Option<String>,
    /// Whether an unconnected input is an error. Defaults to true.
    pub required: Option<bool>,
    /// A layout tag, a list, or `variable`. Required for `audio`.
    pub channel_set: Option<String>,
    /// `audio` | `block`. Required for `cv`.
    pub rate: Option<String>,
    /// `bipolar` | `unipolar`. Required for `cv`.
    pub range: Option<String>,
    /// `single` | `multi_in` | `multi_out`.
    pub multiplicity: Option<String>,
    /// Samples this port path adds.
    pub latency_contribution: Option<u32>,
    /// How an audio-rate source collapses for a block-rate input (decision G3). Default `last`.
    pub cv_reduce: Option<String>,
    /// How the host expands a block-rate source for an audio-rate input (Q3). Default `hold`.
    pub cv_interp: Option<String>,
}

/// One `params[]` entry, as written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParamSpec {
    /// Stable id, unique within the module.
    pub id: Option<String>,
    /// Display label.
    pub name: Option<String>,
    /// One of [`PARAM_TYPES`].
    pub kind: Option<String>,
    /// One of [`UNITS`], required when numeric.
    pub unit: Option<String>,
    /// Inclusive minimum, required when numeric.
    pub min: Option<f64>,
    /// Inclusive maximum, required when numeric.
    pub max: Option<f64>,
    /// Must fall inside `[min, max]`.
    pub default: Option<f64>,
    /// Whether the value differs per voice. Requires `voices.policy != none`.
    pub per_voice: Option<bool>,
    /// `continuous` | `discrete`. Discrete parameters are held constant during a morph.
    pub morph: Option<String>,
}

/// `state` — serialisation is mandatory for every module, without exception.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateDecl {
    /// The state schema's id.
    pub schema_id: Option<String>,
    /// The state schema's version.
    pub schema_version: Option<u32>,
}

/// `resources` — what the module costs and what it needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceDecl {
    /// Declared latency in samples, for the constant form of `resources.latency`. Required:
    /// `E-LATENCY-UNDECLARED` exists precisely because a module that reports non-zero measured
    /// latency while declaring zero is lying.
    pub latency_samples: Option<u32>,
    /// The parameter id latency follows, for the `latency = "param:<id>"` form (convolution,
    /// granular). Exactly one of this and `latency_samples` may be present.
    pub latency_param: Option<String>,
    /// How long output continues after input stops.
    pub tail_samples: Option<u32>,
    /// `trivial` | `light` | `medium` | `heavy` | `very_heavy`.
    pub cpu_class: Option<String>,
    /// One of `none` | `x2` | `x4` | `x8`; the host provides it.
    pub oversampling: Option<String>,
    /// Feature flags this module needs: `fft`, `midi2`, `spatial`, `ml`, `gpu_compute`, `network`.
    pub requires: Vec<String>,
}

/// A manifest as written. Required fields are `Option` so absence is representable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    /// `identity`.
    pub identity: Identity,
    /// `classification`.
    pub classification: Classification,
    /// `ports[]`.
    pub ports: Vec<PortSpec>,
    /// `params[]`.
    pub params: Vec<ParamSpec>,
    /// `state`.
    pub state: StateDecl,
    /// `resources`.
    pub resources: ResourceDecl,
    /// `voices.policy`. Only the policy is modelled in v0; `per_voice` params are checked against it.
    pub voices_policy: Option<String>,
}

/// A port as the engine will use it: every closed vocabulary parsed into a type.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// The stable id.
    pub id: String,
    /// Input or output.
    pub direction: Direction,
    /// One of the six.
    pub port_type: PortType,
    /// Whether an unconnected input is an error.
    pub required: bool,
    /// The layout tag, for `audio`. `None` means `variable`, resolved at `prepare`.
    pub channel_set: Option<ChannelSet>,
    /// Whether the channel set is `variable`.
    pub channel_set_variable: bool,
    /// For `cv`.
    pub cv_rate: Option<CvRate>,
    /// For `cv`.
    pub cv_range: Option<CvRange>,
    /// Fan-in/fan-out policy.
    pub multiplicity: Multiplicity,
    /// Samples this port path adds.
    pub latency_contribution: u32,
}

/// A manifest that passed validation. The executor only ever sees this type, which is what makes
/// "a bad manifest never loads" a property of the type system rather than of caller discipline.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedManifest {
    manifest: Manifest,
    ports: Vec<Port>,
}

impl ValidatedManifest {
    /// The manifest as written.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The parsed ports.
    #[must_use]
    pub fn ports(&self) -> &[Port] {
        &self.ports
    }

    /// The module's stable id. Present because validation required it.
    #[must_use]
    pub fn id(&self) -> &str {
        self.manifest.identity.id.as_deref().unwrap_or("")
    }

    /// Declared latency in samples. Zero when latency is parametric, because the value is not
    /// known until the parameter is; use [`Self::latency_is_parametric`] before treating a zero as
    /// a claim.
    #[must_use]
    pub fn latency_samples(&self) -> u32 {
        self.manifest.resources.latency_samples.unwrap_or(0)
    }

    /// Whether latency follows a parameter (`latency = "param:<id>"`), and so cannot be checked
    /// statically against a measurement.
    #[must_use]
    pub fn latency_is_parametric(&self) -> bool {
        self.manifest.resources.latency_param.is_some()
    }

    /// The check that gives `E-LATENCY-UNDECLARED` its meaning: measured against declared.
    ///
    /// Returns the error to record when a module reported non-zero measured latency while declaring
    /// zero. Dishonesty about latency is not a warning, because latency compensation and the whole
    /// "latency is a timbral resource" position (ADR-006) depend on the declared number being true.
    #[must_use]
    pub fn check_declared_latency(&self, measured_samples: u32) -> Option<ValidationError> {
        if self.latency_is_parametric() {
            // A parametric latency is recomputed by the executor when the parameter changes, so
            // there is no static declaration to contradict the measurement.
            return None;
        }
        let declared = self.latency_samples();
        if measured_samples > 0 && declared == 0 {
            Some(
                ValidationError::new(
                    CodeKind::LatencyUndeclared,
                    "resources.latency",
                    "declare the measured latency, or make the module genuinely zero-latency",
                )
                .with_found(format!("measured {measured_samples} samples, declared 0")),
            )
        } else {
            // Over-declaring latency is allowed: it costs the patch compensation it may not need,
            // but it does not lie about a delay that is really there.
            None
        }
    }

    /// Consumes the wrapper, returning the manifest.
    #[must_use]
    pub fn into_manifest(self) -> Manifest {
        self.manifest
    }
}

/// Records `E-KEY-MISSING:<path>` when a required string is absent or empty.
fn req_str(r: &mut ValidationReport, path: &str, v: Option<&str>, fix: &str) -> Option<String> {
    match v {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => {
            r.push(ValidationError::new(CodeKind::KeyMissing, path, fix));
            None
        },
    }
}

/// Records `E-KEY-MISSING:<path>` when a required number is absent.
fn req_num<T: Copy>(r: &mut ValidationReport, path: &str, v: Option<T>, fix: &str) -> Option<T> {
    match v {
        Some(n) => Some(n),
        None => {
            r.push(ValidationError::new(CodeKind::KeyMissing, path, fix));
            None
        },
    }
}

/// Validates an optional closed-vocabulary field: absent is legal, present-and-unknown is
/// `E-ENUM-UNKNOWN:<path>`.
fn opt_enum(
    r: &mut ValidationReport,
    path: &str,
    v: Option<&str>,
    allowed: &[&str],
) -> Option<String> {
    let s = v?;
    if allowed.contains(&s) {
        Some(s.to_string())
    } else {
        r.push(
            ValidationError::new(
                CodeKind::EnumUnknown,
                path,
                "use one of the values this field allows",
            )
            .with_found(s)
            .with_allowed(allowed.join(" ")),
        );
        None
    }
}

impl Manifest {
    /// Validates the whole manifest, returning every failure at once.
    ///
    /// # Errors
    /// A [`ValidationReport`] holding all failures if the manifest is not loadable.
    pub fn validate(&self) -> Result<ValidatedManifest, ValidationReport> {
        let mut r = ValidationReport::new();
        let mut ports = Vec::new();
        self.check_identity(&mut r);
        self.check_classification(&mut r);
        self.check_ports(&mut r, &mut ports);
        self.check_params(&mut r);
        self.check_state(&mut r);
        self.check_resources(&mut r);
        self.check_cross(&mut r);
        if r.is_empty() {
            Ok(ValidatedManifest { manifest: self.clone(), ports })
        } else {
            Err(r)
        }
    }

    /// The report on its own, for callers that want to log failures without branching.
    #[must_use]
    pub fn report(&self) -> ValidationReport {
        match self.validate() {
            Ok(_) => ValidationReport::new(),
            Err(r) => r,
        }
    }

    fn check_identity(&self, r: &mut ValidationReport) {
        let i = &self.identity;
        req_str(
            r,
            "identity.id",
            i.id.as_deref(),
            "give the module a stable `namespace/category/name` id",
        );
        req_str(r, "identity.version", i.version.as_deref(), "declare a semver version");
        req_num(
            r,
            "identity.host_api.min",
            i.host_api_min,
            "declare the lowest host API version this module speaks",
        );
        req_num(
            r,
            "identity.host_api.max",
            i.host_api_max,
            "declare the highest host API version this module speaks",
        );
        req_str(
            r,
            "identity.display_name",
            i.display_name.as_deref(),
            "give the module a name the browser can show",
        );
        req_str(r, "identity.license", i.license.as_deref(), "declare an SPDX license identifier");
        if i.authors.is_empty() {
            r.push(ValidationError::new(
                CodeKind::KeyMissing,
                "identity.authors",
                "list at least one author",
            ));
        }
        match &i.summary {
            None => r.push(ValidationError::new(
                CodeKind::KeyMissing,
                "identity.summary",
                "write a one-line summary; it is what tooltips and search use",
            )),
            Some(s) if s.chars().count() > SUMMARY_MAX_CHARS => r.push(
                ValidationError::new(
                    CodeKind::ValueTooLong,
                    "identity.summary",
                    format!("shorten it to {SUMMARY_MAX_CHARS} characters or fewer"),
                )
                .with_found(format!("{} characters", s.chars().count())),
            ),
            Some(_) => {},
        }
    }

    fn check_classification(&self, r: &mut ValidationReport) {
        let c = &self.classification;
        req_str(
            r,
            "classification.category",
            c.category.as_deref(),
            "give a taxonomy path, e.g. synth/oscillator/wavetable",
        );
        let top =
            req_str(r, "classification.top", c.top.as_deref(), "pick the taxonomy's first segment");
        if let Some(t) = top {
            opt_enum(r, "classification.top", Some(&t), &TOPS);
        }
        let kind = req_str(r, "classification.kind", c.kind.as_deref(), "say what the module is");
        if let Some(k) = kind {
            opt_enum(r, "classification.kind", Some(&k), &KINDS);
        }
        let tier = req_str(
            r,
            "classification.tier",
            c.tier.as_deref(),
            "declare the execution tier (ADR-002)",
        );
        if let Some(t) = tier {
            opt_enum(r, "classification.tier", Some(&t), &TIERS);
        }
        let stability = req_str(
            r,
            "classification.stability",
            c.stability.as_deref(),
            "declare how settled the module is",
        );
        if let Some(s) = stability {
            opt_enum(r, "classification.stability", Some(&s), &STABILITIES);
        }
    }

    fn check_ports(&self, r: &mut ValidationReport, out: &mut Vec<Port>) {
        if self.ports.len() > MAX_PORTS {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "ports",
                    "split the module; v0 caps a module at 64 ports",
                )
                .with_found(format!("{} ports", self.ports.len()))
                .with_allowed(format!("at most {MAX_PORTS}")),
            );
        }
        for (n, p) in self.ports.iter().enumerate() {
            let base = format!("ports[{n}]");
            let id =
                req_str(r, &format!("{base}.id"), p.id.as_deref(), "give the port a stable id");
            req_str(r, &format!("{base}.name"), p.name.as_deref(), "give the port a human label");

            // Duplicate ids are checked once, after the loop, so both paths are named.
            let direction = match p.direction.as_deref() {
                None => {
                    r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.direction"),
                        "declare `in` or `out`",
                    ));
                    None
                },
                Some(s) => match Direction::parse(s) {
                    Some(d) => Some(d),
                    None => {
                        r.push(
                            ValidationError::new(
                                CodeKind::EnumUnknown,
                                format!("{base}.direction"),
                                "use `in` or `out`",
                            )
                            .with_found(s)
                            .with_allowed("in out"),
                        );
                        None
                    },
                },
            };

            let port_type = match p.port_type.as_deref() {
                None => {
                    r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.type"),
                        "declare one of the six port types (ADR-005)",
                    ));
                    None
                },
                Some(s) => match PortType::parse(s) {
                    Some(t) => Some(t),
                    None => {
                        // E-PORT-TYPE-UNKNOWN, not E-ENUM-UNKNOWN: this vocabulary has its own code,
                        // and the fix is different — the set is closed by ADR, not by convention.
                        let names: Vec<&str> = PortType::ALL.iter().map(|t| t.as_str()).collect();
                        r.push(
                            ValidationError::new(
                                CodeKind::PortTypeUnknown,
                                format!("{base}.type"),
                                "the port type set is closed (ADR-005); new content is a subtype or an `atom` payload, never a new port type",
                            )
                            .with_found(s)
                            .with_allowed(names.join(" ")),
                        );
                        None
                    },
                },
            };

            let mut channel_set = None;
            let mut variable = false;
            if port_type == Some(PortType::Audio) {
                match p.channel_set.as_deref() {
                    None => r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.channel_set"),
                        "an audio port must declare a layout tag, a list, or `variable`",
                    )),
                    Some("variable") => variable = true,
                    Some(s) => match ChannelSet::parse(s) {
                        Ok(cs) => channel_set = Some(cs),
                        Err(ChannelSetError::UnknownTag) => r.push(
                            ValidationError::new(
                                CodeKind::ChannelSetUnknown,
                                format!("{base}.channel_set"),
                                "use a layout tag from ADR-005",
                            )
                            .with_found(s)
                            .with_allowed(
                                "mono stereo quad 5.1 7.1.4 ambisonics:N objects:K raw:M variable",
                            ),
                        ),
                        Err(ChannelSetError::AmbiOrderOutOfRange(n)) => r.push(
                            ValidationError::new(
                                CodeKind::AmbiOrderRange,
                                format!("{base}.channel_set"),
                                "AmbiX order is 1..=7",
                            )
                            .with_found(format!("ambisonics:{n}")),
                        ),
                    },
                }
            }

            let mut cv_rate = None;
            let mut cv_range = None;
            if port_type == Some(PortType::Cv) {
                match p.rate.as_deref() {
                    None => r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.rate"),
                        "a cv port must declare `audio` or `block` rate",
                    )),
                    Some(s) => match CvRate::parse(s) {
                        Some(rt) => cv_rate = Some(rt),
                        None => r.push(
                            ValidationError::new(
                                CodeKind::EnumUnknown,
                                format!("{base}.rate"),
                                "use `audio` or `block`",
                            )
                            .with_found(s)
                            .with_allowed("audio block"),
                        ),
                    },
                }
                match p.range.as_deref() {
                    None => r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.range"),
                        "a cv port must declare `bipolar` or `unipolar`",
                    )),
                    Some(s) => match CvRange::parse(s) {
                        Some(rg) => cv_range = Some(rg),
                        None => r.push(
                            ValidationError::new(
                                CodeKind::EnumUnknown,
                                format!("{base}.range"),
                                "use `bipolar` or `unipolar`",
                            )
                            .with_found(s)
                            .with_allowed("bipolar unipolar"),
                        ),
                    },
                }
                // Decisions G3/G4: the policies are declared, so a rate change is reproducible.
                opt_enum(
                    r,
                    &format!("{base}.cv_reduce"),
                    p.cv_reduce.as_deref(),
                    &["last", "first", "mean", "min", "max", "peak"],
                );
                opt_enum(
                    r,
                    &format!("{base}.cv_interp"),
                    p.cv_interp.as_deref(),
                    &["hold", "linear", "spline"],
                );
            }

            let multiplicity = match p.multiplicity.as_deref() {
                None => Multiplicity::Single,
                Some(s) => match Multiplicity::parse(s) {
                    Some(m) => m,
                    None => {
                        r.push(
                            ValidationError::new(
                                CodeKind::EnumUnknown,
                                format!("{base}.multiplicity"),
                                "use `single`, `multi_in` or `multi_out`",
                            )
                            .with_found(s)
                            .with_allowed("single multi_in multi_out"),
                        );
                        Multiplicity::Single
                    },
                },
            };

            if let (Some(id), Some(direction), Some(port_type)) = (id, direction, port_type) {
                out.push(Port {
                    id,
                    direction,
                    port_type,
                    required: p.required.unwrap_or(true),
                    channel_set,
                    channel_set_variable: variable,
                    cv_rate,
                    cv_range,
                    multiplicity,
                    latency_contribution: p.latency_contribution.unwrap_or(0),
                });
            }
        }
        report_duplicate_ids(r, "ports", self.ports.iter().filter_map(|p| p.id.as_deref()));
    }

    fn check_params(&self, r: &mut ValidationReport) {
        if self.params.len() > MAX_PARAMS {
            r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "params",
                    "v0 caps a module at 32 parameters, because the snapshot must stay `Copy` to cross to the audio thread without allocating",
                )
                .with_found(format!("{} parameters", self.params.len()))
                .with_allowed(format!("at most {MAX_PARAMS}")),
            );
        }
        for (n, p) in self.params.iter().enumerate() {
            let base = format!("params[{n}]");
            req_str(r, &format!("{base}.id"), p.id.as_deref(), "give the parameter a stable id");
            req_str(
                r,
                &format!("{base}.name"),
                p.name.as_deref(),
                "give the parameter a display label",
            );

            let kind = match p.kind.as_deref() {
                None => {
                    r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.type"),
                        "declare the parameter's value type",
                    ));
                    None
                },
                Some(s) => match ParamKind::parse(s) {
                    Some(k) => Some(k),
                    None => {
                        r.push(
                            ValidationError::new(
                                CodeKind::EnumUnknown,
                                format!("{base}.type"),
                                "use one of the six parameter types",
                            )
                            .with_found(s)
                            .with_allowed(PARAM_TYPES.join(" ")),
                        );
                        None
                    },
                },
            };

            let numeric = matches!(kind, Some(ParamKind::Float) | Some(ParamKind::Int));
            if numeric {
                if let Some(u) = p.unit.as_deref() {
                    if !UNITS.contains(&u) {
                        r.push(
                            ValidationError::new(
                                CodeKind::UnitUnknown,
                                format!("{base}.unit"),
                                "the unit vocabulary is closed; express it in one of these",
                            )
                            .with_found(u)
                            .with_allowed(UNITS.join(" ")),
                        );
                    }
                } else {
                    r.push(ValidationError::new(
                        CodeKind::KeyMissing,
                        format!("{base}.unit"),
                        "a numeric parameter must declare a unit from the closed vocabulary",
                    ));
                }
                let min =
                    req_num(r, &format!("{base}.min"), p.min, "declare the inclusive minimum");
                let max =
                    req_num(r, &format!("{base}.max"), p.max, "declare the inclusive maximum");
                let def = req_num(
                    r,
                    &format!("{base}.default"),
                    p.default,
                    "declare a default inside the range",
                );
                if let (Some(lo), Some(hi), Some(d)) = (min, max, def) {
                    if lo > hi {
                        r.push(
                            ValidationError::new(
                                CodeKind::ParamDefaultOutOfRange,
                                format!("{base}.min"),
                                "min must not exceed max",
                            )
                            .with_found(format!("min {lo} > max {hi}")),
                        );
                    } else if d < lo || d > hi {
                        r.push(
                            ValidationError::new(
                                CodeKind::ParamDefaultOutOfRange,
                                format!("{base}.default"),
                                "move the default inside [min, max]",
                            )
                            .with_found(format!("default {d} outside [{lo}, {hi}]")),
                        );
                    }
                }
            }
            opt_enum(r, &format!("{base}.morph"), p.morph.as_deref(), &MORPHS);
        }
        report_duplicate_ids(r, "params", self.params.iter().filter_map(|p| p.id.as_deref()));
    }

    fn check_state(&self, r: &mut ValidationReport) {
        // E-STATE-SCHEMA-MISSING rather than E-KEY-MISSING: this one has its own catalogue code,
        // because "a module that cannot save its state cannot be in a project" is a rule of its own.
        // Written as a match rather than `is_none_or` because the workspace MSRV is 1.80 and
        // `Option::is_none_or` landed in 1.82 (clippy::incompatible_msrv).
        match self.state.schema_id.as_deref() {
            None | Some("") => r.push(ValidationError::new(
                CodeKind::StateSchemaMissing,
                "state.schema_id",
                "declare a state schema id; serialisation is mandatory for every module",
            )),
            Some(_) => {},
        }
        if self.state.schema_version.is_none() {
            r.push(ValidationError::new(
                CodeKind::StateSchemaMissing,
                "state.schema_version",
                "declare the state schema version, so an old project can run its migration chain",
            ));
        }
    }

    fn check_resources(&self, r: &mut ValidationReport) {
        // `latency` is `int | "param:<id>"`: exactly one form must be present.
        match (&self.resources.latency_samples, &self.resources.latency_param) {
            (None, None) => r.push(ValidationError::new(
                CodeKind::KeyMissing,
                "resources.latency",
                "declare latency in samples, even when it is 0 — `E-LATENCY-UNDECLARED` exists because an undeclared latency is a lie about the patch",
            )),
            (Some(_), Some(_)) => r.push(
                ValidationError::new(
                    CodeKind::CrossField,
                    "resources.latency",
                    "latency is either a constant sample count or `param:<id>`, not both",
                )
                .with_found(format!(
                    "{} samples and param:{}",
                    self.resources.latency_samples.unwrap_or(0),
                    self.resources.latency_param.as_deref().unwrap_or("")
                )),
            ),
            (Some(_), None) | (None, Some(_)) => {}
        }
        opt_enum(
            r,
            "resources.cpu_class",
            self.resources.cpu_class.as_deref(),
            &["trivial", "light", "medium", "heavy", "very_heavy"],
        );
        if let Some(o) = self.resources.oversampling.as_deref() {
            if Oversampling::parse(o).is_none() {
                r.push(
                    ValidationError::new(
                        CodeKind::EnumUnknown,
                        "resources.oversampling",
                        "the host provides oversampling; declare how much you need",
                    )
                    .with_found(o)
                    .with_allowed("none x2 x4 x8"),
                );
            }
        }
        for (n, f) in self.resources.requires.iter().enumerate() {
            const FEATURES: [&str; 6] = ["fft", "midi2", "spatial", "ml", "gpu_compute", "network"];
            if !FEATURES.contains(&f.as_str()) {
                r.push(
                    ValidationError::new(
                        CodeKind::RequiresUnsupported,
                        format!("resources.requires[{n}]"),
                        "declare only features a host can report on",
                    )
                    .with_found(f)
                    .with_allowed(FEATURES.join(" ")),
                );
            }
        }
    }

    fn check_cross(&self, r: &mut ValidationReport) {
        // `params[].per_voice` requires `voices.policy != none`.
        let polyphonic =
            matches!(self.voices_policy.as_deref(), Some(p) if p != "none" && !p.is_empty());
        if !polyphonic {
            for (n, p) in self.params.iter().enumerate() {
                if p.per_voice == Some(true) {
                    r.push(
                        ValidationError::new(
                            CodeKind::CrossField,
                            format!("params[{n}].per_voice"),
                            "a per-voice parameter needs `voices.policy` set to something other than `none`",
                        )
                        .with_found(format!("voices.policy = {:?}", self.voices_policy)),
                    );
                }
            }
        }
        // An audio input that is not required must still be declared, and a `gpu`/`atom` port may
        // not claim an audio-thread rate. Both are cross-field, neither has a code of its own.
        for (n, p) in self.ports.iter().enumerate() {
            if p.port_type.as_deref() == Some("cv")
                && p.rate.as_deref() == Some("audio")
                && p.cv_reduce.is_some()
            {
                r.push(
                    ValidationError::new(
                        CodeKind::CrossField,
                        format!("ports[{n}].cv_reduce"),
                        "`cv_reduce` only applies to a block-rate cv input; an audio-rate input has nothing to reduce",
                    )
                    .with_found("rate = audio"),
                );
            }
        }
    }
}

/// Reports `E-ID-DUP` once per duplicated id, naming every index that shares it.
fn report_duplicate_ids<'a, I>(r: &mut ValidationReport, section: &str, ids: I)
where
    I: Iterator<Item = &'a str>,
{
    let ids: Vec<&str> = ids.collect();
    for (i, id) in ids.iter().enumerate() {
        if ids[..i].contains(id) {
            continue; // reported once, at the first duplicate
        }
        let dupes: Vec<String> = ids
            .iter()
            .enumerate()
            .filter(|(_, x)| *x == id)
            .map(|(n, _)| format!("{section}[{n}].id"))
            .collect();
        if dupes.len() > 1 {
            r.push(
                ValidationError::new(
                    CodeKind::IdDup,
                    dupes[0].clone(),
                    "ids are stable and unique within a module; rename one, and add the old name to `aliases` if it ever shipped",
                )
                .with_found(*id)
                .with_allowed(dupes.join(", ")),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A manifest that passes, so each test can break exactly one thing.
    fn minimal() -> Manifest {
        Manifest {
            identity: Identity {
                id: Some("sparq/util/gain".into()),
                version: Some("0.1.0".into()),
                host_api_min: Some(1),
                host_api_max: Some(1),
                display_name: Some("Gain".into()),
                summary: Some("Gain and trim".into()),
                authors: vec!["sparq".into()],
                license: Some("MIT".into()),
            },
            classification: Classification {
                category: Some("utility/gain".into()),
                top: Some("util".into()),
                kind: Some("processor".into()),
                tier: Some("t1".into()),
                stability: Some("stable".into()),
            },
            ports: vec![
                PortSpec {
                    id: Some("in".into()),
                    name: Some("Input".into()),
                    direction: Some("in".into()),
                    port_type: Some("audio".into()),
                    channel_set: Some("stereo".into()),
                    ..PortSpec::default()
                },
                PortSpec {
                    id: Some("out".into()),
                    name: Some("Output".into()),
                    direction: Some("out".into()),
                    port_type: Some("audio".into()),
                    channel_set: Some("stereo".into()),
                    ..PortSpec::default()
                },
            ],
            params: vec![ParamSpec {
                id: Some("gain".into()),
                name: Some("Gain".into()),
                kind: Some("float".into()),
                unit: Some("ratio".into()),
                min: Some(0.0),
                max: Some(2.0),
                default: Some(1.0),
                ..ParamSpec::default()
            }],
            state: StateDecl {
                schema_id: Some("sparq/util/gain/state".into()),
                schema_version: Some(1),
            },
            resources: ResourceDecl { latency_samples: Some(0), ..ResourceDecl::default() },
            voices_policy: Some("none".into()),
        }
    }

    #[test]
    fn a_minimal_manifest_validates_and_its_ports_come_out_parsed() {
        let v = minimal().validate().unwrap_or_else(|r| panic!("{r}"));
        assert_eq!(v.id(), "sparq/util/gain");
        assert_eq!(v.ports().len(), 2);
        assert_eq!(v.ports()[0].port_type, PortType::Audio);
        assert_eq!(v.ports()[0].direction, Direction::In);
        assert_eq!(v.ports()[0].channel_set, Some(ChannelSet::Stereo));
        assert!(v.ports()[0].required, "`required` defaults to true");
        assert_eq!(v.latency_samples(), 0);
    }

    #[test]
    fn unknown_port_types_are_rejected_naming_the_closed_set() {
        // Acceptance criterion: "rejects unknown port types ... with an actionable error message".
        let mut m = minimal();
        m.ports[0].port_type = Some("midi".into());
        let r = m.report();
        assert!(r.has(CodeKind::PortTypeUnknown));
        let e = r.first_at("ports[0].type").unwrap();
        assert_eq!(e.code(), "E-PORT-TYPE-UNKNOWN");
        assert!(e.to_string().contains("found `midi`"), "{e}");
        assert!(e.to_string().contains("audio cv event data gpu atom"), "{e}");
        assert!(e.to_string().contains("closed"), "{e}");
    }

    #[test]
    fn duplicate_ids_are_rejected_naming_both_paths() {
        let mut m = minimal();
        m.ports[1].id = Some("in".into());
        let r = m.report();
        assert!(r.has(CodeKind::IdDup));
        let e = r.iter().find(|e| e.kind == CodeKind::IdDup).unwrap();
        assert!(e.to_string().contains("ports[0].id"), "{e}");
        assert!(e.to_string().contains("ports[1].id"), "{e}");
    }

    #[test]
    fn a_missing_required_port_field_is_rejected_by_path() {
        // Acceptance criterion: "rejects ... missing required ports". Defect #55 was that no code
        // existed for this at all; E-KEY-MISSING is the derived one that does.
        let mut m = minimal();
        m.ports[0].port_type = None;
        let r = m.report();
        assert!(r.has(CodeKind::KeyMissing));
        let e = r.first_at("ports[0].type").unwrap();
        assert_eq!(e.code(), "E-KEY-MISSING:ports[0].type");
        assert!(e.to_string().contains("Fix: declare one of the six port types"), "{e}");
    }

    #[test]
    fn undeclared_latency_is_rejected_both_ways() {
        // (a) absent from the manifest at all.
        let mut m = minimal();
        m.resources.latency_samples = None;
        assert!(m.report().has_path("resources.latency"));

        // (b) declared zero while measuring non-zero — what E-LATENCY-UNDECLARED is for.
        let v = minimal().validate().unwrap();
        let e = v.check_declared_latency(64).unwrap();
        assert_eq!(e.code(), "E-LATENCY-UNDECLARED");
        assert!(e.to_string().contains("measured 64 samples, declared 0"), "{e}");
        assert!(v.check_declared_latency(0).is_none(), "an honest zero latency is fine");
    }

    #[test]
    fn unknown_units_are_rejected_naming_the_vocabulary() {
        let mut m = minimal();
        m.params[0].unit = Some("bananas".into());
        let r = m.report();
        assert!(r.has(CodeKind::UnitUnknown));
        let e = r.first_at("params[0].unit").unwrap();
        assert!(e.to_string().contains("found `bananas`"), "{e}");
        assert!(e.to_string().contains("LUFS"), "{e}");
    }

    #[test]
    fn an_invalid_enum_value_is_rejected_even_where_no_specific_code_exists() {
        // Defect #55's second half: `tier = "t9"` had no code to produce.
        let mut m = minimal();
        m.classification.tier = Some("t9".into());
        let r = m.report();
        let e = r.first_at("classification.tier").unwrap();
        assert_eq!(e.code(), "E-ENUM-UNKNOWN:classification.tier");
        assert!(e.to_string().contains("found `t9`"), "{e}");
        assert!(e.to_string().contains("t1 t2 t3"), "{e}");
    }

    #[test]
    fn an_over_long_summary_is_rejected_with_its_length() {
        let mut m = minimal();
        m.identity.summary = Some("x".repeat(SUMMARY_MAX_CHARS + 1));
        let r = m.report();
        assert!(r.has(CodeKind::ValueTooLong));
        let e = r.first_at("identity.summary").unwrap();
        assert!(e.to_string().contains("121 characters"), "{e}");
    }

    #[test]
    fn a_default_outside_its_range_is_rejected() {
        let mut m = minimal();
        m.params[0].default = Some(4.0);
        let r = m.report();
        assert!(r.has(CodeKind::ParamDefaultOutOfRange));
        let e = r.first_at("params[0].default").unwrap();
        assert!(e.to_string().contains("outside [0, 2]"), "{e}");
    }

    #[test]
    fn a_min_above_max_is_rejected_as_its_own_case() {
        let mut m = minimal();
        m.params[0].min = Some(3.0);
        let r = m.report();
        assert!(r
            .first_at("params[0].min")
            .unwrap()
            .to_string()
            .contains("min must not exceed max"));
    }

    #[test]
    fn per_voice_without_a_voice_policy_is_a_cross_field_error() {
        let mut m = minimal();
        m.params[0].per_voice = Some(true);
        m.voices_policy = Some("none".into());
        let r = m.report();
        assert!(r.has(CodeKind::CrossField));
        assert!(
            r.first_at("params[0].per_voice").unwrap().to_string().contains("voices.policy"),
            "{r}"
        );

        m.voices_policy = Some("poly:8".into());
        assert!(!m.report().has(CodeKind::CrossField), "a real voice policy satisfies the rule");
    }

    #[test]
    fn too_many_params_is_refused_rather_than_silently_truncated() {
        let mut m = minimal();
        m.params = (0..=MAX_PARAMS)
            .map(|i| ParamSpec {
                id: Some(format!("p{i}")),
                name: Some(format!("P{i}")),
                kind: Some("bool".into()),
                ..ParamSpec::default()
            })
            .collect();
        let r = m.report();
        let e = r.first_at("params").unwrap();
        assert_eq!(e.kind, CodeKind::CrossField);
        assert!(e.to_string().contains("Copy"), "the message says why the cap exists: {e}");
    }

    #[test]
    fn channel_set_failures_keep_their_two_distinct_codes() {
        let mut m = minimal();
        m.ports[0].channel_set = Some("ambisonics:9".into());
        assert!(m.report().has(CodeKind::AmbiOrderRange));
        m.ports[0].channel_set = Some("septaphonic".into());
        let r = m.report();
        assert!(r.has(CodeKind::ChannelSetUnknown));
        assert!(
            r.first_at("ports[0].channel_set").unwrap().to_string().contains("objects:K"),
            "{r}"
        );
        m.ports[0].channel_set = None;
        assert!(m.report().has_path("ports[0].channel_set"), "an audio port must declare one");
        m.ports[0].channel_set = Some("variable".into());
        let v = m.validate().unwrap();
        assert!(v.ports()[0].channel_set_variable, "`variable` resolves at prepare");
        assert_eq!(v.ports()[0].channel_set, None);
    }

    #[test]
    fn cv_ports_must_declare_rate_and_range_and_their_policies_are_checked() {
        let mut m = minimal();
        m.ports.push(PortSpec {
            id: Some("mod".into()),
            name: Some("Mod".into()),
            direction: Some("in".into()),
            port_type: Some("cv".into()),
            ..PortSpec::default()
        });
        let r = m.report();
        assert!(r.has_path("ports[2].rate") && r.has_path("ports[2].range"));

        m.ports[2].rate = Some("block".into());
        m.ports[2].range = Some("bipolar".into());
        assert!(m.validate().is_ok());

        // Decisions G3/G4: the policies are closed vocabularies too.
        m.ports[2].cv_reduce = Some("median".into());
        let rep = m.report();
        let e = rep.first_at("ports[2].cv_reduce").unwrap();
        assert!(e.to_string().contains("last first mean min max peak"), "{e}");
        m.ports[2].cv_reduce = Some("mean".into());
        m.ports[2].cv_interp = Some("cubic".into());
        assert!(m.report().has_path("ports[2].cv_interp"));
    }

    #[test]
    fn cv_reduce_on_an_audio_rate_input_is_a_cross_field_error() {
        let mut m = minimal();
        m.ports.push(PortSpec {
            id: Some("mod".into()),
            name: Some("Mod".into()),
            direction: Some("in".into()),
            port_type: Some("cv".into()),
            rate: Some("audio".into()),
            range: Some("unipolar".into()),
            cv_reduce: Some("last".into()),
            ..PortSpec::default()
        });
        let rep = m.report();
        let e = rep.first_at("ports[2].cv_reduce").unwrap();
        assert_eq!(e.kind, CodeKind::CrossField);
        assert!(e.to_string().contains("nothing to reduce"), "{e}");
    }

    #[test]
    fn a_missing_state_schema_has_its_own_code() {
        let mut m = minimal();
        m.state.schema_id = None;
        assert!(m.report().has(CodeKind::StateSchemaMissing));
        m.state = StateDecl::default();
        let r = m.report();
        assert_eq!(r.iter().filter(|e| e.kind == CodeKind::StateSchemaMissing).count(), 2);
    }

    #[test]
    fn an_unknown_feature_requirement_is_rejected() {
        let mut m = minimal();
        m.resources.requires = vec!["fft".into(), "telepathy".into()];
        let r = m.report();
        assert!(r.has(CodeKind::RequiresUnsupported));
        assert!(
            r.first_at("resources.requires[1]").unwrap().to_string().contains("gpu_compute"),
            "{r}"
        );
    }

    #[test]
    fn every_failure_comes_back_at_once() {
        // "An author fixes a module in one pass instead of one error per run."
        let mut m = minimal();
        m.identity.summary = None;
        m.classification.top = Some("synth".into());
        m.ports[0].port_type = Some("midi".into());
        m.params[0].unit = Some("dBFS".into());
        m.resources.latency_samples = None;
        let r = m.report();
        assert!(r.len() >= 5, "got {}: {r}", r.len());
        for path in [
            "identity.summary",
            "classification.top",
            "ports[0].type",
            "params[0].unit",
            "resources.latency",
        ] {
            assert!(r.has_path(path), "no failure reported at {path}: {r}");
        }
        assert_eq!(r.to_string().lines().count(), r.len(), "one actionable line per failure");
    }

    /// The paths reported as missing, in order.
    fn missing_paths(r: &ValidationReport) -> Vec<&str> {
        r.iter()
            .filter(|e| matches!(e.kind, CodeKind::KeyMissing | CodeKind::StateSchemaMissing))
            .map(|e| e.path.as_str())
            .collect()
    }

    #[test]
    fn the_required_field_list_matches_the_ratified_table() {
        // `manifest-fields.toml` ratifies **22** required rows: 15 module-level and 7 per-entry
        // (`ports[].id/name/direction/type`, `params[].id/name/type`). One module-level row —
        // `identity.host_api` — is a `{min, max}` pair and so reports as two paths, which is why an
        // empty manifest yields 16 reports and a manifest with one port and one param yields 23.
        // Asserting the paths and not just the count is what keeps the table and the code honest.
        let empty = Manifest::default().report();
        let module_level = missing_paths(&empty);
        assert_eq!(module_level.len(), 16, "{module_level:?}");
        assert!(module_level.contains(&"identity.host_api.min"));
        assert!(module_level.contains(&"identity.host_api.max"));
        assert!(module_level.contains(&"identity.authors"));
        assert!(module_level.contains(&"resources.latency"));
        assert!(module_level.contains(&"state.schema_id"));

        let mut m = Manifest::default();
        m.ports.push(PortSpec::default());
        m.params.push(ParamSpec::default());
        let r = m.report();
        let all = missing_paths(&r);
        assert_eq!(all.len(), 23, "{all:?}");
        for p in [
            "ports[0].id",
            "ports[0].name",
            "ports[0].direction",
            "ports[0].type",
            "params[0].id",
            "params[0].name",
            "params[0].type",
        ] {
            assert!(all.contains(&p), "no E-KEY-MISSING for {p}: {all:?}");
        }

        // The conditionally-required numeric fields are not required until the type says numeric.
        assert!(!all.contains(&"params[0].unit"), "unit is required only if numeric");
        m.params[0].kind = Some("float".into());
        let r2 = m.report();
        let all2 = missing_paths(&r2);
        for p in ["params[0].unit", "params[0].min", "params[0].max", "params[0].default"] {
            assert!(all2.contains(&p), "{p} becomes required once the param is numeric: {all2:?}");
        }
    }
}
