//! The built-in registry, and what "a new module needs zero engine edits" actually requires.
//!
//! Acceptance criterion 1 of WO-007 is not a promise about intentions; it is a property with a
//! mechanical meaning: adding a module must not require changing any file that is not the module's
//! own. Two things make that true here, and both are tested rather than assumed.
//!
//! 1. **Registration is data.** A module enters the registry as a manifest plus a [`Factory`] — a
//!    plain function pointer, so storing one cannot allocate and registering cannot fail for want of
//!    memory. Nothing in this crate names any particular module.
//! 2. **The manifest and the implementation are cross-checked at registration**, not at first use.
//!    [`Registry::register`] builds the module once and refuses the registration if its
//!    [`Module::id`] disagrees with `identity.id`, because a module that lies about who it is breaks
//!    patch provenance silently: the project file records the id, and the wrong code runs.
//!
//! # Conflicts
//!
//! module-api §11: *"Conflicts resolve by explicit version pin in the project, never by 'latest
//! wins'."* So a second registration of an already-registered id is an error carrying both versions,
//! not a replacement. Silent replacement is how a patch starts sounding different after an update
//! nobody decided on.
//!
//! # Discovery order
//!
//! Also §11: built-in registry → user `modules/` → project-local → registry cache. This crate holds
//! the first step and the *shape* of the rest; reading a directory is I/O and lives in `sparq-app`
//! (see [`crate::discovery`], which takes text and never touches the filesystem).

use std::fmt;

use crate::decode;
use crate::error::ValidationReport;
use crate::manifest::ValidatedManifest;
use crate::module::Module;

/// Builds a module instance. A plain function pointer: `Copy`, no allocation to store, and no
/// captured environment that could hide a dependency.
pub type Factory = fn() -> Box<dyn Module>;

/// One registered module: what it declared, and how to build it.
#[derive(Clone)]
pub struct Registration {
    manifest: ValidatedManifest,
    factory: Factory,
}

impl Registration {
    /// The validated manifest this module was registered with.
    #[must_use]
    pub fn manifest(&self) -> &ValidatedManifest {
        &self.manifest
    }

    /// Builds a fresh instance.
    #[must_use]
    pub fn create(&self) -> Box<dyn Module> {
        (self.factory)()
    }

    /// The module's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        self.manifest.id()
    }

    /// The module's declared version. Empty if the manifest omitted it, which validation forbids —
    /// so an empty string here means the manifest was never validated, and that is a bug.
    #[must_use]
    pub fn version(&self) -> &str {
        self.manifest.manifest().identity.version.as_deref().unwrap_or("")
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration")
            .field("id", &self.id())
            .field("version", &self.version())
            .finish()
    }
}

/// Why a registration was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The manifest did not validate. Carries every failure at once.
    Invalid(ValidationReport),
    /// The implementation's `id()` disagrees with the manifest's `identity.id`.
    IdMismatch {
        /// What the manifest declared.
        manifest: String,
        /// What the code said.
        implementation: String,
    },
    /// The id is already registered. Conflicts resolve by an explicit pin in the project, never by
    /// latest-wins, so this is an error rather than a replacement.
    Duplicate {
        /// The module id both registrations claim.
        id: String,
        /// The version already in the registry.
        registered: String,
        /// The version that was refused.
        incoming: String,
    },
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(r) => write!(f, "the manifest does not validate:\n{r}"),
            Self::IdMismatch {
                manifest,
                implementation,
            } => write!(
                f,
                "the implementation says `{implementation}` but the manifest declares `{manifest}`; \
                 a module that disagrees about its own id breaks patch provenance silently"
            ),
            Self::Duplicate {
                id,
                registered,
                incoming,
            } => write!(
                f,
                "`{id}` is already registered at version {registered}; refusing {incoming}. \
                 Conflicts resolve by an explicit version pin in the project, never by latest-wins \
                 (module-api §11)"
            ),
        }
    }
}

impl std::error::Error for RegisterError {}

