#!/usr/bin/env python3
"""sparq token generator (WO-002).

Reads design/tokens/*.toml (the single source of truth for the visual identity)
and emits every derived artefact:

    design/tokens/generated/tokens.rs      Rust constants for sparq-ui / sparq-visual
    design/tokens/generated/tokens.json    for the thin client, docs site, external tools
    design/tokens/generated/token-bundle.json  the runtime hand-off to instrument guests (v1)
    design/tokens/generated/tokens.css     for static mockups and documentation
    design/tokens/generated/tokens.svg     <defs> swatches + ramp gradients for mockups
    design/tokens/generated/colormaps.json baked 256-entry LUTs (uploaded as 1D textures)
    design/tokens/preview.html             the token preview page (self-contained, no network)

Generation is one-directional: TOML -> everything. Never edit a generated file.
CI fails if a generated file is stale (`--check`).

The Rust emitter exists so that the app consumes exactly the same values the mockups do;
until the Rust workspace exists (WO-000) it is simply a checked-in artefact.

The token bundle (contract v1.1, WO-017): the serialised runtime form handed to instrument
guests at `prepare` (WIT `sparq:instrument/tokens`, tokens.wit). The envelope carries the
tokens' own semver (from colors.toml `[meta]`), the encoding (`json` at v1 — the tokens.json
artefact is the payload), a sha256 of the canonical payload bytes, and the payload itself.
Invariant, machine-checked here AND by `crates/sparq-module-api/tests/token_bundle.rs`:
re-serialising `payload` with this tool's canonical settings (indent=2, sort_keys) reproduces
`tokens.json` BYTE-FOR-BYTE, and `payload_sha256` is the sha256 of those bytes. A token value
change therefore propagates to bundle, tokens.rs, CSS and JSON in one commit, or `--check` fails.

Usage:  python3 tools/token_gen.py [--check] [--quiet]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
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


def write_text(path, content: str) -> None:
    """Write a text file as UTF-8 with LF endings, on every platform."""
    p = pathlib.Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


ROOT = pathlib.Path(__file__).resolve().parent.parent
TOK = ROOT / "design" / "tokens"
GEN = TOK / "generated"

FILES = ["colors.toml", "typography.toml", "layout.toml", "motion.toml", "colormaps.toml"]
LUT_SIZE = 256


# --------------------------------------------------------------------------- colour maths
def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    return tuple(int(h[i : i + 2], 16) for i in (0, 2, 4))  # type: ignore[return-value]


def rgb_to_hex(r: float, g: float, b: float) -> str:
    q = lambda v: max(0, min(255, int(round(v))))
    return f"#{q(r):02X}{q(g):02X}{q(b):02X}"


def _lin(u: float) -> float:
    u /= 255.0
    return ((u + 0.055) / 1.055) ** 2.4 if u > 0.04045 else u / 12.92


def _unlin(u: float) -> float:
    u = max(0.0, min(1.0, u))
    return 1.055 * u ** (1 / 2.4) - 0.055 if u > 0.0031308 else 12.92 * u


def srgb_to_oklab(rgb: tuple[int, int, int]) -> tuple[float, float, float]:
    r, g, b = (_lin(c) for c in rgb)
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = l ** (1 / 3), m ** (1 / 3), s ** (1 / 3)
    return (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )


def oklab_to_srgb(L: float, a: float, b: float) -> tuple[int, int, int]:
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_**3, m_**3, s_**3
    r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    bb = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    return tuple(round(_unlin(v) * 255) for v in (r, g, bb))  # type: ignore[return-value]


def rel_luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = (_lin(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: str, b: str) -> float:
    la, lb = rel_luminance(hex_to_rgb(a)), rel_luminance(hex_to_rgb(b))
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def lift_lightness(hexv: str, lift: float) -> str:
    """Raise an accent's oklab L by `lift`, hue and chroma untouched (colors.toml
    [theme.contrast-high]: "unchanged in hue but raised in lightness"). Clamped to L <= 1."""
    L, a, b = srgb_to_oklab(hex_to_rgb(hexv))
    return rgb_to_hex(*oklab_to_srgb(min(L + lift, 1.0), a, b))


def derive_high_contrast(data: dict) -> dict:
    """colors.toml [theme.contrast-high] -> `color.hc.*` token constants.

    The theme EXTENDS phosphor-dark: only the keys it overrides and the accent lightness lift
    differ; everything else reuses the base constants at the consumer. Deriving it here (rather
    than in the shell) is what makes the WO-012 criterion true: "high-contrast theme switch
    works and is GENERATED FROM TOKENS" -- the Rust adapter below contains no colour arithmetic
    and no literals, only names.
    """
    colors = data["colors"]
    theme = colors.get("theme", {}).get("contrast-high", {})
    tf = flatten(theme)  # TOML dotted keys (`ground.base = ...`) arrive nested; flatten them
    lift = float(tf.get("accent_lightness_lift", 0.0))
    out: dict = {}
    for key in ("ground.base", "hairline.regular", "hairline.strong",
                "text.secondary", "text.tertiary"):
        if key in tf:
            out[f"color.hc.{key}"] = tf[key]
    # Accent lift: the five signal-class accents and the state colours (colour/dim/glow slots).
    for section in ("signal", "state"):
        for name, table in colors.get(section, {}).items():
            if not isinstance(table, dict):
                continue
            for slot in ("colour", "dim", "glow"):
                v = table.get(slot)
                if isinstance(v, str) and re.fullmatch(r"#[0-9A-Fa-f]{6}", v):
                    out[f"color.hc.{section}.{name}.{slot}"] = lift_lightness(v, lift)
    return out


def mix_oklab(c1: str, c2: str, t: float) -> str:
    L1, a1, b1 = srgb_to_oklab(hex_to_rgb(c1))
    L2, a2, b2 = srgb_to_oklab(hex_to_rgb(c2))
    return rgb_to_hex(*oklab_to_srgb(L1 + (L2 - L1) * t, a1 + (a2 - a1) * t, b1 + (b2 - b1) * t))


def bake_lut(stops: list[dict], n: int = LUT_SIZE) -> list[str]:
    """Interpolate a stop list in oklab into an n-entry LUT of hex strings."""
    pts = sorted(stops, key=lambda s: float(s["at"]))
    out = []
    for i in range(n):
        t = i / (n - 1)
        if t <= float(pts[0]["at"]):
            out.append(pts[0]["rgb"])
            continue
        if t >= float(pts[-1]["at"]):
            out.append(pts[-1]["rgb"])
            continue
        for a, b in zip(pts, pts[1:]):
            ta, tb = float(a["at"]), float(b["at"])
            if ta <= t <= tb:
                span = tb - ta or 1.0
                out.append(mix_oklab(a["rgb"], b["rgb"], (t - ta) / span))
                break
    return out


# --------------------------------------------------------------------------- helpers
def iter_maps(node: dict, prefix: str = ""):
    """Yield (dotted_name, table) for every colour map, including nested `[a.b]` tables."""
    for k, v in node.items():
        if not isinstance(v, dict) or k == "meta":
            continue
        name = f"{prefix}.{k}" if prefix else k
        if "stops" in v or "values" in v:
            yield name, v
        yield from iter_maps(v, name)


def flatten(d: dict, prefix: str = "") -> dict:
    out = {}
    for k, v in d.items():
        key = f"{prefix}.{k}" if prefix else str(k)
        if isinstance(v, dict):
            out.update(flatten(v, key))
        else:
            out[key] = v
    return out


def rs_ident(path: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", path.lower()).strip("_")


def cmap_slug(name: str) -> str:
    """Canonical key for a colour map across all emitters (`bipolar.cb` -> `bipolar_cb`)."""
    return name.replace("-", "_").replace(".", "_")


def rust_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return f"{v}_i32" if abs(v) < 2**31 else f"{v}_i64"
    if isinstance(v, float):
        return f"{v}_f32"
    if isinstance(v, str):
        return f'"{v}"'
    if isinstance(v, list):
        inner = ", ".join(rust_value(x) for x in v)
        return f"&[{inner}]"
    return f'"{v}"'


def rust_type(v) -> str:
    if isinstance(v, bool):
        return "bool"
    if isinstance(v, int):
        return "i32"
    if isinstance(v, float):
        return "f32"
    if isinstance(v, str):
        return "&str"
    if isinstance(v, list):
        if all(isinstance(x, str) for x in v):
            return "&[&str]"
        if all(isinstance(x, int) and not isinstance(x, bool) for x in v):
            return "&[i32]"
        if all(isinstance(x, (int, float)) and not isinstance(x, bool) for x in v):
            return "&[f32]"
        return "&[&str]"
    return "&str"


def css_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, list):
        return ", ".join(str(x) for x in v)
    return str(v)


# --------------------------------------------------------------------------- load
def load() -> dict:
    data = {}
    for f in FILES:
        p = TOK / f
        if not p.exists():
            sys.exit(f"token_gen: missing {p}")
        with p.open("rb") as fh:
            data[f.split(".")[0]] = tomllib.load(fh)
    return data


# --------------------------------------------------------------------------- emitters
def emit_rust(data: dict, flat: dict) -> str:
    # This file is `include!`d at the crate root of `sparq-ui::tokens`, so it may carry a
    # crate-level inner attribute but must NOT carry inner doc comments (those belong to the
    # including module). Human-facing explanation lives in tools/README.md and token-spec.md.
    lines = [
        "// GENERATED by tools/token_gen.py from design/tokens/*.toml -- DO NOT EDIT.",
        "// The single source of truth is the TOML; regenerate with `python3 tools/token_gen.py`.",
        "// CI fails if this file is stale (`token_gen.py --check`).",
        "// No attributes are emitted here: this file is `include!`d inside `sparq-ui::tokens`, which",
        "// carries the crate-level allows. Inner attributes are not valid in an include! body.",
        "",
        "/// A colour token: sRGB 8-bit channels plus the CSS-style hex used by mockups.",
        "pub struct Color {",
        "    pub r: u8,",
        "    pub g: u8,",
        "    pub b: u8,",
        "    pub hex: &'static str,",
        "}",
        "",
        "impl Color {",
        "    pub const fn from_hex(r: u8, g: u8, b: u8, hex: &'static str) -> Self {",
        "        Self { r, g, b, hex }",
        "    }",
        "    /// Linear-light relative luminance (WCAG).",
        "    pub fn luminance(&self) -> f32 {",
        "        let f = |c: u8| {",
        "            let u = c as f32 / 255.0;",
        "            if u > 0.04045 { ((u + 0.055) / 1.055).powf(2.4) } else { u / 12.92 }",
        "        };",
        "        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)",
        "    }",
        "}",
        "",
    ]
    for path, v in flat.items():
        name = rs_ident(path)
        if isinstance(v, str) and re.fullmatch(r"#[0-9A-Fa-f]{6}", v):
            r, g, b = hex_to_rgb(v)
            lines.append(
                f'pub const {name.upper()}: Color = Color::from_hex({r}, {g}, {b}, "{v}");'
            )
        else:
            lines.append(f"pub const {name.upper()}: {rust_type(v)} = {rust_value(v)};")
    lines.append("")
    return "\n".join(lines)


def emit_css(flat: dict, luts: dict) -> str:
    out = [
        "/* GENERATED by tools/token_gen.py -- DO NOT EDIT. Source: design/tokens/*.toml */",
        ":root {",
    ]
    for path, v in flat.items():
        out.append(f"  --sparq-{rs_ident(path).replace('_', '-')}: {css_value(v)};")
    for name, lut in luts.items():
        stops = ", ".join(f"{c} {(i / (len(lut) - 1)) * 100:.2f}%" for i, c in enumerate(lut))
        out.append(f"  --sparq-cmap-{rs_ident(name)}: linear-gradient(to right, {stops});")
    out.append("}")
    out.append("")
    return "\n".join(out)


def emit_svg_defs(data: dict, luts: dict) -> str:
    colors = data["colors"]
    sig = colors["signal"]
    out = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" style="position:absolute">',
        "  <!-- GENERATED by tools/token_gen.py -- DO NOT EDIT. Import into mockups via xlink or copy <defs>. -->",
        "  <defs>",
    ]
    for name, ramp in luts.items():
        stops = "".join(
            f'<stop offset="{(i / (len(ramp) - 1)) * 100:.2f}%" stop-color="{c}"/>'
            for i, c in enumerate(ramp)
        )
        out.append(f'    <linearGradient id="cmap-{rs_ident(name)}" x1="0" y1="0" x2="1" y2="0">{stops}</linearGradient>')
    out.append("    <g id=\"signal-legend\">")
    enc = {
        "audio": ("solid-stroke", 2, ""),
        "cv": ("solid-thin", 1, ""),
        "event": ("dashed-6-3", 2, 'stroke-dasharray="6 3"'),
        "data": ("dotted-2-4", 2, 'stroke-dasharray="2 4"'),
        "spatial": ("double-stroke", 1, ""),
    }
    y = 0
    for key, (name, w, extra) in enc.items():
        col = sig[key]["colour"]
        if key == "spatial":
            out.append(
                f'      <line x1="0" y1="{y - 2}" x2="40" y2="{y - 2}" stroke="{col}" stroke-width="1"/>'
            )
            out.append(
                f'      <line x1="0" y1="{y + 2}" x2="40" y2="{y + 2}" stroke="{col}" stroke-width="1"/>'
            )
        else:
            out.append(
                f'      <line x1="0" y1="{y}" x2="40" y2="{y}" stroke="{col}" stroke-width="{w}" {extra}/>'
            )
        out.append(
            f'      <text x="50" y="{y + 4}" font-family="ui-monospace, monospace" font-size="11" fill="#9FB0C0">{key} · {sig[key]["label"]}</text>'
        )
        y += 20
    out.append("    </g>")
    out.append("  </defs>")
    out.append("</svg>")
    out.append("")
    return "\n".join(out)


CONTRAST_TEXT_KEYS = ["text.primary", "text.secondary", "text.tertiary", "text.disabled"]


def build_preview(data: dict, flat: dict, luts: dict, report: dict) -> str:
    c = data["colors"]
    ty = data["typography"]
    lay = data["layout"]
    mo = data["motion"]
    ground = c["ground"]["base"]
    panel = c["ground"]["panel"]
    inset = c["ground"]["inset"]
    mono = "ui-monospace, 'Cascadia Mono', 'SF Mono', Menlo, Consolas, monospace"

    def swatch(name, hexv, extra=""):
        cr = contrast(hexv, ground)
        ok = "pass" if cr >= 3.0 else "LOW"
        return (
            f'<div class="sw"><span class="chip" style="background:{hexv}"></span>'
            f'<code>{name}</code><b>{hexv}</b><i>{cr:.2f}:1 <em class="{ok}">{ok}</em></i>'
            f"<small>{extra}</small></div>"
        )

    signal_rows = "".join(
        swatch(f"signal.{k}", v["colour"], f'{v["encoding"]} · glyph {v["label"]}')
        for k, v in c["signal"].items()
    )
    ground_rows = "".join(swatch(f"ground.{k}", v) for k, v in c["ground"].items())
    text_rows = "".join(swatch(f"text.{k}", v) for k, v in c["text"].items() if isinstance(v, str))
    state_rows = "".join(
        swatch(f"state.{k}", v["colour"], v.get("encoding", ""))
        for k, v in c["state"].items()
        if isinstance(v, dict) and "colour" in v
    )

    scale_rows = "".join(
        f'<div class="ty"><span class="k">type.scale.{k}</span>'
        f'<span style="font-size:{v}px">1 240.0 Hz −14.2 dB 0.710 φ≈∞</span>'
        f'<span class="k">{v} px</span></div>'
        for k, v in ty["scale"].items()
    )

    fmt = ty["format"]
    fmt_rows = "".join(
        f'<tr><td>{k}</td><td class="num">{json.dumps(v, separators=(",", ":"))}</td></tr>'
        for k, v in fmt.items()
    )

    touch_rows = "".join(
        f'<div class="tt"><span class="box" style="width:{min(v,160)}px;height:{min(v,160) if v<200 else 24}px"></span>'
        f"<code>touch.{k}</code><b>{v} px</b></div>"
        for k, v in lay["touch"].items()
        if isinstance(v, int) and v >= 6
    )

    stroke_rows = "".join(
        f'<div class="stk"><span style="border-top:{v}px solid {c["text"]["primary"]};display:block;width:120px"></span>'
        f"<code>stroke.{k}</code><b>{v} px</b></div>"
        for k, v in lay["stroke"].items()
        if isinstance(v, int)
    )

    cmap_rows = ""
    for name, lut in luts.items():
        stops = ", ".join(f"{col} {i / (len(lut) - 1) * 100:.1f}%" for i, col in enumerate(lut))
        cmap_rows += (
            f'<div class="cm"><code>colormap.{name}</code>'
            f'<div class="ramp" style="background:linear-gradient(to right,{stops})"></div>'
            f"<small>256 entries · {lut[0]} → {lut[-1]}</small></div>"
        )

    motion_rows = "".join(
        f'<div class="mo"><code>motion.duration.{k}</code><b>{v} ms</b>'
        f'<span class="bar" style="width:{min(int(v) / 20, 100):.0f}%"></span></div>'
        for k, v in mo["duration"].items()
    )

    checks = "".join(
        f'<li class="{"ok" if v["pass"] else "fail"}"><b>{"PASS" if v["pass"] else "FAIL"}</b> {k} — {v["detail"]}</li>'
        for k, v in report.items()
    )

    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>sparq · design token preview v{data['colors']['meta']['version']}</title>
<style>
  * {{ box-sizing: border-box; }}
  body {{ margin:0; background:{ground}; color:{c['text']['primary']}; font-family:{mono};
         font-variant-numeric: tabular-nums; font-size:13px; line-height:1.45; }}
  header {{ position:sticky; top:0; background:{panel}; border-bottom:1px solid rgba(232,241,248,.25);
            padding:12px 24px; display:flex; gap:24px; align-items:baseline; z-index:9; }}
  header h1 {{ font-size:15px; margin:0; font-weight:600; letter-spacing:.02em; }}
  header span {{ color:{c['text']['tertiary']}; font-size:11px; }}
  main {{ padding:24px; display:grid; gap:32px; }}
  section {{ border:1px solid rgba(232,241,248,.25); background:{panel}; padding:20px 24px; }}
  h2 {{ font-size:11px; letter-spacing:.06em; text-transform:uppercase; color:{c['text']['tertiary']};
       margin:0 0 16px; font-weight:500; }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(240px,1fr)); gap:12px; }}
  .sw {{ background:{inset}; border:1px solid rgba(232,241,248,.10); padding:10px 12px;
         display:grid; grid-template-columns:24px 1fr auto; grid-template-rows:auto auto; gap:4px 10px; align-items:center; }}
  .chip {{ width:24px; height:24px; grid-row:span 2; border:1px solid rgba(232,241,248,.25); }}
  .sw code {{ color:{c['text']['secondary']}; font-size:11px; }}
  .sw b {{ text-align:right; font-size:12px; }}
  .sw i {{ grid-column:2/4; font-style:normal; font-size:10px; color:{c['text']['tertiary']}; }}
  .sw small {{ grid-column:2/4; font-size:10px; color:{c['text']['tertiary']}; }}
  em.pass {{ color:{c['signal']['data']['colour']}; font-style:normal; }}
  em.LOW {{ color:{c['state']['error']['colour']}; font-style:normal; }}
  .ty {{ display:grid; grid-template-columns:140px 1fr 70px; gap:12px; align-items:baseline;
         padding:6px 0; border-bottom:1px solid rgba(232,241,248,.10); }}
  .ty .k {{ color:{c['text']['tertiary']}; font-size:10px; }}
  .num {{ color:{c['signal']['audio']['colour']}; }}
  table {{ border-collapse:collapse; width:100%; }}
  td, th {{ text-align:left; padding:4px 8px; border-bottom:1px solid rgba(232,241,248,.10); font-size:11px; }}
  td:first-child {{ color:{c['text']['secondary']}; }}
  .tt, .stk, .mo {{ display:flex; gap:12px; align-items:center; padding:6px 0;
                    border-bottom:1px solid rgba(232,241,248,.10); }}
  .tt code, .stk code, .mo code {{ color:{c['text']['tertiary']}; font-size:11px; min-width:220px; }}
  .tt .box {{ background:{c['signal']['cv']['colour']}; opacity:.55; display:inline-block; }}
  .cm {{ margin-bottom:14px; }}
  .cm code {{ font-size:11px; color:{c['text']['secondary']}; }}
  .ramp {{ height:22px; border:1px solid rgba(232,241,248,.25); margin:4px 0; }}
  .cm small {{ font-size:10px; color:{c['text']['tertiary']}; }}
  .mo .bar {{ height:6px; background:{c['signal']['event']['colour']}; opacity:.7; }}
  ul.checks {{ list-style:none; padding:0; margin:0; }}
  ul.checks li {{ padding:6px 0; border-bottom:1px solid rgba(232,241,248,.10); font-size:12px; }}
  ul.checks b {{ display:inline-block; width:52px; }}
  li.ok b {{ color:{c['signal']['data']['colour']}; }}
  li.fail b {{ color:{c['state']['error']['colour']}; }}
  .rules li {{ font-size:12px; color:{c['text']['secondary']}; margin-bottom:4px; }}
</style></head>
<body>
<header>
  <h1>sparq · design token preview</h1>
  <span>colors v{c['meta']['version']} · typography v{ty['meta']['version']} · layout v{lay['meta']['version']} · motion v{mo['meta']['version']}</span>
  <span>generated by tools/token_gen.py — do not edit</span>
</header>
<main>

<section><h2>Automated conformance checks</h2><ul class="checks">{checks}</ul></section>

<section><h2>Signal classes — the only permitted accents</h2><div class="grid">{signal_rows}</div></section>
<section><h2>Ground &amp; surface — depth by tint, never by shadow</h2><div class="grid">{ground_rows}</div></section>
<section><h2>Text tiers</h2><div class="grid">{text_rows}</div></section>
<section><h2>States — pattern first, colour redundant</h2><div class="grid">{state_rows}</div></section>

<section><h2>Type scale · tabular numerals always</h2>{scale_rows}</section>
<section><h2>Numeric formatting rules</h2><table><tr><th>unit</th><th>rule</th></tr>{fmt_rows}</table></section>

<section><h2>Strokes — only {', '.join(str(v) for v in lay['stroke']['widths_allowed'])} px</h2>{stroke_rows}</section>
<section><h2>Touch targets (px)</h2>{touch_rows}</section>
<section><h2>Motion durations</h2>{motion_rows}</section>
<section><h2>Colour maps — baked {LUT_SIZE}-entry LUTs, interpolated in oklab</h2>{cmap_rows}</section>

<section><h2>The ten hard rules (CI-enforced)</h2><ol class="rules">
<li>No literal colours, sizes, spacings, radii or durations in widget/panel/display code.</li>
<li>Stroke widths from the approved set only.</li>
<li>Corners are 0, 2 or 4 px; circles only for knobs and ports.</li>
<li>No drop shadows, no gradient decoration (gradients live in colour maps and meter fills).</li>
<li>No pure white; pure black only in the high-contrast theme and full-bleed visuals.</li>
<li>Accents are assigned by signal class only — never by module, category or taste.</li>
<li>Every state that uses colour also uses a non-colour encoding.</li>
<li>Every interactive element ≥ touch.min_target (44 px), dense exceptions badged.</li>
<li>Every number renders with its unit, tabular numerals, fixed precision.</li>
<li>Colour maps are chosen by data kind: magnitude → sequential, signed → diverging, category → qualitative.</li>
</ol></section>

</main></body></html>
"""


