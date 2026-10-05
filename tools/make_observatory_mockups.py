#!/usr/bin/env python3
"""WO-020 mockup sheet — `dat/observatory` · THE OBSERVATORY (the first instrument).

Generates the four plan-review mockups from design/tokens (via the baked LUTs that
tools/token_gen.py emits) — the make_display_sheet.py discipline: every colour, stroke and
metric comes from the TOKEN FILES, never from this script's memory.

    design/mockups/observatory-wall.svg    mockup 1 of 4 — the wall: 4x4, 16 live cells,
                                           design-mode shell at the 2560x1600 reference
    design/mockups/observatory-full.svg    mockup 2 of 4 — SOLO+FULL: swpc.aurora takes the wall
    design/mockups/observatory-picker.svg  mockup 3 of 4 — the toolbar + STREAM dropdown
                                           (round 4's OWED item 11), 2x2 layout
    design/mockups/observatory-canvas.svg  mockup 4 of 4 — the wall in a patch: scale against
                                           the backbone set at zoom 45 %

Regenerate after any token change; PNG siblings are rendered with rsvg-convert when present
(they are review artefacts, the SVG is the source). The map cells need the Natural Earth 110m
land/coastline GeoJSON cached in tools/data/ — fetch recipe (public domain data):

    python3 - <<'PY'
    import urllib.request, pathlib
    for n in ("ne_110m_land", "ne_110m_coastline"):
        u = f"https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/{n}.geojson"
        req = urllib.request.Request(u, headers={"User-Agent": "sparq-mockups/0.1"})
        pathlib.Path(f"tools/data/{n}.geojson").write_bytes(urllib.request.urlopen(req, timeout=60).read())
    PY

The stream DATA in the cells is illustrative and seeded (deterministic, like every house
generator); the endpoints and payload shapes behind it were probe-verified live on
2026-10-06 — WO020-OBSERVATORY-PLAN.md §3 is the probe log.
"""
from __future__ import annotations

import json
import math
import pathlib
import random
import subprocess
import sys
import tomllib
import xml.sax.saxutils

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Every text read/write goes through these helpers so the tool behaves identically on Windows,
# where the default encoding is the ANSI code page and text writes translate "\n" to "\r\n"
# (tools/README.md -> "Text I/O portability"; the `tools-text-io` gate enforces it).
ENCODING = "utf-8"

# Windows consoles and redirected logs default to the ANSI code page: printing the UTF-8
# characters this project uses (·, —, →, °, µ) raises UnicodeEncodeError mid-report (defect #37).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass


def read_text(path) -> str:
    return pathlib.Path(path).read_text(encoding=ENCODING)


def write_text(path, content: str) -> None:
    pp = pathlib.Path(path)
    pp.parent.mkdir(parents=True, exist_ok=True)
    with pp.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


# ── tokens: read from the files, never remembered ────────────────────────────────────────────
_C = tomllib.loads(read_text(ROOT / "design/tokens/colors.toml"))
_L = tomllib.loads(read_text(ROOT / "design/tokens/layout.toml"))
_MAPS = json.loads(read_text(ROOT / "design/tokens/generated/colormaps.json"))["maps"]

GROUND = _C["ground"]
HAIR = _C["hairline"]
TEXT = _C["text"]
SIG = _C["signal"]
STATE = _C["state"]
METER = _C["meter"]
SP = _L["space"]
SH = _L["shell"]
CV = _L["canvas"]

HAIRC = HAIR["colour"]


def _rgb(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgba(h: str, a: float) -> str:
    r, g, b = _rgb(h)
    return f"rgba({r},{g},{b},{a:.3f})"


def cmap(name: str, t: float) -> str:
    """Sample a baked colormap LUT (colormaps.json — the generated artefact)."""
    lut = _MAPS[name]
    t = min(1.0, max(0.0, t))
    x = t * (len(lut) - 1)
    i = int(x)
    j = min(i + 1, len(lut) - 1)
    f = x - i
    a, b = _rgb(lut[i]), _rgb(lut[j])
    return "#%02X%02X%02X" % tuple(round(a[k] + (b[k] - a[k]) * f) for k in range(3))


DATA_G = SIG["data"]["colour"]
DATA_DIM = SIG["data"]["dim"]
DATA_GLOW = SIG["data"]["glow"]
AUDIO = SIG["audio"]["colour"]
CV_C = SIG["cv"]["colour"]
EVENT = SIG["event"]["colour"]
SPATIAL = SIG["spatial"]["colour"]

FONT = "ui-monospace, 'Cascadia Mono', 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace"

# ── geography (Natural Earth 110m, public domain, cached in tools/data) ─────────────────────
_LAND = json.loads(read_text(ROOT / "tools/data/ne_110m_land.geojson"))
_COAST = json.loads(read_text(ROOT / "tools/data/ne_110m_coastline.geojson"))


def _rings(gj):
    out = []
    for f in gj["features"]:
        g = f["geometry"]
        if g is None:
            continue
        if g["type"] == "Polygon":
            out.append(g["coordinates"][0])
        elif g["type"] == "MultiPolygon":
            out.extend(p[0] for p in g["coordinates"])
        elif g["type"] == "LineString":
            out.append(g["coordinates"])
        elif g["type"] == "MultiLineString":
            out.extend(g["coordinates"])
    return out


_LAND_RINGS = _rings(_LAND)
_COAST_RINGS = _rings(_COAST)


def eq(lat, lon, x, y, w, h):
    """Equirectangular projection into rect (x, y, w, h)."""
    return x + (lon + 180.0) / 360.0 * w, y + (90.0 - lat) / 180.0 * h


def rings_path(rings, x, y, w, h, step=2):
    parts = []
    for ring in rings:
        pts = ring[::step] if len(ring) > 2 * step else ring
        if len(pts) < 2:
            continue
        seg = []
        for lon, lat in pts:
            px, py = eq(lat, lon, x, y, w, h)
            seg.append(f"{px:.1f},{py:.1f}")
        parts.append("M" + "L".join(seg))
    return "".join(parts)


# ── svg primitives ───────────────────────────────────────────────────────────────────────────
def esc(s):
    return xml.sax.saxutils.escape(str(s))


class Svg:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.defs = []
        self.body = []
        self._clip = 0

    def add(self, s):
        self.body.append(s)

    def clip(self, x, y, w, h):
        self._clip += 1
        cid = f"c{self._clip}"
        self.defs.append(f'<clipPath id="{cid}"><rect x="{x}" y="{y}" width="{w}" height="{h}"/></clipPath>')
        return cid

    def g(self, clip=None, op=None, tr=None):
        a = []
        if clip:
            a.append(f'clip-path="url(#{clip})"')
        if op is not None:
            a.append(f'opacity="{op}"')
        if tr:
            a.append(f'transform="{tr}"')
        return f'<g {" ".join(a)}>'

    def text(self, x, y, s, size=11, fill=None, anchor=None, op=None, glow=False, weight=None, ls=None):
        a = [f'x="{x:.1f}"', f'y="{y:.1f}"', f'font-size="{size}"']
        a.append(f'fill="{fill or TEXT["primary"]}"')
        if anchor:
            a.append(f'text-anchor="{anchor}"')
        if op is not None:
            a.append(f'opacity="{op}"')
        if glow:
            a.append('filter="url(#g)"')
        if weight:
            a.append(f'font-weight="{weight}"')
        if ls:
            a.append(f'letter-spacing="{ls}"')
        return f'<text {" ".join(a)}>{esc(s)}</text>'

    def out(self):
        glow = ('<filter id="g" x="-80%" y="-80%" width="260%" height="260%">'
                '<feGaussianBlur stdDeviation="3.5" result="b"/>'
                '<feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>')
        glow2 = ('<filter id="g2" x="-80%" y="-80%" width="260%" height="260%">'
                 '<feGaussianBlur stdDeviation="1.6" result="b"/>'
                 '<feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>')
        head = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {self.w} {self.h}" '
                f'width="{self.w}" height="{self.h}" font-family="{FONT}">')
        return head + "<defs>" + glow + glow2 + "".join(self.defs) + "</defs>" + "".join(self.body) + "</svg>"


def rect(s, x, y, w, h, fill=None, stroke=None, sw=1, op=None, so=None, r=0, dash=None):
    a = [f'x="{x:.1f}"', f'y="{y:.1f}"', f'width="{max(0, w):.1f}"', f'height="{max(0, h):.1f}"']
    if r:
        a.append(f'rx="{r}"')
    a.append(f'fill="{fill}"' if fill else 'fill="none"')
    if stroke:
        a.append(f'stroke="{stroke}" stroke-width="{sw}"')
        if so is not None:
            a.append(f'stroke-opacity="{so}"')
        if dash:
            a.append(f'stroke-dasharray="{dash}"')
    if op is not None:
        a.append(f'opacity="{op}"')
    s.add(f'<rect {" ".join(a)}/>')


def line(s, x1, y1, x2, y2, stroke, sw=1, op=None, dash=None):
    a = [f'x1="{x1:.1f}"', f'y1="{y1:.1f}"', f'x2="{x2:.1f}"', f'y2="{y2:.1f}"',
         f'stroke="{stroke}"', f'stroke-width="{sw}"']
    if op is not None:
        a.append(f'stroke-opacity="{op}"')
    if dash:
        a.append(f'stroke-dasharray="{dash}"')
    s.add(f'<line {" ".join(a)}/>')


def poly(s, pts, stroke, sw=1.5, op=None, dash=None, fill=None, glow=False):
    d = "M" + "L".join(f"{x:.1f},{y:.1f}" for x, y in pts)
    a = [f'd="{d}"']
    a.append(f'fill="{fill}"' if fill else 'fill="none"')
    if stroke:
        a.append(f'stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"')
        if op is not None:
            a.append(f'stroke-opacity="{op}"')
        if dash:
            a.append(f'stroke-dasharray="{dash}"')
    if glow:
        a.append('filter="url(#g2)"')
    s.add(f'<path {" ".join(a)}/>')


def path_d(s, d, stroke, sw=1, op=None, dash=None, fill=None):
    a = [f'd="{d}"', f'fill="{fill}"' if fill else 'fill="none"']
    if stroke:
        a.append(f'stroke="{stroke}" stroke-width="{sw}"')
        if op is not None:
            a.append(f'stroke-opacity="{op}"')
        if dash:
            a.append(f'stroke-dasharray="{dash}"')
    s.add(f'<path {" ".join(a)}/>')


def circle(s, cx, cy, r, fill=None, stroke=None, sw=1, op=None, glow=False, dash=None, so=None):
    a = [f'cx="{cx:.1f}"', f'cy="{cy:.1f}"', f'r="{r:.1f}"']
    a.append(f'fill="{fill}"' if fill else 'fill="none"')
    if op is not None and fill:
        a.append(f'fill-opacity="{op}"')
    if stroke:
        a.append(f'stroke="{stroke}" stroke-width="{sw}"')
        if so is not None:
            a.append(f'stroke-opacity="{so}"')
        elif op is not None and not fill:
            a.append(f'stroke-opacity="{op}"')
        if dash:
            a.append(f'stroke-dasharray="{dash}"')
    if glow:
        a.append('filter="url(#g2)"')
    s.add(f'<circle {" ".join(a)}/>')


