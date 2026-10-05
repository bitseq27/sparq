//! The five-file package model (ADR-010 decision 5; MODULE-BUILD-GUIDE §3) and the `instruments/`
//! drop-in discovery slot (module-api §11).
//!
//! A handed-in instrument is a directory of **no more than five files**: `sparqmod.toml`,
//! `<name>.wasm`, `example.sparqpatch`, `preview.svg`, `README.md`. The cap is contract, and the
//! sixth file is `E-PACKAGE-FILECOUNT` — bulk assets ride **by hash** (`state.assets[]`) from the
//! content-addressed library, never as extra files.
//!
//! # The two profiles, and the one rule that separates them
//!
//! * The **cap**, the **manifest** and the **component** are fatal in BOTH profiles: without them
//!   there is nothing to load, and no launch is ever excused a sixth file.
//! * A missing `README.md` / `example.sparqpatch` is **words at launch** (the instrument plays;
//!   the library card is thinner, and the browser says so verbatim) and a **failure at hand-in**
//!   (`sparq mod validate` is the strict door — the guide's stage 2 wants the package complete).
//! * `preview.svg` is **never a required input**: it is stage 6's OUTPUT (validator-generated,
//!   never hand-drawn). Requiring it would make a new package's first validation impossible, so
//!   its absence is advisory words in both profiles.
//!
//! # What this module is not
//!
//! Manifest *content* — schema, layer, capability honour, budgets — is [`crate::validate`]'s
//! stage 1; this module checks the *shape of the directory*, guide stage 2. Discovery here is
//! filesystem-only: it inventories package directories, it does not register anything (pairing a
//! manifest with an implementation is the loader's job, `sparq-module-api::discovery`'s own
//! header). And nothing here is the audio thread: this is the control-thread door, where
//! filesystem access is legal.

use std::path::{Path, PathBuf};

use sparq_module_api::error::{CodeKind, ValidationError, ValidationReport};

/// The manifest's fixed file name (manifest-schema §9's package note).
pub const MANIFEST_FILE: &str = "sparqmod.toml";
/// The example patch's fixed file name — file 3 of 5, "the instrument doing its job".
pub const EXAMPLE_FILE: &str = "example.sparqpatch";
/// The library-card render — file 4 of 5, validator-GENERATED, never a required input.
pub const PREVIEW_FILE: &str = "preview.svg";
/// The package's readme — file 5 of 5.
pub const README_FILE: &str = "README.md";
/// The component's extension — file 2 of 5 is the only `*.wasm` in a package.
pub const COMPONENT_EXT: &str = "wasm";
/// The cap. ADR-010 decision 5; the sixth file is `E-PACKAGE-FILECOUNT`.
pub const MAX_PACKAGE_FILES: usize = 5;

/// The role one file plays in the five-file model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// `sparqmod.toml` — the manifest (file 1).
    Manifest,
    /// The one `*.wasm` — the component (file 2).
    Component,
    /// `example.sparqpatch` (file 3).
    Example,
    /// `preview.svg` (file 4, validator-generated).
    Preview,
    /// `README.md` (file 5).
    Readme,
    /// Anything else. Counts against the cap and is named in words in both profiles: a package
    /// is five RECOGNISED files, and an unrecognised one is either a mistake or a sixth file's
    /// first draft.
    Unknown,
}

impl Role {
    /// The role a file name plays.
    ///
    /// Case-sensitivity is deliberate: a package is a distributed artefact, and `readme.md` on a
    /// case-sensitive filesystem is a different file than the contract names. Better to say so in
    /// words than to accept two spellings of one role and let the author find out on the other
    /// platform.
    #[must_use]
    pub fn of(name: &str) -> Self {
        if name == MANIFEST_FILE {
            Self::Manifest
        } else if name == EXAMPLE_FILE {
            Self::Example
        } else if name == PREVIEW_FILE {
            Self::Preview
        } else if name == README_FILE {
            Self::Readme
        } else if name.len() > COMPONENT_EXT.len() + 1
            && name.ends_with(&format!(".{COMPONENT_EXT}"))
        {
            Self::Component
        } else {
            Self::Unknown
        }
    }

    /// The role's fixed file name — `None` for [`Role::Component`] (named after the module) and
    /// [`Role::Unknown`] (named after nothing).
    #[must_use]
    pub fn file_name(self) -> Option<&'static str> {
        match self {
            Self::Manifest => Some(MANIFEST_FILE),
            Self::Example => Some(EXAMPLE_FILE),
            Self::Preview => Some(PREVIEW_FILE),
            Self::Readme => Some(README_FILE),
            Self::Component | Self::Unknown => None,
        }
    }
}

