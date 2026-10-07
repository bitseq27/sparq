#!/usr/bin/env python3
"""Gate: text I/O in tools/ must be explicit about encoding and newlines.

This gate exists because of a real defect that shipped and was only caught by running the gates on
Windows — the *primary* platform (ADR-004):

* `Path.read_text()` uses the platform default encoding. On Windows that is the ANSI code page
  (cp1252 in the field), which cannot decode the UTF-8 characters used throughout the design docs
  and mockups. Both `token_audit.py` and `unsafe_audit.py` crashed with `UnicodeDecodeError`.
* `Path.write_text()` translates "\\n" to "\\r\\n" on Windows. The committed generated files are
  LF-only, so `token_gen.py --check` reported every one of them as stale.

Every text read/write under `tools/` must therefore go through an explicit
`encoding="utf-8"` read and an `encoding="utf-8", newline="\\n"` write. Each tool defines a local
`read_text`/`write_text` pair for this; calling them bare is correct and does not trip this gate.

Binary I/O (`open(..., "rb")`, `read_bytes`, `write_bytes`) is encoding-independent and permitted.

Rule 2 (added after defect #37): every tool must also reconfigure `sys.stdout`/`sys.stderr` to
UTF-8 — *file* encodings were fixed after defect #19, but the tools still crashed when PRINTING
the same characters to a cp1252 console or a redirected log.

Rule 3 (added after defect #42): every `scripts/*.bat` must be **pure ASCII**. cmd.exe reads
batch files through the console code page; with `chcp 65001` a multibyte UTF-8 character makes
the parser lose byte-sync and the script dies *silently mid-run* — build.bat shipped a single
UTF-8 middle dot in a PowerShell line and exited without a log. The `chcp 65001` in the wrapper
is for the BINARY's output; the scripts themselves stay ASCII, always.

Rule 4 (added after defect #69): inside a parenthesised block, the first unescaped `)` in an
`echo` line **ends the block** — a `(` earlier on the same line does not nest against it — and
the rest of the line is re-read as command syntax. build.bat's BUILD-FAILED hint
`... Cargo.toml (the per-target wgpu features live there).` left a stray `.` that the IF parser
rejected with `. was unexpected at this time.`, killing the script after *every successful
build* for a whole increment (the block is parsed on every run whether or not the condition
fires, so the healthy path died too). Escape as `^(` and `^)`. Top-level echo text is exempt —
cmd only tracks depth inside `( ... )` — and so are parens inside double quotes or behind
carets. Scope: `echo` lines at block depth > 0, the demonstrated killer class; a single-line
`for ... do echo (x)` after `do` is outside the tracked depth and not caught.

Usage:  python3 tools/check_text_io.py      (exit 1 on violation)
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent
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


# A `.read_text()` / `.write_text(...)` *method call* — i.e. someone went straight to pathlib.
# The local helpers are called as `read_text(path)` with no leading dot, so they do not match.
BARE_METHOD = re.compile(r"\.(?:read_text|write_text)\s*\(")
# The helper implementations themselves must call pathlib with an explicit encoding; that is the
# one legitimate use, so a call that names `encoding=` is accepted.
EXPLICIT_ENCODING = re.compile(r"\.(?:read_text|write_text)\s*\([^)]*encoding\s*=")
# Text-mode `open()` with no encoding argument.
OPEN_CALL = re.compile(r"(?<![\w.])open\s*\(([^)]*)\)")
# Per-line opt-out, for the rare place that genuinely needs a platform-default read.
ALLOW_MARK = "# gate:allow"
# This script must be able to read the files it audits, so it is exempt from its own rule.
SELF = "check_text_io.py"

# Rule 4 exemptions, named and justified (the unsafe_audit pattern): probe-stamp.bat's section
# S0a carries defect #69's killer line VERBATIM, unescaped parens included — a reproduction that
# was "fixed" would prove nothing. Every other .bat is audited.
BLOCK_PAREN_EXEMPT = {"probe-stamp.bat"}


def _strip_quoted_and_escaped(line: str) -> str:
    """Remove double-quoted spans and caret-escaped characters — the parts of a batch line whose
    parens cmd's block reader does not count. `^^` collapses to a literal caret, so a following
    `)` stays visible to the check, as it should."""
    out: list[str] = []
    in_quote = False
    i = 0
    while i < len(line):
        c = line[i]
        if c == "^" and i + 1 < len(line):
            if line[i + 1] == "^":
                out.append("^")
                i += 2
                continue
            i += 2  # escaped char: inert for paren tracking
            continue
        if c == '"':
            in_quote = not in_quote
            i += 1
            continue
        if not in_quote:
            out.append(c)
        i += 1
    return "".join(out)


def main() -> int:
    offenders: list[str] = []
    scripts = sorted(p for p in ROOT.glob("*.py"))
    audited = [p for p in scripts if p.name != SELF]
    for path in audited:
        text = path.read_text(encoding=ENCODING)
        for lineno, line in enumerate(text.splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith("#") or ALLOW_MARK in line:
                continue
            if BARE_METHOD.search(line) and not EXPLICIT_ENCODING.search(line):
                offenders.append(f"{path.name}:{lineno}: pathlib .read_text()/.write_text() — "
                                 f"use the module's read_text()/write_text() helpers")
                continue
            for m in OPEN_CALL.finditer(line):
                args = m.group(1)
                if '"rb"' in args or "'rb'" in args or '"wb"' in args or "'wb'" in args:
                    continue  # binary: encoding does not apply
                if "encoding" not in args:
                    offenders.append(f"{path.name}:{lineno}: text-mode open() without encoding= — "
                                     f"defaults to the platform ANSI code page on Windows")

    # Rule 2 (defect #37): every tool must reconfigure its stdout/stderr to UTF-8. A tool that
    # can crash mid-report on a cp1252 console is a tool whose failure hides its own findings.
    for path in audited:
        text = path.read_text(encoding=ENCODING)
        if ".reconfigure(" not in text:
            offenders.append(f"{path.name}: stdout/stderr are not reconfigured to UTF-8 — "
                             "printing '·'/'—'/'→' on a cp1252 console raises UnicodeEncodeError "
                             "mid-report (defect #37)")

    # Rule 3 (defect #42): .bat files are pure ASCII, byte-checked. A multibyte character makes
    # cmd's parser lose sync under chcp 65001 and kills the script silently — no log, no error,
    # just a window that closes. Binary read on purpose: this rule is about bytes, not encoding.
    scripts_dir = ROOT.parent / "scripts"
    if scripts_dir.is_dir():
        for path in sorted(scripts_dir.glob("*.bat")):
            data = path.read_bytes()
            bad = [i for i, b in enumerate(data) if b > 127]
            if bad:
                line_no = data[: bad[0]].count(b"\n") + 1
                offenders.append(
                    f"scripts/{path.name}: {len(bad)} non-ASCII byte(s), first on line {line_no} "
                    "(cmd loses parser sync on multibyte characters and dies silently — "
                    "defect #42; .bat files are ASCII-only by rule)"
                )

    # Rule 4 (defect #69): unescaped parens in echo text inside a parenthesised block. The first
    # bare `)` ends the block and the line's tail becomes garbage tokens the IF/FOR parser
    # rejects — `. was unexpected at this time.` — killing the script even on its healthy path,
    # because cmd parses the whole block when it reaches the IF, whatever the condition does.
    if scripts_dir.is_dir():
        for path in sorted(scripts_dir.glob("*.bat")):
            if path.name in BLOCK_PAREN_EXEMPT:
                continue
            text = path.read_bytes().decode("ascii", errors="replace")
            depth = 0
            for lineno, line in enumerate(text.splitlines(), 1):
                bare = _strip_quoted_and_escaped(line)
                s = bare.strip()
                if depth > 0 and (s.lower().startswith("echo ") or s.lower().startswith("echo.")):
                    body = _strip_quoted_and_escaped(s[4:])
                    if "(" in body or ")" in body:
                        offenders.append(
                            f"scripts/{path.name}:{lineno}: unescaped paren in echo text inside a "
                            "block — the first bare `)` ends the block and the rest of the line "
                            "becomes unexpected tokens; escape as ^( and ^) (defect #69)"
                        )
                depth += bare.count("(") - bare.count(")")
                if depth < 0:
                    depth = 0

    # Rule 5 (2026-10-07, the test007 step-[0] incident): an ESCAPED open paren on a structural
    # if/for line un-forms the block — cmd sees a literal `(`, the "guarded" body runs
    # UNCONDITIONALLY, and a fail-fast gate refuses everyone whatever the check returned.
    # Escaped parens belong in echo text (rule 4); structural parens stay bare.
    if scripts_dir.is_dir():
        for path in sorted(scripts_dir.glob("*.bat")):
            text = path.read_bytes().decode("ascii", errors="replace")
            for lineno, line in enumerate(text.splitlines(), 1):
                low = line.strip().lower()
                if (low.startswith("if ") or low.startswith("for ")) and "^(" in line:
                    offenders.append(
                        f"scripts/{path.name}:{lineno}: escaped ^( on a structural if/for line — "
                        "the block never forms and its body runs unconditionally (the test007 "
                        "step-[0] incident); structural parens stay bare, only echo text escapes"
                    )

    if offenders:
        print("check_text_io: FAIL")
        for o in offenders:
            print(f"  {o}")
        print(f"\n{len(offenders)} violation(s) across {len(audited)} script(s). "
              "See tools/README.md -> 'Text I/O portability'.")
        return 1
    print(f"check_text_io: clean ({len(audited)} script(s) under tools/, self exempt)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
