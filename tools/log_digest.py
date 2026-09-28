#!/usr/bin/env python3
"""log_digest.py - compact copies of the sparq device logs, for sending back without the bulk.

Why this tool exists (2026-09-28, test004 attempt 2)
----------------------------------------------------
The acceptance logs are the evidence, and the evidence is long: a 2 h soak prints 240 periodic
reports, and a cold build prints every crate it compiles. test004.log reached ~67 KB of which the
bulk was the SAME status line repeated with two numbers moving - and pasting it into a session
drowned the dozen lines that actually decide the run. The operator asked for a non-verbose copy;
this is it.

The digest is a COPY, not a rewrite: every line the source has is reproduced byte-identical
except inside runs of exactly three classes:

  1. PERIODIC report runs - the `[t+ ...]` status lines of `play --seconds` (every 2 s) and
    `soak --report-every` (every 30 s). A run of 5+ keeps its FIRST 2 and LAST 2 lines; the
    middle becomes ONE count line carrying the blocks/xrun ranges, so the trend and the deltas
    survive the collapse.
  2. CARGO build noise - `Compiling`/`Downloaded`/`Updating`/... runs of 3+ become one count
    line. Verdict-carrying lines (`Finished`, `error`, `warning`) never match the class.
  3. CONSECUTIVE IDENTICAL lines - a run of 4+ keeps the first and counts the rest.

Nothing else is touched: the run banners, [RUN]/[RC] markers, SUMMARY blocks, the open and
NEGOTIATED lines, probe tables, HRESULTs, PASS/FAIL/NOT CLEAN verdicts, operator answers, hint
text and the device listings all pass through as they are. The digest header records what was
collapsed and how much, so a reader always knows what they are NOT seeing, and the full log
stays on disk - the digest is what gets SENT. If a diagnosis ever needs a line the digest
collapsed, the source file is one `type` away (and the periodic class keeps both ends of every
run precisely so the soak's start and end state are always in the copy).

Usage:
    python tools\\log_digest.py test004.log            # -> test004-digest.log beside it
    python tools\\log_digest.py --stdout test004.log    # print the digest instead of writing
    python tools\\log_digest.py --self-test             # prove every class fires, no logs needed

Exit codes: 0 digest(s) written / self-test passed - 1 a file was unreadable or the self-test
failed - 2 nothing to do (usage).
"""
from __future__ import annotations

import argparse
import datetime
import pathlib
import re
import sys

ENCODING = "utf-8"

# Rule 2 of tools/check_text_io.py (defect #37): reconfigure, or a tool that prints a middle dot
# on a cp1252 console dies mid-report and takes its findings with it.
for _stream in (sys.stdout, sys.stderr):
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding=ENCODING, errors="replace")


def read_text(path: pathlib.Path) -> str:
    """Read a log, whatever Windows wrote it as (log_check.py's reader).

    `cmd` redirection produces the console code page (cp850/cp1252 unless the script ran
    `chcp 65001`, which the test scripts do - then UTF-8), PowerShell's tee produces UTF-8 or
    UTF-16LE, and trying them in order is cheaper than asking the operator which shell they used.
    """
    raw = path.read_bytes()
    for enc in ("utf-8-sig", "utf-16", ENCODING, "cp1252", "cp850"):
        try:
            return raw.decode(enc)
        except (UnicodeDecodeError, UnicodeError):
            continue
    return raw.decode("latin-1", errors="replace")


def write_text(path: pathlib.Path, text: str) -> None:
    """Write UTF-8 with LF newlines - the committed-artefact convention (defect #19)."""
    path.write_text(text, encoding=ENCODING, newline="\n")