/// The built-in registry: every T1 module compiled into this binary, plus anything the host
/// registers at startup.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    entries: Vec<Registration>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a module from its manifest text: decode, validate, cross-check, insert.
    ///
    /// This is the whole of "adding a module" from the engine's point of view. Nothing here names
    /// the module, which is what makes criterion 1 mechanical rather than aspirational.
    ///
    /// # Errors
    /// [`RegisterError::Invalid`] if the manifest does not validate, [`RegisterError::IdMismatch`]
    /// if the implementation disagrees with it, or [`RegisterError::Duplicate`] if the id is taken.
    pub fn register(&mut self, manifest_text: &str, factory: Factory) -> Result<(), RegisterError> {
        let manifest = decode(manifest_text).map_err(RegisterError::Invalid)?;
        self.register_validated(manifest, factory)
    }

    /// Registers a module whose manifest is already validated.
    ///
    /// # Errors
    /// As [`Self::register`], minus the parse and validate stages.
    pub fn register_validated(
        &mut self,
        manifest: ValidatedManifest,
        factory: Factory,
    ) -> Result<(), RegisterError> {
        // Build once, now, so a disagreement about identity is a registration failure rather than a
        // wrong module running in someone's patch.
        let probe = factory();
        let declared = manifest.id().to_string();
        let actual = probe.id().to_string();
        if declared != actual {
            return Err(RegisterError::IdMismatch { manifest: declared, implementation: actual });
        }
        let version = manifest.manifest().identity.version.clone().unwrap_or_default();
        if let Some(existing) = self.entries.iter().find(|r| r.id() == declared) {
            return Err(RegisterError::Duplicate {
                id: declared,
                registered: existing.version().to_string(),
                incoming: version,
            });
        }
        self.entries.push(Registration { manifest, factory });
        Ok(())
    }

    /// How many modules are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Looks a module up by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Registration> {
        self.entries.iter().find(|r| r.id() == id)
    }

    /// Looks a module up by id **and** version, which is how a project pins what it needs.
    #[must_use]
    pub fn get_pinned(&self, id: &str, version: &str) -> Option<&Registration> {
        self.entries.iter().find(|r| r.id() == id && r.version() == version)
    }

    /// Builds a fresh instance of `id`.
    #[must_use]
    pub fn create(&self, id: &str) -> Option<Box<dyn Module>> {
        self.get(id).map(Registration::create)
    }

    /// Every registered id, in registration order.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(Registration::id).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::module::{AudioCtx, BlockStatus, ModuleError, Resources};

    /// A module that exists only in this test file.
    struct Throwaway {
        gained: bool,
    }

    impl Module for Throwaway {
        fn id(&self) -> &str {
            "sparq/test/throwaway"
        }
        fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
        fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
            Ok(())
        }
        fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
            self.gained = true;
            for s in ctx.output.iter_mut() {
                *s = 0.5;
            }
            BlockStatus::Ok
        }
        fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    fn throwaway() -> Box<dyn Module> {
        Box::new(Throwaway { gained: false })
    }

    /// A lying module: the manifest will say something else.
    struct Impostor;
    impl Module for Impostor {
        fn id(&self) -> &str {
            "sparq/test/someone-else"
        }
        fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
        fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
            Ok(())
        }
        fn process(&mut self, _ctx: &mut AudioCtx<'_>) -> BlockStatus {
            BlockStatus::Ok
        }
        fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    fn impostor() -> Box<dyn Module> {
        Box::new(Impostor)
    }

    /// The smallest manifest that validates, for a given id.
    fn manifest_for(id: &str, version: &str) -> String {
        format!(
            r#"
[identity]
id = "{id}"
version = "{version}"
host_api = {{ min = 1, max = 1 }}
display_name = "Throwaway"
summary = "Proves a module can be added without touching engine code"
authors = ["the test"]
license = "MIT"

[classification]
category = "test/throwaway"
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
    fn a_module_registered_from_text_can_be_created_and_run() {
        let mut reg = Registry::new();
        assert!(reg.is_empty());
        reg.register(&manifest_for("sparq/test/throwaway", "0.1.0"), throwaway)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.ids(), vec!["sparq/test/throwaway"]);

        let r = reg.get("sparq/test/throwaway").unwrap();
        assert_eq!(r.version(), "0.1.0");
        let mut m = r.create();
        assert_eq!(m.id(), "sparq/test/throwaway");

        let block = sparq_kernel::block::BlockContext::offline(48_000, 64, 1);
        let params = crate::params::ParamSet::zeroed();
        let mut out = vec![0.0f32; 64];
        {
            let mut ctx = AudioCtx { block: &block, params: &params, input: &[], output: &mut out };
            assert_eq!(m.process(&mut ctx), BlockStatus::Ok);
        }
        assert!(out.iter().all(|v| *v == 0.5), "the registered module really ran");
        assert!(reg.create("sparq/test/throwaway").is_some());
        assert!(reg.create("sparq/test/absent").is_none());
    }

    #[test]
    fn an_invalid_manifest_is_refused_with_every_reason() {
        let mut reg = Registry::new();
        let broken = manifest_for("sparq/test/throwaway", "0.1.0")
            .replace("top = \"util\"", "top = \"synth\"");
        let e = reg.register(&broken, throwaway).unwrap_err();
        match e {
            RegisterError::Invalid(r) => {
                assert!(r.has(crate::error::CodeKind::EnumUnknown), "{r}");
                assert!(r.first_at("classification.top").is_some(), "{r}");
            },
            other => panic!("expected Invalid, got {other}"),
        }
        assert!(reg.is_empty(), "a refused registration must leave nothing behind");
    }

    #[test]
    fn a_module_that_disagrees_about_its_own_id_is_refused() {
        let mut reg = Registry::new();
        let e = reg.register(&manifest_for("sparq/test/throwaway", "0.1.0"), impostor).unwrap_err();
        match &e {
            RegisterError::IdMismatch { manifest, implementation } => {
                assert_eq!(manifest, "sparq/test/throwaway");
                assert_eq!(implementation, "sparq/test/someone-else");
            },
            other => panic!("expected IdMismatch, got {other}"),
        }
        assert!(
            e.to_string().contains("provenance"),
            "the message says why this matters, not just what happened: {e}"
        );
        assert!(reg.is_empty());
    }

    #[test]
    fn a_second_version_of_the_same_id_is_refused_not_silently_replaced() {
        let mut reg = Registry::new();
        reg.register(&manifest_for("sparq/test/throwaway", "0.1.0"), throwaway).unwrap();
        let e =
            reg.register(&manifest_for("sparq/test/throwaway", "0.2.0"), throwaway).unwrap_err();
        match &e {
            RegisterError::Duplicate { id, registered, incoming } => {
                assert_eq!(id, "sparq/test/throwaway");
                assert_eq!(registered, "0.1.0");
                assert_eq!(incoming, "0.2.0");
            },
            other => panic!("expected Duplicate, got {other}"),
        }
        assert!(e.to_string().contains("latest-wins"), "the message names the rule: {e}");
        assert_eq!(reg.len(), 1, "the first registration stands");
        assert_eq!(reg.get("sparq/test/throwaway").unwrap().version(), "0.1.0");
    }

    #[test]
    fn a_project_can_pin_the_version_it_needs() {
        let mut reg = Registry::new();
        reg.register(&manifest_for("sparq/test/throwaway", "0.1.0"), throwaway).unwrap();
        assert!(reg.get_pinned("sparq/test/throwaway", "0.1.0").is_some());
        assert!(
            reg.get_pinned("sparq/test/throwaway", "9.9.9").is_none(),
            "a pin that does not match must fail, not fall back to whatever is installed"
        );
    }

    #[test]
    fn a_factory_is_a_function_pointer_so_storing_one_cannot_allocate() {
        // The point of `Factory = fn() -> Box<dyn Module>` rather than a closure: a closure that
        // captures an environment is a heap allocation the registry would own forever. A fn pointer
        // is one word and Copy. `Registration` itself is Clone but not Copy, because it holds a
        // `ValidatedManifest` — strings and vectors — and pretending otherwise would be a lie.
        assert_eq!(std::mem::size_of::<Factory>(), std::mem::size_of::<usize>());
        fn assert_copy<T: Copy>() {}
        fn assert_clone<T: Clone>() {}
        assert_copy::<Factory>();
        assert_clone::<Registration>();
        assert_clone::<Registry>();
    }
}
