//! Validation errors: a catalogue **derived** from the schema rather than accumulated by hand.
//!
//! `docs/api/manifest-schema.md` lists 19 codes that grew case-by-case as the design was written,
//! which left two holes big enough to block WO-007's acceptance criteria: nothing rejected a
//! *missing* required field, and nothing rejected an invalid value for the ~20 closed vocabularies
//! that had no code of their own (defect #55). Both are here as the five derived kinds, and the rule
//! going forward is that a new field in `docs/api/manifest-fields.toml` brings its own codes with it
//! instead of waiting for someone to notice.
//!
//! Every message names the field, the value found, the values allowed and the fix — because the
//! catalogue is shown verbatim in the module browser, so an author sees exactly why their module
//! did not load.

use std::fmt;

/// The kind of a validation failure. The first nineteen are the catalogue in
/// `docs/api/manifest-schema.md`; the last five are derived (defect #55).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CodeKind {
    /// Two ports or two params share an id.
    IdDup,
    /// A stable id changed without an `aliases` entry.
    IdUnstableRename,
    /// A port type outside the closed set of six.
    PortTypeUnknown,
    /// A unit outside the closed vocabulary.
    UnitUnknown,
    /// A param default outside its own `[min, max]`.
    ParamDefaultOutOfRange,
    /// A channel-set string that is not a layout tag.
    ChannelSetUnknown,
    /// `ambisonics:N` with N outside 1..=7.
    AmbiOrderRange,
    /// A `data` port referencing a schema nobody registered.
    SchemaUnregistered,
    /// A declared asset whose hash does not match.
    AssetHashMismatch,
    /// A `requires` feature this host does not have.
    RequiresUnsupported,
    /// The module's host-API range excludes this host.
    HostApiIncompatible,
    /// No state schema declared. Serialisation is mandatory for every module.
    StateSchemaMissing,
    /// A hole in the declared `from -> to` migration chain.
    MigrationChainGap,
    /// No signature, and the host is in Perform mode.
    SignatureMissing,
    /// A signature that does not verify.
    SignatureInvalid,
    /// A widget below its declared touch class minimum.
    TouchClassTooSmall,
    /// A widget kind outside the closed vocabulary.
    WidgetKindUnknown,
    /// Measured latency is non-zero but the manifest declares zero.
    LatencyUndeclared,
    /// A key nobody declared. Typo protection.
    UnknownKey,
    /// **Derived.** A required field is absent: `E-KEY-MISSING:<path>`.
    KeyMissing,
    /// **Derived.** A value outside a closed domain: `E-ENUM-UNKNOWN:<path>`.
    EnumUnknown,
    /// **Derived.** A cross-field rule broken: `E-CROSS-FIELD:<rule id>`.
    CrossField,
    /// **Derived.** A string over its declared maximum: `E-VALUE-TOO-LONG:<path>`.
    ValueTooLong,
    /// **Derived.** A scalar that does not parse: `E-VALUE-MALFORMED:<path>`.
    ValueMalformed,
}

impl CodeKind {
    /// Every code kind, catalogue first and derived last.
    pub const ALL: [Self; 24] = [
        Self::IdDup,
        Self::IdUnstableRename,
        Self::PortTypeUnknown,
        Self::UnitUnknown,
        Self::ParamDefaultOutOfRange,
        Self::ChannelSetUnknown,
        Self::AmbiOrderRange,
        Self::SchemaUnregistered,
        Self::AssetHashMismatch,
        Self::RequiresUnsupported,
        Self::HostApiIncompatible,
        Self::StateSchemaMissing,
        Self::MigrationChainGap,
        Self::SignatureMissing,
        Self::SignatureInvalid,
        Self::TouchClassTooSmall,
        Self::WidgetKindUnknown,
        Self::LatencyUndeclared,
        Self::UnknownKey,
        Self::KeyMissing,
        Self::EnumUnknown,
        Self::CrossField,
        Self::ValueTooLong,
        Self::ValueMalformed,
    ];

