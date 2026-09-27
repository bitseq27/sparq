//! WO-007 acceptance criterion 1, made mechanical:
//!
//! > A new module can be added by creating a crate + manifest, with **zero** edits to engine code
//! > (verified by a CI test that adds a throwaway module).
//!
//! Two halves, because the first alone proves nothing. This file adds a module that appears nowhere
//! else in the repository — a manifest as text, an implementation, a registration — and runs it.
//! Then [`nothing_in_the_engine_names_the_throwaway`] walks every `.rs` file under every crate's
//! `src/` and asserts that **not one of them mentions it**. That walk is what turns "zero engine
//! edits" from an intention into a fact a gate can fail on: if a future change makes adding a module
//! require a line in `port.rs`, `manifest.rs` or the registry's defaults, this test names the file.
//!
//! The filesystem walk lives in a *test*, not in the crate. `sparq-module-api` holds no I/O at all —
//! discovery takes text ([`sparq_module_api::discover`]) and reading a directory belongs to
//! `sparq-app`'s control thread.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use sparq_kernel::block::BlockContext;
use sparq_module_api::discovery::{Origin, Source};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::{Factory, Registry};
use sparq_module_api::{decode, discover};

/// The id of the module this file adds. Nothing under any `crates/*/src` may contain this string.
const THROWAWAY_ID: &str = "sparq/scratch/zero-edit-ci";

/// Its manifest, exactly as an author would write `sparqmod.toml`.
const THROWAWAY_MANIFEST: &str = r#"
[identity]
id = "sparq/scratch/zero-edit-ci"
version = "0.0.1"
host_api = { min = 1, max = 1 }
display_name = "Zero-edit probe"
summary = "Added by a CI test to prove no engine file had to change"
authors = ["the throwaway-module test"]
license = "MIT"

[classification]
category = "scratch/probe"
top = "util"
kind = "processor"
tier = "t1"
stability = "experimental"

[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"
channel_set = "mono"

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "mono"

[[params]]
id = "bias"
name = "Bias"
type = "float"
unit = "ratio"
min = -1.0
max = 1.0
default = 0.25

[state]
schema_id = "sparq/scratch/zero-edit-ci/state"
schema_version = 1

[resources]
latency = 0
cpu_class = "trivial"
"#;

/// The implementation. Adds the `bias` parameter to every input sample — enough to prove parameters
/// reach `process`, without being interesting enough to distract from the point.
struct ZeroEditProbe;

impl Module for ZeroEditProbe {
    fn id(&self) -> &str {
        THROWAWAY_ID
    }

    fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }

    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let bias = ctx.param(0);
        let input = ctx.input();
        for (o, i) in ctx.output().iter_mut().zip(input.iter()) {
            *o = *i + bias;
        }
        BlockStatus::Ok
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
}

fn factory() -> Box<dyn Module> {
    Box::new(ZeroEditProbe)
}

#[test]
fn a_new_module_registers_from_text_and_runs() {
    // The whole of "adding a module", from the engine's side: one manifest string and one factory.
    let mut registry = Registry::new();
    let f: Factory = factory;
    registry
        .register(THROWAWAY_MANIFEST, f)
        .unwrap_or_else(|e| panic!("the throwaway module should register: {e}"));

    assert_eq!(registry.ids(), vec![THROWAWAY_ID]);
    let registration = registry.get(THROWAWAY_ID).unwrap();
    assert_eq!(registration.version(), "0.0.1");
    assert_eq!(registration.manifest().ports().len(), 2);
    assert_eq!(
        registration.manifest().ports()[0].channel_set,
        Some(sparq_module_api::port::ChannelSet::Mono)
    );

    // And it runs, with its parameter reaching process().
    let mut module = registry.create(THROWAWAY_ID).unwrap();
    module.prepare(&Resources::from_block(&BlockContext::offline(48_000, 64, 1))).unwrap();
    let block = BlockContext::offline(48_000, 64, 1);
    let params = ParamSet::new(1, &[0.25]).unwrap();
    let input = vec![1.0f32; 64];
    let mut out = vec![0.0f32; 64];
    {
        let mut ctx = AudioCtx::single(&block, &params, &input, &mut out);
        assert_eq!(module.process(&mut ctx), BlockStatus::Ok);
    }
    assert!(out.iter().all(|v| (*v - 1.25).abs() < 1e-6), "input 1.0 + bias 0.25: {out:?}");
}

