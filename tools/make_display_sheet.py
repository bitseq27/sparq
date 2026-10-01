#!/usr/bin/env python3
"""WO-004 mockup 3 of 3 — the display-module sheet.

Generates design/mockups/display-sheet.svg from design/tokens (via the baked LUTs that
tools/token_gen.py emits). Regenerate after any token change; then run tools/token_audit.py.

    python3 tools/make_display_sheet.py
"""
from __future__ import annotations

import json
import tomllib
import math
import pathlib
import random
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Every text read/write goes through these helpers so the tool behaves identically on Windows,
# where the default encoding is the ANSI code page and text writes translate "\n" to "\r\n".
# See tools/README.md -> "Text I/O portability". The `tools-text-io` gate enforces it.
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
    return pathlib.Path(path).read_text(encoding=ENCODING)


def write_text(path, content: str) -> None:
    pp = pathlib.Path(path)
    pp.parent.mkdir(parents=True, exist_ok=True)
    with pp.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


_MAPS = json.loads(read_text(ROOT / "design/tokens/generated/colormaps.json"))["maps"]
random.seed(11)

LUT = _MAPS["phosphor"]
GREY = _MAPS["greyscale"]

# Colours come from the TOKEN FILE, never from this script's memory (defect class: a generator
# with literals re-freezes every palette move — the warm shift of 2026-09-30 caught these).
_TOK = tomllib.loads(read_text(ROOT / "design/tokens/colors.toml"))
A = _TOK["signal"]["audio"]["colour"]
C = _TOK["signal"]["cv"]["colour"]
E = _TOK["signal"]["event"]["colour"]
D = _TOK["signal"]["data"]["colour"]
S = _TOK["signal"]["spatial"]["colour"]
T1 = _TOK["text"]["primary"]
T2 = _TOK["text"]["secondary"]
T3 = _TOK["text"]["tertiary"]
T4 = _TOK["text"]["disabled"]
GND = _TOK["ground"]["base"]
CAN = _TOK["ground"]["canvas"]
PAN = _TOK["ground"]["panel"]
ALT = _TOK["ground"]["panel_alt"]
INS = _TOK["ground"]["inset"]
WARN = _TOK["state"]["warning"]["colour"]

W, MARGIN, GAP, HEADER = 2560, 24, 16, 104
COLS, ROWS = 4, 3
TW = (W - 2 * MARGIN - (COLS - 1) * GAP) // COLS
TH = 480
H = HEADER + ROWS * (TH + GAP) - GAP + 76


def _sample(ramp: list[str], v: float) -> str:
    return ramp[max(0, min(len(ramp) - 1, int(max(0.0, min(1.0, v)) * (len(ramp) - 1))))]


def lut_at(v: float) -> str:
    """Sample the phosphor sequential map (magnitude data)."""
    return _sample(LUT, v)


def grey_at(v: float) -> str:
    """Sample the greyscale sequential map (depth/ordered scalar data)."""
    return _sample(GREY, v)


def tile(i: int, title: str, meta: str):
    c, r = i % COLS, i // COLS
    x = MARGIN + c * (TW + GAP)
    y = HEADER + r * (TH + GAP)
    px, py = x + 16, y + 44
    pw, ph = TW - 32, TH - 44 - 46
    head = (
        f'<rect x="{x}" y="{y}" width="{TW}" height="{TH}" fill="{PAN}" stroke="{T1}" stroke-opacity=".25"/>'
        f'<line x1="{x}" y1="{y+32}" x2="{x+TW}" y2="{y+32}" stroke="{T1}" stroke-opacity=".10"/>'
        f'<text x="{x+16}" y="{y+22}" font-size="13" fill="{T1}">{title}</text>'
        f'<text x="{x+TW-16}" y="{y+22}" font-size="10" fill="{T3}" text-anchor="end">{meta}</text>'
        f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" fill="{INS}" stroke="{T1}" stroke-opacity=".10"/>'
    )
    return x, y, px, py, pw, ph, head