def corner_ticks(s, x, y, w, h, arm=14, col=None, sw=1):
    """state.selected — white corner ticks, the one pure-white exemption (operator ruling 2026-10-01)."""
    col = col or STATE["selected"]["colour"]
    for cx, cy, dx, dy in ((x, y, 1, 1), (x + w, y, -1, 1), (x, y + h, 1, -1), (x + w, y + h, -1, -1)):
        line(s, cx, cy, cx + arm * dx, cy, col, sw)
        line(s, cx, cy, cx, cy + arm * dy, col, sw)


# ── map base ─────────────────────────────────────────────────────────────────────────────────
def map_base(s, x, y, w, h, graticule=True, land_op=0.13, coast_op=0.30):
    rect(s, x, y, w, h, fill=GROUND["inset"])
    if graticule:
        for lon in range(-180, 181, 30):
            px, _ = eq(0, lon, x, y, w, h)
            line(s, px, y, px, y + h, HAIRC, 1, 0.07 if lon else 0.14)
        for lat in range(-60, 61, 30):
            _, py = eq(lat, 0, x, y, w, h)
            line(s, x, py, x + w, py, HAIRC, 1, 0.07 if lat else 0.14)
    path_d(s, rings_path(_LAND_RINGS, x, y, w, h), None, fill=HAIRC, op=None)
    s.body[-1] = s.body[-1].replace("<path ", f'<path fill-opacity="{land_op}" ', 1)
    path_d(s, rings_path(_COAST_RINGS, x, y, w, h, 2), HAIRC, 1, coast_op)


def map_dots(s, events, x, y, w, h, size_fn, color_fn, op_fn=None, glow_big=False):
    for ev in events:
        px, py = eq(ev["lat"], ev["lon"], x, y, w, h)
        if not (x - 4 <= px <= x + w + 4 and y - 4 <= py <= y + h + 4):
            continue
        r = size_fn(ev)
        c = color_fn(ev)
        o = op_fn(ev) if op_fn else 1.0
        circle(s, px, py, r, fill=c, op=o, glow=(glow_big and r > 2.6))


# ── seeded stream data (illustrative; shapes probe-verified 2026-10-06, plan §3) ────────────
def gen_quakes(seed=101, n=42):
    rng = random.Random(seed)
    belts = [
        ((30, 46), (128, 146), 3), ((5, 20), (118, 128), 2), ((-11, 6), (95, 165), 4),
        ((-36, -14), (-179, -168), 2), ((50, 55), (-180, -148), 2), ((38, 61), (-152, -126), 2),
        ((7, 20), (-106, -83), 3), ((-36, 4), (-80, -69), 4), ((-60, 62), (-46, -14), 1),
        ((34, 41), (-6, 46), 2), ((27, 39), (44, 96), 3), ((-11, 9), (27, 41), 1),
        ((60, 68), (-24, -13), 1),
    ]
    ev = []
    total = sum(b[2] for b in belts)
    for _ in range(n):
        r = rng.uniform(0, total)
        acc = 0
        for la, lo, wt in belts:
            acc += wt
            if r <= acc:
                break
        lat = rng.uniform(*la)
        lon = rng.uniform(*lo)
        mag = round(rng.choice([rng.uniform(2.5, 4.2), rng.uniform(2.5, 4.2), rng.uniform(4.2, 5.4),
                                rng.uniform(5.4, 6.6)]), 1)
        depth = rng.choice([rng.uniform(4, 35), rng.uniform(35, 120), rng.uniform(120, 560)])
        age = rng.uniform(0, 3600)
        ev.append({"lat": lat, "lon": lon, "mag": mag, "depth": depth, "age": age})
    ev.append({"lat": 36.9, "lon": -120.7, "mag": 4.3, "depth": 8.2, "age": 47})
    return ev


def gen_eonet(seed=202, n=30):
    rng = random.Random(seed)
    cats = ["wildfires", "volcanoes", "severeStorms", "icebergs", "dustHaze"]
    regions = {
        "wildfires": [((44, 58), (-126, -104)), ((-14, -2), (-62, -44)), ((-8, 6), (14, 32)), ((48, 62), (85, 125))],
        "volcanoes": [((-9, 6), (98, 130)), ((30, 36), (138, 141)), ((62, 65), (-20, -16)), ((12, 16), (-92, -86))],
        "severeStorms": [((24, 34), (-96, -78)), ((4, 18), (96, 110)), ((50, 58), (-2, 8))],
        "icebergs": [((-70, -62), (-60, 40))],
        "dustHaze": [((14, 26), (-14, 24))],
    }
    ev = []
    for i in range(n):
        c = cats[min(i % 5, 4)] if i < 5 else rng.choice(cats)
        (la, lo) = rng.choice(regions[c])
        ev.append({"cat": c, "lat": rng.uniform(*la), "lon": rng.uniform(*lo), "age": rng.uniform(0, 72)})
    return ev


def gen_gdacs(seed=303, n=13):
    rng = random.Random(seed)
    ev = [
        {"sev": "Orange", "kind": "EQ", "lat": -6.2, "lon": 150.2, "mag": 5.8, "place": "Papua New Guinea"},
        {"sev": "Green", "kind": "EQ", "lat": 38.1, "lon": 141.7, "mag": 4.9, "place": "near East coast of Honshu"},
        {"sev": "Green", "kind": "FL", "lat": 9.1, "lon": 7.4, "mag": 0, "place": "Niger basin flooding"},
        {"sev": "Orange", "kind": "TC", "lat": 16.4, "lon": 126.8, "mag": 2, "place": "NW Pacific — CAT 2"},
        {"sev": "Green", "kind": "EQ", "lat": -21.4, "lon": -68.9, "mag": 4.6, "place": "Chile-Bolivia border"},
    ]
    spots = [((-8, 8), (118, 134)), ((34, 40), (44, 60)), ((-35, -20), (-72, -64)), ((8, 20), (92, 100))]
    while len(ev) < n:
        (la, lo) = rng.choice(spots)
        ev.append({"sev": "Green", "kind": rng.choice(["EQ", "FL"]),
                   "lat": rng.uniform(*la), "lon": rng.uniform(*lo),
                   "mag": round(rng.uniform(3.8, 5.0), 1), "place": ""})
    return ev


def gen_series(seed, n, lo, hi, walk, spike=None):
    rng = random.Random(seed)
    v = rng.uniform(lo, hi)
    out = []
    for i in range(n):
        v += rng.gauss(0, walk)
        v = max(lo - (hi - lo) * 0.35, min(hi + (hi - lo) * 0.35, v))
        if spike and spike[0] <= i < spike[1]:
            v = spike[2](v, (i - spike[0]) / (spike[1] - spike[0]))
        out.append(v)
    return out


def gen_kp(seed=404, n=17):
    rng = random.Random(seed)
    vals = [round(rng.choice([1, 1, 2, 2, 2, 3, 3, 4]) + rng.choice([0, 0, 0.33, 0.67]), 2) for _ in range(n - 2)]
    return vals + [4.67, 4.33]


def gen_flares(seed=505, n=21):
    rng = random.Random(seed)
    out = []
    for i in range(n):
        cls = rng.choices(["B", "C", "M"], weights=[13, 6, 1.4])[0]
        num = round(rng.uniform(1, 9.4), 1)
        t = rng.uniform(0, 7 * 24)
        out.append({"cls": cls, "num": num, "t": t})
    out.append({"cls": "M", "num": 1.2, "t": 52.5})
    return out


def gen_neo(seed=606, n=11):
    rng = random.Random(seed)
    out = []
    for i in range(n):
        ld = 10 ** rng.uniform(0.4, 1.7)
        out.append({"t": rng.uniform(0, 7), "ld": ld, "size": round(rng.uniform(12, 340)),
                    "haz": rng.random() < 0.18})
    out.append({"t": 2.6, "ld": 4.7, "size": 141, "haz": True})
    return out


def gen_aurora(kp=4.7):
    """OVATION-style 2° grid: [lon, lat, intensity 0..~22] — the broker-regridded shape (§6.4)."""
    north = (80.9, -71.9)   # geomagnetic north pole (approx, 2026)
    south = (-64.1, 135.9)
    subsolar_lon = -144.8   # 21:38 UTC probe time
    night = subsolar_lon + 180.0
    rng = random.Random(707)
    cells = []
    for lat_i in range(-90, 90, 2):
        lat = lat_i + 1.0
        for lon_i in range(-180, 180, 2):
            lon = lon_i + 1.0
            v = 0.0
            for (plat, plon), hemi in ((north, 1.0), (south, 0.55)):
                d = math.degrees(math.acos(max(-1, min(1,
                    math.sin(math.radians(lat)) * math.sin(math.radians(plat)) +
                    math.cos(math.radians(lat)) * math.cos(math.radians(plat)) *
                    math.cos(math.radians(lon - plon))))))
                maglat = 90.0 - d
                nf = max(0.0, math.cos(math.radians(lon - night)))
                band = 67.0 - 4.5 * nf - 0.7 * kp
                width = 3.0 + 2.0 * nf
                amp = (2.7 + 1.0 * kp) * (1.0 + 0.55 * nf) * hemi
                v += amp * math.exp(-((maglat - band) / width) ** 2)
            v *= 0.72 + 0.5 * rng.random()
            if v > 0.45:
                cells.append((lon_i, lat_i, min(22.0, v)))
    return cells


WWV = [
    ":Product: Geophysical Alert Message wwv.txt",
    ":Issued: 2026 Oct 06 2105 UTC",
    "# Prepared by the US Dept. of Commerce, NOAA,",
    "# Space Weather Prediction Center",
    "#",
    "#          Geophysical Alert Message",
    "#",
    "Solar-terrestrial indices for 06 October follow.",
    "Solar flux 132 and estimated planetary A-index 12.",
    "The estimated planetary K-index at 2100 UTC was 5.",
    "No M-class or greater solar flares were observed",
    "in the past 24 hours. The largest observed was",
    "a C2.1 at 1419Z, from NOAA AR 4238.",
    "The largest coronal mass ejection (CME) observed",
    "in association with an eruption on 04 OCT at 0847Z.",
    "Geomagnetic field: UNSETTLED to ACTIVE, with",
    "isolated MINOR STORM (G1) periods possible late",
    "on 07 OCT (CME glancing blow, ±6 h arrival window).",
]

FORECAST = [
    ":Product: 3-Day Geomagnetic Forecast",
    ":Issued: 2026 Oct 06 1200 UTC",
    "",
    "NOAA/SWPC 3-Day Geomagnetic Forecast",
    "",
    "Oct 06 12/18UTC  Oct 06 18/00UTC  Oct 07 00/06UTC",
    "NOAA G-scale:      1(R2)          2(R3)          2(R3)",
    "Probabilities(%):  M 05 10 01     M 10 15 02     M 15 20 05",
    "",
    "Oct 07 06/12UTC  Oct 07 12/18UTC  Oct 08 00/06UTC",
    "NOAA G-scale:      1(R2)          1(R1)          1(R1)",
    "Probabilities(%):  M 10 15 02     M 05 10 01     M 05 05 01",
    "",
    "CME arrival window 07/0942Z ±6 h — glancing blow,",
    "Kp 5- (G1 MINOR) likely for 2-3 intervals.",
]