    /// The code's spelling, without any `<path>` suffix.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::IdDup => "E-ID-DUP",
            Self::IdUnstableRename => "E-ID-UNSTABLE-RENAME",
            Self::PortTypeUnknown => "E-PORT-TYPE-UNKNOWN",
            Self::UnitUnknown => "E-UNIT-UNKNOWN",
            Self::ParamDefaultOutOfRange => "E-PARAM-DEFAULT-OUT-OF-RANGE",
            Self::ChannelSetUnknown => "E-CHANNELSET-UNKNOWN",
            Self::AmbiOrderRange => "E-AMBI-ORDER-RANGE",
            Self::SchemaUnregistered => "E-SCHEMA-UNREGISTERED",
            Self::AssetHashMismatch => "E-ASSET-HASH-MISMATCH",
            Self::RequiresUnsupported => "E-REQUIRES-UNSUPPORTED",
            Self::HostApiIncompatible => "E-HOST-API-INCOMPATIBLE",
            Self::StateSchemaMissing => "E-STATE-SCHEMA-MISSING",
            Self::MigrationChainGap => "E-MIGRATION-CHAIN-GAP",
            Self::SignatureMissing => "E-SIGNATURE-MISSING",
            Self::SignatureInvalid => "E-SIGNATURE-INVALID",
            Self::TouchClassTooSmall => "E-TOUCH-CLASS-TOO-SMALL",
            Self::WidgetKindUnknown => "E-WIDGET-KIND-UNKNOWN",
            Self::LatencyUndeclared => "E-LATENCY-UNDECLARED",
            Self::UnknownKey => "E-UNKNOWN-KEY",
            Self::KeyMissing => "E-KEY-MISSING",
            Self::EnumUnknown => "E-ENUM-UNKNOWN",
            Self::CrossField => "E-CROSS-FIELD",
            Self::ValueTooLong => "E-VALUE-TOO-LONG",
            Self::ValueMalformed => "E-VALUE-MALFORMED",
        }
    }

    /// Whether this kind renders with a `:<path>` suffix. The five derived kinds always do; the
    /// catalogue kinds carry the path in the message body instead, so that existing messages do not
    /// change shape.
    #[must_use]
    pub fn is_derived(self) -> bool {
        matches!(
            self,
            Self::KeyMissing
                | Self::EnumUnknown
                | Self::CrossField
                | Self::ValueTooLong
                | Self::ValueMalformed
        )
    }
}

/// One actionable validation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    /// Which catalogue entry this is.
    pub kind: CodeKind,
    /// The manifest path, e.g. `ports[2].type` or `identity.summary`.
    pub path: String,
    /// The offending value, when there is one to show.
    pub found: Option<String>,
    /// The legal values, when the set is small enough to print.
    pub allowed: Option<String>,
    /// What the author should do. Never empty — a message without a fix is half a message.
    pub fix: String,
}

impl ValidationError {
    /// An error with a path and a fix; `found` and `allowed` are added by the builders below.
    #[must_use]
    pub fn new(kind: CodeKind, path: impl Into<String>, fix: impl Into<String>) -> Self {
        Self { kind, path: path.into(), found: None, allowed: None, fix: fix.into() }
    }

    /// Adds the value that was found.
    #[must_use]
    pub fn with_found(mut self, found: impl Into<String>) -> Self {
        self.found = Some(found.into());
        self
    }

    /// Adds the set of allowed values.
    #[must_use]
    pub fn with_allowed(mut self, allowed: impl Into<String>) -> Self {
        self.allowed = Some(allowed.into());
        self
    }

    /// The code as it appears in logs and in the module browser, e.g.
    /// `E-KEY-MISSING:ports[2].type`.
    #[must_use]
    pub fn code(&self) -> String {
        if self.kind.is_derived() {
            format!("{}:{}", self.kind.prefix(), self.path)
        } else {
            self.kind.prefix().to_string()
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.code(), self.path)?;
        if let Some(found) = &self.found {
            write!(f, " — found `{found}`")?;
        }
        if let Some(allowed) = &self.allowed {
            write!(f, ", allowed: {allowed}")?;
        }
        write!(f, ". Fix: {}", self.fix)
    }
}