/// Which door is asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Launch discovery of `instruments/`: load what can play; a missing README or example is
    /// words, not a failure (see [`advisories`]).
    Launch,
    /// Hand-in — `sparq mod validate`, the guide's stage 2: the package must be complete.
    Strict,
}

/// One file in a package directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// The file's name inside the package directory.
    pub name: String,
    /// The role the name plays.
    pub role: Role,
    /// Byte length (0 for a directory entry — which is itself an error, named in words).
    pub len: u64,
    /// Whether the entry is a directory. A package is five FILES; a subdirectory is an
    /// [`Role::Unknown`] entry with its own line in the report.
    pub is_dir: bool,
}

/// A package directory's files, sorted by name.
///
/// Sorted on purpose: discovery order feeds registration order, and a package list that varies
/// between runs is a journal replay that does not replay (ADR-007 — the same discipline as
/// `sparq modules`' scan).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    /// Every entry in the directory, roles assigned, sorted by name.
    pub files: Vec<FileEntry>,
}

impl Inventory {
    /// Reads a package directory's entries (names, roles, sizes). Does not read file contents.
    ///
    /// # Errors
    /// The directory's own I/O error. Callers that must not fail — discovery — route it into
    /// words via [`open`].
    pub fn read(dir: &Path) -> std::io::Result<Self> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = entry.metadata()?;
            files.push(FileEntry {
                name,
                role: Role::Unknown, // filled below, so `is_dir` entries stay Unknown
                len: if meta.is_dir() { 0 } else { meta.len() },
                is_dir: meta.is_dir(),
            });
        }
        for f in &mut files {
            if !f.is_dir {
                f.role = Role::of(&f.name);
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { files })
    }

    /// The entry playing a role, when exactly one does.
    #[must_use]
    pub fn get(&self, role: Role) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.role == role)
    }

    /// Every entry playing a role — more than one [`Role::Component`] is an ambiguity the check
    /// names in words.
    #[must_use]
    pub fn all(&self, role: Role) -> Vec<&FileEntry> {
        self.files.iter().filter(|f| f.role == role).collect()
    }
}

/// One package directory, read: its inventory, its manifest text, and any I/O words.
///
/// Reading never panics and never fails the round — an unreadable directory becomes a [`Package`]
/// whose check report names the problem, because one broken package must never hide the others.
#[derive(Clone, Debug)]
pub struct Package {
    /// The package directory.
    pub dir: PathBuf,
    /// Its files. Empty when the directory itself could not be read (then `io_words` says why).
    pub inventory: Inventory,
    /// `sparqmod.toml`'s text, when present and readable as UTF-8.
    pub manifest_text: Option<String>,
    /// I/O problems, as words for the report: the directory unreadable, or the manifest present
    /// but not readable. Kept separate from contract failures because the fix is different —
    /// this is the filesystem talking, not the schema.
    pub io_words: Option<String>,
}

/// Reads one package directory: inventory + manifest text, I/O problems as words.
#[must_use]
pub fn open(dir: &Path) -> Package {
    let mut io_words: Option<String> = None;
    let inventory = match Inventory::read(dir) {
        Ok(inv) => inv,
        Err(e) => {
            io_words =
                Some(format!("`{}` is not readable as a package directory: {e}", dir.display()));
            Inventory::default()
        },
    };
    let manifest_text = inventory.get(Role::Manifest).and_then(|_| {
        match std::fs::read_to_string(dir.join(MANIFEST_FILE)) {
            Ok(text) => Some(text),
            Err(e) => {
                if io_words.is_none() {
                    io_words = Some(format!(
                        "`{MANIFEST_FILE}` is present but not readable as UTF-8 text: {e}"
                    ));
                }
                None
            },
        }
    });
    Package { dir: dir.to_path_buf(), inventory, manifest_text, io_words }
}

