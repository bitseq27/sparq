#!/usr/bin/env python3
"""
tools/streams_record.py — the WO-020 stream recorder (INC1).

Records the live payload of every stream in `crates/sparq-streams/streams.toml` into
`reference/fixtures/observatory/<stream-id>.<ext>` (the RAW payload, verbatim — so a fixture is
byte-faithful to the live response and the ≤ 1.2 × recorded-size acceptance holds at ratio ~1.0),
with the fetch metadata in a single `_meta.json` sidecar, and writes a probe log
(`PROBE-LOG-<date>.md`) that is the machine sibling of the plan's §3 table: one row per stream with
its real HTTP status, byte size, and fetch time.

Why this exists (plan §3, §12.6, §15.1 step 4): the sandbox has python and a proven network, so it
records fixtures ONCE, while the network is up, and every later increment is hermetic — normalizers,
windows, replay, goldens and tests all run from these files, never from a socket. A stream catalogue
rots (the DONKI lesson, §3.3); re-running this file re-probes and re-records in one command, so API
drift is a recorded fact, not a surprise at draw time.

The registry is the single source of truth: this script reads `streams.toml` with the stdlib
`tomllib` (Python ≥ 3.11) and never re-types an endpoint.

Fixture layout (so `replay.rs` reads raw payloads and the size acceptance holds at ratio ~1.0):
  * `<stream-id>.<ext>` — the RAW payload, verbatim, where `<ext>` is `.json` / `.txt` / `.csv`
    chosen from the response content type. This file IS the recorded bytes.
  * `_meta.json` — one sidecar object mapping each stream id to its fetch metadata:
        {
          "swpc.wwv": {
            "file":          "swpc.wwv.txt",
            "endpoint":      "<the resolved URL that was fetched>",
            "fetched_utc":   "2026-10-06T12:34:56Z",
            "fetched_unix":  1780000000,
            "status":        200,            # 0 = transport error / not attempted
            "content_type":  "text/plain",
            "byte_length":   527,
            "key_source":    "none",         # none | fallback:DEMO_KEY | env:SPARQ_...  (NEVER the secret)
            "synthetic":     false,          # true only for the keyless-FIRMS sample (see below)
            "note":          ""              # transport error text, or a synthetic-sample note
          }, ...
        }

Keys (plan D14, §11.4): NASA endpoints run on DEMO_KEY (or SPARQ_NASA_API_KEY if set). FIRMS needs
SPARQ_FIRMS_KEY (free registration); with no key it cannot be fetched, so this script records a
SMALL SYNTHETIC FIRMS CSV sample — clearly flagged `"synthetic": true` in the metadata and in the
probe log — so the FIRMS normalizer is still testable. A real key replaces the sample on the next
run. This script never writes a secret into any file: the metadata records only WHICH key source was
used, never its value.

Etiquette (plan §11): one descriptive User-Agent on every request, a polite delay between hosts, at
most one retry on 429/5xx, and cadences are the broker's concern (this records once).

Usage:
    python3 tools/streams_record.py --all --out reference/fixtures/observatory/
    python3 tools/streams_record.py --only swpc.kp,sat.iss --out /tmp/fx
    python3 tools/streams_record.py --list          # print the registry and exit (no network)

Environment (all optional; defaults match the plan §6.2):
    SPARQ_NASA_API_KEY          raises NASA streams off DEMO_KEY
    SPARQ_FIRMS_KEY             lights geo.firms (else a synthetic sample is recorded)
    SPARQ_OBSERVATORY_LOC       "lat,lon" for wx.* and space.power (default 51.5,-0.1)
    SPARQ_OBSERVATORY_TIDE_STATION   CO-OPS station for wx.tides (default 9447130)
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import sys
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

# Windows consoles and redirected logs default to the ANSI code page: printing '·'/'—'/'→' on a
# cp1252 stream raises UnicodeEncodeError mid-report (defect #37). The house rule (tools/README.md
# -> 'Text I/O portability', enforced by tools/check_text_io.py).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

# Every text read/write goes through these helpers so the tool behaves identically on Windows,
# where the default encoding is the ANSI code page and text writes translate "\n" to "\r\n"
# (tools/README.md -> "Text I/O portability"; the `tools-text-io` gate enforces it).
ENCODING = "utf-8"


def read_text(path) -> str:
    return Path(path).read_text(encoding=ENCODING)


def write_text(path, content: str) -> None:
    pp = Path(path)
    pp.parent.mkdir(parents=True, exist_ok=True)
    with pp.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


# The crate's UA, mirrored so the recorder and the live transport identify themselves identically
# (§11.2). NWS *requires* a descriptive UA; the others ask for attribution.
USER_AGENT = "sparq-observatory/0.0.0-phase0 (+https://github.com/bitseq27/sparq; WO-020 recorder)"

REPO_ROOT = Path(__file__).resolve().parent.parent
REGISTRY = REPO_ROOT / "crates" / "sparq-streams" / "streams.toml"
DEFAULT_OUT = REPO_ROOT / "reference" / "fixtures" / "observatory"

# A small, clearly-synthetic FIRMS VIIRS CSV sample (the documented column order), recorded only
# when SPARQ_FIRMS_KEY is unset so the FIRMS normalizer has something real-shaped to parse. It is
# flagged synthetic everywhere; a real key overwrites it on the next run.
FIRMS_SYNTHETIC_CSV = (
    "latitude,longitude,brightness,scan,track,acq_date,acq_time,satellite,instrument,"
    "confidence,version,bright_t31,frp,daynight\n"
    "34.052,-118.243,318.4,1.0,1.0,{d},1432,N,suomi-npp-viirs-c2,n,1,288.0,14.7,day\n"
    "35.681,-105.938,305.9,1.2,1.1,{d},1433,N,suomi-npp-viirs-c2,n,1,281.5,9.2,night\n"
    "40.713,-111.891,342.1,0.9,1.0,{d},1433,N,suomi-npp-viirs-c2,h,1,295.0,41.3,day\n"
)


def load_registry(path: Path) -> list[dict]:
    """Parse the [[stream]] rows of streams.toml (the single source of truth)."""
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    rows = data.get("stream", [])
    if not rows:
        raise SystemExit(f"{path}: no [[stream]] rows — refusing to record an empty registry")
    return rows


def resolve_placeholders(endpoint: str, stream: dict, now: _dt.datetime) -> tuple[str, str]:
    """Fill the endpoint template. Returns (url, key_source). key_source is 'none',
    'fallback:<value>' or 'env:<VAR>' — never a bare secret."""
    lat, lon = os.environ.get("SPARQ_OBSERVATORY_LOC", "51.5,-0.1").split(",")[:2]
    station = os.environ.get(
        "SPARQ_OBSERVATORY_TIDE_STATION", stream.get("default_tide_station", "9447130")
    )
    country = stream.get("default_country", "USA")

    def day(offset: int) -> str:
        return (now - _dt.timedelta(days=offset)).strftime("%Y-%m-%d")

    url = endpoint
    url = url.replace("{lat}", lat.strip()).replace("{lon}", lon.strip())
    url = url.replace("{st}", station.strip()).replace("{CC}", country)
    # Lowercase {d…} -> YYYY-MM-DD (ISO, what GDACS/NEO/CO-OPS want).
    for token, off in (("{d-30}", 30), ("{d-7}", 7), ("{d-3}", 3), ("{d-1}", 1), ("{d}", 0)):
        url = url.replace(token, day(off))
    # Uppercase {D…} -> YYYYMMDD (compact integer, what NASA POWER now requires — it 422s on the
    # dashed form: "Input should be a valid integer". API drift recorded 2026-10-06; plan §12.6.)
    for token, off in (("{D-30}", 30), ("{D-7}", 7), ("{D-3}", 3), ("{D-1}", 1), ("{D}", 0)):
        url = url.replace(token, (now - _dt.timedelta(days=off)).strftime("%Y%m%d"))

    # The key: env value if set, else the declared fallback (NASA's DEMO_KEY), else none.
    key_source = "none"
    key_env = stream.get("key_env")
    if key_env:
        env_val = os.environ.get(key_env, "").strip()
        if env_val:
            url = url.replace("{KEY}", env_val)
            key_source = f"env:{key_env}"
        elif stream.get("key_fallback"):
            url = url.replace("{KEY}", stream["key_fallback"])
            key_source = f"fallback:{stream['key_fallback']}"
        else:
            key_source = "KEY_NEEDED"
    return url, key_source


def fetch(url: str, timeout: float = 20.0) -> tuple[int, str, str, str]:
    """GET url once (one retry on 429/5xx). Returns (status, content_type, body_text, note)."""
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "*/*"})
    last_note = ""
    for attempt in (0, 1):
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                raw = resp.read()
                ctype = resp.headers.get("Content-Type", "")
                # Decode as UTF-8, replacing undecodable bytes so a binary-ish payload still records.
                return resp.status, ctype, raw.decode("utf-8", "replace"), ""
        except urllib.error.HTTPError as e:
            status = e.code
            body = ""
            try:
                body = e.read().decode("utf-8", "replace")
            except Exception:  # noqa: BLE001 - a failed error-body read is not fatal to the record
                body = ""
            if status in (429, 500, 502, 503, 504) and attempt == 0:
                last_note = f"HTTP {status}, retrying once"
                time.sleep(1.5)
                continue
            return status, e.headers.get("Content-Type", "") if e.headers else "", body, f"HTTP {status}"
        except urllib.error.URLError as e:
            return 0, "", "", f"transport error: {e.reason}"
        except Exception as e:  # noqa: BLE001 - record any other failure honestly, move on
            return 0, "", "", f"error: {type(e).__name__}: {e}"
    return 0, "", "", last_note or "unknown failure"


def record_stream(stream: dict, now: _dt.datetime, timeout: float) -> dict:
    """Fetch (or synthesise) one stream and return its in-memory record (metadata + raw body)."""
    sid = stream["id"]
    url, key_source = resolve_placeholders(stream["endpoint"], stream, now)
    envelope = {
        "stream_id": sid,
        "endpoint": url,
        "fetched_utc": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "fetched_unix": int(now.timestamp()),
        "status": 0,
        "content_type": "",
        "byte_length": 0,
        "key_source": key_source,
        "synthetic": False,
        "note": "",
        "body": "",
    }

    if key_source == "KEY_NEEDED":
        # FIRMS without a key: record the synthetic sample, flagged, rather than a silent hole.
        if sid == "geo.firms":
            body = FIRMS_SYNTHETIC_CSV.replace("{d}", now.strftime("%Y-%m-%d"))
            envelope.update(
                status=200, content_type="text/csv", byte_length=len(body.encode()),
                synthetic=True,
                note="SYNTHETIC sample: SPARQ_FIRMS_KEY unset (free registration lights the live "
                     "feed). Column order per the FIRMS VIIRS CSV API; values are illustrative.",
                body=body,
            )
        else:
            envelope["note"] = f"KEY NEEDED: {stream.get('key_env')} unset and no fallback — not fetched (plan D14)"
        return envelope

    status, ctype, body, note = fetch(url, timeout)
    envelope.update(status=status, content_type=ctype, byte_length=len(body.encode()), note=note, body=body)
    return envelope


def ext_for(content_type: str, body: str) -> str:
    """Pick a faithful file extension from the content type (raw payloads are stored verbatim, so
    the file IS the recorded bytes — the fixture-size acceptance is ratio ~1.0)."""
    ct = (content_type or "").lower()
    if "csv" in ct:
        return ".csv"
    if "json" in ct:
        return ".json"
    if "text" in ct:
        return ".txt"
    # Fall back to sniffing: a body that parses as JSON is .json, else .txt.
    try:
        json.loads(body)
        return ".json"
    except Exception:  # noqa: BLE001 - a non-JSON body is text
        return ".txt"


def write_probe_log(rows: list[dict], envelopes: dict[str, dict], path: Path, now: _dt.datetime) -> None:
    """Write the machine sibling of the plan's §3 table: id, status, bytes, content type, note."""
    lines = [
        f"# Stream probe log — recorded {now.strftime('%Y-%m-%d %H:%M:%S UTC')}",
        "",
        "Machine sibling of `WO020-OBSERVATORY-PLAN.md` §3.1, produced by `tools/streams_record.py`.",
        "Every row is a real fetch from the recording sandbox (or a flagged synthetic sample where a",
        "free key was unavailable). Re-run the recorder to re-probe; the fixtures and this log move",
        "together. `status 0` = transport error (see note); `KEY NEEDED` = not fetched (plan D14).",
        "",
        "| # | stream id | dom | status | bytes | content-type | key source | note |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for i, row in enumerate(rows, 1):
        sid = row["id"]
        e = envelopes.get(sid, {})
        note = (e.get("note", "") or "").replace("\n", " ").replace("|", "/")
        if e.get("synthetic"):
            note = "SYNTHETIC — " + note
        lines.append(
            f"| {i} | `{sid}` | {row.get('domain','')} | {e.get('status','?')} | "
            f"{e.get('byte_length','?')} | {e.get('content_type','') or '—'} | "
            f"{e.get('key_source','')} | {note or '—'} |"
        )
    ok = sum(1 for e in envelopes.values() if e.get("status") == 200)
    lines += [
        "",
        f"**{ok}/{len(rows)} streams recorded with HTTP 200** (synthetic samples count as recorded).",
        "Fixtures ride `reference/fixtures/observatory/` as raw payloads + a `_meta.json` sidecar;",
        "the broker's replay door (`crates/sparq-streams/src/replay.rs`) reads them, and the",
        "normalizer tests assert against them, so no downstream test touches the network (plan D11).",
        "",
    ]
    write_text(path, "\n".join(lines))


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Record sparq stream fixtures (WO-020 INC1).")
    ap.add_argument("--all", action="store_true", help="record every stream in the registry")
    ap.add_argument("--only", metavar="IDS", help="comma-separated stream ids to record")
    ap.add_argument("--list", action="store_true", help="print the registry and exit (no network)")
    ap.add_argument("--out", default=str(DEFAULT_OUT), help="fixture output directory")
    ap.add_argument("--timeout", type=float, default=20.0, help="per-request timeout (seconds)")
    ap.add_argument("--delay", type=float, default=0.4, help="polite delay between requests (seconds)")
    ap.add_argument("--no-probe-log", action="store_true", help="skip writing PROBE-LOG-<date>.md")
    args = ap.parse_args(argv)

    rows = load_registry(REGISTRY)

    if args.list:
        print(f"{len(rows)} streams in {REGISTRY.relative_to(REPO_ROOT)}:")
        for r in rows:
            key = f" [key:{r['key_env']}]" if r.get("key_env") else ""
            print(f"  {r['id']:<20} {r['domain']:<4} {r['cadence_s']:>5}s  {r['schema']:<26}{key}")
        return 0

    if args.only:
        want = {s.strip() for s in args.only.split(",") if s.strip()}
        rows = [r for r in rows if r["id"] in want]
        missing = want - {r["id"] for r in rows}
        if missing:
            print(f"streams_record: unknown id(s): {', '.join(sorted(missing))}", file=sys.stderr)
            return 2
    elif not args.all:
        print("streams_record: pass --all, --only IDS, or --list", file=sys.stderr)
        return 2

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    now = _dt.datetime.now(_dt.timezone.utc)

    metas: dict[str, dict] = {}
    for i, row in enumerate(rows):
        sid = row["id"]
        env = record_stream(row, now, args.timeout)
        # Store the RAW payload verbatim as its own file (so the fixture IS the recorded bytes —
        # the ≤1.2× acceptance is ratio ~1.0), and keep the fetch metadata in a single `_meta.json`
        # sidecar that `replay.rs` reads. No per-stream envelope, no JSON-escaping bloat.
        body = env.pop("body", "")
        ext = ext_for(env.get("content_type", ""), body)
        filename = f"{sid}{ext}"
        write_text(out / filename, body)
        env["file"] = filename
        metas[sid] = env
        flag = "SYNTHETIC" if env.get("synthetic") else ("OK" if env["status"] == 200 else "FAIL")
        print(
            f"  [{i+1:>2}/{len(rows)}] {sid:<20} {env['status']:>3}  {env['byte_length']:>8} B  {filename:<26} {flag}"
            + (f"  ({env['note']})" if env["note"] and env["status"] != 200 else "")
        )
        if i + 1 < len(rows):
            time.sleep(args.delay)

    write_text(out / "_meta.json", json.dumps(metas, ensure_ascii=False, indent=2))

    if not args.no_probe_log:
        log_path = out / f"PROBE-LOG-{now.strftime('%Y-%m-%d')}.md"
        write_probe_log(rows, metas, log_path, now)
        print(f"\nprobe log -> {log_path.relative_to(REPO_ROOT) if log_path.is_relative_to(REPO_ROOT) else log_path}")

    ok = sum(1 for e in metas.values() if e["status"] == 200)
    print(f"recorded {ok}/{len(rows)} streams to {out} (+ _meta.json sidecar)")
    return 0 if ok == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