def axes(px, py, pw, ph, xl, yl, cols=4, rows=3):
    g = f'<g stroke="{T1}" stroke-opacity=".10" stroke-width="1">'
    for k in range(1, cols):
        g += f'<line x1="{px+pw*k/cols:.0f}" y1="{py}" x2="{px+pw*k/cols:.0f}" y2="{py+ph}"/>'
    for k in range(1, rows):
        g += f'<line x1="{px}" y1="{py+ph*k/rows:.0f}" x2="{px+pw}" y2="{py+ph*k/rows:.0f}"/>'
    g += "</g>"
    g += f'<text x="{px}" y="{py+ph+18}" font-size="10" fill="{T4}">{xl}</text>'
    g += f'<text x="{px+pw}" y="{py+ph+18}" font-size="10" fill="{T4}" text-anchor="end">{yl}</text>'
    return g


def caption(x, y, txt):
    return f'<text x="{x+16}" y="{y+TH-14}" font-size="10" fill="{T4}">{txt}</text>'


p: list[str] = []

# ---------------------------------------------------------- 1 · dsp/scope
x, y, px, py, pw, ph, c = tile(0, "dsp/scope", "waveform + persistence · audio class")
p.append(c + axes(px, py, pw, ph, "0 ms", "21.3 ms", 6, 4))
mid = py + ph / 2
for k, (phase, op, wid, centre) in enumerate(((0.0, ".14", 1, 260), (0.4, ".28", 1, 300), (0.9, ".95", 2, 340))):
    pts = []
    for i in range(601):
        v = (math.sin(i * 0.09 + phase) * 0.32 + math.sin(i * 0.021 + phase / 2) * 0.50
             + math.sin(i * 0.37) * 0.12) * ph * 0.42 * math.exp(-((i - centre) / 250) ** 2)
        pts.append(f"{px+i*pw/600:.1f},{mid+v:.1f}")
    extra = ' filter="url(#g)"' if wid == 2 else ""
    p.append(f'<polyline points="{" ".join(pts)}" fill="none" stroke="{A}" stroke-width="{wid}" stroke-opacity="{op}"{extra}/>')
ix, iy, iw, ih = px + pw - 190, py + 14, 176, 150
p.append(f'<rect x="{ix}" y="{iy}" width="{iw}" height="{ih}" fill="{CAN}" stroke="{T1}" stroke-opacity=".25"/>')
p.append(f'<line x1="{ix+iw/2}" y1="{iy}" x2="{ix+iw/2}" y2="{iy+ih}" stroke="{T1}" stroke-opacity=".10"/>')
p.append(f'<line x1="{ix}" y1="{iy+ih/2}" x2="{ix+iw}" y2="{iy+ih/2}" stroke="{T1}" stroke-opacity=".10"/>')
lp = " ".join(
    f"{ix+iw/2+math.cos(t*0.7)*iw*0.38*math.sin(t*0.11):.1f},{iy+ih/2+math.sin(t)*ih*0.40*math.cos(t*0.07):.1f}"
    for t in (i * 0.05 for i in range(400))
)
p.append(f'<polyline points="{lp}" fill="none" stroke="{C}" stroke-width="1" stroke-opacity=".85"/>')
p.append(f'<text x="{ix+6}" y="{iy+14}" font-size="10" fill="{T3}">x/y lissajous</text>')
p.append(caption(x, y, "persistence half-life 300 ms · glow ∝ amplitude^1.5 · inset: L/R phase portrait"))

# ---------------------------------------------------------- 2 · dsp/spectrum
x, y, px, py, pw, ph, c = tile(1, "dsp/spectrum", "magnitude · log f · phosphor · 8192-pt fft")
p.append(c + axes(px, py, pw, ph, "20 Hz · 100 · 1k · 10k · 20 kHz", "0 dB / −120 dB", 5, 4))
NB = 150
for i in range(NB):
    f = i / NB
    mag = (math.exp(-((f - 0.05) ** 2) / 0.002) * 0.90
           + math.exp(-((f - 0.12) ** 2) / 0.004) * 0.60
           + math.exp(-((f - 0.28) ** 2) / 0.010) * 0.35
           + max(0.0, (0.5 - f)) * 0.5 * random.uniform(0.4, 1.0)
           + 0.06 * random.random() - f * 0.25)
    mag = max(0.02, min(1.0, mag))
    bw = pw / NB
    p.append(f'<rect x="{px+i*bw:.1f}" y="{py+ph-mag*ph:.1f}" width="{bw+0.6:.1f}" height="{mag*ph:.1f}" '
             f'fill="{lut_at(mag)}" fill-opacity="{0.45+0.55*mag:.2f}"/>')
    pk = min(1.0, mag + random.uniform(0.02, 0.12))
    p.append(f'<rect x="{px+i*bw:.1f}" y="{py+ph-pk*ph:.1f}" width="{bw+0.6:.1f}" height="1.5" fill="{T1}" fill-opacity=".55"/>')