/// The package-level rules — the guide's stage 2 — for one profile.
///
/// Fatal in both profiles: the cap (`E-PACKAGE-FILECOUNT`), unrecognised entries, the manifest,
/// the component (exactly one, non-empty). Strict adds: `README.md` and `example.sparqpatch`
/// required. `preview.svg` is required by NEITHER — it is stage 6's output; see [`advisories`].
#[must_use]
pub fn check(pkg: &Package, profile: Profile) -> ValidationReport {
    let mut r = ValidationReport::new();
    if let Some(words) = &pkg.io_words {
        // The filesystem's own refusal: named verbatim, with the door's fix.
        r.push(
            ValidationError::new(
                CodeKind::ValueMalformed,
                "package",
                "an instruments/ entry must be a readable directory of at most five files \
                 (MODULE-BUILD-GUIDE §3)",
            )
            .with_found(words.clone()),
        );
        return r; // with the directory unreadable, every rule below would be noise
    }

    // The cap: five files, contract (ADR-010 decision 5). One error, naming the overflow.
    if pkg.inventory.files.len() > MAX_PACKAGE_FILES {
        let extras: Vec<&str> =
            pkg.inventory.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
        r.push(
            ValidationError::new(
                CodeKind::PackageFilecount,
                "package",
                "bulk assets ride BY HASH (state.assets[], fetched from the content-addressed \
                 library) — the cap governs the distributed package (MODULE-BUILD-GUIDE §3); \
                 remove the extras",
            )
            .with_found(format!("{} files: {}", extras.len(), extras.join(", ")))
            .with_allowed(format!("at most {MAX_PACKAGE_FILES}: sparqmod.toml, <name>.wasm, example.sparqpatch, preview.svg, README.md")),
        );
    }

    // Unrecognised entries, one line each: a package is five RECOGNISED files.
    for f in pkg.inventory.files.iter().filter(|f| f.role == Role::Unknown) {
        let fix = if f.is_dir {
            "a package is five FILES — a subdirectory is not one of them; bulk assets ride by \
             hash (MODULE-BUILD-GUIDE §3)"
        } else {
            "a package file is one of the five roles: sparqmod.toml, <name>.wasm, \
             example.sparqpatch, preview.svg, README.md — remove or rename this one"
        };
        r.push(
            ValidationError::new(CodeKind::ValueMalformed, format!("package/{}", f.name), fix)
                .with_found(if f.is_dir { "a directory" } else { "an unrecognised file" }),
        );
    }

    // The manifest: fatal in both profiles. (Its CONTENT is validate's stage 1.)
    if pkg.inventory.get(Role::Manifest).is_none() {
        r.push(ValidationError::new(
            CodeKind::KeyMissing,
            MANIFEST_FILE,
            "file 1 of 5 is the manifest — every package carries sparqmod.toml \
             (MODULE-BUILD-GUIDE §3); a bad manifest never loads, and a missing one never \
             loaded either",
        ));
    } else if pkg.manifest_text.is_none() {
        r.push(
            ValidationError::new(
                CodeKind::ValueMalformed,
                MANIFEST_FILE,
                "the manifest must be readable UTF-8 text — the contract crate consumes text, \
                 not bytes",
            )
            .with_found(pkg.io_words.clone().unwrap_or_else(|| "unreadable".to_string())),
        );
    }

    // The component: exactly one *.wasm, non-empty. Fatal in both profiles.
    let components = pkg.inventory.all(Role::Component);
    match components.len() {
        0 => r.push(ValidationError::new(
            CodeKind::KeyMissing,
            "*.wasm",
            "file 2 of 5 is the component — build the source project and emit <name>.wasm \
             (MODULE-BUILD-GUIDE §3; the template's instrument.rs is source, not package)",
        )),
        1 => {
            if components[0].len == 0 {
                r.push(
                    ValidationError::new(
                        CodeKind::ValueMalformed,
                        format!("package/{}", components[0].name),
                        "the component is EMPTY — an empty wasm never instantiates; rebuild it \
                         (cargo component build → wasm-tools component new, \
                         docs/api/instrument-wit/README.md §7)",
                    )
                    .with_found("0 bytes"),
                );
            }
        },
        n => r.push(
            ValidationError::new(
                CodeKind::CrossField,
                "package",
                "one component per package — distribution.entrypoint names the wasm the loader \
                 instantiates; two candidates make the door a coin flip, and this door does not \
                 flip coins",
            )
            .with_found(format!(
                "{n} .wasm files: {}",
                components.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            )),
        ),
    }

    // The strict door wants the package complete: README + example required at hand-in.
    if profile == Profile::Strict {
        if pkg.inventory.get(Role::Readme).is_none() {
            r.push(ValidationError::new(
                CodeKind::KeyMissing,
                README_FILE,
                "file 5 of 5 at hand-in: what it is, what it needs, licence — one screen \
                 (MODULE-BUILD-GUIDE §3; the template's README is the shape)",
            ));
        }
        if pkg.inventory.get(Role::Example).is_none() {
            r.push(ValidationError::new(
                CodeKind::KeyMissing,
                EXAMPLE_FILE,
                "file 3 of 5 at hand-in: your instrument doing its job — the smallest patch in \
                 which it is audible and visible (MODULE-BUILD-GUIDE §3)",
            ));
        }
    }
    r
}

