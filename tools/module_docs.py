#!/usr/bin/env python3
"""Generate the module reference docs from the manifests (WO-014 acceptance).

The acceptance criterion, verbatim: "Docs for all modules are generated from manifests (no
hand-written duplicates)." This is the generator. One source of truth: `modules/**/sparqmod.toml`
— the same bytes `sparq modules` discovers and `include_str!` compiles in. A hand-written module
doc is drift waiting to happen; a generated one is a view.

Usage:
    python3 tools/module_docs.py            regenerate docs/modules/*.md
    python3 tools/module_docs.py --check    gate: fail (exit 1) if any doc is stale, missing,
                                            or orphaned (a doc whose manifest is gone)

Text I/O discipline (tools/check_text_io.py): explicit UTF-8, explicit LF newlines, lossy reads
where a corrupt byte must not crash the gate.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

# Windows consoles may default to cp1252: this report and the generated text carry '·'/'—',
# so the streams are explicitly UTF-8 with replacement (defect #37's remedy, house pattern).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper (captured/redirected oddly)
        pass

ROOT = Path(__file__).resolve().parent.parent
MODULES_DIR = ROOT / "modules"
DOCS_DIR = ROOT / "docs" / "modules"

HEADER = (
    "> **Generated** from `{rel}` by `tools/module_docs.py`. Do not edit by hand —\n"
    "> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).\n"
)


def read_text_lossy(path: Path) -> str:
    """UTF-8 with replacement — a corrupt byte must never crash the gate (defect #20's rule)."""
    return path.read_bytes().decode("utf-8", errors="replace")


def doc_path_for(module_id: str) -> Path:
    """`sparq/syn/sine` -> `docs/modules/syn-sine.md`."""
    tail = module_id.removeprefix("sparq/")
    return DOCS_DIR / f"{tail.replace('/', '-')}.md"


def fmt(v: object) -> str:
    if v is None:
        return "—"
    if isinstance(v, bool):
        return "yes" if v else "no"
    if isinstance(v, float):
        return f"{v:g}"
    return str(v)


def render(manifest_path: Path, m: dict) -> str:
    ident = m.get("identity", {})
    cls = m.get("classification", {})
    mid = ident.get("id", "?")
    lines: list[str] = []
    lines.append(f"# {mid} — {fmt(ident.get('display_name'))}")
    lines.append("")
    rel = manifest_path.relative_to(ROOT).as_posix()
    lines.append(HEADER.format(rel=rel))
    lines.append("")
    if ident.get("summary"):
        lines.append(f"*{ident['summary']}*")
        lines.append("")
    host = ident.get("host_api", {})
    lines.append("| | |")
    lines.append("|---|---|")
    lines.append(f"| Version | {fmt(ident.get('version'))} |")
    lines.append(
        f"| Host API | {fmt(host.get('min'))}…{fmt(host.get('max'))} |"
    )
    lines.append(f"| Category | {fmt(cls.get('category'))} |")
    lines.append(
        f"| Kind / tier / stability | {fmt(cls.get('kind'))} · {fmt(cls.get('tier'))}"
        f" · {fmt(cls.get('stability'))} |"
    )
    authors = ident.get("authors") or []
    lines.append(f"| Authors | {', '.join(authors) if authors else '—'} |")
    lines.append(f"| License | {fmt(ident.get('license'))} |")
    lines.append("")

    ports = m.get("ports", [])
    lines.append("## Ports")
    lines.append("")
    if not ports:
        lines.append("_None declared._")
    else:
        lines.append("| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |")
        lines.append("|---|---|---|---|---|---|---|---|")
        for p in ports:
            shape_parts = []
            for key in ("channel_set", "rate", "range"):
                if p.get(key) is not None:
                    shape_parts.append(f"{key.replace('_', ' ')}: {p[key]}")
            shape = ", ".join(shape_parts) if shape_parts else "—"
            lines.append(
                f"| `{fmt(p.get('id'))}` | {fmt(p.get('name'))} | {fmt(p.get('direction'))}"
                f" | {fmt(p.get('type'))} | {shape} | {fmt(p.get('required'))}"
                f" | {fmt(p.get('multiplicity'))} | {fmt(p.get('latency_contribution', 0))} |"
            )
    lines.append("")

    params = m.get("params", [])
    lines.append("## Parameters")
    lines.append("")
    if not params:
        lines.append("_None declared._")
    else:
        lines.append("| # | ID | Name | Type | Unit | Min | Max | Default |")
        lines.append("|---|---|---|---|---|---|---|---|")
        for i, prm in enumerate(params):
            lines.append(
                f"| {i} | `{fmt(prm.get('id'))}` | {fmt(prm.get('name'))}"
                f" | {fmt(prm.get('type'))} | {fmt(prm.get('unit'))} | {fmt(prm.get('min'))}"
                f" | {fmt(prm.get('max'))} | {fmt(prm.get('default'))} |"
            )
        lines.append("")
        lines.append(
            "Parameter order is the snapshot order: `param(i)` in `process` reads row `i`."
        )
    lines.append("")

    state = m.get("state", {})
    lines.append("## State")
    lines.append("")
    lines.append(
        f"Schema `{fmt(state.get('schema_id'))}` v{fmt(state.get('schema_version'))}"
        " — the blob `configure` accepts; a project save/restore round-trips through it."
    )
    lines.append("")

    res = m.get("resources", {})
    lines.append("## Resources")
    lines.append("")
    lat = res.get("latency", 0)
    if isinstance(lat, str) and lat.startswith("param:"):
        lines.append(
            f"- Declared latency: **parametric** — follows `{lat.removeprefix('param:')}`"
            " (the executor cannot compensate it statically, and says so)."
        )
    else:
        lines.append(f"- Declared latency: {fmt(lat)} samples")
    lines.append(f"- CPU class: {fmt(res.get('cpu_class'))}")
    if res.get("oversampling"):
        lines.append(f"- Oversampling: {fmt(res.get('oversampling'))}")
    lines.append("")
    return "\n".join(lines)


def collect() -> list[tuple[Path, dict]]:
    found = []
    for path in sorted(MODULES_DIR.rglob("sparqmod.toml")):
        with path.open("rb") as fh:
            data = tomllib.load(fh)
        found.append((path, data))
    return found


def main() -> int:
    check = "--check" in sys.argv
    manifests = collect()
    if not manifests:
        print("module_docs: no manifests found under modules/ — nothing to do")
        return 0
    failures: list[str] = []
    expected: set[Path] = set()
    for path, data in manifests:
        mid = data.get("identity", {}).get("id")
        if not mid:
            failures.append(f"{path}: manifest has no identity.id")
            continue
        out = doc_path_for(mid)
        expected.add(out)
        text = render(path, data)
        if check:
            if not out.exists():
                failures.append(f"MISSING {out.relative_to(ROOT)} (manifest: {mid})")
            elif read_text_lossy(out) != text:
                failures.append(f"STALE   {out.relative_to(ROOT)} (regenerate: python3 tools/module_docs.py)")
        else:
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(text, encoding="utf-8", newline="\n")
    if check:
        # orphan docs: a page whose manifest is gone is a hand-written duplicate in waiting
        if DOCS_DIR.exists():
            for doc in sorted(DOCS_DIR.glob("*.md")):
                if doc not in expected:
                    failures.append(f"ORPHAN  {doc.relative_to(ROOT)} (no manifest generates it)")
    if failures:
        mode = "check" if check else "generate"
        print(f"module_docs ({mode}): {len(failures)} problem(s):")
        for f in failures:
            print(f"  {f}")
        return 1
    if check:
        print(f"module_docs: {len(manifests)} doc(s) match their manifests — no hand-written duplicates")
    else:
        print(f"module_docs: wrote {len(manifests)} doc(s) into docs/modules/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