# --------------------------------------------------------------------------- validation
def validate(data: dict, luts: dict, cmap_tables: dict) -> dict:
    c = data["colors"]
    lay = data["layout"]
    ty = data["typography"]
    ground = c["ground"]["base"]
    rep = {}

    # 1. contrast of text tiers. `disabled` is intentionally dim and therefore excluded from the
    #    pass/fail gate but still reported; `inverse` sits on an accent, not on the ground.
    tiers = {k: v for k, v in c["text"].items() if isinstance(v, str) and k not in ("inverse", "disabled")}
    worst = min(contrast(v, ground) for v in tiers.values())
    worst_k = min(tiers, key=lambda k: contrast(tiers[k], ground))
    dis = c["text"].get("disabled")
    rep["text contrast vs ground"] = {
        "pass": worst >= 4.5,
        "detail": (
            f"worst body tier {worst_k} {worst:.2f}:1 (min 4.5:1); "
            f"disabled {contrast(dis, ground):.2f}:1 (excluded, decorative)"
        ),
    }

    # 2. accents distinguishable by lightness as well as hue (monochrome survival)
    ls = []
    for k, v in c["signal"].items():
        L, _, _ = srgb_to_oklab(hex_to_rgb(v["colour"]))
        ls.append((k, round(L, 3)))
    spread = max(l for _, l in ls) - min(l for _, l in ls)
    rep["accent lightness spread"] = {
        "pass": spread >= 0.10,
        "detail": f"{spread:.3f} oklab L across {len(ls)} classes ({', '.join(f'{k} {l}' for k, l in ls)})",
    }

    # 3. every accent has a non-colour encoding
    missing = [k for k, v in c["signal"].items() if not v.get("encoding") or not v.get("label")]
    rep["redundant encoding present"] = {
        "pass": not missing,
        "detail": "all 5 classes" if not missing else f"missing: {missing}",
    }

    # 4. every state has an encoding
    smissing = [
        k for k, v in c["state"].items() if isinstance(v, dict) and not v.get("encoding")
    ]
    rep["state encodings"] = {
        "pass": not smissing,
        "detail": f"{len(c['state'])} states" if not smissing else f"missing: {smissing}",
    }

    # 5. sequential maps are monotonic in lightness
    bad = []
    for name, lut in luts.items():
        cm = cmap_tables.get(name, {})
        kind = cm.get("kind")
        if kind != "sequential":
            continue
        prev = None
        mono = True
        for hx in lut:
            L, _, _ = srgb_to_oklab(hex_to_rgb(hx))
            if prev is not None and L < prev - 1e-6:
                mono = False
            prev = L
        if not mono:
            bad.append(name)
    rep["sequential maps monotonic in L"] = {
        "pass": not bad,
        "detail": "all sequential ramps increase in lightness" if not bad else f"non-monotonic: {bad}",
    }

    # 6. every default map has a colour-blind alternative
    need_cb = ["phosphor", "bipolar", "categorical_6"]
    miss = [f"{n}.cb" for n in need_cb if cmap_slug(n) + "_cb" not in luts]
    rep["cb-safe alternatives"] = {
        "pass": not miss,
        "detail": "phosphor.cb, bipolar.cb, categorical-6.cb present" if not miss else f"missing {miss}",
    }

    # 7. no pure white anywhere in the token set, EXCEPT the documented allowlist
    #    (operator ruling 2026-10-01: state.selected reads white — the one exemption).
    allow = set(c.get("forbidden", {}).get("pure_white_allowlist", []))
    whites = [
        k
        for k, v in flatten(data).items()
        if isinstance(v, str) and v.upper() == "#FFFFFF" and k not in allow
    ]
    unlisted = [k for k in allow if flatten(data).get(k, "").upper() != "#FFFFFF"]
    rep["no pure white"] = {
        "pass": not whites and not unlisted,
        "detail": (
            f"offenders: {whites}" if whites
            else f"stale allowlist entries: {unlisted}" if unlisted
            else f"clean (allowlisted: {sorted(allow)})" if allow
            else "clean"
        ),
    }

    # 8. touch minimums
    tm = lay["touch"]
    rep["touch minimums"] = {
        "pass": tm["min_target"] >= 44 and tm["stage"]["min_macro_pad"] >= 96,
        "detail": f"min_target {tm['min_target']} px · stage macro {tm['stage']['min_macro_pad']} px · port capture {tm['port_capture_radius']} px",
    }

    # 9. type scale covers stage legibility
    sc = ty["scale"]
    rep["stage type sizes"] = {
        "pass": sc["stage"] >= 44 and sc["stage_xl"] >= 88,
        "detail": f"stage {sc['stage']} px · stage_xl {sc['stage_xl']} px · min {sc['xs']} px",
    }

    # 10. stroke widths approved set
    sw = lay["stroke"]["widths_allowed"]
    rep["stroke width set"] = {"pass": set(sw) <= {1, 2, 3, 4}, "detail": f"{sw}"}

    return rep