# ---------------------------------------------------------------- the three classes
# Periodic reports: `[t+  2.0s] blocks ...` (play) and `[t+    30s] blocks ...` (soak). The
# shape is sparq's own status printer; matching on the timestamp bracket keeps the class off
# every other line in the log.
PERIODIC = re.compile(r"^\s*\[t\+\s*[0-9.]+\s*s\]")
# Cargo's progress chatter. Deliberately NOT matching `Finished`, `error`, `warning`, or the
# stamp-guard lines - only the lines that name a crate being fetched/built.
CARGO = re.compile(r"^\s*(?:Compiling|Downloaded|Downloading|Updating|Removing|Fetching|Adding|Locking)\s")
BLOCKS = re.compile(r"blocks\s+(\d+)")
XRUN = re.compile(r"xrun\s+(\d+)")

MIN_PERIODIC_RUN = 5  # runs shorter than this stay whole - two status lines are context, not bulk
KEEP_PERIODIC_EDGE = 2  # first 2 + last 2 of every collapsed run survive
MIN_CARGO_RUN = 3
MIN_DUP_RUN = 4


def _periodic_marker(run: list[str]) -> str:
    """The count line replacing a collapsed periodic run: how many went away, and the blocks /
    xrun range between the first and last survivor, so the run's direction survives."""

    def _grab(pattern: re.Pattern[str], line: str) -> str | None:
        m = pattern.search(line)
        return m.group(1) if m else None

    first, last = run[0], run[-1]
    parts = [f"{len(run) - 2 * KEEP_PERIODIC_EDGE} similar periodic reports collapsed"]
    b_first, b_last = _grab(BLOCKS, first), _grab(BLOCKS, last)
    x_first, x_last = _grab(XRUN, first), _grab(XRUN, last)
    if b_first is not None and b_last is not None:
        parts.append(f"blocks {b_first}->{b_last}")
    if x_first is not None and x_last is not None:
        parts.append(f"xrun {x_first}->{x_last}")
    # ASCII marker on purpose: the digest may be `type`d on any console code page.
    return "  ---- digest: " + "; ".join(parts) + " ----"


def digest_lines(lines: list[str]) -> tuple[list[str], dict[str, int]]:
    """Collapse the three classes; everything else passes through byte-identical."""
    out: list[str] = []
    stats = {"periodic": 0, "cargo": 0, "duplicate": 0}
    i, n = 0, len(lines)
    while i < n:
        line = lines[i]
        if PERIODIC.match(line):
            j = i
            while j < n and PERIODIC.match(lines[j]):
                j += 1
            run = lines[i:j]
            if len(run) >= MIN_PERIODIC_RUN:
                stats["periodic"] += len(run) - 2 * KEEP_PERIODIC_EDGE
                out.extend(run[:KEEP_PERIODIC_EDGE])
                out.append(_periodic_marker(run))
                out.extend(run[-KEEP_PERIODIC_EDGE:])
            else:
                out.extend(run)
            i = j
            continue
        if CARGO.match(line):
            j = i
            while j < n and CARGO.match(lines[j]):
                j += 1
            run = lines[i:j]
            if len(run) >= MIN_CARGO_RUN:
                stats["cargo"] += len(run)
                out.append(f"  ---- digest: {len(run)} cargo build lines collapsed ----")
            else:
                out.extend(run)
            i = j
            continue
        if line.strip():
            j = i + 1
            while j < n and lines[j] == line:
                j += 1
            if j - i >= MIN_DUP_RUN:
                stats["duplicate"] += j - i - 1
                out.append(line)
                out.append(f"  ---- digest: {j - i - 1} identical lines collapsed ----")
                i = j
                continue
        out.append(line)
        i += 1
    return out, stats


def make_digest(text: str, source_name: str) -> str:
    """The full digest file text: an honest header, the collapsed body, a closing line."""
    lines = text.splitlines()
    body, stats = digest_lines(lines)
    today = datetime.date.today().isoformat()
    header = [
        f"==== DIGEST of {source_name} - tools/log_digest.py {today} ====",
        "  source {} lines -> digest {} body lines; collapsed: {} periodic, {} cargo, "
        "{} duplicate".format(len(lines), len(body), stats["periodic"], stats["cargo"],
                              stats["duplicate"]),
        "  every line outside a collapsed run is reproduced byte-identical; collapsed periodic",
        "  runs keep their first 2 and last 2 lines. The full log stays on disk beside this file.",
        " ",
    ]
    footer = [" ", f"==== END DIGEST of {source_name} (source had {len(lines)} lines) ===="]
    return "\n".join(header + body + footer) + "\n"