p.append(caption(x, y, "peak-hold 1.5 s · hann window · every bin exportable as a data stream"))

# ---------------------------------------------------------- 3 · dsp/sonogram
x, y, px, py, pw, ph, c = tile(2, "dsp/sonogram", "waterfall · log magnitude · newest at bottom")
p.append(c + axes(px, py, pw, ph, "20 Hz → 20 kHz", "time ↓ 12.0 s", 4, 1))
NR, NC = 44, 96
for r in range(NR):
    for k in range(NC):
        f = k / NC
        v = (math.exp(-((f - 0.06) ** 2) / 0.002) * 0.90
             + math.exp(-((f - 0.18) ** 2) / 0.006) * 0.50 * (0.6 + 0.4 * math.sin(r * 0.3))
             + max(0.0, (0.35 - f)) * 0.4 * random.uniform(0.3, 1.0)
             + (0.70 if random.random() < 0.03 else 0.0) + random.uniform(0, 0.05))
        v *= 0.55 + 0.45 * (1 - r / NR)
        v = max(0.0, min(1.0, v))
        if v < 0.09:
            continue
        p.append(f'<rect x="{px+k*pw/NC:.1f}" y="{py+r*ph/NR:.1f}" width="{pw/NC+0.6:.1f}" height="{ph/NR+0.6:.1f}" '
                 f'fill="{lut_at(v)}" fill-opacity="{0.35+0.65*v:.2f}"/>')
p.append(caption(x, y, "same LUT as the Perform-mode full-bleed visual · gpu compute, never the audio thread"))

# ---------------------------------------------------------- 4 · dsp/phase-portrait
x, y, px, py, pw, ph, c = tile(3, "dsp/phase-portrait", "delay embedding τ=7 · lorenz σ10 ρ28 β8/3")
p.append(c + axes(px, py, pw, ph, "x(t) · normalised −1…+1", "x(t−τ) · −1…+1", 4, 4))
sx, sy, sz, dt, pts = 0.1, 0.0, 0.0, 0.006, []
for _ in range(3200):
    dx, dy, dz = 10 * (sy - sx), sx * (28 - sz) - sy, sx * sy - 8 / 3 * sz
    sx, sy, sz = sx + dx * dt, sy + dy * dt, sz + dz * dt
    pts.append((sx / 22.0, sy / 30.0))
poly = " ".join(f"{px+pw/2+u*pw*0.44:.1f},{py+ph/2-v*ph*0.44:.1f}" for u, v in pts)
p.append(f'<polyline points="{poly}" fill="none" stroke="{C}" stroke-width="1" stroke-opacity=".38"/>')
tail = " ".join(f"{px+pw/2+u*pw*0.44:.1f},{py+ph/2-v*ph*0.44:.1f}" for u, v in pts[-260:])
p.append(f'<polyline points="{tail}" fill="none" stroke="{C}" stroke-width="2" filter="url(#g)"/>')
p.append(f'<circle cx="{px+pw/2+pts[-1][0]*pw*0.44:.1f}" cy="{py+ph/2-pts[-1][1]*ph*0.44:.1f}" r="3" fill="{T1}"/>')
p.append(caption(x, y, "attractor of ANY signal · chaos measures (lyapunov, entropy) exported as cv"))