impl std::error::Error for ValidationError {}

/// Every failure from one validation pass. Validation is all-or-nothing at discovery: a bad
/// manifest never loads, and the author gets the whole list at once rather than one error per run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    errors: Vec<ValidationError>,
}

impl ValidationReport {
    /// An empty report.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one failure.
    pub fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Whether the manifest passed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// How many failures were recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// The failures, in the order they were found.
    pub fn iter(&self) -> std::slice::Iter<'_, ValidationError> {
        self.errors.iter()
    }

    /// Whether any failure has this kind.
    #[must_use]
    pub fn has(&self, kind: CodeKind) -> bool {
        self.errors.iter().any(|e| e.kind == kind)
    }

    /// Whether any failure is at this exact path.
    #[must_use]
    pub fn has_path(&self, path: &str) -> bool {
        self.errors.iter().any(|e| e.path == path)
    }

    /// The first failure at this path, if any.
    #[must_use]
    pub fn first_at(&self, path: &str) -> Option<&ValidationError> {
        self.errors.iter().find(|e| e.path == path)
    }

    /// The distinct code kinds present, in first-seen order. Used by the tests that assert the
    /// acceptance criteria, and by the module browser to group failures.
    #[must_use]
    pub fn kinds(&self) -> Vec<CodeKind> {
        let mut out: Vec<CodeKind> = Vec::new();
        for e in &self.errors {
            if !out.contains(&e.kind) {
                out.push(e.kind);
            }
        }
        out
    }

    /// Consumes the report, yielding the failures.
    #[must_use]
    pub fn into_errors(self) -> Vec<ValidationError> {
        self.errors
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.errors.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationReport {}

impl IntoIterator for ValidationReport {
    type Item = ValidationError;
    type IntoIter = std::vec::IntoIter<ValidationError>;
    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_prefix() {
        let mut seen = Vec::new();
        for k in CodeKind::ALL {
            let p = k.prefix();
            assert!(p.starts_with("E-"), "{p} does not look like a code");
            assert!(!seen.contains(&p), "{p} is used by two kinds");
            seen.push(p);
        }
        assert_eq!(seen.len(), 24, "the catalogue is 19 codes plus 5 derived");
    }

    #[test]
    fn derived_codes_render_with_their_path() {
        let e = ValidationError::new(CodeKind::KeyMissing, "ports[2].type", "declare a port type");
        assert_eq!(e.code(), "E-KEY-MISSING:ports[2].type");
        let e2 =
            ValidationError::new(CodeKind::PortTypeUnknown, "ports[2].type", "use one of the six")
                .with_found("midi")
                .with_allowed("audio cv event data gpu atom");
        assert_eq!(e2.code(), "E-PORT-TYPE-UNKNOWN");
        let s = e2.to_string();
        assert!(s.contains("found `midi`"), "{s}");
        assert!(s.contains("audio cv event data gpu atom"), "{s}");
        assert!(s.contains("Fix: use one of the six"), "{s}");
    }

    #[test]
    fn every_message_carries_a_fix() {
        // The acceptance criterion is "actionable", and a message without a fix is not actionable.
        for k in CodeKind::ALL {
            let e = ValidationError::new(k, "some.path", "do the thing");
            assert!(e.to_string().contains("Fix: do the thing"));
        }
    }

    #[test]
    fn a_report_returns_every_failure_at_once() {
        let mut r = ValidationReport::new();
        assert!(r.is_empty());
        r.push(ValidationError::new(CodeKind::IdDup, "ports[1].id", "rename one of them"));
        r.push(ValidationError::new(CodeKind::UnitUnknown, "params[0].unit", "use a closed unit"));
        assert_eq!(r.len(), 2);
        assert!(r.has(CodeKind::IdDup));
        assert!(r.has_path("params[0].unit"));
        assert_eq!(r.kinds(), vec![CodeKind::IdDup, CodeKind::UnitUnknown]);
        assert_eq!(r.to_string().lines().count(), 2, "one failure per line");
        assert_eq!(r.into_errors().len(), 2);
    }
}
