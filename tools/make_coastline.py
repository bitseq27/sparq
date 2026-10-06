#!/usr/bin/env python3
"""
tools/make_coastline.py — the WO-020 coastline asset generator (INC3, plan D12).

The Observatory's MAP renderers need coastlines. The display vocabulary has no image
primitive by design (ADR-010 decision 3), so geography rides `path` items — which means the
geometry has to be small enough to embed in the wasm data section (MODULE-BUILD-GUIDE §3's
small-asset rule; no hash-declared asset store in v1). This tool reads the public-domain
Natural Earth 110m land polygons (cached in `tools/data/`, fetch recipe below) and writes
one deterministic binary asset:

    instruments-src/observatory/assets/coastline-110m.bin

Budget (plan D12): <= ~3500 points, <= 160 KB. The 110m land set arrives at ~5.1 k points /
138 KB GeoJSON; Visvalingam-Whyatt simplification to the point target lands the binary at
~15 KB (4 bytes/point, centidegree i16 pairs) — an order of magnitude inside the budget.

Binary format (little-endian; wasm32 is LE, and the reader is
`instruments-src/observatory/core/src/coastline.rs`):

    u32   magic   0x53505251 ("SPRQ")
    u32   version 1
    u32   ring_count
    per ring:
      u32 point_count          (>= 3; rings are implicitly CLOSED — the last point joins the
                                first, and the closing point is NOT stored twice)
      point_count * (i16 lon_centidegrees, i16 lat_centidegrees)

    lon_cd = round(lon * 100) clamped to [-18000, 18000]
    lat_cd = round(lat * 100) clamped to [-9000, 9000]

    Centidegrees = 0.01 deg ~ 1.1 km at the equator. The wall's map cells are ~532 px wide
    for 360 deg (6 px/deg at the 4x4 layout), so 0.01 deg is ~0.06 px — below the pixel the
    renderer draws, i.e. lossless for every breakpoint the instrument ships at.

Determinism: the simplification is a pure function of the input GeoJSON (Visvalingam-Whyatt
with an index tie-break, one shared area threshold binary-searched to the point target, ring
order = file order, no randomness, no timestamps). `--check` regenerates and compares bytes —
the token_gen.py discipline: CI fails if the checked-in asset is stale.

Source data (PUBLIC DOMAIN — Natural Earth, https://www.naturalearthdata.com/about/terms-of-use/):
cached at tools/data/ne_110m_land.geojson. Re-fetch with:

    python3 - <<'PY'
    import urllib.request, pathlib
    u = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_land.geojson"
    req = urllib.request.Request(u, headers={"User-Agent": "sparq-make-coastline/0.1"})
    pathlib.Path("tools/data/ne_110m_land.geojson").write_bytes(urllib.request.urlopen(req, timeout=60).read())
    PY

Usage:
    python3 tools/make_coastline.py            # regenerate the asset
    python3 tools/make_coastline.py --check    # gate: fail if the checked-in asset is stale
    python3 tools/make_coastline.py --target-points 3000 --out /tmp/coast.bin
"""
from __future__ import annotations

import argparse
import json
import pathlib
import struct
import sys

# Windows consoles and redirected logs default to the ANSI code page (defect #37); every tool
# reconfigures its streams (tools/README.md -> 'Text I/O portability'; check_text_io.py gate).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

ENCODING = "utf-8"
ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tools" / "data" / "ne_110m_land.geojson"
DEFAULT_OUT = ROOT / "instruments-src" / "observatory" / "assets" / "coastline-110m.bin"

MAGIC = 0x53505251  # "SPRQ"
VERSION = 1
DEFAULT_TARGET_POINTS = 3500  # plan D12: <= ~3500 points


def read_text(path) -> str:
    return pathlib.Path(path).read_text(encoding=ENCODING)


# ── Visvalingam-Whyatt (area-based polyline simplification) ─────────────────────────────────


def _tri_area(a, b, c) -> float:
    """Triangle area (x2) of three (lon, lat) points — the VW metric, in degree^2."""
    return abs((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]))


def vw_simplify(ring: list[tuple[float, float]], min_area: float) -> list[tuple[float, float]]:
    """Visvalingam-Whyatt: repeatedly drop the point whose removal loses the least area,
    until every remaining point's effective area is >= min_area. Deterministic: ties break to
    the lowest index. Open-ring semantics (endpoints are never dropped)."""
    if len(ring) <= 3:
        return list(ring)
    pts = list(ring)
    while len(pts) > 3:
        n = len(pts)
        # Triangle areas of the interior points; the two endpoints carry +inf sentinels (they
        # are never dropped — a ring must stay a ring).
        areas = [float("inf")] * n
        for i in range(1, n - 1):
            areas[i] = _tri_area(pts[i - 1], pts[i], pts[i + 1])
        # The VW monotonic fix-up (Visvalingam's own rule): an effective area never reads below
        # its left neighbour's, so removing a flat-ish point never re-introduces a smaller area
        # later in the sweep. Interior points only — the sentinels must not poison the pass.
        for i in range(2, n - 1):
            if areas[i] < areas[i - 1]:
                areas[i] = areas[i - 1]
        # Find the minimum-area interior point (lowest index on ties).
        best, best_area = -1, float("inf")
        for i in range(1, n - 1):
            if areas[i] < best_area:
                best, best_area = i, areas[i]
        if best < 0 or best_area >= min_area:
            break
        del pts[best]
    return pts


