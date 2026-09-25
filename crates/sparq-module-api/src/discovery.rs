//! Discovery: from manifest **text** to a report, without touching the filesystem.
//!
//! module-api §11 fixes the order — built-in registry → user `modules/` → project-local → registry
//! cache — and requires that a bad manifest never loads and that validation output is shown verbatim
//! in the module browser. Both are properties of this module: one broken file cannot hide the others,
//! and every failure keeps the label of the file it came from.
//!
//! # Why no I/O
//!
//! [`discover`] takes [`Source`]s, not paths. Reading a directory is a control-thread activity that
//! belongs to `sparq-app`; `clippy.toml` bans `std::fs::read` workspace-wide, and rather than rely on
//! `read_to_string` not being named in that list, this crate holds no filesystem access at all. The
//! split also makes discovery testable without creating a single file.
//!
//! # What discovery does *not* do
//!
//! It produces manifests, not modules. Pairing a manifest with a [`crate::registry::Factory`] is the
//! loader's job, because where the implementation comes from depends on the tier (ADR-002): a T1
//! module is compiled in and already has a factory, while a T2 or T3 module's implementation arrives
//! as a wasm component or a process. Keeping the two apart is what lets the registry stay tier-blind.

use crate::error::ValidationReport;
use crate::manifest::ValidatedManifest;
use crate::toml;

/// Where a manifest came from, in module-api §11's precedence order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Compiled into this binary.
    BuiltIn,
    /// The user's `modules/` directory.
    UserModules,
    /// A module local to the open project.
    ProjectLocal,
    /// Downloaded from the registry and cached.
    RegistryCache,
}

impl Origin {
    /// Lower is higher precedence. `Origin` derives `Ord` in declaration order, so this exists to
    /// make the intent explicit rather than to reorder anything.
    #[must_use]
    pub fn precedence(self) -> u8 {
        match self {
            Self::BuiltIn => 0,
            Self::UserModules => 1,
            Self::ProjectLocal => 2,
            Self::RegistryCache => 3,
        }
    }
}

/// One manifest to consider, already read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Where it came from, for precedence.
    pub origin: Origin,
    /// What to call it in an error — a path, or `built-in:sparq/util/gain`.
    pub label: String,
    /// The manifest text.
    pub text: String,
}

impl Source {
    /// A source from a string label and text, without allocating a `String` twice at the call site.
    #[must_use]
    pub fn new(origin: Origin, label: impl Into<String>, text: impl Into<String>) -> Self {
        Self { origin, label: label.into(), text: text.into() }
    }
}

/// A manifest that loaded.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// Where it came from.
    pub origin: Origin,
    /// Its label.
    pub label: String,
    /// The validated manifest.
    pub manifest: ValidatedManifest,
}

/// A manifest that did not.
#[derive(Clone, Debug)]
pub struct Failed {
    /// Where it came from.
    pub origin: Origin,
    /// Its label — kept so the browser can say *which* module failed.
    pub label: String,
    /// Every failure at once, syntax and type before semantic.
    pub report: ValidationReport,
}

/// One id that more than one source claimed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shadowed {
    /// The module id several sources declared.
    pub id: String,
    /// The label that wins by precedence.
    pub kept: String,
    /// The labels it hides, highest precedence first.
    pub hidden: Vec<String>,
}

/// The result of one discovery pass.
#[derive(Clone, Debug, Default)]
pub struct DiscoveryReport {
    /// Everything that loaded, in precedence order.
    pub loaded: Vec<Loaded>,
    /// Everything that did not, in input order.
    pub failed: Vec<Failed>,
}

