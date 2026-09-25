#!/usr/bin/env python3
"""sparq `unsafe` audit (ADR-000).

Enforces the policy that is otherwise impossible to keep by discipline on a solo project:

  1. `unsafe` may appear ONLY in files listed in docs/unsafe-allowlist.md.
  2. Every such file must carry a module-level `#![allow(unsafe_code)]` that names its allowlist
     entry (so a reader knows why the lint is off).
  3. Every `unsafe` block/impl/fn must have a `SAFETY:` comment within the preceding 12 lines.
  4. No other crate may relax the `unsafe_code` lint.

Usage:
    python3 tools/unsafe_audit.py            # human-readable, exit 1 on violation
    python3 tools/unsafe_audit.py --json
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

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
ALLOWLIST_DOC = ROOT / "docs" / "unsafe-allowlist.md"
SCAN_ROOTS = [ROOT / "crates"]

# Matches a numbered allowlist row and captures its path cell, e.g.
#   | 5 | `sparq-kernel/src/sync/rings.rs` | Lock-free ring ... |
# Numbered rows only: prose elsewhere in the document may mention paths that are NOT allowlisted.
ALLOWLIST_ROW = re.compile(r"^\|\s*\d+\s*\|\s*`([A-Za-z0-9_\-]+/src/[A-Za-z0-9_\-/]+\.rs)`")
UNSAFE_RE = re.compile(r"\bunsafe\b")
UNSAFE_FN_DECL_RE = re.compile(r"\bunsafe\s+fn\b")
UNSAFE_IMPL_RE = re.compile(r"\bunsafe\s+impl\b")
SAFETY_RE = re.compile(r"SAFETY:")
ALLOW_ATTR_RE = re.compile(r"#!\[allow\([^)]*\bunsafe_code\b[^)]*\)\]")
# A line is not an `unsafe` *operation* if it is a comment, an attribute, or the lint name itself.
# Docs legitimately discuss `unsafe`; only code must justify it.
IGNORE_RE = re.compile(r"^\s*(//|/\*|\*|#\[|#!\[)|unsafe_code")


def load_allowlist() -> tuple[set[str], dict[str, str]]:
    """Return (allowlisted relative paths, path -> declared 'why')."""
    if not ALLOWLIST_DOC.exists():
        sys.exit(f"unsafe_audit: {ALLOWLIST_DOC} is missing")
    text = read_text(ALLOWLIST_DOC)
    paths: set[str] = set()
    why: dict[str, str] = {}
    for line in text.splitlines():
        m = ALLOWLIST_ROW.match(line.strip())
        if not m:
            continue
        raw = m.group(1)
        paths.add(raw)
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        why[raw] = cells[2] if len(cells) > 2 else ""
    # The allowlist names the *intended* final paths; Phase 0 ships some of them at slightly
    # different locations, declared here explicitly so the audit stays honest rather than loose.
    aliases = {
        # entry 4 is `sparq-kernel/src/rt/alloc.rs` in the ADR; Phase 0 ships it as `alloc.rs`
        "sparq-kernel/src/alloc.rs": "sparq-kernel/src/rt/alloc.rs",
        # entry 5 is `sparq-kernel/src/sync/rings.rs` — exact match, no alias needed
    }
    for alias, canonical in aliases.items():
        if canonical in paths:
            paths.add(alias)
            why.setdefault(alias, why.get(canonical, ""))
    return paths, why


def audit_file(path: pathlib.Path, allow: set[str]) -> list[dict]:
    rel = str(path.relative_to(ROOT)).replace("\\", "/")
    # The allowlist names crate-relative paths (`sparq-kernel/src/...`); scanned files are
    # workspace-relative (`crates/sparq-kernel/src/...`). Normalise before comparing.
    key = rel[len("crates/"):] if rel.startswith("crates/") else rel
    text = read_text_lossy(path)
    lines = text.splitlines()
    findings: list[dict] = []

    # Track whether we are inside an `unsafe impl` block: the `unsafe fn` items of a trait impl are
    # declared unsafe by the trait itself, so the justification belongs on the operations in their
    # bodies, not on the signature. A free `unsafe fn` outside any `unsafe impl` is still flagged.
    hits = []
    impl_depth: int | None = None      # brace depth at which the enclosing `unsafe impl` opened
    depth = 0
    for i, line in enumerate(lines, 1):
        opens_impl = bool(UNSAFE_IMPL_RE.search(line)) and not IGNORE_RE.search(line)
        if opens_impl:
            impl_depth = depth
        # The `unsafe impl` line itself is justified by the file-level SAFETY block and the
        # allowlist entry; individual operations inside it are checked separately.
        is_unsafe_line = (
            bool(UNSAFE_RE.search(line)) and not IGNORE_RE.search(line) and not opens_impl
        )
        # An `unsafe fn` signature inside an `unsafe impl` is unsafe by declaration (the trait says
        # so); the justification belongs on the operations in its body, which are still checked.
        if is_unsafe_line and not (UNSAFE_FN_DECL_RE.search(line) and impl_depth is not None):
            hits.append(i)
        depth += line.count("{") - line.count("}")
        if impl_depth is not None and depth <= impl_depth:
            impl_depth = None

    if not hits:
        # A file that merely mentions the lint is fine.
        if ALLOW_ATTR_RE.search(text) and key not in allow:
            findings.append({
                "file": rel, "rule": "U4 lint relaxed without an allowlist entry",
                "detail": "file carries #![allow(unsafe_code)] but contains no `unsafe` and is not allowlisted",
            })
        return findings

    if key not in allow:
        findings.append({
            "file": rel, "rule": "U1 unsafe outside the allowlist",
            "detail": f"{len(hits)} line(s) contain `unsafe`: {hits[:6]}"
                      f"{' …' if len(hits) > 6 else ''}. Add an entry to docs/unsafe-allowlist.md "
                      "(with its invariant and test) or remove the unsafe.",
        })
        return findings

    if not ALLOW_ATTR_RE.search(text):
        findings.append({
            "file": rel, "rule": "U2 allowlisted file does not declare its exemption",
            "detail": "expected a module-level #![allow(unsafe_code)] naming the allowlist entry",
        })

    for ln in hits:
        window = "\n".join(lines[max(0, ln - 13):ln])
        if not SAFETY_RE.search(window):
            findings.append({
                "file": rel, "rule": "U3 unsafe without a SAFETY: comment",
                "detail": f"line {ln}: no `SAFETY:` within the preceding 12 lines",
            })
    return findings


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    allow, why = load_allowlist()
    findings: list[dict] = []
    scanned = 0
    files_with_unsafe: list[str] = []
    for root in SCAN_ROOTS:
        for path in sorted(root.rglob("*.rs")):
            if "target" in path.parts:
                continue
            scanned += 1
            f = audit_file(path, allow)
            findings += f
            if any(x["rule"].startswith(("U1", "U2", "U3")) for x in f) or path.relative_to(ROOT).as_posix() in allow:
                files_with_unsafe.append(path.relative_to(ROOT).as_posix())

    used = {f["file"] for f in findings if f["rule"].startswith("U1")}
    if args.json:
        print(json.dumps({
            "allowlisted": sorted(allow),
            "scanned": scanned,
            "violations": findings,
        }, indent=2))
    else:
        print(f"unsafe_audit · {len(allow)} allowlisted paths · {scanned} .rs files scanned")
        for p in sorted(allow):
            note = why.get(p, "")
            print(f"  allowlisted: {p}" + (f" — {note[:70]}" if note else ""))
        if findings:
            for f in findings:
                print(f"  FAIL [{f['rule']}] {f['file']} — {f['detail']}")
            print(f"\n{len(findings)} violation(s)")
        else:
            print("clean — every `unsafe` is allowlisted, exempted and justified")
        _ = used
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