#[test]
fn discovery_finds_it_among_others_and_reports_it_by_label() {
    let report = discover([
        Source::new(Origin::BuiltIn, "built-in:probe", THROWAWAY_MANIFEST),
        Source::new(Origin::UserModules, "modules/broken/sparqmod.toml", "[identity\n"),
    ]);
    assert_eq!(report.len(), 1, "the good manifest loads despite the broken one");
    assert_eq!(report.ids(), vec![THROWAWAY_ID]);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].label, "modules/broken/sparqmod.toml");
    assert!(report.render().contains("modules/broken/sparqmod.toml"), "{}", report.render());
}

#[test]
fn a_second_registration_of_the_same_id_is_refused() {
    let mut registry = Registry::new();
    registry.register(THROWAWAY_MANIFEST, factory).unwrap();
    let upgraded = THROWAWAY_MANIFEST.replace("version = \"0.0.1\"", "version = \"0.0.2\"");
    let err = registry.register(&upgraded, factory).unwrap_err();
    assert!(err.to_string().contains("latest-wins"), "{err}");
    assert_eq!(registry.len(), 1);
}

/// Every `.rs` file under every crate's `src/`, recursively.
///
/// Only `src/`: `tests/`, `benches/` and `examples/` are allowed to know about a module, and
/// including them would make this test fail for a reason that is not a violation of criterion 1.
fn engine_sources() -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    let Ok(crate_dirs) = std::fs::read_dir(&crates) else {
        return out;
    };
    for entry in crate_dirs.flatten() {
        let src = entry.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            let Ok(children) = std::fs::read_dir(&dir) else { continue };
            for child in children.flatten() {
                let path = child.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn nothing_in_the_engine_names_the_throwaway() {
    let sources = engine_sources();
    assert!(
        sources.len() > 40,
        "the walk found only {} files — it is not walking what it thinks",
        sources.len()
    );
    assert!(
        sources.iter().all(|p| p.components().any(|c| c.as_os_str() == "src")),
        "the walk must cover src/ only; tests and benches are allowed to know about a module \
         (matched by path component, not by the string \"/src/\" — defect #67: the string form \
         assumed the Unix separator and failed on this test's first Windows run)"
    );

    let mut offenders = Vec::new();
    for path in &sources {
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if text.contains(THROWAWAY_ID)
            || text.contains("ZeroEditProbe")
            || text.contains("zero-edit-ci")
        {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "adding this module required editing engine code, which is what acceptance criterion 1 forbids: {}",
        offenders.join(", ")
    );
}

#[test]
fn the_manifest_itself_is_not_special_cased_anywhere_either() {
    // The stronger form of the same claim: not just the id, but any of the module's distinguishing
    // strings. A registry with a hand-written list of built-ins would fail here the moment someone
    // added to that list, which is exactly the drift criterion 1 exists to prevent.
    let sources = engine_sources();
    for needle in ["scratch/probe", "Zero-edit probe", "zero_edit"] {
        for path in &sources {
            let text = std::fs::read_to_string(path).unwrap();
            assert!(!text.contains(needle), "`{needle}` appears in {}", path.display());
        }
    }
    // And the decoder is not the reason it worked: decoding is generic over the manifest, so the
    // same text validates through the plain path too.
    let v = decode(THROWAWAY_MANIFEST).unwrap_or_else(|r| panic!("{r}"));
    assert_eq!(v.id(), THROWAWAY_ID);
}