impl DiscoveryReport {
    /// Whether nothing loaded at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty() && self.failed.is_empty()
    }

    /// How many manifests loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.loaded.len()
    }

    /// The ids that loaded, in precedence order.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.loaded.iter().map(|l| l.manifest.id()).collect()
    }

    /// The highest-precedence manifest for an id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Loaded> {
        self.loaded.iter().find(|l| l.manifest.id() == id)
    }

    /// Every id claimed by more than one source, with the winner and what it hides.
    ///
    /// Shadowing is reported rather than resolved quietly: a module that silently stops being the one
    /// that loads is a patch that changes sound because of a directory somebody else edited.
    #[must_use]
    pub fn shadowed(&self) -> Vec<Shadowed> {
        let mut out: Vec<Shadowed> = Vec::new();
        for (i, l) in self.loaded.iter().enumerate() {
            let id = l.manifest.id();
            // Only the first — highest-precedence — occurrence is a winner. Without this guard every
            // later duplicate also reported the ones below it, so a three-way collision listed the
            // lowest-precedence module twice and the browser would have shown a lie.
            if out.iter().any(|s| s.id == id) {
                continue;
            }
            let hidden: Vec<String> = self.loaded[i + 1..]
                .iter()
                .filter(|o| o.manifest.id() == id)
                .map(|o| o.label.clone())
                .collect();
            if hidden.is_empty() {
                continue;
            }
            out.push(Shadowed { id: id.to_string(), kept: l.label.clone(), hidden });
        }
        out
    }

    /// The whole report as text, one block per failure — the form the module browser shows verbatim,
    /// so an author sees exactly why their module did not load.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for f in &self.failed {
            out.push_str(&format!("{} ({:?}) refused:\n", f.label, f.origin));
            for line in f.report.to_string().lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
        for s in self.shadowed() {
            out.push_str(&format!("{}: {} wins, hiding {}\n", s.id, s.kept, s.hidden.join(", ")));
        }
        out
    }
}