/// Whether this package can load at launch — the `instruments/` discovery door.
///
/// The launch profile: cap, roles, manifest, component. A missing README or example does not
/// stop a load; [`advisories`] carries the words the browser shows instead.
///
/// # Errors
/// The launch-profile report, when anything is fatal.
pub fn is_loadable(pkg: &Package) -> Result<(), ValidationReport> {
    let r = check(pkg, Profile::Launch);
    if r.is_empty() {
        Ok(())
    } else {
        Err(r)
    }
}

/// The launch-time words: what does not stop a load but is shown verbatim in the browser.
///
/// One line per missing advisory file (README, example, preview) — a thin library card says why
/// it is thin. Empty when the package is complete.
#[must_use]
pub fn advisories(pkg: &Package) -> Vec<String> {
    let mut out = Vec::new();
    if pkg.inventory.get(Role::Readme).is_none() {
        out.push(format!(
            "no {README_FILE} — the library card cannot say what this instrument is or who owns it"
        ));
    }
    if pkg.inventory.get(Role::Example).is_none() {
        out.push(format!("no {EXAMPLE_FILE} — the card has no \"try it\" patch"));
    }
    if pkg.inventory.get(Role::Preview).is_none() {
        out.push(format!(
            "no {PREVIEW_FILE} — it is generated by `sparq mod validate` stage 6, which needs the runtime half (WO-018); until then the card renders from the manifest"
        ));
    }
    out
}

/// Every package directory immediately under `root`, sorted by directory name.
///
/// A missing root is an empty result (the CLI names it in words — an absent `instruments/` is a
/// legal state of a young project, not a failure). Hidden entries (dot-prefixed) are skipped:
/// editor droppings are not packages. Loose FILES are not packages either; [`loose_files`]
/// collects them so the door can say so rather than silently ignore a misdropped package.
#[must_use]
pub fn discover(root: &Path) -> Vec<Package> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden =
                path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false);
            if hidden {
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }
    dirs.sort();
    dirs.iter().map(|d| open(d)).collect()
}