# ---------------------------------------------------------------- self-test
FIXTURE = "\n".join(
    [
        "===== testXXX RUN 01/01/2026 00:00:00.00 on BOX user op =====",
        "---- [RUN] build via scripts\\build.bat ----",
        "   Compiling smallvec v1.16.2",
        "   Compiling zerocopy v0.8.59",
        "  Downloaded harfrust v0.12.0",
        "    Finished `release` profile [optimized] target(s) in 1m 03s",
        "---- [RC 0] build via scripts\\build.bat ----",
        "---- [RUN] play short ----",
        "  [t+  2.0s] blocks 3097 \u00b7 xrun 0 \u00b7 p99 4.1 \u00b5s",
        "  [t+  4.0s] blocks 6112 \u00b7 xrun 0 \u00b7 p99 4.1 \u00b5s",
        "  final state Stopped",
        "---- [RC 0] play short ----",
        "---- [RUN] play long ----",
        "  [t+  2.0s] blocks 1590 \u00b7 xrun 32 \u00b7 p99 4.1 \u00b5s",
        "  [t+  4.0s] blocks 3133 \u00b7 xrun 64 \u00b7 p99 4.1 \u00b5s",
        "  [t+  6.0s] blocks 4672 \u00b7 xrun 96 \u00b7 p99 4.1 \u00b5s",
        "  [t+  8.1s] blocks 6216 \u00b7 xrun 128 \u00b7 p99 4.1 \u00b5s",
        "  [t+ 10.1s] blocks 7732 \u00b7 xrun 159 \u00b7 p99 4.1 \u00b5s",
        "  [t+ 12.1s] blocks 9271 \u00b7 xrun 191 \u00b7 p99 4.1 \u00b5s",
        "  [t+ 14.1s] blocks 10810 \u00b7 xrun 223 \u00b7 p99 4.1 \u00b5s",
        "",
        "  final state Stopped",
        "play: NOT CLEAN \u2014 xruns 223, allocs 0, dev-err 0, state Stopped.",
        "DUPLICATE LINE",
        "DUPLICATE LINE",
        "DUPLICATE LINE",
        "DUPLICATE LINE",
        "SUMMARY - testXXX",
        " [04] play EXCLUSIVE : FAIL rc=1",
    ]
)


