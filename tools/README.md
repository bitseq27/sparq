# sparq tools

Design-time and CI tooling. These are **not** the instrument — they are the machinery that keeps the
identity and the contracts honest while the instrument is built. All are Python 3.11+ with no
third-party dependencies (stdlib `tomllib` only), so they run in CI and on any machine instantly.

| Tool | Work order | What it does | CI usage |
|---|---|---|---|
| [`token_gen.py`](token_gen.py) | WO-002 | Reads `design/tokens/*.toml` (the single source of truth) and emits every derived artefact. Runs 10 validation checks on the token set itself. | `python3 tools/token_gen.py --check --quiet` → fails if any generated file is stale or a check fails |
| [`token_audit.py`](token_audit.py) | WO-002/004 | Audits SVG mockups and (later) widget/panel/display source against the hard rules R1–R6. | `python3 tools/token_audit.py` → exit 1 on any violation |
| [`make_display_sheet.py`](make_display_sheet.py) | WO-004 | Regenerates `design/mockups/display-sheet.svg` from the tokens and the baked LUTs. | Run after any token change; then re-audit |
| [`log_check.py`](log_check.py) | WO-006/012 | Reads a device log directory and compares every number the gates assert on against the measured sandbox baseline: stamp, golden hash, test totals, allocations, xruns, reopen-leak, throughput, and the dispatch figures ADR-009 cites. HARD failures (a stale stamp, a changed golden, any xrun) exit non-zero; SOFT numbers are reported as ratios, because a stage device beating a 2-core VM is the point. Includes a DECISION check that fires if enum dispatch ever measures *slower* than trait objects per sample, which would reopen ADR-009 executor decision 7. Reads cp1252/cp850/UTF-16 logs, which is what Windows actually writes. | `python3 tools/log_check.py --dir logs`; `--self-test` proves the parser and the verdicts without any logs |
| [`check_text_io.py`](check_text_io.py) | — | Fails if any tool reads or writes text without an explicit encoding, or writes without forcing LF. | exit 1 on any violation; proven against injected violations |
| [`sync_check.py`](sync_check.py) | WO-013 inc2b | Verifies the working tree against `SYNC-STAMP.txt`: sha256 + byte size for every file that decides what the binary is (`crates/*/src/**/*.rs`, `build.rs`, every `Cargo.toml`, `modules/**`, `scripts/*.bat`, `tools/*.py`, the toolchain configs), plus the `Nf/NB` source fingerprint that `build.rs` and `build.bat` also compute. Verdicts: OK · MISSING · SIZE · HASH · EOL (line endings only — a warning) · EXTRA (on disk, not in the stamp — how a `(2)` copy artefact gets named) · FP. `Cargo.lock` is soft. Exists because a half-applied sync used to arrive as six rustc errors in a crate that was not the one that was wrong (defect #78). `--write` regenerates the stamp before a sync is packed. | `python3 tools/sync_check.py` → exit 1 if the tree is not the stamped sync; `scripts\synccheck.bat` on Windows; `--self-test` proves all seven verdicts fire without a repo (11 cases, four of them the `--freshness` marker `build.bat` purges on) |
| [`unsafe_audit.py`](unsafe_audit.py) | WO-000 | Enforces ADR-000: `unsafe` only in allowlisted files, each with a module-level exemption naming its entry and a `SAFETY:` comment on every block. Also fails if a crate relaxes the lint without justification. | `python3 tools/unsafe_audit.py` → exit 1 on any violation |
| [`dispatch-bench/`](dispatch-bench) | WO-007 | Standalone cargo project (own `[workspace]`, excluded from the root one) that measures trait-object vs enum dispatch over a 100-module × 64-sample block. The number ADR-009 executor decision 7 cites. `cargo run --release` from inside the directory; re-run on the stage device before the ADR is treated as final. | not in CI — run by hand |

## Generated artefacts (`design/tokens/generated/`, `design/tokens/preview.html`)

| File | Consumer |
|---|---|
| `tokens.rs` | `sparq-ui` / `sparq-visual` — 295 constants, one per token. Checked in so the app and the mockups cannot drift |
| `tokens.json` | thin client, docs site, external tooling |
| `tokens.css` | static mockups and documentation |
| `tokens.svg` | `<defs>` swatches, colour-map gradients and the signal-class legend for mockups |
| `colormaps.json` | baked 256-entry LUTs per colour map, interpolated in **oklab** — uploaded as 1D textures by displays |
| `preview.html` | self-contained token preview page: every colour with its contrast ratio, the type scale with real numeric specimens, stroke/touch/motion tables, every colour-map ramp, and the live conformance report. No network, no CDN |

**Never edit a generated file.** Change the TOML and re-run `token_gen.py`.

## The audit rules

| Rule | Enforced how |
|---|---|
| **R1** no literal colours | Every hex in an SVG must be a declared token colour, **or** an exact sample of a baked colour-map LUT. Named CSS colours (`red`, `white`) and `rgb()/rgba()` are always violations |
| **R1b** LUT samples are data encoding only | Fails if a LUT colour is used as text colour, or to stroke a chrome element (`rect`, `g`, `use`, `image`, `text`, `tspan`). Whether a *fill* is a data mark or chrome cannot be inferred from markup — so **every display mockup must list its colour-map-filled regions in `mockup-review.md`** and those are review-checked |
| **R2** stroke widths | Only the approved set (currently 1, 2, 3, and 4 for Perform-mode hero glyphs) |
| **R3** corner radii | Only 0, 2, 4 px |
| **R4** no pure white; pure black only where permitted | `#FFFFFF` always fails; `#000000` fails unless the file is a canvas/full-bleed/high-contrast artefact |
| **R5** no drop shadows | Any `feDropShadow` fails — depth comes from tint and hairlines |
| **R6** no literals in source | Once `crates/sparq-ui` and `crates/sparq-visual` exist, any hex or `rgb()` literal outside the generated token modules fails. Comments are skipped; the generated files and the tools themselves are allowlisted |

### Verifying the auditor (do this whenever you change it)

The auditor must be proven to fail on bad input, or it is decoration:

```
python3 tools/token_audit.py                 # expect: clean, exit 0
python3 tools/token_audit.py /tmp/neg/bad.svg  # expect: R1..R6 all fire, exit 1
```

The negative fixture is a five-line SVG containing an off-token hex, a named colour, an `rgba()`,
a 7 px stroke, a 9 px radius, `#FFFFFF`, an `feDropShadow` and an inline style literal. Keep it.

## Rust-side gates (WO-000, now live)

`just gates` runs the whole set; `.github/workflows/ci.yml` runs the same commands on Windows (primary),
Linux and macOS.

| Gate | Command |
|---|---|
| Format | `cargo fmt --all --check` |
| Lint, zero warnings, both feature configs | `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p sparq-app --features bootstrap-audio --all-targets -- -D warnings` |
| Tests | `cargo test --workspace` (48 tests; no audio device required) |
| Golden reference | `cargo test --release -p sparq-audio --test golden` |
| Determinism | render twice, compare hashes **and** `cmp` the WAV bytes |
| Phase 0 gate table | `sparq selftest --golden` |
| Allocation probe | `cargo run --release -p sparq-app --example probe_alloc` |
| Soak | `sparq soak --minutes 120` (bounded memory; fails fast on the first allocation or underrun) |

Two CI gates deserve a note because they were added after being caught out:

* **`clippy::disallowed_methods` / `disallowed_types`** (`clippy.toml`) deny `Instant::now`,
  `thread::sleep`, file I/O and `Mutex` workspace-wide. This is how "no clock reads, no locks and no
  I/O on the audio path" becomes a build failure instead of a review comment. It immediately caught a
  clock-reading test hook that had been placed *inside* a DSP module; the hook now lives in the
  harness (`RenderConfig::stall`), and the bootstrap's mutex carries a dated `#[allow]` that names
  WO-006 as its removal date.
* **The counting allocator is installed in `sparq-app` itself**, not just in test binaries. Without
  it, `sparq render` printed `alloc 0` as an unmeasured default — a gate that silently passes. The
  `selftest` now includes a positive control that deliberately allocates and asserts the counter
  sees it.

## Text I/O portability (the rule `check_text_io.py` enforces)

Windows is the primary platform (ADR-004), and Python's text I/O defaults are platform-dependent in
two ways that broke every gate on the first Windows run:

| Default | On Windows | Consequence |
|---|---|---|
| `Path.read_text()` | decodes with the ANSI code page (cp1252) | `UnicodeDecodeError` on the first `·`, `→` or `—` in a design document |
| `Path.read_text(errors="replace")` | decodes with cp1252, replaces unknown bytes | **worse**: no crash, silently corrupted text, so the audit checks the wrong bytes |
| `Path.write_text()` | translates `\n` → `\r\n` | every generated file differs from its LF-only committed copy, so `--check` reports the whole set as stale |

So: every tool defines a local `read_text()` that passes `encoding="utf-8"`, a `write_text()` that
also passes `newline="\n"`, and — for the auditors, which must not crash on hostile input — a
`read_text_lossy()` that combines `encoding="utf-8"` with `errors="replace"`. Binary I/O
(`open(..., "rb")`) is unaffected and allowed.

`check_text_io.py` runs first among the Python gates, in both `scripts\gates.bat` and CI. It exempts
itself (it must read the files it audits) and accepts any call that names `encoding=` explicitly,
which is how the helper definitions pass.

## Known limits (recorded so nobody trusts them too far)
* The oklch values in `colormaps.toml` are **derived from** the canonical `rgb` stops and are
  informational (for reasoning about lightness ramps); the `rgb` values are authoritative.
* `token_audit.py` and `unsafe_audit.py` are regex-based, not parsers. It is deliberately strict rather than clever:
  a false positive costs a minute, a false negative costs the identity.
* `token_gen.py`'s 10 checks validate the token set, not the rendered output. The rendered output is
  validated by `token_audit.py` plus the human protocols in `look-board.md` §10.
