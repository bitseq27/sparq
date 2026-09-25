#!/usr/bin/env python3
"""sparq token conformance audit (WO-002, gate for WO-004/WO-012/WO-013).

Enforces the hard rules from design/token-spec.md §3 and design/look-board.md:

  R1  no literal colours outside the token set
  R2  stroke widths from the approved set only
  R3  corner radii from the approved set only
  R4  no pure white; pure black only where permitted
  R5  no drop shadows (filter primitives that fake elevation)
  R6  source files carry no colour/size literals (widget code must read tokens)

Audits SVG mockups and, once they exist, widget/panel/display source files.

Usage:
    python3 tools/token_audit.py                    # audit the defaults
    python3 tools/token_audit.py path [path ...]    # audit specific files/dirs
    python3 tools/token_audit.py --json             # machine-readable report

Exit code 0 = clean, 1 = violations found (this is the CI gate).
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib

# --------------------------------------------------------------------------- text I/O
# Every text read and write in this tool is explicit about encoding and newline handling.
#
# This is not pedantry, it is a bug that was actually shipped: on Windows, `read_text(Path)`
# defaults to the ANSI code page (cp1252 in the field), which cannot decode the UTF-8 characters
# used throughout the design documents and mockups, and `write_text(Path, )` translates "\n" to
# "\r\n", which made every generated file look stale against its LF-only committed copy. Both
# failures appeared only on Windows, which is the *primary* platform (ADR-004).
#
# Rule: no bare read_text()/write_text()/open() in text mode anywhere under tools/. The
# `tools-text-io` gate in scripts/gates.bat and CI fails the build if one appears.
ENCODING = "utf-8"

# Windows consoles and redirected logs default to the ANSI code page (cp1252 in the field):
# printing the UTF-8 characters this project uses (·, —, →) raises UnicodeEncodeError mid-report
# — defect #37, real: unsafe_audit crashed while printing its allowlist summary on SATURN.
# Reconfiguring stdout/stderr to UTF-8 with errors="replace" makes reports degrade visibly
# instead of dying. File I/O is covered separately by the read_text/write_text helpers below.
import sys as _sys

for _stream in (_sys.stdout, _sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper (captured/redirected oddly)
        pass



def read_text(path) -> str:
    """Read a text file as UTF-8, on every platform."""
    return pathlib.Path(path).read_text(encoding=ENCODING)


def read_text_lossy(path) -> str:
    """Read as UTF-8 but never fail on malformed bytes.

    Auditors scan source files that could in principle be non-UTF-8 or truncated; a crash in the
    auditor would hide the thing it was checking. Note this still passes `encoding=` explicitly:
    `errors="replace"` without an encoding decodes UTF-8 as cp1252 on Windows and silently corrupts
    the text, which is worse than failing.
    """
    return pathlib.Path(path).read_text(encoding=ENCODING, errors="replace")



def write_text(path, content: str) -> None:
    """Write a text file as UTF-8 with LF endings, on every platform."""
    p = pathlib.Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


ROOT = pathlib.Path(__file__).resolve().parent.parent
TOK = ROOT / "design" / "tokens"
GEN = TOK / "generated"

SOURCE_SUFFIXES = {".rs", ".ts", ".tsx", ".js", ".wgsl", ".glsl", ".py", ".css", ".html"}
# Files permitted to contain literal values: the generated token modules and the generator itself.
SOURCE_ALLOWLIST = {
    "design/tokens/generated/tokens.rs",
    "design/tokens/generated/tokens.css",
    "design/tokens/generated/tokens.json",
    "design/tokens/generated/tokens.svg",
    "design/tokens/generated/colormaps.json",
    "design/tokens/preview.html",
    "tools/token_gen.py",
    "tools/token_audit.py",
}
# Shadow-like filter primitives: elevation must come from tint and hairlines, never from blur-below.
SHADOW_PRIMITIVES = ("feDropShadow",)
PERMITTED_BLACK_CONTEXTS = ("canvas", "contrast-high", "full-bleed")
# Elements whose stroke is a data trace (a LUT-sampled stroke colour is legitimate there).
DATA_TRACE_TAGS = {"line", "polyline", "path", "polygon"}
# Elements that are structure/chrome and therefore may never be painted with a colour-map sample.
# Everything else (line, polyline, path, polygon, circle, ellipse, rect) can legitimately be a data
# mark -- a spectrogram bin is a <rect>, a scatter/node plot is a <circle>.
CHROME_TAGS = {"rect", "g", "use", "image", "text", "tspan"}


# SVG paint keywords are not colour literals.
PAINT_KEYWORDS = {"none", "currentcolor", "inherit", "transparent", "context-fill", "context-stroke"}


def load_lut_colours() -> set[str]:
    """Every colour present in the baked colour-map LUTs (generated/colormaps.json)."""
    f = GEN / "colormaps.json"
    if not f.exists():
        return set()
    return {c.upper() for c in re.findall(r"#[0-9A-Fa-f]{6}", read_text(f))}


def load_token_colours() -> set[str]:
    """Declared token colours only: every hex in design/tokens/*.toml.

    Deliberately excludes the baked 256-entry LUTs -- accepting interpolated ramp values would let
    almost any colour pass R1. Displays sample ramps at runtime from `colormaps.json`; static
    mockups must stick to declared stops.
    """
    colours: set[str] = set()
    for f in sorted(TOK.glob("*.toml")):
        with f.open("rb") as fh:
            data = tomllib.load(fh)
        colours.update(re.findall(r"#[0-9A-Fa-f]{6}", json.dumps(data)))
    return {c.upper() for c in colours}


def load_approved_numbers() -> tuple[set[int], set[int]]:
    with (TOK / "layout.toml").open("rb") as fh:
        lay = tomllib.load(fh)
    widths = {int(x) for x in lay["stroke"]["widths_allowed"]}
    corners = {int(lay["corner"][k]) for k in ("none", "micro", "panel", "max")}
    return widths, corners


def rel_display(path: pathlib.Path) -> str:
    """Repo-relative where possible; absolute for files audited from outside the tree."""
    try:
        return str(path.relative_to(ROOT)).replace("\\", "/")
    except ValueError:
        return str(path)


# --------------------------------------------------------------------------- SVG audit
def audit_svg(path: pathlib.Path, colours: set[str], widths: set[int], corners: set[int],
              lut_colours: set[str], stats: dict | None = None) -> list[dict]:
    s = read_text(path)
    found: list[dict] = []
    col_key = str(path.name)
    if stats is not None:
        stats.setdefault("lut_samples", {})[col_key] = 0
    sampled = stats["lut_samples"] if stats is not None else {col_key: 0}

    def add(rule, detail, count=1):
        found.append({"file": rel_display(path), "rule": rule, "detail": detail, "count": count})

    # R1 literal colours.
    # Two-tier rule (token-spec §3.11): every colour must be either a declared token, or an exact
    # sample of a baked colour-map LUT *used as a data-encoding fill*. LUT samples are never
    # permitted for text, strokes or chrome -- the attribute check below enforces that.
    used = {c.upper() for c in re.findall(r"#[0-9A-Fa-f]{6}\b", s)}
    off = sorted(used - colours)
    lut_only, unknown = [], []
    for col in off:
        if col in lut_colours:
            lut_only.append(col)
        else:
            unknown.append(col)
    if unknown:
        add("R1 off-token colour", ", ".join(unknown[:12]) + (" …" if len(unknown) > 12 else ""), len(unknown))
    if lut_only:
        # R1b: a LUT sample is a DATA-ENCODING paint. Two violations are unambiguous and fail CI:
        #   (a) a LUT colour used as text colour, (b) a LUT colour used to stroke chrome.
        # Whether a given *fill* is a data mark (spectrogram bin, heat cell) or chrome (panel,
        # slider track) cannot be inferred from markup, so it is covered by review, not by this
        # gate: every display mockup must list its colormap-filled regions in mockup-review.md.
        lutset = set(lut_only)
        bad_use = []
        for m in re.finditer(r"<(\w+)([^>]*)>", s):
            tag, attrs = m.group(1).lower(), m.group(2)
            if tag in ("text", "tspan"):
                f = re.search(r'fill="(#[0-9A-Fa-f]{6})"', attrs)
                if f and f.group(1).upper() in lutset:
                    bad_use.append(f"<text> fill={f.group(1)}")
            elif tag in CHROME_TAGS:
                st = re.search(r'stroke="(#[0-9A-Fa-f]{6})"', attrs)
                if st and st.group(1).upper() in lutset:
                    bad_use.append(f"<{tag}> stroke={st.group(1)} (chrome)")
        if bad_use:
            uniq = sorted(set(bad_use))
            add("R1b LUT sample outside data encoding",
                "; ".join(uniq[:8]) + (" …" if len(uniq) > 8 else ""), len(bad_use))
        else:
            sampled[col_key] = len(lut_only)

    # R1 (continued) named CSS colours and rgb()/rgba() are literals too
    named = {n.lower() for n in re.findall(r'(?:fill|stroke|stop-color)="([a-zA-Z-]+)"', s)} - PAINT_KEYWORDS
    if named:
        add("R1 named colour literal", ", ".join(sorted(named)), len(named))
    rgba = re.findall(r"rgba?\([^)]*\)", s)
    if rgba:
        add("R1 rgb()/rgba() literal", f"{len(rgba)} occurrence(s); use a token + fill-opacity", len(rgba))

    # R2 stroke widths
    sw = {int(float(x)) for x in re.findall(r'stroke-width[:="\s]+([0-9.]+)', s)}
    bad_w = sorted(sw - widths)
    if bad_w:
        add("R2 stroke width", f"{bad_w} not in approved {sorted(widths)}", len(bad_w))

    # R3 corner radii
    rx = {int(float(x)) for x in re.findall(r'\brx="([0-9.]+)"', s)}
    bad_r = sorted(rx - corners)
    if bad_r:
        add("R3 corner radius", f"{bad_r} not in approved {sorted(corners)}", len(bad_r))

    # R4 pure white / black
    if re.search(r"#(?:FFFFFF|ffffff)\b", s):
        add("R4 pure white", "#FFFFFF is forbidden (look-board §4)")
    blacks = re.findall(r"#000000\b", s)
    if blacks and not any(k in path.name.lower() for k in PERMITTED_BLACK_CONTEXTS):
        add("R4 pure black", f"{len(blacks)} use(s); permitted only in full-bleed visuals / high-contrast theme")

    # R5 shadows
    for prim in SHADOW_PRIMITIVES:
        if prim in s:
            add("R5 drop shadow", f"<{prim}> found; depth comes from tint + hairline (look-board §8)")

    # R6 (svg variant) inline styles that hardcode colour
    inline = re.findall(r'style="[^"]*(?:#[0-9A-Fa-f]{6}|rgba?\()[^"]*"', s)
    if inline:
        add("R6 inline style literal", f"{len(inline)} element(s) with hardcoded colour in style=")

    return found


# --------------------------------------------------------------------------- source audit
HEX_RE = re.compile(r"#[0-9A-Fa-f]{6}\b")
RGB_RE = re.compile(r"\brgba?\(\s*\d")
NUM_LITERAL_RE = re.compile(r"(?<![\w.])(\d{1,3})(?:\.\d+)?\s*(?:px|f32|i32)?\b")
SKIP_LINE_RE = re.compile(r"^\s*(?://|/\*|\*|#)")


def audit_source(path: pathlib.Path) -> list[dict]:
    rel = rel_display(path)
    if rel in SOURCE_ALLOWLIST:
        return []
    found: list[dict] = []
    lines = read_text_lossy(path).split("\n")
    hexes, rgbs = [], []
    for i, line in enumerate(lines, 1):
        if SKIP_LINE_RE.match(line):
            continue
        # a line that references a token is fine; only bare literals are violations
        hexes += [(i, h) for h in HEX_RE.findall(line)]
        rgbs += [(i, m) for m in RGB_RE.findall(line)]
    if hexes:
        found.append({
            "file": rel, "rule": "R6 colour literal in source",
            "detail": "; ".join(f"L{i}: {h}" for i, h in hexes[:8]) + (" …" if len(hexes) > 8 else ""),
            "count": len(hexes),
        })
    if rgbs:
        found.append({
            "file": rel, "rule": "R6 rgb()/rgba() literal in source",
            "detail": "; ".join(f"L{i}" for i, _ in rgbs[:8]), "count": len(rgbs),
        })
    return found


# --------------------------------------------------------------------------- driver
def collect(paths: list[str]) -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    for p in paths:
        fp = pathlib.Path(p)
        if not fp.is_absolute():
            fp = ROOT / fp
        if fp.is_dir():
            out += sorted(x for x in fp.rglob("*") if x.suffix.lower() in SOURCE_SUFFIXES | {".svg"} and x.is_file())
        elif fp.is_file():
            out.append(fp)
    return [f for f in out if "generated" not in f.parts or f.suffix == ".svg"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="*", help="files or directories (default: mockups + sparq-ui + sparq-visual)")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    colours = load_token_colours()
    lut_colours = load_lut_colours()
    widths, corners = load_approved_numbers()
    stats: dict = {"lut_samples": {}}
    defaults = args.paths or [
        "design/mockups",
        "crates/sparq-ui",
        "crates/sparq-visual",
        # WO-012: the egui shell is widget code — acceptance says "no hard-coded colour/size
        # values in widget code (grep-clean audit in CI)", so it lives under the scanner from
        # day one. R6 catches colour literals; size discipline is structural (every dimension
        # is a LAYOUT_*/TYPOGRAPHY_* constant) and outcome-enforced by `sparq ui --audit`.
        "crates/sparq-app/src/ui",
        "modules",
    ]
    files = collect(defaults)

    findings: list[dict] = []
    audited = {"svg": 0, "source": 0}
    for f in files:
        if f.suffix.lower() == ".svg":
            audited["svg"] += 1
            findings += audit_svg(f, colours, widths, corners, lut_colours, stats)
        elif f.suffix.lower() in SOURCE_SUFFIXES:
            audited["source"] += 1
            findings += audit_source(f)

    if args.json:
        print(json.dumps({
            "token_colours": len(colours),
            "approved_stroke_widths": sorted(widths),
            "approved_radii": sorted(corners),
            "audited": audited,
            "violations": findings,
        }, indent=2))
    else:
        print(f"token_audit · {len(colours)} token colours · {len(lut_colours)} LUT samples available · "
              f"stroke widths {sorted(widths)} · radii {sorted(corners)}")
        print(f"audited {audited['svg']} svg + {audited['source']} source file(s)")
        used_lut = {k: v for k, v in stats["lut_samples"].items() if v}
        if used_lut:
            print("  LUT-sampled colours (data encoding; fills are review-checked, see mockup-review.md): "
                  + ", ".join(f"{k} {v}" for k, v in used_lut.items()))
        if not findings:
            print("clean — no violations")
        else:
            for v in findings:
                print(f"  FAIL [{v['rule']}] {v['file']} — {v['detail']}")
            print(f"\n{len(findings)} violation group(s), {sum(v['count'] for v in findings)} occurrence(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