def self_test() -> int:
    """Prove every verdict fires: each class collapses, each guard keeps, and everything else
    is byte-identical. Exact-match assertions - a regex that silently stops matching fails here,
    not on the operator's next acceptance run."""
    failures: list[str] = []
    checks = 0
    digest = make_digest(FIXTURE, "fixture.log")
    lines = digest.splitlines()

    def check(name: str, ok: bool, detail: str = "") -> None:
        nonlocal checks
        checks += 1
        if not ok:
            failures.append(f"{name} {detail}".rstrip())

    # 1. the periodic run of 7 collapses to 2 + marker + 2, with the ranges carried
    check(
        "periodic marker",
        any(
            "digest: 3 similar periodic reports collapsed; blocks 1590->10810; xrun 32->223" in ln
            for ln in lines
        ),
        "- the collapsed run must report its count and the blocks/xrun ranges",
    )
    kept_periodic = [ln for ln in lines if PERIODIC.match(ln)]
    check(
        "periodic edges kept",
        len(kept_periodic) == 6,
        f"- expected the 2-line short run plus first 2 and last 2 of the long run, got {len(kept_periodic)}",
    )
    check(
        "short periodic run untouched",
        "  [t+  2.0s] blocks 3097 \u00b7 xrun 0 \u00b7 p99 4.1 \u00b5s" in lines
        and "  [t+  4.0s] blocks 6112 \u00b7 xrun 0 \u00b7 p99 4.1 \u00b5s" in lines,
        "- a 2-line run is context, not bulk",
    )
    # 2. cargo noise collapses; the verdict line survives
    check(
        "cargo marker",
        any("digest: 3 cargo build lines collapsed" in ln for ln in lines),
        "- the Compiling/Downloaded run must collapse",
    )
    check(
        "cargo verdict kept",
        any(ln.strip().startswith("Finished") for ln in lines),
        "- `Finished` never matches the noise class",
    )
    # 3. identical runs collapse to one + marker
    check(
        "duplicate marker",
        any("digest: 3 identical lines collapsed" in ln for ln in lines),
        "- four identical lines keep one and count three",
    )
    check(
        "duplicate survivor",
        sum(1 for ln in lines if ln == "DUPLICATE LINE") == 1,
        "- exactly one original survives",
    )
    # 4. everything verdict-shaped passes through byte-identical
    for must in (
        "===== testXXX RUN 01/01/2026 00:00:00.00 on BOX user op =====",
        "---- [RUN] build via scripts\\build.bat ----",
        "---- [RC 0] build via scripts\\build.bat ----",
        "  final state Stopped",
        "play: NOT CLEAN \u2014 xruns 223, allocs 0, dev-err 0, state Stopped.",
        "SUMMARY - testXXX",
        " [04] play EXCLUSIVE : FAIL rc=1",
    ):
        check(f"passthrough {must[:40]!r}", must in lines, "- verdict lines are never collapsed")
    # 5. the header tells the truth about the counts
    check(
        "header counts",
        any("collapsed: 3 periodic, 3 cargo, 3 duplicate" in ln for ln in lines),
        "- the header must state exactly what went away",
    )
    check(
        "utf-8 body intact",
        any("\u00b7" in ln and "\u00b5s" in ln for ln in lines) and any("\u2014" in ln for ln in lines),
        "- middot, micro sign and em dash must survive the round trip",
    )

    if failures:
        print("log_digest --self-test: FAIL")
        for f in failures:
            print(f"  [FAIL] {f}")
        return 1
    print(f"log_digest --self-test: PASS ({checks} checks - 3 collapse classes, edges kept, passthrough intact)")
    return 0


# ---------------------------------------------------------------- cli
def main() -> int:
    ap = argparse.ArgumentParser(description="compact copy of a sparq device log")
    ap.add_argument("logs", nargs="*", help="log file(s) to digest")
    ap.add_argument("--stdout", action="store_true", help="print the digest instead of writing it")
    ap.add_argument("--self-test", action="store_true", help="prove every class fires, no logs needed")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.logs:
        print(__doc__)
        return 2

    rc = 0
    for name in args.logs:
        path = pathlib.Path(name)
        if not path.is_file():
            print(f"log_digest: no such file: {name}")
            rc = 1
            continue
        if path.stem.endswith("-digest"):
            # Idempotence guard: digesting a digest would overwrite it with a copy whose
            # header names the wrong source. The globbing wrapper (scripts\digest.bat) can
            # then sweep a directory twice without corrupting the first digest.
            print(f"log_digest: {path.name} is already a digest - skipped")
            continue
        try:
            text = read_text(path)
        except OSError as e:
            print(f"log_digest: cannot read {name}: {e}")
            rc = 1
            continue
        digest = make_digest(text, path.name)
        if args.stdout:
            sys.stdout.write(digest)
            continue
        out = path.with_name(f"{path.stem}-digest{path.suffix}")
        write_text(out, digest)
        src_bytes = max(1, path.stat().st_size)
        out_bytes = out.stat().st_size
        saved = 100.0 * (1.0 - out_bytes / src_bytes)
        print(
            f"log_digest: {path.name} -> {out.name} "
            f"({len(text.splitlines())} lines, {src_bytes} B -> {out_bytes} B, {saved:.0f}% smaller)"
        )
    return rc


if __name__ == "__main__":
    sys.exit(main())
