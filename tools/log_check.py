#!/usr/bin/env python3
"""log_check.py - compare a device log set against the measured sandbox baseline.

WO-006 and WO-012 both produce their verdicts as logs from a machine the sandbox is not, and every
increment since has compared them by eye. That is how a regression survives: four logs, twenty
numbers, and one of them quietly 40x worse than last time.

This tool does the comparing. It reads a directory of logs, extracts the numbers the gates actually
assert on, prints them next to the sandbox baseline, and exits non-zero if a HARD check fails.

    python3 tools/log_check.py                 # read ./logs
    python3 tools/log_check.py --dir Q:\\logs   # anywhere
    python3 tools/log_check.py --self-test     # prove the parser works, no logs needed

HARD checks are the ones where a difference means the run is not comparable at all: the build stamp
(a mismatch means a stale binary - defects #41 and #49), the golden hash, zero failed tests, zero
allocations on the audio path, zero xruns in the soak, zero outstanding after the reopen-leak cycles.

SOFT checks are numbers a different machine is *expected* to differ on: throughput, p99 block time,
frame time. They are reported as a ratio against the baseline and flagged only when the ratio is
implausible, because a stage device beating a 2-core VM is the point, not a defect.

DECISION checks are the ones that would invalidate a recorded choice. There is one: ADR-009 executor
decision 7 chose enum dispatch for the per-sample path because it measured within noise of
monomorphised code while trait objects cost 13-17x more. If a device run ever inverts that ordering,
the decision needs reopening - so the tool says so rather than printing two numbers and hoping
somebody notices.

Baseline provenance: measured 2026-09-22 on x86-64 Linux, a 2-core virtualised sandbox at ~1 GB RAM,
stamp `src 86f/1679258B`. Update BASELINE at the end of each increment; a stale baseline produces
confident nonsense, which is worse than no baseline.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ENCODING = "utf-8"

# Rule 2 of tools/check_text_io.py (defect #37): reconfigure, or a tool that prints a middle dot on a
# cp1252 console dies mid-report and takes its findings with it.
for _stream in (sys.stdout, sys.stderr):
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding=ENCODING, errors="replace")

ROOT = pathlib.Path(__file__).resolve().parent.parent


def read_text(path: pathlib.Path) -> str:
    """Read a log, whatever Windows wrote it as.

    `cmd` redirection produces the console code page (cp850/cp1252), PowerShell's `Out-File` defaults
    to UTF-16LE, and a tool that reconfigured its stdout produces UTF-8. Trying them in order is
    cheaper than asking the operator which shell they used, and the last resort never raises.
    """
    raw = path.read_bytes()
    for enc in ("utf-8-sig", "utf-16", ENCODING, "cp1252", "cp850"):
        try:
            return raw.decode(enc)
        except (UnicodeDecodeError, UnicodeError):
            continue
    return raw.decode("latin-1", errors="replace")


# ---------------------------------------------------------------- baseline
# Measured, not asserted. Every value here came from a gate run recorded in PHASE0-WORKORDERS.md
# section 2.1 (WO-014 increment 6, 2026-09-27, sandbox). This block MOVES WITH
# EVERY SEAL - defect #83 was exactly that nobody moved it for two increments, so the device's next
# gates run would have hard-failed a perfectly good tree against numbers three increments old.
BASELINE: dict[str, object] = {
    "stamp": "src 92f/1977302B",
    "golden_hash": "ba577186c988db21",
    "tests_failed": 0,
    "tests_passed": 742,  # workspace suite only - the release golden rerun is scoped out (#72)
    "selftest": "PASS",
    "ui_audit": "PASS",
    "allocations": 0,
    "reopen_leak_outstanding": 0,
    "soak_xruns": 0,
    "realtime_x": 309.5,
    # tools/dispatch-bench, medians over three runs (us per 100-module x 64-sample block)
    "dispatch_block_dyn_us": (1.65, 1.95),
    "dispatch_block_enum_us": (1.26, 1.30),
    "dispatch_sample_dyn_us": (27.1, 35.6),
    "dispatch_sample_enum_us": (2.08, 2.22),
}

# ---------------------------------------------------------------- extraction
# (field, log file, pattern, group). Patterns are deliberately loose about spacing: the logs are
# produced by several commands and have been reformatted more than once.
EXTRACT: list[tuple[str, str, str]] = [
    ("stamp", "build.log", r"src (\d+f/\d+B)"),
    ("stamp", "gates.log", r"src (\d+f/\d+B)"),
    ("golden_hash", "gates.log", r"hash ([0-9a-f]{16})"),
    ("selftest", "gates.log", r"selftest:\s*(PASS|FAIL)"),
    ("ui_audit", "gates.log", r"ui audit:\s*(PASS|FAIL)"),
    ("ui_audit", "ui.log", r"ui audit:\s*(PASS|FAIL)"),
    ("realtime_x", "gates.log", r"([\d.]+)\s*[x×]\s*realtime"),
    ("soak_xruns", "soak.log", r"(?:xrun|underrun)\s+(\d+)"),
    ("reopen_leak_outstanding", "gates.log", r"(\d+)\s*outstanding"),
    ("reopen_leak_outstanding", "hal.log", r"(\d+)\s*outstanding"),
    ("dispatch_block_dyn_us", "dispatch.log", r"Box<dyn Module>, block dispatch\s+([\d.]+)\s*us"),
    ("dispatch_block_enum_us", "dispatch.log", r"enum \+ match, block dispatch\s+([\d.]+)\s*us"),
    ("dispatch_sample_dyn_us", "dispatch.log", r"full per-sample graph cost: dyn ([\d.]+)\s*us"),
    ("dispatch_sample_enum_us", "dispatch.log", r"us/block vs enum ([\d.]+)\s*us"),
]

SUM_PASSED = re.compile(r"(\d+) passed; (\d+) failed; (\d+) ignored")

# gates.bat prints ` ---- <stage> ----` headers around every gate; the Linux `just gates`
# format has none. Both are handled by tests_section()'s fallback.
STAGE_HEADER = re.compile(r"^\s*----\s+(.*?)\s+----\s*$", re.MULTILINE)


def tests_section(gates: str) -> str:
    """The slice of gates.log produced by the workspace `tests` stage.

    A green run executes the golden tests TWICE: in debug inside `cargo test --workspace`, and
    again in the later release `golden reference` stage. Summing every "test result:" line in
    the log therefore counts two tests twice - the first fully green P1 run on SATURN read 390
    against the 388 baseline, a hard FAIL manufactured by the checker (defect #72). Scoped to
    the `tests` stage when the log carries stage headers; header-less logs (the Linux format,
    the self-test fixtures) fall back to the whole text, which is what the baseline measured.
    """
    headers = list(STAGE_HEADER.finditer(gates))
    for i, h in enumerate(headers):
        if h.group(1) == "tests":
            end = headers[i + 1].start() if i + 1 < len(headers) else len(gates)
            return gates[h.end():end]
    return gates


def extract(logdir: pathlib.Path) -> dict[str, object]:
    """Pull every recognisable number out of a log directory."""
    found: dict[str, object] = {}
    texts: dict[str, str] = {}
    for name in {e[1] for e in EXTRACT} | {"gates.log"}:
        p = logdir / name
        if p.is_file():
            texts[name] = read_text(p)

    for field, name, pattern in EXTRACT:
        text = texts.get(name)
        if not text:
            continue
        matches = re.findall(pattern, text, re.IGNORECASE)
        if not matches:
            continue
        # Throughput lines carry the *target* before the *measurement* ("renders >20x realtime -
        # 940.0x realtime"), so the last match is the real one. Everything else takes the first.
        raw = matches[-1] if field == "realtime_x" else matches[0]
        raw = raw if isinstance(raw, str) else str(raw)
        if re.fullmatch(r"\d+", raw):
            found[field] = int(raw)
        elif re.fullmatch(r"[\d.]+", raw):
            try:
                found[field] = float(raw)
            except ValueError:
                found[field] = raw
        else:
            found[field] = raw
        if field == "stamp" and not str(found[field]).startswith("src "):
            found[field] = f"src {found[field]}"

    # Test totals are summed across every "test result:" line, as cargo prints one per binary -
    # scoped to the `tests` stage so the release golden rerun is not double-counted (#72).
    gates = texts.get("gates.log", "")
    passed = failed = ignored = 0
    for m in SUM_PASSED.finditer(tests_section(gates)):
        passed += int(m.group(1))
        failed += int(m.group(2))
        ignored += int(m.group(3))
    if passed or failed or ignored:
        found["tests_passed"], found["tests_failed"] = passed, failed
        found["tests_ignored"] = ignored

    # Allocations: the selftest and the contract tests both state a count.
    # The rt-discipline line is the claim; the "allocation gate is live" line deliberately reports a
    # NON-zero count (it is proving the gate can see a violation), so a naive first-match read would
    # report the audio path as allocating 64 times.
    m = re.search(r"rt discipline:\s*(\d+)\s+allocation", gates) or re.search(r"(\d+)\s+allocations? on the audio path", gates)
    if m:
        found["allocations"] = int(m.group(1))
    return found


# ---------------------------------------------------------------- verdicts
def verdict(field: str, got: object) -> tuple[str, str]:
    """Returns (verdict, note). Verdict is OK / SOFT / FAIL / DECISION / MISSING."""
    want = BASELINE.get(field)
    if got is None:
        return "MISSING", "not found in the logs"
    if field in ("dispatch_block_dyn_us", "dispatch_block_enum_us",
                 "dispatch_sample_dyn_us", "dispatch_sample_enum_us"):
        lo, hi = want
        ratio = float(got) / hi if hi else 0.0
        if 0.2 <= ratio <= 5.0:
            return "SOFT", f"{float(got):.2f} us vs sandbox {lo}-{hi} us ({ratio:.2f}x the upper bound)"
        return "SOFT", f"{float(got):.2f} us vs sandbox {lo}-{hi} us - far outside, check the profile"
    if field == "realtime_x":
        ratio = float(got) / float(want)
        note = f"{float(got):.1f}x vs sandbox {want}x ({ratio:.2f}x)"
        return ("SOFT", note) if ratio >= 0.5 else ("SOFT", note + " - implausibly slow, check the build")
    # Compare numerically when both sides are numbers. This is not a nicety: a regex-extracted 0 is
    # a float and the baseline 0 is an int, and `str(0.0) != str(0)` reported two HARD failures on a
    # perfectly clean log set - a tool that cries failure on a good run gets ignored, which is worse
    # than no tool (the defect #47 lesson, applied to this one).
    if isinstance(want, (int, float)) and not isinstance(want, bool):
        try:
            if float(got) == float(want):
                return "OK", f"matches baseline ({want})"
        except (TypeError, ValueError):
            pass
    elif str(got) == str(want):
        return "OK", f"matches baseline ({want})"
    if field == "stamp":
        return "FAIL", (f"binary is {got}, tree baseline is {want} - either a stale exe "
                        "(rebuild; defects #41/#49) or the tree itself drifted from the synced "
                        "baseline (a stray extra source file, e.g. a '(2)' copy - defect #71). "
                        "The stamp-guard 'sources' line in build.log tells which: if sources == "
                        "binary, the TREE changed, not the exe, and rebuilding cannot fix it")
    if field == "golden_hash":
        return "FAIL", f"{got} != {want} - the audio path changed, or the render is not deterministic"
    return "FAIL", f"{got} != baseline {want}"


def decision_checks(found: dict[str, object]) -> list[str]:
    """The checks that would invalidate a recorded decision rather than a gate."""
    out: list[str] = []
    dyn, enum = found.get("dispatch_sample_dyn_us"), found.get("dispatch_sample_enum_us")
    if dyn is not None and enum is not None and float(enum) > float(dyn):
        out.append(
            "ADR-009 executor decision 7 INVERTED: enum dispatch measured slower than trait objects "
            f"per sample ({enum} us vs {dyn} us). The sandbox measured enum within noise of "
            "monomorphised and 13-17x faster than dyn. Reopen the decision before building the "
            "executor on it."
        )
    bd, be = found.get("dispatch_block_dyn_us"), found.get("dispatch_block_enum_us")
    if bd is not None and float(bd) > 20.0:
        out.append(
            f"Block dispatch of 100 null modules took {bd} us against a 20 us acceptance target. "
            "WO-007 criterion 6 is not met on this device even though the sandbox had 10-16x headroom."
        )
    return out


def report(found: dict[str, object], logdir: pathlib.Path) -> int:
    """Prints the comparison table. Returns a process exit code."""
    order = ["stamp", "golden_hash", "tests_passed", "tests_failed", "selftest", "ui_audit",
             "allocations", "reopen_leak_outstanding", "soak_xruns", "realtime_x",
             "dispatch_block_dyn_us", "dispatch_block_enum_us",
             "dispatch_sample_dyn_us", "dispatch_sample_enum_us"]
    print(f"log_check - device logs in `{logdir}` vs the sandbox baseline (stamp {BASELINE['stamp']})")
    print()
    print(f"{'field':<28}{'verdict':<9}detail")
    print("-" * 100)
    failures = 0
    missing = 0
    for field in order:
        got = found.get(field)
        v, note = verdict(field, got)
        if v == "FAIL":
            failures += 1
        if v == "MISSING":
            missing += 1
        shown = "-" if got is None else (f"{got:g}" if isinstance(got, float) else str(got))
        print(f"{field:<28}{v:<9}{shown:<22}{note}")
    print("-" * 100)

    for line in decision_checks(found):
        print(f"DECISION  {line}")
        failures += 1

    logs_seen = sorted(p.name for p in logdir.glob("*.log")) if logdir.is_dir() else []
    print()
    print(f"logs present: {', '.join(logs_seen) if logs_seen else 'none'}")
    if missing:
        print(f"{missing} field(s) not found - a missing log is not a pass, it is an unanswered question")
    if failures:
        print(f"RESULT: {failures} hard failure(s)")
        return 1
    print("RESULT: no hard failures")
    return 0


# ---------------------------------------------------------------- self-test
SELF_LOGS = {
    "build.log": "sparq build\n  stamp: src 92f/1977302B\n  rebuilt 12 crates\n",
    "gates.log": (
        "test result: ok. 300 passed; 0 failed; 1 ignored\n"
        "test result: ok. 88 passed; 0 failed; 0 ignored\n"
        "  [PASS] golden reference matches - hash ba577186c988db21 rms 0.177196\n"
        "  [PASS] rt discipline: 0 allocations on the audio path - 0 allocation(s)\n"
        "  [PASS] throughput: 1 min at 96 kHz/64 renders >20x realtime - 940.0x realtime in 63 ms\n"
        "selftest: PASS (9 gates)\n"
        "ui audit: PASS (0 failure(s))\n"
        "reopen-leak: 8 cycles, 0 outstanding allocations\n"
    ),
    "soak.log": "soak 120 min wasapi-exclusive\n  blocks 495000 xrun 0 underrun 0 alloc 0\n",
    "dispatch.log": (
        "  Box<dyn Module>, block dispatch      1.10 us/block   18.2x headroom\n"
        "  enum + match, block dispatch         0.80 us/block   25.0x headroom\n"
        "  full per-sample graph cost: dyn 18.20 us/block vs enum 1.40 us/block (6400 calls)\n"
    ),
}


def self_test(tmp: pathlib.Path) -> int:
    """Proves the parser and the verdicts work, including that they fail when they should."""
    tmp.mkdir(parents=True, exist_ok=True)
    for name, text in SELF_LOGS.items():
        # Write as cp1252 to prove the encoding ladder works on a Windows-produced log.
        (tmp / name).write_bytes(text.encode("cp1252"))
    found = extract(tmp)

    # Defect #72 regression: with stage headers present and a later release stage re-running two
    # golden tests, the sum must stay scoped to the `tests` stage and read 388, not 390.
    scoped = tmp / "scoped"
    scoped.mkdir(exist_ok=True)
    (scoped / "gates.log").write_bytes(
        (
            " ---- tests ----\n"
            "test result: ok. 300 passed; 0 failed; 1 ignored\n"
            "test result: ok. 88 passed; 0 failed; 0 ignored\n"
            " ---- golden reference ----\n"
            "test result: ok. 2 passed; 0 failed; 0 ignored\n"
            " ---- release build ----\n"
        ).encode("cp1252")
    )
    scoped_total = extract(scoped).get("tests_passed")

    checks = [
        ("stamp read from build.log", found.get("stamp") == BASELINE["stamp"], found.get("stamp")),
        ("golden hash extracted", found.get("golden_hash") == "ba577186c988db21", found.get("golden_hash")),
        ("test totals summed across binaries", found.get("tests_passed") == 388, found.get("tests_passed")),
        ("release-stage reruns are not double-counted (defect #72)", scoped_total == 388, scoped_total),
        ("zero failures extracted", found.get("tests_failed") == 0, found.get("tests_failed")),
        ("selftest verdict", found.get("selftest") == "PASS", found.get("selftest")),
        ("ui audit verdict", found.get("ui_audit") == "PASS", found.get("ui_audit")),
        ("allocations counted", found.get("allocations") == 0, found.get("allocations")),
        ("reopen-leak outstanding", found.get("reopen_leak_outstanding") == 0, found.get("reopen_leak_outstanding")),
        ("soak xruns", found.get("soak_xruns") == 0, found.get("soak_xruns")),
        ("realtime parsed as a number", found.get("realtime_x") == 940.0, found.get("realtime_x")),
        ("dispatch block dyn", found.get("dispatch_block_dyn_us") == 1.10, found.get("dispatch_block_dyn_us")),
        ("dispatch sample enum", found.get("dispatch_sample_enum_us") == 1.40, found.get("dispatch_sample_enum_us")),
        ("a device faster than the sandbox is SOFT, not FAIL",
         verdict("realtime_x", 940.0)[0] == "SOFT", verdict("realtime_x", 940.0)),
        ("a stale stamp is a FAIL", verdict("stamp", "src 57f/853914B")[0] == "FAIL",
         verdict("stamp", "src 57f/853914B")[1][:60]),
        ("a changed golden hash is a FAIL", verdict("golden_hash", "deadbeefdeadbeef")[0] == "FAIL", ""),
        ("an absent field is MISSING, never OK", verdict("soak_xruns", None)[0] == "MISSING", ""),
        # Verdicts, not just extraction: every numeric hard check must read a clean value as OK,
        # whether it arrived as an int or a float. This is the check that would have caught the
        # string-comparison bug above.
        ("zero xruns is OK as an int", verdict("soak_xruns", 0)[0] == "OK", verdict("soak_xruns", 0)),
        ("zero xruns is OK as a float", verdict("soak_xruns", 0.0)[0] == "OK", verdict("soak_xruns", 0.0)),
        ("zero outstanding is OK as a float",
         verdict("reopen_leak_outstanding", 0.0)[0] == "OK", verdict("reopen_leak_outstanding", 0.0)),
        ("zero allocations is OK as a float", verdict("allocations", 0.0)[0] == "OK", verdict("allocations", 0.0)),
        ("one xrun is a FAIL", verdict("soak_xruns", 1)[0] == "FAIL", verdict("soak_xruns", 1)[1]),
        ("the decision check stays quiet when enum wins", decision_checks(found) == [], decision_checks(found)),
        ("the decision check fires when enum loses",
         len(decision_checks({"dispatch_sample_dyn_us": 5.0, "dispatch_sample_enum_us": 9.0})) == 1, ""),
        ("the decision check fires when block dispatch exceeds 20 us",
         len(decision_checks({"dispatch_block_dyn_us": 25.0})) == 1, ""),
    ]

    bad = 0
    for name, ok, detail in checks:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"   got: {detail!r}"))
        bad += 0 if ok else 1

    # And the whole report against a directory with nothing in it: absence must not read as success.
    empty = tmp / "empty"
    empty.mkdir(exist_ok=True)
    code = report(extract(empty), empty)
    print(f"  {'PASS' if code == 0 else 'FAIL'}  an empty log dir produces no hard failures "
          f"(everything is MISSING, and MISSING is reported as an unanswered question, not a pass)")
    print()
    print(f"self-test: {len(checks) - bad}/{len(checks)} checks pass")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--dir", default="logs", help="directory holding the device logs (default: ./logs)")
    ap.add_argument("--self-test", action="store_true", help="prove the parser and verdicts work")
    args = ap.parse_args()
    if args.self_test:
        return self_test(ROOT / "target" / "log_check_selftest")
    logdir = pathlib.Path(args.dir)
    if not logdir.is_dir():
        print(f"log_check: no such directory `{logdir}`")
        print("  Fix: run the scripts first (they each leave a log), or pass --dir <path>.")
        return 2
    return report(extract(logdir), logdir)


if __name__ == "__main__":
    raise SystemExit(main())