# ---------------------------------------------------------- 5 · dsp/vector-field
x, y, px, py, pw, ph, c = tile(4, "dsp/vector-field", "2 streams → 2D field · imu9 roll/pitch @ 200 Hz")
p.append(c + axes(px, py, pw, ph, "stream ch0 · −1…+1", "stream ch1 · −1…+1", 4, 4))
NX, NY = 17, 10
for i in range(NX):
    for j in range(NY):
        u = (i / (NX - 1)) * 2 - 1
        v = (j / (NY - 1)) * 2 - 1
        ax = math.sin(v * 2.4 + 0.6) * 0.9 - u * 0.35
        ay = math.cos(u * 2.1) * 0.9 - v * 0.35
        m = math.hypot(ax, ay) or 1e-6
        L = min(pw / NX, ph / NY) * 0.46 * (0.35 + 0.65 * min(1.0, m))
        cxp, cyp = px + (i + 0.5) * pw / NX, py + (j + 0.5) * ph / NY
        ex, ey = cxp + ax / m * L, cyp - ay / m * L
        col = lut_at(min(1.0, m / 1.4))
        p.append(f'<line x1="{cxp:.1f}" y1="{cyp:.1f}" x2="{ex:.1f}" y2="{ey:.1f}" stroke="{col}" stroke-width="1"/>')
        p.append(f'<circle cx="{ex:.1f}" cy="{ey:.1f}" r="1.6" fill="{col}"/>')
p.append(caption(x, y, "magnitude → phosphor LUT · direction → arrow · grid density is a display parameter"))

# ---------------------------------------------------------- 6 · dsp/geometry
x, y, px, py, pw, ph, c = tile(5, "dsp/geometry", "l-system turtle · F→F[+F]F[−F]F · 25.7°")
p.append(c + axes(px, py, pw, ph, "x · geometry units", "y", 4, 4))
ax_, ay_, ang, step, stack, seg = px + pw * 0.5, py + ph * 0.96, -90.0, ph * 0.075, [], []
s = "F"
for _ in range(3):
    s = "".join("F[+F]F[-F]F" if ch == "F" else ch for ch in s)
for ch in s[:1400]:
    if ch == "F":
        nx, ny = ax_ + step * math.cos(math.radians(ang)), ay_ + step * math.sin(math.radians(ang))
        seg.append((ax_, ay_, nx, ny))
        ax_, ay_ = nx, ny
    elif ch == "+":
        ang += 25.7
    elif ch == "-":
        ang -= 25.7
    elif ch == "[":
        stack.append((ax_, ay_, ang))
    elif ch == "]" and stack:
        ax_, ay_, ang = stack.pop()