/// Loose files sitting directly in `root` — a package misdropped without its directory is the
/// failure this names. `README.md` is exempt: it is the folder's own layout artefact (the repo
/// ships one in `instruments/` explaining the drop-in rule), not a stray package.
#[must_use]
pub fn loose_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy().to_string();
                    if name != README_FILE && !name.starts_with('.') {
                        out.push(name);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Tests legitimately create files: the clippy.toml ban is on the audio path, and this is a
    // control-thread test harness for a control-thread door. `File::create` + `write_all` keeps
    // even the test off the banned `std::fs::write` spelling, so no allow is needed.
    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sparq-pkg-{}-{name}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(dir: &Path, name: &str, bytes: &[u8]) {
        let mut f = std::fs::File::create(dir.join(name)).unwrap();
        f.write_all(bytes).unwrap();
    }

    /// The five roles' canonical names — a legal package minus the preview (which is generated).
    fn put_legal_package(dir: &Path) {
        put(dir, MANIFEST_FILE, b"[identity]\nid = \"t/t/t\"\n");
        put(dir, "probe.wasm", b"\0asm\x01\x00\x00\x00 not really, but not empty");
        put(dir, EXAMPLE_FILE, b"[meta]\nformat = \"sparqpatch\"\n");
        put(dir, README_FILE, b"# probe\n");
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn roles_are_recognised_by_name() {
        assert_eq!(Role::of("sparqmod.toml"), Role::Manifest);
        assert_eq!(Role::of("fm-terrain.wasm"), Role::Component);
        assert_eq!(Role::of("example.sparqpatch"), Role::Example);
        assert_eq!(Role::of("preview.svg"), Role::Preview);
        assert_eq!(Role::of("README.md"), Role::Readme);
        assert_eq!(Role::of("notes.txt"), Role::Unknown);
        assert_eq!(Role::of("second.wasm.bak"), Role::Unknown);
        // Case is contract: a distributed artefact does not get two spellings of one role.
        assert_eq!(Role::of("readme.md"), Role::Unknown);
        assert_eq!(Role::of(".wasm"), Role::Unknown, "a bare extension is not a component name");
        // The fixed names round-trip; Component/Unknown have none.
        assert_eq!(Role::Manifest.file_name(), Some(MANIFEST_FILE));
        assert_eq!(Role::Component.file_name(), None);
    }

    #[test]
    fn the_cap_refuses_the_sixth_file() {
        let dir = scratch("cap");
        put_legal_package(&dir);
        put(&dir, PREVIEW_FILE, b"<svg/>");
        // Five files: legal in both profiles.
        let pkg = open(&dir);
        assert!(
            check(&pkg, Profile::Strict).is_empty(),
            "five recognised files pass the strict door"
        );
        // The sixth — even a benign one — is E-PACKAGE-FILECOUNT, the frozen spelling.
        put(&dir, "notes.txt", b"author's shopping list");
        let pkg = open(&dir);
        let r = check(&pkg, Profile::Launch);
        assert!(r.has(CodeKind::PackageFilecount), "{r}");
        let e = r.iter().find(|e| e.kind == CodeKind::PackageFilecount).unwrap();
        assert_eq!(e.code(), "E-PACKAGE-FILECOUNT", "catalogue codes carry no path suffix");
        assert!(e.to_string().contains("notes.txt"), "the report names the overflow: {e}");
        assert!(e.to_string().contains("BY HASH"), "the fix names the asset rule: {e}");
        assert!(is_loadable(&pkg).is_err(), "the cap is fatal at launch too");
        cleanup(&dir);
    }

    #[test]
    fn launch_and_strict_disagree_only_about_the_words() {
        // A package without README/example/preview LOADS at launch — with words — and FAILS at
        // hand-in. The preview is never a required input in either profile: it is stage 6's
        // output, and requiring it would make first validation impossible.
        let dir = scratch("profiles");
        put(&dir, MANIFEST_FILE, b"[identity]\nid = \"t/t/t\"\n");
        put(&dir, "probe.wasm", b"\0asm nonempty");
        let pkg = open(&dir);

        assert!(is_loadable(&pkg).is_ok(), "manifest + component load");
        let words = advisories(&pkg);
        assert_eq!(words.len(), 3, "README, example and preview each get a line: {words:?}");
        assert!(words.iter().any(|w| w.contains(README_FILE)));
        assert!(words.iter().any(|w| w.contains(EXAMPLE_FILE)));
        assert!(words.iter().any(|w| w.contains("generated")));

        let strict = check(&pkg, Profile::Strict);
        assert!(strict.has(CodeKind::KeyMissing));
        assert!(strict.has_path(README_FILE), "{strict}");
        assert!(strict.has_path(EXAMPLE_FILE), "{strict}");
        assert!(
            !strict.has_path(PREVIEW_FILE),
            "the preview is an OUTPUT — the strict door must not require it: {strict}"
        );
        cleanup(&dir);
    }

    #[test]
    fn an_empty_component_is_refused_in_both_profiles() {
        let dir = scratch("empty-wasm");
        put(&dir, MANIFEST_FILE, b"[identity]\nid = \"t/t/t\"\n");
        put(&dir, "probe.wasm", b"");
        let pkg = open(&dir);
        for profile in [Profile::Launch, Profile::Strict] {
            let r = check(&pkg, profile);
            assert!(r.has_path("package/probe.wasm"), "{profile:?}: {r}");
            let e = r.first_at("package/probe.wasm").unwrap();
            assert!(e.to_string().contains("EMPTY"), "{e}");
            assert!(e.to_string().contains("component new"), "the fix names the build recipe: {e}");
        }
        assert!(is_loadable(&pkg).is_err());
        // And two components are an ambiguity, refused with the entrypoint rule named.
        put(&dir, "other.wasm", b"\0asm nonempty");
        let pkg = open(&dir);
        let r = check(&pkg, Profile::Launch);
        assert!(r.has(CodeKind::CrossField), "{r}");
        assert!(r.first_at("package").unwrap().to_string().contains("entrypoint"), "{r}");
        cleanup(&dir);
    }

    #[test]
    fn discover_is_sorted_and_skips_non_packages() {
        let root = scratch("discover");
        std::fs::create_dir_all(root.join("b-second")).unwrap();
        std::fs::create_dir_all(root.join("a-first")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        put(&root, "stray.txt", b"a package misdropped without its directory");
        put(&root, README_FILE, b"# instruments/\nthe layout artefact\n");

        let found = discover(&root);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.dir.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names,
            vec!["a-first", "b-second"],
            "sorted, hidden skipped, files are not packages"
        );

        // The misdrop is NAMED, not silently ignored — an invisible failure is the one thing the
        // house forbids. The folder's own README is exempt: it is the layout artefact.
        let loose = loose_files(&root);
        assert_eq!(loose, vec!["stray.txt".to_string()], "{loose:?}");

        // A missing root is an empty result, not an error: a young project has no instruments/.
        assert!(discover(&root.join("nope")).is_empty());
        cleanup(&root);
    }
}
