# sparq design token spec

**Version:** 0.1.0 · **Work order:** WO-002 · **Related:** ADR-003, `look-board.md`
**Principle:** the visual identity is authored as **data**, independently of any UI toolkit, so the Phase 6 renderer swap cannot destroy it. Tokens are the mechanism by which 140 modules look like one product.

---

## 1. Files

| File | Owns |
|---|---|
| `tokens/colors.toml` | ground/surface, hairline tiers, text tiers, the five signal-class accents, states, meters, high-contrast theme, forbidden list |
| `tokens/typography.toml` | families + selection requirements, type scale, metrics, **numeric formatting rules**, zoom range |
| `tokens/layout.toml` | spacing, strokes, corners, elevation-by-tint, **touch targets**, shell dimensions, canvas rules, LOD, breakpoints |
| `tokens/motion.toml` | durations, curves, frame budget, trace/phosphor behaviour, glow law, accessibility switches |
| `tokens/colormaps.toml` | scientific colour maps (sequential / diverging / qualitative) + per-domain bindings |

Token ids are dotted paths: `color.signal.audio`, `type.scale.m`, `space.4`, `stroke.hairline`, `touch.min_target`, `motion.duration.fast`, `colormap.spectrum.sonogram`.

## 2. Generator

One tool, `xtask tokens`, consumes the TOML and emits:

| Output | Consumer |
|---|---|
| `sparq-ui/src/tokens.rs` (generated, `#[allow]`-free, checked in) | Rust widgets, canvas, shell |
| `sparq-visual/src/colormaps.rs` + baked 256-entry LUTs | displays, GPU shaders (uploaded as a 1D texture) |
| `design/tokens/tokens.json` | thin client, docs site, external tooling |
| `design/tokens/tokens.css` + `tokens.svg` defs | **static mockups** (WO-004) and documentation |
| `design/tokens/preview.html` | the token preview page: every colour, every type size, every stroke, every colour map, at 1×/1.5×/2×, in both themes |
| **token bundle v1** (serialised runtime form, WO-017) | **instrument guests (T2)**: handed to a sandboxed module at `prepare` so its declared UI and emitted display lists/scene descriptors resolve against the *current* tokens — the mechanism by which a token change re-themes every handed-in instrument with zero intervention (ADR-010). Semver'd with the tokens; golden snapshot test in CI |

Generation is one-directional (TOML → everything). Generated files are checked in and CI fails if they are stale, so a mockup can never drift from the values the app uses.

`tokens.rs` is **compiled and tested**: `sparq-ui/src/tokens.rs` is a one-line `include!` of it, so a token change that produced invalid Rust would fail the build, and the widget code that consumes the constants is type-checked against them.

**Implemented now** by `tools/token_gen.py` (Python, stdlib only) — see `tools/README.md`. It emits all six artefacts above, runs the ten conformance checks in §6, and supports `--check` for CI. The Rust emitter exists from day one so that the app consumes exactly the values the mockups do; `tokens.rs` becomes compile-checked in WO-000 when the workspace exists.

## 3. Hard rules (CI-enforced)

1. **No literal colours, sizes, spacings, radii or durations in widget/panel/display code.** A grep audit fails the build. Only the generated `tokens.rs` may contain literals.
2. **Only stroke widths 1, 2, 3 exist.** Anything else is an error.
3. **Corners are 0, 2 or 4 px.** Circles only for knobs and ports.
4. **No drop shadows, no gradient decoration.** Gradients exist only inside colour maps and meter fills.
5. **No `#FFFFFF`**, and `#000000` only in the high-contrast theme and full-bleed visuals.
6. **Accents are assigned by signal class only** — never by module, category or author taste.
7. **Every state that uses colour also uses a non-colour encoding** (dash pattern, hatch, shape, label). Colour-blind-safe and print-safe by construction.
8. **Every interactive element ≥ `touch.min_target` (44 px)**; the 32 px dense exception is Design-mode-only, non-destructive, and flagged in the audit report.
9. **Every number renders with its unit**, tabular numerals, per `typography.toml [format]`.
10. **Colour maps are chosen by data kind** (magnitude → sequential, signed → diverging, category → qualitative). The UI only offers valid maps.
11. **A colour-map sample is a data encoding, never a style.** A colour taken from a baked LUT may fill a data mark (a spectrogram bin, a heat cell, a node in a depth-encoded tree) or stroke a data trace — but it may never colour text, and never stroke or fill chrome (panels, frames, slider tracks, dividers). `tools/token_audit.py` fails CI on the unambiguous cases (text colour, chrome stroke); because "is this fill a data mark?" cannot be inferred from markup, **every display surface must list its colour-map-filled regions in `design/mockups/mockup-review.md`** and those are review-checked.

## 4. Versioning and governance

* Tokens are semver'd. **Additive** changes (new token) = minor. **Value** changes that alter appearance = minor + a screenshot re-baseline with a reviewed diff. **Removals/renames** = major, with an alias table and a deprecation window (same discipline as the module API).
* Adding a token requires a one-line justification in the PR: *why can't an existing token do this?* Three consecutive "just this once" additions means the scale is wrong — fix the scale, not the exception.
* The `look-board.md` is the conformance authority; when tokens and the board disagree, the board wins and the tokens are amended.
* Theme derivation: `contrast-high` is generated from the base theme by declared transforms (ground → pure black, hairline opacity lift, accent lightness lift +8% in oklch). No hand-maintained second palette.

## 5. Font selection gate (WO-002 task 1)

A candidate monospace is accepted only if it passes **all** of:

* [ ] variable weight axis, Regular → Bold, without synthetic bolding
* [ ] true tabular numerals; digit widths identical across weights
* [ ] `0` distinguishable from `O`; `1` has a base serif; `l`/`I`/`1` distinguishable
* [ ] full glyph set from `typography.toml [family.mono]` including Greek, math operators, unit symbols (° µ Ω ‰ ′ ″) and arrows
* [ ] legible at `scale.xs` (10 px) on the stage device at 150 % DPI
* [ ] legible at `scale.stage` (44 px) from 3 m on the projector — no hairline strokes that vanish
* [ ] licence covers desktop app embedding, web serving (thin client) and marketing screenshots
* [ ] renders identically on Windows (primary), Linux and macOS — or has a documented fallback stack

Both families are chosen and recorded here in WO-002; `chosen = ""` in the TOML is a build warning until filled.

## 6. Verification checklist (run at WO-002 close, re-run at WO-004 and every phase gate)

* [ ] Token preview page renders all values; no placeholders.
* [ ] Contrast report: all text tiers ≥ 4.5:1 against their ground; hairline structure ≥ 3.0:1.
* [ ] All five signal accents are distinguishable under deuteranopia, protanopia and tritanopia simulation **without** relying on hue (i.e. the dash/label encoding alone is sufficient).
* [ ] Every default colour map has a `.cb` alternative that preserves lightness ordering.
* [ ] 200 % type scale does not break any panel layout at the three breakpoints.
* [ ] Mockups produced from the exported CSS/SVG match the in-app rendering pixel-for-pixel at the same DPI (this is the proof that tokens are the single source of truth).