def dedupe(ring: list[tuple[float, float]]) -> list[tuple[float, float]]:
    """Drop consecutive duplicate points (and the explicit closing point of a polygon ring)."""
    out: list[tuple[float, float]] = []
    for p in ring:
        if not out or p != out[-1]:
            out.append(p)
    while len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out


# ── input → rings ──────────────────────────────────────────────────────────────────────────


def extract_rings(geojson: dict) -> list[list[tuple[float, float]]]:
    """All polygon rings from the GeoJSON, in file order, as (lon, lat) float pairs."""
    rings: list[list[tuple[float, float]]] = []

    def walk(coords, kind):
        if kind == "Polygon":
            for ring in coords:
                rings.append([(float(p[0]), float(p[1])) for p in ring])
        elif kind == "MultiPolygon":
            for poly in coords:
                for ring in poly:
                    rings.append([(float(p[0]), float(p[1])) for p in ring])

    for feat in geojson.get("features", []):
        geom = feat.get("geometry")
        if not geom:
            continue
        walk(geom["coordinates"], geom["type"])
    return [dedupe(r) for r in rings]


# ── rings → binary ─────────────────────────────────────────────────────────────────────────


def encode(rings: list[list[tuple[float, float]]]) -> bytes:
    """The binary asset: header + per-ring (count, i16 centidegree lon/lat pairs), LE."""
    out = bytearray(struct.pack("<III", MAGIC, VERSION, len(rings)))
    for ring in rings:
        out += struct.pack("<I", len(ring))
        for lon, lat in ring:
            lon_cd = max(-18000, min(18000, round(lon * 100.0)))
            lat_cd = max(-9000, min(9000, round(lat * 100.0)))
            out += struct.pack("<hh", lon_cd, lat_cd)
    return bytes(out)


def build(target_points: int) -> tuple[bytes, int, int]:
    """Source GeoJSON → (asset bytes, ring count, point count). One shared VW threshold,
    binary-searched so the total point count lands at or just under the target."""
    if not SOURCE.is_file():
        raise SystemExit(
            f"{SOURCE}: Natural Earth 110m land GeoJSON not found — fetch it with the recipe\n"
            "in this tool's docstring (public-domain data, cached in tools/data/)."
        )
    rings = extract_rings(json.loads(read_text(SOURCE)))
    if not rings:
        raise SystemExit(f"{SOURCE}: no polygon rings found — is this the ne_110m_land file?")

    def simplify_all(min_area: float) -> list[list[tuple[float, float]]]:
        out = []
        for ring in rings:
            s = vw_simplify(ring, min_area)
            if len(s) >= 3:  # a ring that degenerates below a triangle is not drawable
                out.append(s)
        return out

    # Binary-search the area threshold (degree^2) for the largest simplification that still
    # fits the point target. Deterministic: fixed iteration count, no early exit on ties.
    # 32 halvings of [0, 64] reach ~1.5e-8 deg^2 — far below the geometry's own area quantum.
    lo, hi = 0.0, 64.0
    result = simplify_all(0.0)
    for _ in range(32):
        mid = (lo + hi) / 2.0
        cand = simplify_all(mid)
        total = sum(len(r) for r in cand)
        if total > target_points:
            lo = mid
        else:
            hi = mid
            result = cand
    total = sum(len(r) for r in result)
    if total > target_points:
        raise SystemExit(
            f"could not simplify to <= {target_points} points (best effort: {total}) — "
            "raise --target-points or check the source file"
        )
    return encode(result), len(result), total


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate the Observatory coastline asset (WO-020 D12).")
    ap.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT, help="output asset path")
    ap.add_argument("--target-points", type=int, default=DEFAULT_TARGET_POINTS, help="point budget")
    ap.add_argument("--check", action="store_true", help="fail if the checked-in asset is stale")
    args = ap.parse_args(argv)

    data, n_rings, n_points = build(args.target_points)

    if args.check:
        if not args.out.is_file():
            print(f"make_coastline --check: FAIL — {args.out} does not exist (generate it first)")
            return 1
        current = args.out.read_bytes()
        if current != data:
            print(
                f"make_coastline --check: FAIL — {args.out} is stale\n"
                f"  checked in: {len(current)} bytes; regenerated: {len(data)} bytes\n"
                "  fix: python3 tools/make_coastline.py && commit the asset"
            )
            return 1
        print(
            f"make_coastline --check: up to date ({args.out.name}: {n_rings} rings, "
            f"{n_points} points, {len(data)} bytes)"
        )
        return 0

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(data)  # binary I/O — encoding-independent (check_text_io rule)
    print(
        f"make_coastline: wrote {args.out.relative_to(ROOT) if args.out.is_relative_to(ROOT) else args.out} "
        f"— {n_rings} rings, {n_points} points (target <= {args.target_points}), {len(data)} bytes "
        f"(budget <= 163840)"
    )
    if len(data) > 163_840:
        print("make_coastline: FAIL — asset exceeds the 160 KB embed budget (plan D12)")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