/// Validates and orders a batch of manifests.
///
/// A manifest that fails does not stop the others: the whole batch is reported, because discovery
/// runs at startup and one broken download must not take the instrument's module list with it.
#[must_use]
pub fn discover<I>(sources: I) -> DiscoveryReport
where
    I: IntoIterator<Item = Source>,
{
    let mut loaded: Vec<Loaded> = Vec::new();
    let mut failed: Vec<Failed> = Vec::new();
    for s in sources {
        // Parse first, separately, so a syntax failure is reported as one actionable line rather than
        // being flattened into the semantic report.
        match toml::parse(&s.text) {
            Err(e) => {
                let mut report = ValidationReport::new();
                report.push(
                    crate::error::ValidationError::new(
                        crate::error::CodeKind::ValueMalformed,
                        s.label.clone(),
                        "fix the TOML syntax; the parser names the line",
                    )
                    .with_found(e.to_string()),
                );
                failed.push(Failed { origin: s.origin, label: s.label, report });
            },
            Ok(_) => match crate::decode(&s.text) {
                Ok(manifest) => loaded.push(Loaded { origin: s.origin, label: s.label, manifest }),
                Err(report) => failed.push(Failed { origin: s.origin, label: s.label, report }),
            },
        }
    }
    // Precedence order, stable within an origin so registration order stays reproducible — and
    // reproducible means a journal replay sees the same module list (ADR-007).
    loaded.sort_by_key(|l| l.origin);
    DiscoveryReport { loaded, failed }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn manifest(id: &str, version: &str) -> String {
        format!(
            r#"
[identity]
id = "{id}"
version = "{version}"
host_api = {{ min = 1, max = 1 }}
display_name = "Test"
summary = "A module used to test discovery"
authors = ["the test"]
license = "MIT"

[classification]
category = "test/x"
top = "util"
kind = "processor"
tier = "t1"
stability = "experimental"

[state]
schema_id = "{id}/state"
schema_version = 1

[resources]
latency = 0
"#
        )
    }

    #[test]
    fn discovery_reads_text_and_never_touches_a_filesystem() {
        let report = discover([
            Source::new(Origin::BuiltIn, "built-in:a", manifest("sparq/test/a", "0.1.0")),
            Source::new(Origin::BuiltIn, "built-in:b", manifest("sparq/test/b", "0.1.0")),
        ]);
        assert_eq!(report.len(), 2);
        assert!(report.failed.is_empty());
        assert_eq!(report.ids(), vec!["sparq/test/a", "sparq/test/b"]);
        assert!(report.get("sparq/test/b").is_some());
        assert!(report.get("sparq/test/absent").is_none());
    }

    #[test]
    fn one_broken_manifest_does_not_hide_the_others() {
        let report = discover([
            Source::new(Origin::BuiltIn, "good-1", manifest("sparq/test/a", "0.1.0")),
            Source::new(
                Origin::UserModules,
                "modules/bad/sparqmod.toml",
                "[identity\nid = 1\n".to_string(),
            ),
            Source::new(
                Origin::UserModules,
                "modules/wrong/sparqmod.toml",
                manifest("sparq/test/c", "0.1.0").replace("top = \"util\"", "top = \"synth\""),
            ),
            Source::new(Origin::BuiltIn, "good-2", manifest("sparq/test/b", "0.1.0")),
        ]);
        assert_eq!(report.len(), 2, "both good manifests still load");
        assert_eq!(report.failed.len(), 2, "and both failures are reported, not just the first");

        let syntax = &report.failed[0];
        assert_eq!(syntax.label, "modules/bad/sparqmod.toml", "a failure keeps its file");
        assert!(syntax.report.to_string().contains("line 1"), "{syntax:?}");

        let semantic = &report.failed[1];
        assert!(semantic.report.has(crate::error::CodeKind::EnumUnknown), "{semantic:?}");
        assert!(semantic.report.has_path("classification.top"));

        let text = report.render();
        assert!(text.contains("modules/bad/sparqmod.toml"), "{text}");
        assert!(text.contains("E-ENUM-UNKNOWN:classification.top"), "{text}");
    }

    #[test]
    fn precedence_follows_module_api_section_11() {
        let report = discover([
            Source::new(Origin::RegistryCache, "cache:g", manifest("sparq/test/g", "9.9.9")),
            Source::new(Origin::UserModules, "modules:g", manifest("sparq/test/g", "2.0.0")),
            Source::new(Origin::BuiltIn, "built-in:g", manifest("sparq/test/g", "1.0.0")),
            Source::new(Origin::ProjectLocal, "project:g", manifest("sparq/test/g", "3.0.0")),
        ]);
        assert_eq!(
            report.ids(),
            vec!["sparq/test/g"; 4],
            "all four load; precedence decides which one `get` returns"
        );
        assert_eq!(report.loaded[0].origin, Origin::BuiltIn);
        assert_eq!(report.loaded[1].origin, Origin::UserModules);
        assert_eq!(report.loaded[2].origin, Origin::ProjectLocal);
        assert_eq!(report.loaded[3].origin, Origin::RegistryCache);
        assert_eq!(
            report.get("sparq/test/g").unwrap().manifest.manifest().identity.version.as_deref(),
            Some("1.0.0"),
            "the built-in wins, whatever the input order was"
        );
    }

    #[test]
    fn shadowing_is_reported_rather_than_resolved_quietly() {
        let report = discover([
            Source::new(Origin::BuiltIn, "built-in:g", manifest("sparq/test/g", "1.0.0")),
            Source::new(Origin::UserModules, "modules:g", manifest("sparq/test/g", "2.0.0")),
            Source::new(Origin::ProjectLocal, "project:g", manifest("sparq/test/g", "3.0.0")),
            Source::new(Origin::BuiltIn, "built-in:other", manifest("sparq/test/other", "1.0.0")),
        ]);
        let shadowed = report.shadowed();
        assert_eq!(shadowed.len(), 1, "only the duplicated id is shadowed");
        assert_eq!(shadowed[0].id, "sparq/test/g");
        assert_eq!(shadowed[0].kept, "built-in:g");
        assert_eq!(shadowed[0].hidden, vec!["modules:g", "project:g"]);
        assert!(report.render().contains("hiding modules:g, project:g"), "{}", report.render());
    }

    #[test]
    fn an_empty_batch_is_an_empty_report() {
        let report = discover([]);
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(report.render().is_empty());
        assert!(report.ids().is_empty());
    }

    #[test]
    fn origin_ordering_is_the_documented_precedence() {
        assert!(Origin::BuiltIn < Origin::UserModules);
        assert!(Origin::UserModules < Origin::ProjectLocal);
        assert!(Origin::ProjectLocal < Origin::RegistryCache);
        for (o, p) in [
            (Origin::BuiltIn, 0),
            (Origin::UserModules, 1),
            (Origin::ProjectLocal, 2),
            (Origin::RegistryCache, 3),
        ] {
            assert_eq!(o.precedence(), p);
        }
    }
}