# --------------------------------------------------------------------------- main
def emit_bundle(data: dict, tokens_json: str, payload_sha: str) -> str:
    """The token bundle v1 (WO-017): the WIT `tokens.bundle` record, serialised for disk.

    Envelope = {version, encoding, payload_sha256, payload}. `payload` is the tokens.json
    object; the canonical re-serialisation invariant (indent=2, sort_keys, trailing newline)
    is asserted in main()'s report, so the bundle can never claim bytes the sibling artefact
    does not have. `version` is the tokens' own semver (colors.toml `[meta]`) — the semver
    rules are token-spec §4's: additive tokens = minor, value changes = minor + screenshot
    re-baseline, removals/renames = major with an alias table, and a guest pinned below the
    host's major is refused at load IN WORDS (E-TOKEN-BUNDLE-VERSION), never mis-themed
    silently. At runtime the host hands guests these exact bytes as `bundle.payload`
    (encoding `json`, tokens.wit) — this file is that payload's checked-in golden.
    """
    version = data["colors"]["meta"]["version"]
    parts = version.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise SystemExit(f"colors.toml [meta] version {version!r} is not a semver triple")
    bundle = {
        "version": {"major": int(parts[0]), "minor": int(parts[1]), "patch": int(parts[2])},
        "encoding": "json",
        "payload_sha256": payload_sha,
        "payload": json.loads(tokens_json),
    }
    return json.dumps(bundle, indent=2, sort_keys=True) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if generated files are stale")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    data = load()
    flat = {}
    for section in ("colors", "typography", "layout", "motion", "colormaps"):
        d = dict(data[section])
        d.pop("meta", None)
        # drop non-leaf/annotation tables that are not tokens
        for k in ("forbidden", "theme"):
            d.pop(k, None)
        flat.update(flatten(d, section.rstrip("s")))
    # derived high-contrast theme (COLOR_HC_*): overrides + accent lightness lift
    flat.update(derive_high_contrast(data))

    # colour maps -> baked LUTs (sequential/diverging interpolate in oklab; qualitative emit as-is)
    luts: dict[str, list[str]] = {}
    cmap_tables: dict[str, dict] = {}
    for name, cm in iter_maps(data["colormaps"]):
        cmap_tables[cmap_slug(name)] = cm
        if cm.get("kind") == "qualitative":
            luts[cmap_slug(name)] = [str(v) for v in cm["values"]]
        else:
            luts[cmap_slug(name)] = bake_lut(cm["stops"])

    report = validate(data, luts, cmap_tables)

    # tokens.json is the canonical byte string: the bundle's payload re-serialises to EXACTLY
    # this text (checked below), and the Rust golden test pins its sha256 from the bundle.
    tokens_json = json.dumps(
        {"version": data["colors"]["meta"]["version"], "tokens": flat,
         "colormaps": {k: v for k, v in luts.items()}},
        indent=2, sort_keys=True,
    ) + "\n"

    # The token-bundle invariant (docstring): canonical re-serialisation of the payload is
    # byte-identical to tokens.json, and payload_sha256 is the sha256 of those exact bytes.
    bundle_sha = hashlib.sha256(tokens_json.encode(ENCODING)).hexdigest()
    report["token-bundle payload round-trip"] = {
        "pass": json.dumps(json.loads(tokens_json), indent=2, sort_keys=True) + "\n" == tokens_json,
        "detail": f"tokens.json is {len(tokens_json)} B, sha256 {bundle_sha[:16]}…; payload re-serialises byte-identical",
    }

    outputs = {
        GEN / "tokens.rs": emit_rust(data, flat),
        GEN / "tokens.json": tokens_json,
        GEN / "token-bundle.json": emit_bundle(data, tokens_json, bundle_sha),
        GEN / "tokens.css": emit_css(flat, luts),
        GEN / "tokens.svg": emit_svg_defs(data, luts),
        GEN / "colormaps.json": json.dumps({"size": LUT_SIZE, "maps": luts}, indent=1) + "\n",
        ROOT / "design" / "tokens" / "preview.html": build_preview(data, flat, luts, report),
    }

    GEN.mkdir(parents=True, exist_ok=True)
    stale = []
    for path, content in outputs.items():
        existing = read_text(path) if path.exists() else None
        if existing == content:
            continue
        stale.append(path)
        if not args.check:
            write_text(path, content)

    failures = [k for k, v in report.items() if not v["pass"]]
    if not args.quiet:
        for k, v in report.items():
            print(f"  [{'PASS' if v['pass'] else 'FAIL'}] {k}: {v['detail']}")
        verb = "would write" if args.check else "wrote"
        print(f"\n  {verb} {len(stale)} file(s): {', '.join(p.name for p in stale) or 'none (all up to date)'}")

    if args.check and stale:
        print("token_gen --check: stale generated files:", ", ".join(str(p) for p in stale), file=sys.stderr)
        return 1
    if failures:
        print("token_gen: failing checks:", ", ".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