# ── the seven renderers (§5.5) ──────────────────────────────────────────────────────────────
def rnd_map_points(s, bx, by, bw, bh, events, kind, seed):
    map_base(s, bx, by, bw, bh)
    if kind == "quakes":
        def size(e):
            return 1.0 + e["mag"] * 0.52
        def colr(e):
            return cmap("inferno_class", min(1.0, e["depth"] / 620.0))
        def op(e):
            return 0.45 + 0.55 * (1.0 - e["age"] / 3600.0)
        map_dots(s, events, bx, by, bw, bh, size, colr, op, glow_big=True)
        newest = min(events, key=lambda e: e["age"])
        px, py = eq(newest["lat"], newest["lon"], bx, by, bw, bh)
        circle(s, px, py, 9, stroke=TEXT["primary"], sw=1, op=0.8)
        s.add(s.text(px + 13, py + 3.5, f"M {newest['mag']:.1f}", 10.5, TEXT["primary"], glow=True))
        # legend: magnitude size ramp
        lx, ly = bx + 10, by + bh - 12
        for i, m in enumerate((2.5, 4.0, 5.5, 7.0)):
            circle(s, lx + i * 42 + 6, ly, 1.0 + m * 0.52, fill=None, stroke=TEXT["tertiary"], sw=1, op=0.7)
            s.add(s.text(lx + i * 42 + 16, ly + 3.4, f"M{m:.1f}", 9, TEXT["tertiary"]))
    elif kind == "eonet":
        cats = ["wildfires", "volcanoes", "severeStorms", "icebergs", "dustHaze"]
        lut = _MAPS["categorical_6"]
        counts = {c: 0 for c in cats}
        for e in events:
            counts[e["cat"]] += 1
        def colr(e):
            return lut[cats.index(e["cat"]) % len(lut)]
        map_dots(s, events, bx, by, bw, bh, lambda e: 2.6, colr,
                 lambda e: 0.5 + 0.5 * (1 - min(1.0, e["age"] / 72)), glow_big=True)
        lx, ly = bx + 10, by + 14
        for i, c in enumerate(cats[:4]):
            circle(s, lx + 5, ly + i * 13, 2.6, fill=lut[i % len(lut)])
            s.add(s.text(lx + 13, ly + i * 13 + 3.4, f"{c} {counts[c]}", 9.5, TEXT["tertiary"]))
    elif kind == "gdacs":
        sevcol = {"Green": METER["level_lo"], "Orange": METER["level_mid"], "Red": STATE["error"]["colour"]}
        for e in events:
            px, py = eq(e["lat"], e["lon"], bx, by, bw, bh)
            r = 3.4 if e["sev"] == "Green" else 4.6
            circle(s, px, py, r, fill=sevcol[e["sev"]], glow=(e["sev"] != "Green"))
            if e["kind"] != "EQ":
                s.add(s.text(px + 6, py + 3, e["kind"], 8.5, sevcol[e["sev"]]))
        big = [e for e in events if e["sev"] != "Green" and e["place"]]
        if big:
            e = big[0]
            px, py = eq(e["lat"], e["lon"], bx, by, bw, bh)
            s.add(s.text(min(px + 9, bx + bw - 130), py - 6, f"{e['kind']} {e['sev']} · {e['place']}", 9.5,
                         TEXT["secondary"]))
        lx, ly = bx + 10, by + bh - 12
        for i, (k, c) in enumerate((("Green", METER["level_lo"]), ("Orange", METER["level_mid"]),
                                    ("Red", STATE["error"]["colour"]))):
            circle(s, lx + i * 64 + 4, ly, 3.0, fill=c)
            s.add(s.text(lx + i * 64 + 11, ly + 3.4, k.upper(), 9, TEXT["tertiary"]))
    elif kind == "iss":
        # ground track: 51.6° inclination, drifting; footprint + live marker (probe values)
        lat0, lon0, alt, vel = 34.59, -82.70, 423.0, 27583.0
        th0 = math.degrees(math.asin(max(-1.0, min(1.0, lat0 / 51.6))))
        tr = []
        for k in range(-190, 71, 3):
            th = th0 + k
            la = 51.6 * math.sin(math.radians(th))
            lo = ((lon0 - 1.064 * k + 180) % 360) - 180
            tr.append((la, lo))
        seg = []
        for la, lo in tr:
            if seg and abs(lo - seg[-1][1]) > 180:
                poly(s, [eq(a, b, bx, by, bw, bh) for a, b in seg], DATA_G, 1.2, 0.75, dash="5 4")
                seg = []
            seg.append((la, lo))
        if len(seg) > 1:
            poly(s, [eq(a, b, bx, by, bw, bh) for a, b in seg], DATA_G, 1.2, 0.75, dash="5 4")
        px, py = eq(lat0, lon0, bx, by, bw, bh)
        fp_deg = 4523.0 / 2.0 / 111.32
        circle(s, px, py, fp_deg / 180.0 * bh, fill=DATA_G, op=0.06, stroke=DATA_G, sw=1, so=0.3)
        circle(s, px, py, 4.2, fill=DATA_G, glow=True)
        line(s, px - 9, py, px + 9, py, DATA_G, 1, 0.9)
        line(s, px, py - 9, px, py + 9, DATA_G, 1, 0.9)
        s.add(s.text(px + 12, py - 7, "ISS · 423 km", 10, DATA_GLOW, glow=True))
        s.add(s.text(px + 12, py + 5, f"{vel:,.0f} km/h · daylight".replace(",", " "), 9, TEXT["secondary"]))
    elif kind == "neo":
        pass


def rnd_map_heat(s, bx, by, bw, bh, cells, unit="10⁷ erg/cm²/s? "):
    map_base(s, bx, by, bw, bh, land_op=0.10, coast_op=0.22)
    cw = bw / 180.0
    ch = bh / 90.0
    # blurred under-glow pass, then the crisp cells (host does phosphor; mockup approximates)
    s.add(s.g(clip=None, op=0.5))
    g = []
    for lon, lat, v in cells:
        x = bx + (lon + 180) / 360.0 * bw
        y = by + (90 - (lat + 2)) / 180.0 * bh
        g.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{cw * 2.1:.1f}" height="{ch * 2.1:.1f}" '
                 f'fill="{cmap("thermal", v / 22.0)}" opacity="{min(1.0, 0.25 + v / 26.0):.2f}"/>')
    s.add('<g filter="url(#g)" opacity="0.55">' + "".join(g) + "</g>")
    s.add("</g>")
    for lon, lat, v in cells:
        x = bx + (lon + 180) / 360.0 * bw
        y = by + (90 - (lat + 2)) / 180.0 * bh
        rect(s, x, y, cw + 0.4, ch + 0.4, fill=cmap("thermal", v / 22.0), op=min(1.0, 0.42 + v / 22.0))
    # colourbar (data encoding gets its legend, with units)
    cbx, cby, cbw = bx + bw - 150, by + bh - 16, 138
    for i in range(46):
        rect(s, cbx + i * (cbw / 46), cby, cbw / 46 + 0.5, 6, fill=cmap("thermal", i / 45.0))
    s.add(s.text(cbx, cby - 3, "0", 8.5, TEXT["tertiary"]))
    s.add(s.text(cbx + cbw, cby - 3, "22+ · OVATION prob.", 8.5, TEXT["tertiary"], anchor="end"))


def rnd_timeseries(s, bx, by, bw, bh, series, labels, ylo, yhi, unit, colormap=None,
                   colour=None, fill_under=False, zero=False, second=None, legend=None,
                   bars=None, phosphor=False):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    # grid + y labels (units always — token rule 9)
    rows = 4
    for i in range(rows + 1):
        gy = by + bh * i / rows
        line(s, bx, gy, bx + bw, gy, HAIRC, 1, 0.07)
        v = yhi - (yhi - ylo) * i / rows
        s.add(s.text(bx + 3, gy - 3, f"{v:g}", 9, TEXT["disabled"]))
    cols = 6
    for i in range(cols + 1):
        gx = bx + bw * i / cols
        line(s, gx, by, gx, by + bh, HAIRC, 1, 0.06)
        if labels:
            s.add(s.text(gx, by + bh + 11, labels[i], 9, TEXT["tertiary"], anchor="middle"))
    if zero:
        zy = by + bh * (yhi - 0) / (yhi - ylo)
        line(s, bx, zy, bx + bw, zy, HAIRC, 1, 0.30)
    def X(i, n):
        return bx + bw * i / (n - 1)
    def Y(v):
        return by + bh * (yhi - v) / (yhi - ylo)
    n = len(series)
    if bars is not None:  # bar-underlay variant (precip under temp etc.)
        w = bw / n * 0.62
        for i, v in enumerate(bars):
            h = bh * max(0.0, v) / (yhi - ylo) * 0.9
            rect(s, X(i, n) - w / 2, by + bh - h, w, h, fill=CV_C, op=0.22)
    if fill_under:
        pts = [(X(i, n), Y(v)) for i, v in enumerate(series)]
        poly(s, pts + [(X(n - 1, n), by + bh), (X(0, n), by + bh)], None, fill=colour or cmap("phosphor", 0.62),
             op=None)
        s.body[-1] = s.body[-1].replace("<path ", '<path fill-opacity="0.14" ', 1)
    if colormap:  # per-segment colour by value (data encoding)
        for i in range(n - 1):
            t = (series[i] - ylo) / (yhi - ylo) if not zero else (series[i] - ylo) / (yhi - ylo)
            poly(s, [(X(i, n), Y(series[i])), (X(i + 1, n), Y(series[i + 1]))],
                 cmap(colormap, min(1, max(0, t))), 1.6, 0.95)
    else:
        pts = [(X(i, n), Y(v)) for i, v in enumerate(series)]
        poly(s, pts, colour or AUDIO, 1.8, 1.0, glow=phosphor)
    if second:
        pts = [(X(i, len(second)), Y2) for i, Y2 in
               enumerate([by + bh * (yhi - v) / (yhi - ylo) for v in second])]
        poly(s, pts, TEXT["secondary"], 1.2, 0.55, dash="4 3")
    if legend:
        lx = bx + 10
        for k, (txt, c) in enumerate(legend):
            line(s, lx, by + 10 + k * 13, lx + 22, by + 10 + k * 13, c, 2)
            s.add(s.text(lx + 27, by + 13 + k * 13, txt, 9.5, TEXT["tertiary"]))
    s.add(s.text(bx + bw - 4, by + 12, unit, 9.5, TEXT["tertiary"], anchor="end"))


