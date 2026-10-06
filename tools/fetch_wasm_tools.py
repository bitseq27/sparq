#!/usr/bin/env python3
"""
tools/fetch_wasm_tools.py — fetch the prebuilt `wasm-tools` CLI (WO-020 INC3 component recipe).

The Observatory's component is built with `cargo build --target wasm32-unknown-unknown` +
`wasm-tools component new` (the corrected recipe in docs/api/instrument-wit/README.md — the round-5
notes' `jco componentize` takes a JS source, not Rust core-wasm). `cargo install wasm-tools` compiles
a very large dependency tree and does not fit the ~1 GB dev sandbox; the upstream release binaries do
(version-pinned below). This tool downloads and extracts ONE executable to `--dest` (default
/tmp/wasm-tools on Unix, %TEMP%\\wasm-tools.exe-aware name elsewhere) and prints its path.

Pinned: wasm-tools **1.261.0** (the version that componentised the Observatory on 2026-10-06; a bump
is a recorded re-baselining, like the wasmtime pin — the component model is frozen-contract surface).

Usage:
    python3 tools/fetch_wasm_tools.py                 # -> prints the executable path
    python3 tools/fetch_wasm_tools.py --dest build/wasm-tools
"""
from __future__ import annotations

import argparse
import io
import os
import pathlib
import stat
import sys
import tarfile
import urllib.request
import zipfile

# Windows consoles and redirected logs default to the ANSI code page (defect #37); every tool
# reconfigures its streams (tools/README.md -> 'Text I/O portability'; check_text_io.py gate).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

VERSION = "1.261.0"
BASE = f"https://github.com/bytecodealliance/wasm-tools/releases/download/v{VERSION}"
ASSETS = {
    "linux": (f"{BASE}/wasm-tools-{VERSION}-x86_64-linux.tar.gz", "wasm-tools", "tar"),
    "macos": (f"{BASE}/wasm-tools-{VERSION}-x86_64-macos.tar.gz", "wasm-tools", "tar"),
    "windows": (f"{BASE}/wasm-tools-{VERSION}-x86_64-windows.zip", "wasm-tools.exe", "zip"),
}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Fetch the pinned prebuilt wasm-tools CLI.")
    ap.add_argument("--dest", type=pathlib.Path, default=None, help="output executable path")
    args = ap.parse_args(argv)

    key = "windows" if os.name == "nt" else ("macos" if sys.platform == "darwin" else "linux")
    url, inner, kind = ASSETS[key]
    dest = args.dest or pathlib.Path(("/tmp/wasm-tools.exe" if key == "windows" else "/tmp/wasm-tools"))

    if dest.is_file() and dest.stat().st_size > 1_000_000:
        print(dest)  # already fetched this session/box — do not re-download
        return 0

    req = urllib.request.Request(url, headers={"User-Agent": "sparq-wo020-fetch/0.1"})
    data = urllib.request.urlopen(req, timeout=280).read()
    dest.parent.mkdir(parents=True, exist_ok=True)
    if kind == "tar":
        with tarfile.open(fileobj=io.BytesIO(data)) as tf:
            member = next((m for m in tf.getmembers() if m.name.endswith("/" + inner) or m.name == inner), None)
            if member is None:
                print(f"fetch_wasm_tools: {url}: no {inner} member in the archive", file=sys.stderr)
                return 1
            fh = tf.extractfile(member)
            assert fh is not None
            dest.write_bytes(fh.read())
    else:
        with zipfile.ZipFile(io.BytesIO(data)) as zf:
            name = next((n for n in zf.namelist() if n.endswith(inner)), None)
            if name is None:
                print(f"fetch_wasm_tools: {url}: no {inner} entry in the zip", file=sys.stderr)
                return 1
            dest.write_bytes(zf.read(name))
    dest.chmod(dest.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    print(dest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