for k, (x1, y1, x2, y2) in enumerate(seg):
    op = 0.20 + 0.6 * (k / len(seg))
    p.append(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{D}" stroke-width="1" stroke-opacity="{op:.2f}"/>')
cy = py + ph - 30
rule, st, cw = 30, [1 if random.random() < 0.5 else 0 for _ in range(72)], pw / 72
for gen in range(6):
    for i, v in enumerate(st):
        if v:
            p.append(f'<rect x="{px+i*cw:.1f}" y="{cy-gen*7:.1f}" width="{cw+0.5:.1f}" height="6" fill="{E}" fill-opacity="{0.85-gen*0.11:.2f}"/>')
    st = [(rule >> (st[(i - 1) % 72] * 4 + st[i] * 2 + st[(i + 1) % 72])) & 1 for i in range(72)]
p.append(f'<text x="{px}" y="{cy+22}" font-size="10" fill="{T4}">rule 30 · 6 generations · also a wavetable and a gate source</text>')
p.append(caption(x, y, "the same structures drive sound (wavetable / gate / trajectory) and visuals — geometry as score"))

# ---------------------------------------------------------- 7 · dsp/rig-map
x, y, px, py, pw, ph, c = tile(6, "dsp/rig-map", "quad-4 · 3 objects · calibrated 18:42 · spatial class")
p.append(c + axes(px, py, pw, ph, "x · −3.0 … +3.0 m", "y · −3.0 … +3.0 m", 4, 4))
cxp, cyp = px + pw / 2, py + ph / 2
R = min(pw, ph) * 0.40
p.append(f'<circle cx="{cxp:.0f}" cy="{cyp:.0f}" r="{R:.0f}" fill="none" stroke="{T1}" stroke-opacity=".18"/>')
p.append(f'<circle cx="{cxp:.0f}" cy="{cyp:.0f}" r="{R*0.55:.0f}" fill="none" stroke="{T1}" stroke-opacity=".10"/>')
p.append(f'<line x1="{cxp-R:.0f}" y1="{cyp:.0f}" x2="{cxp+R:.0f}" y2="{cyp:.0f}" stroke="{T1}" stroke-opacity=".10"/>')
p.append(f'<line x1="{cxp:.0f}" y1="{cyp-R:.0f}" x2="{cxp:.0f}" y2="{cyp+R:.0f}" stroke="{T1}" stroke-opacity=".10"/>')
for k, az in enumerate((45, 135, 225, 315)):
    a = math.radians(az)
    sxp, syp = cxp + math.cos(a) * R, cyp - math.sin(a) * R
    p.append(f'<rect x="{sxp-13:.0f}" y="{syp-13:.0f}" width="26" height="26" fill="{ALT}" stroke="{S}" stroke-width="2"/>')
    p.append(f'<text x="{sxp:.0f}" y="{syp+4:.0f}" font-size="11" fill="{S}" text-anchor="middle">{k+1}</text>')
    p.append(f'<text x="{sxp:.0f}" y="{syp-19:.0f}" font-size="9" fill="{T4}" text-anchor="middle">{az}° · 2.10 m</text>')
for oi, (kind, col) in enumerate((("liss", A), ("spiral", D), ("walk", C))):
    tp = []
    for t in range(60):
        u = t / 59
        if kind == "liss":
            aa, bb = math.sin(u * 6.2) * R * 0.62, math.sin(u * 9.4) * R * 0.48
        elif kind == "spiral":
            rr = R * (0.15 + 0.55 * u)
            aa, bb = rr * math.cos(u * 11), rr * math.sin(u * 11)
        else:
            if not tp:
                aa, bb = 0.0, 0.0
            else:
                aa = tp[-1][0] + random.uniform(-14, 14)
                bb = tp[-1][1] + random.uniform(-14, 14)
            aa = max(-R * 0.8, min(R * 0.8, aa))
            bb = max(-R * 0.8, min(R * 0.8, bb))
        tp.append((aa, bb))
    poly = " ".join(f"{cxp+u:.1f},{cyp-v:.1f}" for u, v in tp)
    p.append(f'<polyline points="{poly}" fill="none" stroke="{col}" stroke-width="1" stroke-opacity=".5" stroke-dasharray="2 4"/>')
    p.append(f'<circle cx="{cxp+tp[-1][0]:.1f}" cy="{cyp-tp[-1][1]:.1f}" r="6" fill="{col}" filter="url(#g)"/>')
    p.append(f'<text x="{cxp+tp[-1][0]+11:.1f}" y="{cyp-tp[-1][1]+4:.1f}" font-size="10" fill="{col}">obj{oi+1}</text>')
p.append(caption(x, y, "vbap active triplets as hairlines · per-speaker level heat uses colormap thermal"))

# ---------------------------------------------------------- 8 · dsp/graph-view
x, y, px, py, pw, ph, c = tile(7, "dsp/graph-view", "the patch itself · edge weight ∝ rms · lod 1")
p.append(c)
nodes = [("syn/membrane", 0.10, 0.18, A), ("syn/polyblep", 0.10, 0.44, A), ("mod/lfo", 0.10, 0.72, C),
         ("flt/svf", 0.36, 0.30, A), ("env/ad", 0.36, 0.62, C), ("util/gain", 0.60, 0.30, A),
         ("ana/rms", 0.60, 0.66, D), ("fx/bitcrush", 0.80, 0.16, A), ("out/main", 0.84, 0.50, A)]
pos = {n: (px + u * pw, py + v * ph) for n, u, v, _ in nodes}
colof = {n: cc for n, _, _, cc in nodes}
edges = [("syn/membrane", "flt/svf", A, 0.90), ("syn/polyblep", "flt/svf", A, 0.60),
         ("env/ad", "flt/svf", C, 0.40), ("mod/lfo", "flt/svf", C, 0.25),
         ("flt/svf", "util/gain", A, 0.85), ("util/gain", "ana/rms", A, 0.70),
         ("ana/rms", "flt/svf", C, 0.55), ("util/gain", "fx/bitcrush", A, 0.50),
         ("util/gain", "out/main", A, 0.95), ("fx/bitcrush", "out/main", A, 0.20)]
for a, b, col, wgt in edges:
    x1, y1 = pos[a]
    x2, y2 = pos[b]
    mx = (x1 + x2) / 2
    p.append(f'<path d="M{x1:.0f} {y1:.0f} C {mx:.0f} {y1:.0f} {mx:.0f} {y2:.0f} {x2:.0f} {y2:.0f}" fill="none" '
             f'stroke="{col}" stroke-width="{1 if col == C else 2}" stroke-opacity="{0.25+0.6*wgt:.2f}"/>')
for n, (nx, ny) in pos.items():
    wdt, hgt = 104, 30
    p.append(f'<rect x="{nx-wdt/2:.0f}" y="{ny-hgt/2:.0f}" width="{wdt}" height="{hgt}" fill="{PAN}" stroke="{T1}" stroke-opacity=".25"/>')
    p.append(f'<circle cx="{nx-wdt/2+12:.0f}" cy="{ny:.0f}" r="4" fill="{colof[n]}"/>')
    p.append(f'<text x="{nx-wdt/2+24:.0f}" y="{ny+4:.0f}" font-size="11" fill="{T2}">{n}</text>')
p.append(caption(x, y, "the signature sparq image · live rms on every edge · lod 2 collapses to colour-coded dots"))

# ---------------------------------------------------------- 9 · dsp/dna-view
x, y, px, py, pw, ph, c = tile(8, "dsp/dna-view", "mutation lineage · depth → greyscale · seed a3f19c02")
p.append(c + axes(px, py, pw, ph, "generation →", "branch", 6, 4))
levels = [[0.5]]
for _ in range(1, 7):
    prev, cur = levels[-1], []
    for pv in prev:
        cur.append(pv)
        if random.random() < 0.55:
            cur.append(max(0.08, min(0.94, pv + random.uniform(-0.16, 0.16))))
    levels.append(sorted(cur))
kept = {(3, 1), (5, 2)}
ops = ["rotate", "markov-2", "l-system", "rule110", "lorenz", "spectral-scramble", "mirror+dilate"]
for g, vals in enumerate(levels):
    col = grey_at(0.18 + 0.72 * (g / 6))          # depth encoded via the greyscale colour map
    for i, v in enumerate(vals):
        xx = px + 18 + g * (pw - 36) / 6
        yy = py + v * ph
        if g > 0:
            pv = min(levels[g - 1], key=lambda t: abs(t - v))
            pxx, pyy = px + 18 + (g - 1) * (pw - 36) / 6, py + pv * ph
            p.append(f'<line x1="{pxx:.0f}" y1="{pyy:.0f}" x2="{xx:.0f}" y2="{yy:.0f}" stroke="{col}" stroke-width="1" stroke-opacity=".7"/>')
        is_kept = (g, i) in kept
        r = 6 if is_kept else 4
        p.append(f'<circle cx="{xx:.0f}" cy="{yy:.0f}" r="{r}" fill="{INS}" stroke="{A if is_kept else col}" stroke-width="{2 if is_kept else 1}"/>')
        if is_kept:
            p.append(f'<text x="{xx+10:.0f}" y="{yy+4:.0f}" font-size="10" fill="{A}">kept · op {ops[(g+i)%len(ops)]} · seed 0x{random.randrange(16**8):08x}</text>')
p.append(caption(x, y, "navigate by touch · audition a branch without committing · export a lineage as a generative score"))

# ---------------------------------------------------------- 10 · dsp/streams
x, y, px, py, pw, ph, c = tile(9, "dsp/streams", "every stream · rate, jitter, staleness · data class")
p.append(c)
streams = [("imu9/wrist", "sparq/imu9@1", "200 Hz", "±0.4 ms", "ok", D),
           ("ble/hr", "sparq/timeseries@1", "1.1 Hz", "±120 ms", "stale", WARN),
           ("cam0/flow", "sparq/optical-flow@1", "30 Hz", "±2.1 ms", "ok", D),
           ("net/seismic", "sparq/timeseries@1", "0.05 Hz", "±40 ms", "ok", D),
           ("midi2/kb", "ump · mpe", "event", "—", "ok", E),
           ("osc/live", "1.1 /udp/7000", "bursty", "±1.2 ms", "ok", E),
           ("int/cpu", "sparq/telemetry@1", "10 Hz", "±0.1 ms", "ok", T3)]
rh = (ph - 16) / len(streams)
for i, (nm, sch, rate, jit, st, col) in enumerate(streams):
    yy = py + 8 + i * rh
    p.append(f'<text x="{px+8}" y="{yy+16:.0f}" font-size="11" fill="{T1}">{nm}</text>')
    p.append(f'<text x="{px+8}" y="{yy+30:.0f}" font-size="10" fill="{T4}">{sch}</text>')
    p.append(f'<text x="{px+230}" y="{yy+16:.0f}" font-size="11" fill="{T2}">{rate}</text>')
    p.append(f'<text x="{px+230}" y="{yy+30:.0f}" font-size="10" fill="{T4}">jitter {jit}</text>')
    p.append(f'<text x="{px+348}" y="{yy+16:.0f}" font-size="11" fill="{col}">{st}</text>')
    spark = []
    for k in range(50):
        dv = random.uniform(-9, 9) if st == "ok" else random.uniform(-1.5, 1.5)
        spark.append(f"{px+430+k*3.2:.1f},{yy+24+dv:.1f}")
    p.append(f'<polyline points="{" ".join(spark)}" fill="none" stroke="{col}" stroke-width="1" stroke-opacity=".85"/>')
    p.append(f'<line x1="{px}" y1="{yy+rh-4:.0f}" x2="{px+pw}" y2="{yy+rh-4:.0f}" stroke="{T1}" stroke-opacity=".10"/>')
p.append(caption(x, y, "stale is a first-class state with an explicit hold/decay policy — never silent garbage (ADR-006)"))

# ---------------------------------------------------------- 11 · dsp/meters
x, y, px, py, pw, ph, c = tile(10, "dsp/meters", "8 ch · rms + peak + dBTP · LUFS-M/S/I")
p.append(c)
n = 8
bw = (pw - 40) / n
hh = ph - 56
for i in range(n):
    bx = px + 20 + i * bw
    lv = random.uniform(0.35, 0.92)
    rms = lv * random.uniform(0.55, 0.80)
    pk = min(1.0, lv + random.uniform(0, 0.08))
    p.append(f'<rect x="{bx:.0f}" y="{py+8}" width="{bw-10:.0f}" height="{hh:.0f}" fill="{INS}" stroke="{T1}" stroke-opacity=".10"/>')
    p.append(f'<rect x="{bx:.0f}" y="{py+8+hh*(1-rms):.0f}" width="{bw-10:.0f}" height="{hh*rms:.0f}" fill="{D}" fill-opacity=".35"/>')
    p.append(f'<rect x="{bx:.0f}" y="{py+8+hh*(1-lv):.0f}" width="{bw-10:.0f}" height="{hh*lv:.0f}" fill="{D}" fill-opacity=".18"/>')
    p.append(f'<line x1="{bx-2:.0f}" y1="{py+8+hh*(1-pk):.0f}" x2="{bx+bw-8:.0f}" y2="{py+8+hh*(1-pk):.0f}" stroke="{T1}" stroke-width="2"/>')
    for db in (0, -6, -12, -24, -48, -60):
        yy = py + 8 + hh * (1 - (db + 60) / 60)
        p.append(f'<line x1="{bx+bw-16:.0f}" y1="{yy:.0f}" x2="{bx+bw-10:.0f}" y2="{yy:.0f}" stroke="{T1}" stroke-opacity=".25"/>')
        p.append(f'<text x="{bx+bw-20:.0f}" y="{yy+3:.0f}" font-size="9" fill="{T4}" text-anchor="end">{db}</text>')
    p.append(f'<text x="{bx+(bw-10)/2:.0f}" y="{py+ph-14:.0f}" font-size="11" fill="{T3}" text-anchor="middle">{i+1}</text>')
ceil_y = py + 8 + hh * (1 - (-1 + 60) / 60)
p.append(f'<line x1="{px+20}" y1="{ceil_y:.0f}" x2="{px+pw-20}" y2="{ceil_y:.0f}" stroke="{E}" stroke-width="1" stroke-dasharray="4 3"/>')
p.append(f'<text x="{px+pw-20}" y="{ceil_y-6:.0f}" font-size="10" fill="{E}" text-anchor="end">−1.0 dBTP ceiling</text>')
p.append(caption(x, y, "−7.2 LUFS-M · −8.9 LUFS-I · LRA 6.1 · corr 0.86 · targets club −9…−6 / streaming −14"))

# ---------------------------------------------------------- 12 · dsp/matrix
x, y, px, py, pw, ph, c = tile(11, "dsp/matrix", "util/mixer 8×8 · per-cell gain · spatial routing")
p.append(c)
n = 8
cw, chh = (pw - 64) / n, (ph - 60) / n
for i in range(n):
    for j in range(n):
        if i == j:
            v = random.uniform(0.70, 1.00)
        elif abs(i - j) == 1:
            v = random.uniform(0.15, 0.40)
        else:
            v = random.uniform(0.05, 0.30) if random.random() < 0.12 else 0.0
        bx, by = px + 32 + j * cw, py + 14 + i * chh
        if v > 0.02:
            p.append(f'<rect x="{bx:.0f}" y="{by:.0f}" width="{cw-3:.0f}" height="{chh-3:.0f}" fill="{lut_at(v)}" fill-opacity="{0.25+0.7*v:.2f}"/>')
        else:
            p.append(f'<rect x="{bx:.0f}" y="{by:.0f}" width="{cw-3:.0f}" height="{chh-3:.0f}" fill="none" stroke="{T1}" stroke-opacity=".10"/>')
    p.append(f'<text x="{px+12}" y="{py+14+i*chh+chh/2+4:.0f}" font-size="10" fill="{T3}">{i+1}</text>')
for j in range(n):
    p.append(f'<text x="{px+32+j*cw+cw/2:.0f}" y="{py+ph-20:.0f}" font-size="10" fill="{T3}" text-anchor="middle">{j+1}</text>')
p.append(f'<text x="{px+32}" y="{py+ph-4:.0f}" font-size="10" fill="{T4}">out ch →</text>')
p.append(caption(x, y, "touch a cell to edit gain · drag a row to re-pan · this is also the mapping surface (plan §7.4)"))

# ---------------------------------------------------------------- assemble
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="ui-monospace, 'Cascadia Mono', 'SF Mono', Menlo, Consolas, monospace">
<defs><filter id="g" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3.5" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
<rect width="{W}" height="{H}" fill="{GND}"/>
<text x="{MARGIN}" y="44" font-size="18" fill="{T1}">sparq · display module sheet</text>
<text x="{MARGIN}" y="70" font-size="11" fill="{T3}">WO-004 mockup 3 of 3 · 12 of 14 display modules at the large breakpoint (2560 px) · generated by tools/make_display_sheet.py from design/tokens</text>
<text x="{W-MARGIN}" y="44" font-size="11" fill="{T3}" text-anchor="end">colormaps: phosphor (sequential) · bipolar (diverging) · thermal (rig heat) · greyscale (dna depth)</text>
<text x="{W-MARGIN}" y="70" font-size="11" fill="{T4}" text-anchor="end">tablet 1280×800 and wall 3840×2160: same objects re-flowed — protocol in mockup-review.md</text>
{"".join(p)}
<text x="{MARGIN}" y="{H-28}" font-size="10" fill="{T4}">not shown: dsp/numeric (see perform-mode.svg hero readouts) · dsp/particles · dsp/feedback · every display must pass the monochrome test (look-board §10)</text>
</svg>
'''
out = ROOT / "design/mockups/display-sheet.svg"
write_text(out, svg)
ET.parse(out)
print(f"wrote {out.relative_to(ROOT)} · {len(svg):,} bytes · {H} px tall · {len(p):,} elements")