def rnd_bars(s, bx, by, bw, bh, vals, vmax, unit, thresh=None, thresh_label=None, cmapn="bipolar",
             levels=None):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    n = len(vals)
    w = bw / n
    for i in range(5):
        gy = by + bh * i / 4
        line(s, bx, gy, bx + bw, gy, HAIRC, 1, 0.07)
        s.add(s.text(bx + 3, gy - 3, f"{vmax - vmax * i / 4:g}", 9, TEXT["disabled"]))
    for i, v in enumerate(vals):
        h = bh * v / vmax
        x = bx + i * w + w * 0.18
        if levels:
            col = (METER["level_lo"] if v < 4 else METER["level_mid"] if v < 5
                   else METER["level_hi"] if v < 6 else STATE["error"]["colour"])
        else:
            col = cmap(cmapn, v / vmax)
        rect(s, x, by + bh - h, w * 0.64, h, fill=col, op=0.9)
        if v >= (thresh or 1e9):
            rect(s, x, by + bh - h, w * 0.64, 3, fill=TEXT["primary"], op=0.9)
    if thresh is not None:
        ty = by + bh - bh * thresh / vmax
        line(s, bx, ty, bx + bw, ty, STATE["stale"]["colour"], 1, 0.85, dash="6 4")
        s.add(s.text(bx + bw - 4, ty - 4, thresh_label or "", 9.5, STATE["stale"]["colour"], anchor="end"))
    s.add(s.text(bx + bw - 4, by + 12, unit, 9.5, TEXT["tertiary"], anchor="end"))
    for i in (0, n // 2, n - 1):
        s.add(s.text(bx + i * w + w / 2, by + bh + 11, f"-{(n - 1 - i) * 3} h" if i < n - 1 else "now",
                     9, TEXT["tertiary"], anchor="middle"))


def rnd_timeline_scatter(s, bx, by, bw, bh, flares):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    bands = ["B", "C", "M", "X"]
    rank = {"B": 0, "C": 1, "M": 2, "X": 3}
    bh4 = bh / 4
    for i, b in enumerate(bands):
        gy = by + bh - (i + 1) * bh4
        line(s, bx, gy, bx + bw, gy, HAIRC, 1, 0.08)
        s.add(s.text(bx + 4, gy + bh4 - 5, b, 12, TEXT["tertiary"]))
    for i in range(8):
        gx = bx + bw * i / 7
        line(s, gx, by, gx, by + bh, HAIRC, 1, 0.06)
        s.add(s.text(gx, by + bh + 11, f"-{7 - i} d", 9, TEXT["tertiary"], anchor="middle"))
    for f in flares:
        x = bx + bw * f["t"] / (7 * 24)
        base = by + bh - (rank[f["cls"]] + 1) * bh4
        y = base + bh4 * (1.0 - min(1.0, f["num"] / 9.5)) * 0.8 + bh4 * 0.1
        r = 2.2 + rank[f["cls"]] * 0.9
        circle(s, x, y, r, fill=cmap("thermal", 0.35 + 0.16 * rank[f["cls"]]), glow=(rank[f["cls"]] >= 2))
        if rank[f["cls"]] >= 2:
            s.add(s.text(x + 6, y - 4, f"{f['cls']}{f['num']:.1f}", 9.5, TEXT["secondary"]))
    s.add(s.text(bx + bw - 4, by + 12, "GOES XRS class · 7 d", 9.5, TEXT["tertiary"], anchor="end"))


def rnd_neo_timeline(s, bx, by, bw, bh, ev):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    import math as m
    lo, hi = m.log10(1.0), m.log10(75.0)
    for i, v in enumerate((1, 3, 10, 30)):
        gy = by + bh * (1 - (m.log10(v) - lo) / (hi - lo))
        line(s, bx, gy, bx + bw, gy, HAIRC, 1, 0.08)
        s.add(s.text(bx + 3, gy - 3, f"{v} LD", 9, TEXT["disabled"]))
    for i in range(8):
        gx = bx + bw * i / 7
        s.add(s.text(gx, by + bh + 11, f"-{7 - i} d", 9, TEXT["tertiary"], anchor="middle"))
    for e in ev:
        x = bx + bw * e["t"] / 7.0
        y = by + bh * (1 - (m.log10(max(1.0, e["ld"])) - lo) / (hi - lo))
        r = 1.6 + m.log10(max(10, e["size"])) * 0.7
        if e["haz"]:
            circle(s, x, y, r + 2.4, stroke=STATE["error"]["colour"], sw=1.2, op=0.9)
        circle(s, x, y, r, fill=cmap("greyscale", 0.55), glow=e["haz"])
    haz = [e for e in ev if e["haz"] and e["ld"] < 8]
    if haz:
        e = haz[0]
        x = bx + bw * e["t"] / 7.0
        y = by + bh * (1 - (m.log10(e["ld"]) - lo) / (hi - lo))
        s.add(s.text(min(x + 9, bx + bw - 120), y - 8, f"HAZARDOUS · {e['ld']:.1f} LD · {e['size']} m", 9.5,
                     STATE["error"]["colour"]))
    s.add(s.text(bx + bw - 4, by + 12, "miss distance · log LD", 9.5, TEXT["tertiary"], anchor="end"))


def rnd_gauge(s, bx, by, bw, bh, value, vmax, title, sub, bands):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    cx, cy = bx + bw / 2, by + bh * 0.62
    r = min(bw, bh) * 0.34
    a0, a1 = 135.0, 405.0
    def pt(a, rr):
        ar = math.radians(a)
        return cx + rr * math.cos(ar), cy + rr * math.sin(ar)
    # band arcs (meter tokens — level semantics)
    for (f0, f1, col) in bands:
        sa, ea = a0 + (a1 - a0) * f0, a0 + (a1 - a0) * f1
        steps = 24
        d = []
        for i in range(steps + 1):
            a = sa + (ea - sa) * i / steps
            x, y = pt(a, r)
            d.append(("M" if i == 0 else "L") + f"{x:.1f},{y:.1f}")
        path_d(s, "".join(d), col, 7, 0.85)
    # ticks
    for i in range(11):
        a = a0 + (a1 - a0) * i / 10
        x1, y1 = pt(a, r - 10)
        x2, y2 = pt(a, r - 4)
        line(s, x1, y1, x2, y2, HAIRC, 1, 0.4)
    # needle
    a = a0 + (a1 - a0) * min(1.0, value / vmax)
    x, y = pt(a, r - 12)
    line(s, cx, cy, x, y, TEXT["primary"], 2, 0.95)
    circle(s, cx, cy, 3.5, fill=TEXT["primary"])
    vfs = min(26, bh * 0.15)
    s.add(s.text(cx, cy + r * 0.52, f"{value:g}", vfs, TEXT["primary"], anchor="middle", glow=True))
    s.add(s.text(cx, cy + r * 0.52 + vfs * 0.62, title, min(10.5, bh * 0.06), TEXT["secondary"], anchor="middle"))
    s.add(s.text(cx, by + 14, sub, 9.5, TEXT["tertiary"], anchor="middle"))


def rnd_text(s, bx, by, bw, bh, lines, scroll=0.0, accent=None):
    rect(s, bx, by, bw, bh, fill=GROUND["inset"])
    lh = 15.5
    n_visible = int(bh / lh) + 2
    total = len(lines)
    off = int(scroll * total) % max(1, total)
    cid = s.clip(bx, by, bw, bh)
    s.add(s.g(clip=cid))
    for k in range(n_visible):
        idx = (off + k) % total
        y = by + 4 + (k - (scroll * total - off)) * lh + lh
        txt = lines[idx]
        col = TEXT["secondary"]
        if txt.startswith(":") or txt.startswith("#"):
            col = TEXT["disabled"]
        elif accent and (txt.strip().startswith(("Solar", "Geomagnetic", "The estimated", "CME"))):
            col = TEXT["primary"]
        s.add(s.text(bx + 10, y, txt, 11.5, col))
    s.add("</g>")
    # scroll cue
    s.add(s.text(bx + bw - 8, by + bh - 6, "▲▼ raw · scrolling", 9, TEXT["disabled"], anchor="end"))


# ── cell chrome (§5.3) ───────────────────────────────────────────────────────────────────────
LED = {"LIVE": (DATA_G, True), "STALE": (STATE["stale"]["colour"], False),
       "OFFLINE": (TEXT["disabled"], False), "KEY NEEDED": (STATE["warning"]["colour"], False)}


def cell_frame(s, x, y, w, h, title, desc, status, stamp, headline, selected=False, minimal=False):
    rect(s, x, y, w, h, fill=GROUND["panel"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=2)
    col, glow = LED[status]
    ty = y + 15
    s.add(s.text(x + 10, ty, title.upper(), 12, TEXT["primary"]))
    if not minimal:
        s.add(s.text(x + w - 26, ty, desc, 9.5, TEXT["tertiary"], anchor="end"))
    else:
        s.add(s.text(x + w - 26, ty, status, 9, TEXT["disabled"], anchor="end"))
    circle(s, x + w - 12, ty - 3.6, 4, fill=col if status == "LIVE" else None,
           stroke=col, sw=1.4, glow=glow)
    body = (x + 8, y + 22, w - 16, h - 22 - (0 if minimal else 30))
    if not minimal:
        fy = y + h - 9
        s.add(s.text(x + 10, fy, stamp, 9.5, TEXT["disabled"]))
        s.add(s.text(x + w - 10, fy, headline, 9.5, TEXT["secondary"], anchor="end"))
    if selected:
        rect(s, x, y, w, h, stroke=HAIRC, sw=2, so=HAIR["strong"], r=2)
        corner_ticks(s, x, y, w, h, arm=12)
    return body


# ── toolbar / panel widgets (§5.2) ──────────────────────────────────────────────────────────
def w_button(s, x, y, w, h, label, on=False, accent=None):
    accent = accent or DATA_G
    rect(s, x, y, w, h, fill=rgba(accent, 0.16) if on else GROUND["inset"],
         stroke=accent if on else HAIRC, sw=1, so=0.9 if on else HAIR["regular"], r=2)
    if on:
        circle(s, x + 11, y + h / 2, 2.6, fill=accent, glow=True)
        s.add(s.text(x + 20, y + h / 2 + 4, label, 11.5, accent))
    else:
        s.add(s.text(x + w / 2, y + h / 2 + 4, label, 11.5, TEXT["secondary"], anchor="middle"))


def w_enum(s, x, y, w, h, label, value, open_=False, focus=False):
    rect(s, x, y, w, h, fill=GROUND["inset"], stroke=HAIRC, sw=1,
         so=HAIR["strong"] if (open_ or focus) else HAIR["regular"], r=2)
    s.add(s.text(x + 10, y - 5, label, 9, TEXT["tertiary"], ls=0.8))
    s.add(s.text(x + 10, y + h / 2 + 4, value, 11.5, TEXT["primary"] if focus else TEXT["secondary"]))
    s.add(s.text(x + w - 12, y + h / 2 + 4, "▾", 11, TEXT["tertiary"], anchor="end"))


def w_slider(s, x, y, w, h, label, value_txt, frac):
    s.add(s.text(x, y + 2, label, 9, TEXT["tertiary"], ls=0.8))
    s.add(s.text(x + w, y + 2, value_txt, 10, TEXT["secondary"], anchor="end"))
    ty = y + h - 14
    line(s, x, ty, x + w, ty, HAIRC, 3, 0.18)
    line(s, x, ty, x + w * frac, ty, CV_C, 3, 0.9)
    circle(s, x + w * frac, ty, 6.5, fill=GROUND["panel"], stroke=CV_C, sw=2)


def toolbar(s, x, y, w, cell_no, stream_label, layout, states, open_stream=False):
    # row 1: CELL ▾ · STREAM ▾ · LAYOUT ▾ · [SOLO][FULL][TICKER][GRID][PAUSE] · broker status
    h = 34
    w_enum(s, x, y, 128, h, "CELL", f"CELL {cell_no:02d}", focus=True)
    w_enum(s, x + 144, y, 452, h, "STREAM  (edits the selected cell)", stream_label, open_=open_stream,
           focus=True)
    w_enum(s, x + 612, y, 168, h, "LAYOUT", layout)
    bx = x + 796
    for label, key, wd in (("SOLO", "solo", 84), ("FULL", "full", 84), ("TICKER", "ticker", 104),
                           ("GRID", "grid", 84), ("PAUSE", "pause", 96)):
        w_button(s, bx, y, wd, h, label, on=states.get(key, False))
        bx += wd + 10
    s.add(s.text(x + w - 4, y + h / 2 + 4,
                 "BROKER LIVE · 21/23 streams · 0 errors · next poll 12 s",
                 10.5, DATA_G, anchor="end", op=0.9))
    # row 2: sliders
    y2 = y + h + 12
    w_slider(s, x, y2, 420, 40, "HISTORY SPAN", "180 min", 0.42)
    w_slider(s, x + 452, y2, 300, 40, "TICKER SPEED", "1.0 ×", 0.33)
    w_slider(s, x + 784, y2, 300, 40, "COLORMAP INTENSITY", "1.00", 0.33)
    s.add(s.text(x + w - 4, y2 + 26, "26 params · state dat/observatory/state v1 · gpu medium · fuel 41 k/blk",
                 10, TEXT["disabled"], anchor="end"))


# ── card chrome ──────────────────────────────────────────────────────────────────────────────
CARD_W, CARD_H = 2208, 1280
DISP_X, DISP_Y, DISP_W, DISP_H = 16, 152, 2176, 1112   # display band inside the card
TICKER_H = 40


def card_header(s, x, y, w, status_txt, minimal=False):
    rect(s, x, y, CARD_W, CARD_H, fill=GROUND["panel"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=4)
    rect(s, x + 1, y + 1, w - 2, 2, fill=DATA_G, op=0.9)   # class stripe, 2 px (dat)
    if not minimal:
        s.add(s.text(x + 14, y + 22, "INSTRUMENT · dat/observatory", 12.5, TEXT["secondary"], ls=0.6))
        s.add(s.text(x + w - 14, y + 22, status_txt, 12, TEXT["primary"], anchor="end"))
    else:
        s.add(s.text(x + 14, y + 22, "dat/observatory — THE OBSERVATORY · SOLO+FULL", 12, TEXT["secondary"]))
        s.add(s.text(x + w - 14, y + 22, status_txt, 12, DATA_G, anchor="end"))


def ticker_band(s, x, y, w, h, items, offset=340.0):
    rect(s, x, y, w, h, fill=GROUND["inset"], stroke=HAIRC, sw=1, so=HAIR["faint"] * 2, r=2)
    line(s, x + 34, y + 4, x + 34, y + h - 4, HAIRC, 1, 0.25)
    s.add(s.text(x + 10, y + h / 2 + 4, "◂◂", 11, DATA_G))
    cid = s.clip(x + 40, y, w - 52, h)
    s.add(s.g(clip=cid))
    tx = x + 48 - offset
    for tag, txt in items:
        s.add(s.text(tx, y + h / 2 + 4, f"{tag} ", 11.5, DATA_G))
        tx += 7.05 * (len(tag) + 1)
        s.add(s.text(tx, y + h / 2 + 4, txt, 11.5, TEXT["secondary"]))
        tx += 7.05 * len(txt)
        s.add(s.text(tx, y + h / 2 + 4, " ◂ ", 11.5, TEXT["disabled"]))
        tx += 7.05 * 3
    s.add("</g>")


TICKER_ITEMS = [
    ("USGS·HOUR", "M 4.3 — 12 km S of Idria, CA · 21:38:05Z"),
    ("WWV", "K-index at 2100 UTC was 5 · SFI 132 · A 12 · UNSETTLED to ACTIVE"),
    ("EONET", "EONET_25043 Prescribed Fire D3 Bear RX, Greenlee, Arizona"),
    ("SWPC·FLR", "C2.1 begin 14:19Z max — AR 4238 · B4.4 begin 00:38Z"),
    ("GDACS", "EQ Orange M 5.8 — 68 km E of Kimbe, Papua New Guinea"),
    ("ISS", "34.59°N 82.70°W · 423 km · 27 583 km/h · daylight"),
    ("NEO", "2026 TX12 · 4.7 LD · 141 m · POTENTIALLY HAZARDOUS"),
    ("SWPC·GST", "G1 watch 07 OCT — CME glancing blow ±6 h"),
]


def draw_wall(s, cx, cy, cells_def, layout, states, selected=None, minimal=False, ticker=True,
              stream_label="", cell_no=2, open_stream=False, toolbar_on=True):
    """cells_def: list of draw-callables(cell_body_rect, idx)."""
    if toolbar_on:
        toolbar(s, cx + 16, cy + 40, CARD_W - 32, cell_no, stream_label,
                {1: "SOLO — 1 × 1", 2: "2 × 2 (4)", 4: "2 × 2 (4)", 8: "4 × 2 (8)", 9: "3 × 3 (9)",
                 12: "4 × 3 (12)", 16: "4 × 4 (16)"}[layout], states, open_stream)
    dx, dy = cx + DISP_X, cy + DISP_Y
    dw, dh = DISP_W, DISP_H - (TICKER_H + 8 if ticker else 0)
    rect(s, dx, dy, DISP_W, DISP_H, fill=GROUND["canvas"], stroke=HAIRC, sw=1, so=HAIR["faint"] * 2.2, r=2)
    gap = 16.0
    pad = 8.0
    if layout == 1 and cells_def:
        cols, rows = 1, 1
    else:
        cols = {4: 2, 8: 4, 9: 3, 12: 4, 16: 4}[layout]
        rows = {4: 2, 8: 2, 9: 3, 12: 3, 16: 4}[layout]
    cw = (dw - 2 * pad - (cols - 1) * gap) / cols
    chh = (dh - 2 * pad - (rows - 1) * gap) / rows
    n = len(cells_def)
    for i, draw in enumerate(cells_def):
        r_, c_ = divmod(i, cols)
        if layout == 1:
            r_, c_ = 0, 0
        x = dx + pad + c_ * (cw + gap)
        y = dy + pad + r_ * (chh + gap)
        body = cell_frame(s, x, y, cw, chh, *draw["chrome"], selected=(selected == i), minimal=minimal)
        draw["fn"](s, body, i)
        if layout == 1:
            break
    if ticker:
        ticker_band(s, dx + pad, dy + DISP_H - TICKER_H - 2, DISP_W - 2 * pad, TICKER_H, TICKER_ITEMS)


# ── shell chrome (mockups 1 & 4) ─────────────────────────────────────────────────────────────
def shell_frame(s, oy, zoom_txt, nodes, wires, title_left=True):
    W = 2560
    rect(s, 0, oy, W, 1600, fill=GROUND["base"])
    # rail
    rect(s, 0, oy, SH["rail_width"], 1600, fill=GROUND["panel"])
    line(s, SH["rail_width"], oy, SH["rail_width"], oy + 1600, HAIRC, 1, HAIR["regular"])
    glyphs = ["⌂", "≡", "◉", "♪", "✱"]
    for i, g_ in enumerate(glyphs):
        y = oy + 12 + i * 52
        active = i == 1
        rect(s, 6, y, 44, 44, fill=CV_C if active else None, op=0.10 if active else None,
             stroke=HAIRC, sw=1, so=HAIR["regular"] if active else HAIR["faint"], r=2)
        s.add(s.text(28, y + 28, g_, 15, CV_C if active else TEXT["tertiary"], anchor="middle"))
    # top bar
    rect(s, SH["rail_width"], oy, W - SH["rail_width"], SH["top_bar_height"], fill=GROUND["panel"])
    line(s, SH["rail_width"], oy + SH["top_bar_height"], W, oy + SH["top_bar_height"], HAIRC, 1, HAIR["regular"])
    s.add(s.text(72, oy + 29, "sparq", 14, TEXT["primary"], weight="bold"))
    s.add(s.text(128, oy + 29, "· observatory-demo.sparqpatch — Design mode", 11.5, TEXT["secondary"]))
    # transport (playing: green per state.playing)
    tx = 640
    rect(s, tx, oy + 8, 120, 32, fill=GROUND["inset"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=2)
    s.add(s.text(tx + 22, oy + 29, "▶", 13, STATE["playing"]["colour"], anchor="middle", glow=True))
    s.add(s.text(tx + 60, oy + 29, "■", 12, TEXT["tertiary"], anchor="middle"))
    s.add(s.text(tx + 98, oy + 29, "●", 12, TEXT["disabled"], anchor="middle"))
    s.add(s.text(W - 16, oy + 29, "48 kHz · 64 smp · null-device · cpu 2.1 % · xruns 0 · "
                                   "1 instrument · 23 streams · 21 LIVE", 11, TEXT["tertiary"], anchor="end"))
    # library
    lx = SH["rail_width"]
    ly = oy + SH["top_bar_height"]
    lw = SH["library_width"]
    rect(s, lx, ly, lw, 1600 - SH["top_bar_height"], fill=GROUND["panel"])
    line(s, lx + lw, ly, lx + lw, oy + 1600, HAIRC, 1, HAIR["regular"])
    s.add(s.text(lx + 12, ly + 24, "NODE LIBRARY", 11, TEXT["tertiary"], ls=1.2))
    rect(s, lx + 12, ly + 34, lw - 24, 28, fill=GROUND["inset"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=2)
    s.add(s.text(lx + 22, ly + 52, "search 24 backbone + 1 instrument…", 10.5, TEXT["disabled"]))
    yy = ly + 82
    s.add(s.text(lx + 12, yy, "INSTRUMENTS · 1", 10, DATA_G, ls=1.0))
    yy += 8
    rect(s, lx + 12, yy, lw - 24, SH["library_card_h"], fill=GROUND["panel_alt"], stroke=HAIRC, sw=1,
         so=HAIR["regular"], r=2)
    rect(s, lx + 12, yy, 3, SH["library_card_h"], fill=DATA_G)
    s.add(s.text(lx + 24, yy + 20, "The Observatory", 11.5, TEXT["primary"]))
    s.add(s.text(lx + lw - 20, yy + 20, "dat", 10, TEXT["tertiary"], anchor="end"))
    yy += SH["library_card_h"] + 16
    s.add(s.text(lx + 12, yy, "BACKBONE · 24", 10, TEXT["tertiary"], ls=1.0))
    yy += 6
    backbone = ["ana/rms", "dsp/scope", "env/ad", "flt/svf", "fx/fold", "mod/clk", "mod/lfo", "mod/rand",
                "mod/seq", "out/main", "syn/sine", "util/delay", "util/gain", "util/mult", "util/quant",
                "util/vca"]
    cls_of = {"ana": CV_C, "dsp": AUDIO, "env": CV_C, "flt": AUDIO, "fx": AUDIO, "mod": EVENT,
              "out": AUDIO, "syn": AUDIO, "util": TEXT["secondary"]}
    for name in backbone:
        yy += 26
        if yy > oy + 1600 - 40:
            s.add(s.text(lx + 24, yy, "…", 11, TEXT["disabled"]))
            break
        rect(s, lx + 12, yy - 18, lw - 24, 24, fill=None)
        rect(s, lx + 12, yy - 18, 2, 24, fill=cls_of[name.split("/")[0]], op=0.8)
        s.add(s.text(lx + 24, yy - 1, name, 10.5, TEXT["secondary"]))
    # canvas toolbar
    tby = ly
    cbx = lx + lw
    rect(s, cbx, tby, W - cbx, SH["toolbar_height"], fill=GROUND["panel"])
    line(s, cbx, tby + SH["toolbar_height"], W, tby + SH["toolbar_height"], HAIRC, 1, HAIR["regular"])
    s.add(s.text(cbx + 14, tby + 21, "FIT   RESET   ARRANGE", 11, TEXT["tertiary"]))
    s.add(s.text(cbx + 260, tby + 21, f"−   {zoom_txt}   +", 11, TEXT["secondary"]))
    s.add(s.text(cbx + 380, tby + 21, "wires: bézier-horizontal", 11, TEXT["disabled"]))
    s.add(s.text(W - 16, tby + 21, nodes, 11, TEXT["tertiary"], anchor="end"))
    # dock
    dky = oy + 1600 - SH["dock_height"]
    rect(s, cbx, dky, W - cbx, SH["dock_height"], fill=GROUND["panel"])
    line(s, cbx, dky, W, dky, HAIRC, 1, HAIR["regular"])
    s.add(s.text(cbx + 14, dky + 22, "PALETTE", 11, TEXT["primary"], ls=1.0))
    s.add(s.text(cbx + 100, dky + 22, "LOG", 11, TEXT["disabled"], ls=1.0))
    line(s, cbx + 10, dky + 30, cbx + 76, dky + 30, DATA_G, 2, 0.9)
    tiles = ["mod/clk", "mod/seq", "mod/lfo", "mod/rand", "syn/sine", "util/vca", "util/mult",
             "util/quant", "fx/fold", "flt/svf", "env/ad", "ana/rms", "dsp/scope", "out/main"]
    for i, t in enumerate(tiles):
        r_, c_ = divmod(i, 7)
        tx0 = cbx + 14 + c_ * (SH["dock_card_w"] + 12)
        ty0 = dky + 44 + r_ * (SH["dock_card_h"] + 12)
        if tx0 + SH["dock_card_w"] > W - 10:
            continue
        rect(s, tx0, ty0, SH["dock_card_w"], SH["dock_card_h"], fill=GROUND["panel_alt"], stroke=HAIRC,
             sw=1, so=HAIR["faint"] * 2, r=2)
        rect(s, tx0, ty0, 2, SH["dock_card_h"], fill=cls_of[t.split("/")[0]], op=0.85)
        s.add(s.text(tx0 + 10, ty0 + 24, t.split("/")[1], 11, TEXT["secondary"]))
        s.add(s.text(tx0 + 10, ty0 + 42, t.split("/")[0], 9, TEXT["disabled"]))
    return (cbx, tby + SH["toolbar_height"], W - cbx, dky - (tby + SH["toolbar_height"]))  # canvas rect


def dot_grid(s, x, y, w, h, minor, major, zoom):
    pid = "dots"
    s.defs.append(
        f'<pattern id="{pid}" width="{minor * zoom}" height="{minor * zoom}" patternUnits="userSpaceOnUse">'
        f'<circle cx="{minor * zoom / 2:.2f}" cy="{minor * zoom / 2:.2f}" r="{max(0.4, CV["grid_dot_radius"] * zoom):.2f}" '
        f'fill="{HAIRC}" opacity="{CV["grid_dot_opacity"]}"/></pattern>')
    rect(s, x, y, w, h, fill=f"url(#{pid})")


def wire(s, x1, y1, x2, y2, colour, wdt=2, dash=None, op=1.0):
    dx = abs(x2 - x1) * 0.5
    d = f"M{x1:.1f},{y1:.1f} C{x1 + dx:.1f},{y1:.1f} {x2 - dx:.1f},{y2:.1f} {x2:.1f},{y2:.1f}"
    a = [f'd="{d}"', 'fill="none"', f'stroke="{colour}"', f'stroke-width="{wdt}"']
    if dash:
        a.append(f'stroke-dasharray="{dash}"')
    if op != 1.0:
        a.append(f'stroke-opacity="{op}"')
    s.add(f'<path {" ".join(a)}/>')


def backbone_card(s, x, y, name, cls, z, well=None, params=2):
    """A PN-style backbone card at zoom z (screen-space drawing, world size 240 wide)."""
    w = CV["node_width_default"] * z
    hdr = CV["node_header_height"] * z
    prow = CV["node_param_row"] * z
    well_h = (160 if well == "scope" else 48) * z
    ports = CV["node_port_row"] * z
    h = hdr + prow * params + well_h + ports
    rect(s, x, y, w, h, fill=GROUND["panel"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=2 * z)
    rect(s, x, y + 1, 2 * z, h - 2, fill=cls, op=0.9)
    s.add(s.text(x + 8 * z, y + hdr - 10 * z, name, 11 * z, TEXT["primary"]))
    yy = y + hdr
    for i in range(params):
        yy += prow
        line(s, x + 10 * z, yy - prow / 2, x + w - 10 * z, yy - prow / 2, HAIRC, 2 * z, 0.2)
        circle(s, x + w * (0.3 + 0.13 * i), yy - prow / 2, 3.2 * z, fill=GROUND["panel"], stroke=CV_C, sw=1.4 * z)
    wy = yy
    rect(s, x + 6 * z, wy + 4 * z, w - 12 * z, well_h - 8 * z, fill=GROUND["inset"], r=1)
    if well == "scope":
        rng = random.Random(21)
        pts = []
        n = 60
        for i in range(n):
            v = math.sin(i / n * 4 * math.pi) * (0.7 + 0.3 * rng.random())
            pts.append((x + 8 * z + (w - 16 * z) * i / (n - 1), wy + well_h / 2 - v * well_h * 0.34))
        poly(s, pts, AUDIO, 1.6, 1.0, glow=True)
        for i in range(1, 10):
            line(s, x + 8 * z + (w - 16 * z) * i / 10, wy + 6 * z, x + 8 * z + (w - 16 * z) * i / 10,
                 wy + well_h - 6 * z, HAIRC, 1, 0.08)
    elif well == "meters":
        for k in range(2):
            mx = x + 14 * z + k * 22 * z
            rect(s, mx, wy + 8 * z, 12 * z, well_h - 16 * z, fill=HAIRC, op=0.10)
            fillh = (well_h - 16 * z) * (0.72 - 0.2 * k)
            rect(s, mx, wy + well_h - 8 * z - fillh, 12 * z, fillh, fill=METER["level_lo"], op=0.85)
            rect(s, mx, wy + well_h - 8 * z - fillh, 12 * z, 3 * z, fill=METER["level_mid"])
    elif well == "steps":
        n = 8
        for i in range(n):
            sx = x + 10 * z + i * (w - 20 * z) / n
            on = i == 3
            rect(s, sx, wy + well_h * 0.3, (w - 20 * z) / n - 3 * z, well_h * 0.4,
                 fill=EVENT if on else HAIRC, op=0.95 if on else 0.14, r=1)
    elif well == "ring":
        cx0, cy0 = x + w / 2, wy + well_h / 2
        for k, rr in enumerate((0.42, 0.30, 0.18)):
            circle(s, cx0, cy0, well_h * rr, stroke=EVENT, sw=1.2, op=0.30 + 0.2 * k)
        circle(s, cx0 + well_h * 0.30, cy0, 2.4 * z, fill=EVENT, glow=True)
    elif well == "sine":
        pts = [(x + 8 * z + (w - 16 * z) * i / 40,
                wy + well_h / 2 - math.sin(i / 40 * 2 * math.pi) * well_h * 0.3) for i in range(41)]
        poly(s, pts, CV_C, 1.4, 0.95)
    elif well == "graph":
        rng = random.Random(9)
        pts = []
        v = 0.4
        for i in range(30):
            v = max(0.05, min(0.95, v + rng.gauss(0, 0.09)))
            pts.append((x + 8 * z + (w - 16 * z) * i / 29, wy + well_h - 6 * z - v * (well_h - 12 * z)))
        poly(s, pts, CV_C, 1.4, 0.9, glow=True)
    py = yy + well_h
    rect(s, x, py, w, ports, fill=None)
    return (x, y, w, h, py)


def port(s, cx, cy, cls, z):
    circle(s, cx, cy, _L["touch"]["port_radius"] * z, fill=GROUND["panel"], stroke=cls, sw=1.4 * z)


# ── mockup builders ──────────────────────────────────────────────────────────────────────────
def title_block(s, title, meta, caption, w, h, cap2=None):
    s.add(f'<rect width="{w}" height="{h}" fill="{GROUND["base"]}"/>')
    s.add(s.text(24, 30, title, 18, TEXT["primary"]))
    s.add(s.text(w - 24, 30, meta, 11, TEXT["tertiary"], anchor="end"))
    s.add(s.text(24, 52, "WO-020 plan-of-record mockup · generated by tools/make_observatory_mockups.py "
                         "from design/tokens · cell data is illustrative+seeded; endpoints probe-verified "
                         "2026-10-06 (plan §3)", 10.5, TEXT["disabled"]))
    s.add(s.text(24, h - 26, caption, 10.5, TEXT["tertiary"]))
    if cap2:
        s.add(s.text(24, h - 10, cap2, 10.5, TEXT["tertiary"]))


# ── the 16 cells of the wall (mockup 1) ─────────────────────────────────────────────────────
def cells_16():
    Q = gen_quakes()
    E = gen_eonet()
    G = gen_gdacs()
    A = gen_aurora()
    K = gen_kp()
    F = gen_flares()
    N = gen_neo()
    bz = gen_series(11, 220, -8, 6, 0.45, spike=(120, 150, lambda v, t: v - 6.5 * math.sin(math.pi * t)))
    sw = gen_series(12, 220, 300, 480, 9, spike=(150, 175, lambda v, t: v + 190 * math.sin(math.pi * t)))
    rng = random.Random(13)
    temp = [12.4 + 3.6 * math.sin(i / 48 * 2 * math.pi - 1.9) + rng.gauss(0, 0.5) for i in range(48)]
    wind = [4.6 + 1.9 * math.sin(i / 48 * 2 * math.pi + 0.6) + rng.gauss(0, 0.45) for i in range(48)]
    prec = [max(0, rng.gauss(0.4, 1.1)) for _ in range(48)]
    tide_o = [2.4 + 0.62 * math.sin(i / 62 * 2 * math.pi) + 0.28 * math.sin(i / 12.42 * 2 * math.pi)
              + rng.gauss(0, 0.03) for i in range(62)]
    tide_p = [2.4 + 0.62 * math.sin(i / 62 * 2 * math.pi) + 0.28 * math.sin(i / 12.42 * 2 * math.pi)
              for i in range(62)]
    rng2 = random.Random(14)
    power = [13.5 + 4.2 * math.sin(i / 30 * 2 * math.pi - 0.8) + rng2.gauss(0, 0.9) for i in range(30)]

    def f_quakes(s, b, i):
        rnd_map_points(s, *b, Q, "quakes", 101)

    def f_aurora(s, b, i):
        rnd_map_heat(s, *b, A)

    def f_bz(s, b, i):
        rnd_timeseries(s, *b, bz, ["-3 h", "-2.5", "-2", "-1.5", "-1", "-30 m", "now"],
                       -14, 8, "nT · 1 min", colormap="bipolar", zero=True)

    def f_wwv(s, b, i):
        rnd_text(s, *b, WWV, scroll=0.12, accent=True)

    def f_iss(s, b, i):
        rnd_map_points(s, *b, [], "iss", 15)

    def f_eonet(s, b, i):
        rnd_map_points(s, *b, E, "eonet", 202)

    def f_wx(s, b, i):
        rnd_timeseries(s, *b, temp, ["-48 h", "-40", "-32", "-24", "-16", "-8", "now"], 4, 22,
                       "°C · hourly", colour=AUDIO, second=wind, bars=prec,
                       legend=[("T2M °C", AUDIO), ("wind m/s", TEXT["secondary"]), ("precip mm", CV_C)])

    def f_kp(s, b, i):
        rnd_bars(s, *b, K, 9, "Kp · 3 h", thresh=5, thresh_label="G1 storm line Kp 5", levels=True)

    def f_flares(s, b, i):
        rnd_timeline_scatter(s, *b, F)

    def f_neo(s, b, i):
        rnd_neo_timeline(s, *b, N)

    def f_tides(s, b, i):
        rnd_timeseries(s, *b, tide_o, ["-36 h", "-30", "-24", "-18", "-12", "-6", "now"], 1.2, 3.6,
                       "m MLLW · 6 min", colour=CV_C, second=tide_p,
                       legend=[("observed", CV_C), ("prediction", TEXT["secondary"])])

    def f_aqi(s, b, i):
        rnd_gauge(s, *b, 42, 200, "US AQI 42 · GOOD", "PM2.5 11.8 µg/m³ · London",
                  [(0, 0.25, METER["level_lo"]), (0.25, 0.5, METER["level_mid"]),
                   (0.5, 0.75, METER["level_hi"]), (0.75, 1.0, STATE["error"]["colour"])])

    def f_sw(s, b, i):
        rnd_timeseries(s, *b, sw, ["-3 h", "-2.5", "-2", "-1.5", "-1", "-30 m", "now"], 240, 720,
                       "km/s · 1 min", colour=AUDIO, fill_under=True, phosphor=True)

    def f_offline(s, b, i):
        x, y, w, h = b
        rect(s, x, y, w, h, fill=GROUND["inset"])
        s.add(s.text(x + w / 2, y + h / 2 - 14, "OFFLINE — api.weather.gov unreachable", 13, TEXT["secondary"],
                     anchor="middle"))
        s.add(s.text(x + w / 2, y + h / 2 + 6, "next retry 21:43:58 UTC · backoff 4 m 32 s (×2 to 15 m)",
                     10.5, TEXT["disabled"], anchor="middle"))
        s.add(s.text(x + w / 2, y + h / 2 + 24, "last window kept: 214 alerts · newest 20:58:11 UTC (stale)",
                     10.5, TEXT["disabled"], anchor="middle"))

    def f_gdacs(s, b, i):
        rnd_map_points(s, *b, G, "gdacs", 303)

    def f_power(s, b, i):
        rnd_timeseries(s, *b, power, ["-30 d", "-25", "-20", "-15", "-10", "-5", "-3 d"], 4, 24,
                       "T2M °C · daily", colour=TEXT["secondary"])

    chrome = [
        ("Aurora — OVATION grid", "swpc.aurora · map-heat · 300 s", "LIVE", "21:38:00 UTC · 62 s ago",
         "Kp 5− · oval equatorward · thermal"),
        ("Earthquakes — past hour", "geo.quakes-hour · USGS · 60 s", "LIVE", "21:38:05 UTC · 47 s ago",
         "M 4.3 — 12 km S of Idria, CA"),
        ("Solar wind Bz (IMF)", "swpc.bz · timeseries · 60 s", "LIVE", "21:39:00 UTC · 12 s ago",
         "−6.8 nT southward · bipolar ramp"),
        ("WWV geophysical alert", "swpc.wwv · raw text · 600 s", "LIVE", "Issued 21:05 UTC · 34 m ago",
         "raw feed · scrolling · PAUSE freezes"),
        ("ISS — live position", "sat.iss · map · 5 s", "LIVE", "21:39:30 UTC · 2 s ago",
         "34.59°N 82.70°W · 423 km · daylight"),
        ("NASA EONET — open events", "geo.eonet · map-points · 300 s", "LIVE", "21:35:11 UTC · 4 m ago",
         "30 open · 14 wildfires · categorical-6"),
        ("Weather — London 51.5N", "wx.openmeteo · timeseries · 300 s", "LIVE", "21:30:00 UTC · 9 m ago",
         "12.4 °C · wind 4.6 m/s · precip overlay"),
        ("Planetary Kp — 3 day", "swpc.kp · bars · 300 s", "LIVE", "21:00:00 UTC · 39 m ago",
         "Kp 4.7 latest · G1 watch 07 OCT"),
        ("GOES X-ray flares — 7 d", "swpc.xray-flares · timeline · 300 s", "LIVE", "21:37:44 UTC · 1 m ago",
         "22 events · max M1.2 · 04 OCT 08:47Z"),
        ("NASA NEO — approaches", "space.neo · timeline · 1800 s", "LIVE", "21:12:03 UTC · 27 m ago",
         "2026 TX12 · 4.7 LD · 141 m · HAZ"),
        ("Water level — Seattle", "wx.tides · CO-OPS · 900 s", "LIVE", "21:24:00 UTC · 15 m ago",
         "2.71 m MLLW · obs solid / pred dashed"),
        ("Air quality — London", "wx.air · gauge · 900 s", "LIVE", "21:00:00 UTC · 39 m ago",
         "US AQI 42 GOOD · PM2.5 11.8 µg/m³"),
        ("Solar wind speed", "swpc.solar-wind · timeseries · 60 s", "LIVE", "21:39:00 UTC · 12 s ago",
         "282 → 611 km/s gust · phosphor"),
        ("US weather alerts", "wx.alerts · map-points · 300 s", "OFFLINE", "last record 20:58:11 UTC",
         "OFFLINE in words — the app never dies because a feed died"),
        ("GDACS — EQ / TC / FL", "geo.gdacs · map-points · 900 s", "LIVE", "21:31:02 UTC · 8 m ago",
         "EQ Orange M 5.8 — Papua New Guinea"),
        ("NASA POWER — daily met", "space.power · timeseries · 3600 s", "STALE", "last record 21:27 UTC",
         "STALE — POWER publishes ~2 d behind · last-good window"),
    ]
    fns = [f_aurora, f_quakes, f_bz, f_wwv, f_iss, f_eonet, f_wx, f_kp,
           f_flares, f_neo, f_tides, f_aqi, f_sw, f_offline, f_gdacs, f_power]
    out = []
    for i, (ch, fn) in enumerate(zip(chrome, fns)):
        out.append({"chrome": ch, "fn": fn})
    return out


# ── MOCKUP 1 — the wall in the shell ─────────────────────────────────────────────────────────
def mockup_wall():
    s = Svg(2560, 1700)
    title_block(s,
                "sparq · dat/observatory — mockup 1 of 4: THE WALL (4 × 4 · 16 live cells)",
                "design mode · 2560 × 1600 reference · zoom FIT 97 % · card 2208 × 1280 world px",
                "the card covers 97.5 % of the canvas-rect width at zoom 1.0 (plan D6 — audited number) · "
                "14 LIVE · space.power STALE (publication lag, honest) · wx.alerts OFFLINE in words (D10) · "
                "the global ticker band scrolls raw feed text (D8) · stamps absolute UTC — the guest has no wall clock",
                2560, 1700)
    oy = 64
    canvas = shell_frame(s, oy, "97 %", "17 nodes · 0 wires off-canvas", None)
    cx0, cy0, cw, ch = canvas
    rect(s, cx0, cy0, cw, ch, fill=GROUND["canvas"])
    z = 0.975
    dot_grid(s, cx0, cy0, cw, ch, CV["grid_minor"], CV["grid_major"], z)
    card_x = cx0 + (cw - CARD_W * z) / 2
    card_y = cy0 + (ch - CARD_H * z) / 2
    cid = s.clip(cx0, cy0, cw, ch)
    s.add(s.g(clip=cid))                      # clip in CANVAS space...
    s.add(s.g(tr=f"translate({card_x:.1f},{card_y:.1f}) scale({z})"))   # ...transform inside it
    card_header(s, 0, 0, CARD_W, "THE OBSERVATORY · 16 cells · 14 LIVE · 1 STALE · 1 OFFLINE")
    states = {"ticker": True, "grid": True}
    draw_wall(s, 0, 0, cells_16(), 16, states, selected=1,
              stream_label="geo.quakes-hour — Earthquakes, past hour (USGS)", cell_no=2)
    s.add("</g>")
    s.add("</g>")
    return s


# ── MOCKUP 2 — SOLO + FULL ───────────────────────────────────────────────────────────────────
def mockup_full():
    s = Svg(2400, 1480)
    title_block(s,
                "sparq · dat/observatory — mockup 2 of 4: SOLO + FULL — swpc.aurora takes the wall",
                "the face alone · card at 100 % · FULL = solo + minimal chrome (plan D7)",
                "OVATION 2° grid, broker-regridded from the 925 KB 1° payload (§6.4) · colormap.thermal as data "
                "encoding, never style (token rule 11) · graticule hairline-faint, 30° · coastlines: Natural Earth 110m, "
                "public domain · Kp 5− annotation: the aurora bulges equatorward — the picture and the number agree",
                2400, 1480)
    x0, y0 = 96, 76
    s.add(s.g(tr=f"translate({x0},{y0})"))
    card_header(s, 0, 0, CARD_W, "LIVE · 21:38:00 UTC · Kp 5−", minimal=True)
    A = gen_aurora(kp=4.9)

    def f_solo(s, b, i):
        x, y, w, h = b
        rnd_map_heat(s, x, y, w, h, A)
        # Kp annotation + subsolar marker (host-rendered glyph-run vocabulary)
        s.add(s.text(x + 14, y + 20, "OVATION aurora probability · 30-min forecast · 2° grid", 13,
                     TEXT["secondary"]))
        s.add(s.text(x + 14, y + 40, "Kp 5− (G1 MINOR) — oval equatorward ~4° · substorm sector brightened",
                     11.5, STATE["stale"]["colour"]))
        sx, sy = eq(4.0, -35.0, x, y, w, h)   # approximate sub-solar point 21:38 UTC
        circle(s, sx, sy, 5, stroke=METER["level_mid"], sw=1.5, op=0.9)
        line(s, sx - 11, sy, sx + 11, sy, METER["level_mid"], 1, 0.7)
        line(s, sx, sy - 11, sx, sy + 11, METER["level_mid"], 1, 0.7)
        s.add(s.text(sx + 14, sy - 8, "sub-solar point", 10, METER["level_mid"]))
        # magnetic poles
        for (la, lo, nm) in ((80.9, -71.9, "geomag. N"), (-64.1, 135.9, "geomag. S")):
            px, py = eq(la, lo, x, y, w, h)
            circle(s, px, py, 3, fill=None, stroke=TEXT["tertiary"], sw=1.2)
            s.add(s.text(px + 8, py + 3.5, nm, 9.5, TEXT["tertiary"]))

    solo = [{"chrome": ("Aurora — OVATION grid", "swpc.aurora", "LIVE", "21:38:00 UTC · 62 s ago", ""),
             "fn": f_solo}]
    draw_wall(s, 0, 0, solo, 1, {"solo": True, "full": True, "ticker": True, "grid": True},
              selected=None, minimal=True,
              stream_label="swpc.aurora — Aurora forecast, OVATION grid (NOAA SWPC)", cell_no=1)
    s.add("</g>")
    return s


# ── MOCKUP 3 — the picker ────────────────────────────────────────────────────────────────────
def mockup_picker():
    s = Svg(2400, 1520)
    title_block(s,
                "sparq · dat/observatory — mockup 3 of 4: the toolbar + STREAM picker (round 4's OWED item 11)",
                "2 × 2 layout · CELL selects, STREAM edits the selected cell · visible_if does the switching (D7)",
                "24 options: the 23 registry streams + OFF · grouped by domain (SW/SAT/GEO/WX/SP) · geo.firms lists "
                "KEY NEEDED until SPARQ_FIRMS_KEY is set (D14) — never a silent hole · selection reads white "
                "corner-ticks (operator ruling 2026-10-01) · rows 44 px — the touch floor",
                2400, 1520)
    x0, y0 = 96, 76
    s.add(s.g(tr=f"translate({x0},{y0})"))
    card_header(s, 0, 0, CARD_W, "THE OBSERVATORY · 4 cells · all LIVE")
    Q = gen_quakes()
    bz = gen_series(11, 220, -8, 6, 0.45, spike=(120, 150, lambda v, t: v - 6.5 * math.sin(math.pi * t)))
    K = gen_kp()

    def f_quakes(s, b, i):
        rnd_map_points(s, *b, Q, "quakes", 101)

    def f_bz(s, b, i):
        rnd_timeseries(s, *b, bz, ["-3 h", "-2.5", "-2", "-1.5", "-1", "-30 m", "now"], -14, 8,
                       "nT · 1 min", colormap="bipolar", zero=True)

    def f_wwv(s, b, i):
        rnd_text(s, *b, WWV, scroll=0.3, accent=True)

    def f_kp(s, b, i):
        rnd_bars(s, *b, K, 9, "Kp · 3 h", thresh=5, thresh_label="G1 storm line Kp 5", levels=True)

    cells = [
        {"chrome": ("Earthquakes — past hour", "geo.quakes-hour · USGS · 60 s", "LIVE", "21:38:05 UTC · 47 s ago",
                    "M 4.3 — 12 km S of Idria, CA"), "fn": f_quakes},
        {"chrome": ("Solar wind Bz (IMF)", "swpc.bz · timeseries · 60 s", "LIVE", "21:39:00 UTC · 12 s ago",
                    "−6.8 nT southward"), "fn": f_bz},
        {"chrome": ("WWV geophysical alert", "swpc.wwv · raw text · 600 s", "LIVE", "Issued 21:05 UTC",
                    "raw feed · scrolling"), "fn": f_wwv},
        {"chrome": ("Planetary Kp — 3 day", "swpc.kp · bars · 300 s", "LIVE", "21:00:00 UTC · 39 m ago",
                    "Kp 4.7 latest · G1 watch"), "fn": f_kp},
    ]
    draw_wall(s, 0, 0, cells, 4, {"ticker": True, "grid": True}, selected=1,
              stream_label="swpc.bz — Solar wind Bz (IMF)", cell_no=2, open_stream=True)
    # ── the dropdown, open (overlay ground 95 %, hairline-strong, corner 4, rows 44) ──
    dx, dy = 160, 80
    dw, dh = 620, 640
    rect(s, dx, dy, dw, dh, fill=GROUND["overlay"], op=0.97, stroke=HAIRC, sw=1, so=HAIR["strong"], r=4)
    rect(s, dx + 10, dy + 8, dw - 20, 28, fill=GROUND["inset"], stroke=HAIRC, sw=1, so=HAIR["regular"], r=2)
    s.add(s.text(dx + 20, dy + 27, "filter:  swpc", 11.5, TEXT["secondary"]))
    s.add(s.text(dx + dw - 20, dy + 27, "24 streams", 10, TEXT["disabled"], anchor="end"))
    rows = [
        ("SPACE WEATHER · NOAA SWPC", None, None),
        ("Aurora forecast — OVATION grid", "swpc.aurora", None),
        ("Planetary Kp index — 3 day", "swpc.kp", None),
        ("Solar wind Bz (IMF)", "swpc.bz", "current"),
        ("GOES X-ray flares — 7 day", "swpc.xray-flares", None),
        ("WWV geophysical alert (raw)", "swpc.wwv", None),
        ("SATELLITES", None, None),
        ("ISS — live position", "sat.iss", None),
        ("Satellite TLEs — CelesTrak", "sat.tle-stations", None),
        ("GEOLOGICAL & EARTH EVENTS", None, None),
        ("Earthquakes — past hour (USGS)", "geo.quakes-hour", "hover"),
        ("NASA FIRMS — active fires 24 h", "geo.firms", "key"),
    ]
    ry = dy + 42
    for label, sid, flag in rows:
        if sid is None:
            ry += 6
            s.add(s.text(dx + 16, ry + 16, label, 9.5, TEXT["tertiary"], ls=1.1))
            ry += 26
            continue
        row_h = 44
        if flag == "current":
            rect(s, dx + 8, ry, dw - 16, row_h, fill=DATA_G, op=0.10, r=2)
            corner_ticks(s, dx + 8, ry, dw - 16, row_h, arm=8, sw=1)
        elif flag == "hover":
            rect(s, dx + 8, ry, dw - 16, row_h, fill=HAIRC, op=0.07, r=2)
        if flag == "key":
            s.add(s.text(dx + 20, ry + 19, label, 12, TEXT["disabled"]))
            s.add(s.text(dx + 20, ry + 36, "SPARQ_FIRMS_KEY unset — stream not fetched (words, never a hole)",
                         9, STATE["warning"]["colour"]))
            s.add(s.text(dx + dw - 18, ry + 27, "KEY NEEDED", 10, STATE["warning"]["colour"], anchor="end"))
        else:
            s.add(s.text(dx + 20, ry + 27, label, 12.5, TEXT["primary"] if flag == "current" else TEXT["secondary"]))
            s.add(s.text(dx + dw - 46, ry + 27, sid, 10, TEXT["tertiary"], anchor="end"))
            if flag == "current":
                s.add(s.text(dx + dw - 24, ry + 27, "✓", 12.5, DATA_G, anchor="end"))
        ry += row_h
        if ry > dy + dh - 40:
            break
    line(s, dx + 8, dy + dh - 34, dx + dw - 8, dy + dh - 34, HAIRC, 1, HAIR["regular"])
    s.add(s.text(dx + dw / 2, dy + dh - 14, "▼ 12 more — GEOLOGICAL · WEATHER & OCEAN · SPACE — scroll", 10.5,
                 TEXT["tertiary"], anchor="middle"))
    s.add("</g>")
    return s


# ── MOCKUP 4 — the wall in a patch ───────────────────────────────────────────────────────────
def mockup_canvas():
    s = Svg(2560, 1700)
    title_block(s,
                "sparq · dat/observatory — mockup 4 of 4: the wall in a patch (zoom 45 %)",
                "the example patch (§5.4): the ambient chain plays, the wall watches · event cables solid "
                "(round-4 D9) · control wires dashed",
                "v1 declares ZERO ports (D5) — the instrument is patched to nothing and needs nothing: sonification "
                "is INC6, gated on the Phase-5 data-port arbiter · the card is 9.2× a backbone card's width · "
                "corner-ticks: the Observatory is the selected node · instrument-layer grouping in the library (INC4)",
                2560, 1700)
    oy = 64
    canvas = shell_frame(s, oy, "45 %", "8 nodes · 7 wires", None)
    cx0, cy0, cw, ch = canvas
    rect(s, cx0, cy0, cw, ch, fill=GROUND["canvas"])
    z = 0.45
    dot_grid(s, cx0, cy0, cw, ch, CV["grid_minor"], CV["grid_major"], z)
    cid = s.clip(cx0, cy0, cw, ch)
    s.add(s.g(clip=cid))
    def W(wx, wy):
        return cx0 + (wx + 140) * z, cy0 + (wy + 40) * z
    # the wall, selected
    ox, oy2 = W(0, 60)
    s.add(s.g(tr=f"translate({ox:.1f},{oy2:.1f}) scale({z})"))
    card_header(s, 0, 0, CARD_W, "THE OBSERVATORY · 16 cells · 14 LIVE")
    cells = cells_16()
    draw_wall(s, 0, 0, cells, 16, {"ticker": True, "grid": True}, selected=None)
    s.add("</g>")
    corner_ticks(s, ox, oy2, CARD_W * z, CARD_H * z, arm=16)
    # the backbone ambient chain (world coords → screen)
    chain = [
        ("mod/clk", EVENT, 2500, 470, "ring", 1),
        ("mod/seq", EVENT, 2840, 470, "steps", 1),
        ("syn/sine", AUDIO, 3180, 470, None, 2),
        ("util/vca", AUDIO, 3520, 470, None, 2),
        ("out/main", AUDIO, 4240, 320, "meters", 0),
        ("mod/lfo", CV_C, 3180, 800, "sine", 1),
        ("dsp/scope", AUDIO, 3860, 800, "scope", 1),
        ("ana/rms", CV_C, 4240, 660, "graph", 1),
    ]
    placed = {}
    for name, cls, wx, wy, well, params in chain:
        x, y = W(wx, wy)
        geom = backbone_card(s, x, y, name, cls, z, well=well, params=params)
        placed[name] = geom
    def pr(name, side, idx=0, frac=0.5):
        x, y, w, h, py = placed[name]
        hdr = CV["node_header_height"] * z
        prow = CV["node_param_row"] * z
        off = 6 * z
        if side == "out":
            return x + w + off, py + CV["node_port_row"] * z * frac
        return x - off, py + CV["node_port_row"] * z * frac
    links = [
        ("mod/clk", "mod/seq", EVENT, None, 2),
        ("mod/seq", "syn/sine", EVENT, None, 2),
        ("syn/sine", "util/vca", AUDIO, None, 2),
        ("util/vca", "out/main", AUDIO, None, 2),
        ("mod/lfo", "util/vca", CV_C, "4 4", 1.4),
        ("util/vca", "dsp/scope", AUDIO, None, 2),
        ("util/vca", "ana/rms", AUDIO, None, 2),
    ]
    for a, b, col, dash, wdt in links:
        x1, y1 = pr(a, "out")
        x2, y2 = pr(b, "in")
        wire(s, x1, y1, x2, y2, col, wdt * 1.0, dash)
        port(s, x1, y1, col, z)
        port(s, x2, y2, col, z)
    # a cable node on the sine→vca wire (round 4)
    x1, y1 = pr("syn/sine", "out")
    x2, y2 = pr("util/vca", "in")
    circle(s, (x1 + x2) / 2, (y1 + y2) / 2 + 6, 4, fill=GROUND["panel"], stroke=AUDIO, sw=1.4)
    s.add("</g>")
    return s


# ── main ─────────────────────────────────────────────────────────────────────────────────────
def main():
    outs = [
        ("observatory-wall", mockup_wall),
        ("observatory-full", mockup_full),
        ("observatory-picker", mockup_picker),
        ("observatory-canvas", mockup_canvas),
    ]
    have_rsvg = subprocess.run(["bash", "-c", "command -v rsvg-convert"], capture_output=True).returncode == 0
    for name, fn in outs:
        svg = fn().out()
        p = ROOT / "design/mockups" / f"{name}.svg"
        write_text(p, svg)
        png = ""
        if have_rsvg:
            pp = p.with_suffix(".png")
            r = subprocess.run(["rsvg-convert", str(p), "-o", str(pp)], capture_output=True)
            png = f" + {pp.name}" if r.returncode == 0 else " (rsvg-convert failed)"
        print(f"wrote {p.relative_to(ROOT)} ({len(svg):,} B){png}")
    print("regenerate after any token change; PNG siblings need rsvg-convert (review artefacts only)")


if __name__ == "__main__":
    main()
