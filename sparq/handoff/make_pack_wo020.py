#!/usr/bin/env python3
"""Build the WO-020 INC4 delivery pack: FULL-tree overlay zip + sha256 sidecar.

House pattern (ROUND8-HANDOFF.md §6/§7, SYNC.md's rev-3/rev-4 standing rules):

  * FULL TREE — defect #94's remedy: overlays any tree at or after the r8 pack
    (`sparq-update-2026-10-05-r8.zip` / git `96fe57d`); depends on no base the receiver
    might not have.
  * Excluded by rule: `.git/`, `target/`, `Cargo.lock` (untracked by house rule; cargo
    re-resolves on first build), `logs/`, `*.wav`, `handoff/` (the artefact store — the
    receiver's copies win), and `SYNC-STAMP.txt` — **the stamp does NOT ride**: the
    receiving machine re-stamps after applying
    (`python tools\\sync_check.py --write --sync sparq-wo020-inc4-2026-10-06`). Until then
    `build.bat` / `test006`'s `[00b]` name the mismatch in words — EXPECTED, not a failure.
  * Pre-pack gate (LATER.md's "a zip is verified against its own manifest before it is
    sent" ideal, discharged here): every entry's zipped bytes are re-read and compared
    with the working tree; the working tree is required clean against HEAD; the
    entry/namelist counts are required to equal the numbers printed in the run sheet and
    SYNC.md; and the MUST-NOT-MOVE surfaces (WIT, module manifests, stamp, goldens,
    fixtures) are required byte-identical vs the base.

Usage (from anywhere):
    python3 handoff/make_pack_wo020.py
    python3 handoff/make_pack_wo020.py --base 96fe57d --out /path/to/pack.zip
Exit codes: 0 packed + verified · 1 any assertion failed (named in words, nothing written).
"""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import zipfile

# The numbers the run sheet + SYNC.md print. If the tree moves under them, this fails
# LOUDLY here instead of silently shipping a pack whose documents lie about it.
EXPECT_ENTRIES = 471
EXPECT_MOVED = 95  # vs --base: additions + modifications
EXPECT_MOVED_ADDED = 61
EXPECT_MOVED_MODIFIED = 34
EXPECT_SHIPPED_MOVED = 85  # moved paths that ride the pack (handoff/ does not)

MUST_HAVE = [
    # the delivery documents
    "WO020-INC4-RUN-SHEET.md",
    "WO020-OBSERVATORY-PLAN.md",
    "WO020-STATE.md",
    "CHECKLIST.md",
    "SYNC.md",
    "LATER.md",
    # the shipped instrument package (the 206 081 B component rides)
    "instruments/observatory/sparqmod.toml",
    "instruments/observatory/observatory.wasm",
    "instruments/observatory/example.sparqpatch",
    "instruments/observatory/README.md",
    # the guest source project (D13: the shipped logic and the proven logic are one object)
    "instruments-src/observatory/gen_manifest.py",
    "instruments-src/observatory/assets/coastline-110m.bin",
    "instruments-src/observatory/core/src/wall.rs",
    "instruments-src/observatory/wasm/src/lib.rs",
    "instruments-src/observatory/harness/src/main.rs",
    "instruments-src/observatory/package/sparqmod.toml",
    # the stream plane (INC1, published on main — a FULL tree carries it)
    "crates/sparq-streams/streams.toml",
    "crates/sparq-streams/src/registry.rs",
    "reference/fixtures/observatory/swpc.aurora.json",
    # INC4's host painters + shell doors
    "crates/sparq-ui/src/json.rs",
    "crates/sparq-ui/src/displaylist.rs",
    "crates/sparq-ui/src/canvas/picker.rs",
    "crates/sparq-app/src/ui/atrest.rs",
    "crates/sparq-app/src/ui/displaylist_egui.rs",
    "crates/sparq-host-wasm/src/sources.rs",
    "crates/sparq-audio/tests/portless_instrument.rs",
    # frozen surfaces a FULL tree must still carry
    "docs/api/instrument-wit/wit/world.wit",
    "tools/sparq-module-guest/src/lib.rs",
    "modules/out/main/sparqmod.toml",
    # the tools + scripts the run sheet invokes
    "tools/make_coastline.py",
    "tools/observatory_package.py",
    "tools/fetch_wasm_tools.py",
    "tools/sync_check.py",
    "scripts/build.bat",
    "scripts/gates.bat",
    # tokens + the convergence sheet
    "design/tokens/generated/tokens.rs",
    "reference/observatory/convergence-wo020-inc4.png",
]

# Paths that must NEVER appear in a pack.
MUST_NOT_HAVE_PREFIX = ("handoff/", "target/", "logs/", ".git/")
MUST_NOT_HAVE_EXACT = {"SYNC-STAMP.txt", "Cargo.lock"}


def git(repo: str, *args: str) -> str:
    out = subprocess.run(
        ["git", *args], cwd=repo, capture_output=True, text=True, check=True
    )
    return out.stdout


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main(argv: list[str] | None = None) -> int:
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--base", default="96fe57d", help="namelist base commit (main)")
    ap.add_argument(
        "--out",
        default="/home/user/sparq-update-2026-10-06-wo020-inc4.zip",
        help="where to write the zip",
    )
    args = ap.parse_args(argv)

    fails: list[str] = []

    def require(ok: bool, msg: str) -> None:
        if not ok:
            fails.append(msg)

    # ---- the tree is what git says it is ------------------------------------------
    head = git(repo, "rev-parse", "--short", "HEAD").strip()
    dirty = git(repo, "status", "--porcelain").strip()
    require(not dirty, f"working tree is NOT clean against HEAD ({head}):\n{dirty}")

    tracked = git(repo, "ls-files", "-z").split("\0")
    tracked = [t for t in tracked if t]
    payload = [
        t
        for t in tracked
        if not t.startswith(MUST_NOT_HAVE_PREFIX) and t not in MUST_NOT_HAVE_EXACT
    ]
    require(
        len(payload) == EXPECT_ENTRIES,
        f"payload is {len(payload)} files; the run sheet/SYNC.md print {EXPECT_ENTRIES}",
    )
    for bad in ("Cargo.lock",):
        require(bad not in payload, f"{bad} would ride the pack (house rule: it does not)")
    require(
        not any(t.endswith(".wav") for t in payload),
        "a .wav would ride the pack (r8 rule: none do)",
    )
    for must in MUST_HAVE:
        require(must in payload, f"MISSING from the payload: {must}")

    # ---- MUST-NOT-MOVE surfaces did not move vs the base ---------------------------
    for surface in (
        "docs/api/instrument-wit/",
        "modules/",
        "SYNC-STAMP.txt",
        "reference/fixtures/",
        "reference/golden",
    ):
        moved = git(repo, "diff", "--name-only", f"{args.base}..HEAD", "--", surface)
        require(not moved.strip(), f"MUST-NOT-MOVE surface moved vs {args.base}: {moved}")
    ref_moved = [
        m
        for m in git(repo, "diff", "--name-only", f"{args.base}..HEAD", "--", "reference/")
        .strip()
        .splitlines()
        if m and not m.startswith("reference/observatory/")
    ]
    require(not ref_moved, f"reference/ moved outside reference/observatory/: {ref_moved}")

    # ---- the namelist the documents print ------------------------------------------
    status = git(repo, "diff", "--name-status", f"{args.base}..HEAD")
    moved_rows = [r.split("\t") for r in status.strip().splitlines() if r]
    added = [r for r in moved_rows if r[0] == "A"]
    modified = [r for r in moved_rows if r[0] == "M"]
    other = [r for r in moved_rows if r[0] not in ("A", "M")]
    require(not other, f"the branch DELETES/RENAMES vs base — the docs do not say so: {other}")
    require(
        len(moved_rows) == EXPECT_MOVED,
        f"namelist is {len(moved_rows)} paths vs {args.base}; the docs print {EXPECT_MOVED}",
    )
    require(
        len(added) == EXPECT_MOVED_ADDED and len(modified) == EXPECT_MOVED_MODIFIED,
        f"namelist split is {len(added)}A/{len(modified)}M; the docs print "
        f"{EXPECT_MOVED_ADDED}A/{EXPECT_MOVED_MODIFIED}M",
    )
    shipped_moved = [
        r[1]
        for r in moved_rows
        if not r[1].startswith(MUST_NOT_HAVE_PREFIX)
        and r[1] not in MUST_NOT_HAVE_EXACT
    ]
    require(
        len(shipped_moved) == EXPECT_SHIPPED_MOVED,
        f"shipped moved paths = {len(shipped_moved)}; the docs print {EXPECT_SHIPPED_MOVED}",
    )

    if fails:
        print("PACK REFUSED — nothing written:")
        for f in fails:
            print("  *", f)
        return 1

    # ---- write it -------------------------------------------------------------------
    raw_bytes = 0
    disk_sha: dict[str, str] = {}
    with zipfile.ZipFile(args.out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name in payload:  # git ls-files order = sorted
            path = os.path.join(repo, name)
            with open(path, "rb") as fh:
                data = fh.read()
            disk_sha[name] = sha256_bytes(data)
            raw_bytes += len(data)
            z.writestr(name, data)

    # ---- verify the zip against the tree it was written from (entry by entry) -------
    with zipfile.ZipFile(args.out) as z:
        names = z.namelist()
        require(
            sorted(names) == sorted(payload),
            "zip namelist != payload (a stray rode or a file dropped)",
        )
        require(
            z.testzip() is None,
            f"zip CRC failure in {z.testzip()}",
        )
        for name in names:
            got = sha256_bytes(z.read(name))
            require(
                got == disk_sha[name],
                f"zip bytes != tree bytes for {name} ({got[:12]} vs {disk_sha[name][:12]})",
            )
    if fails:
        print("PACK FAILED VERIFICATION — refusing to name it done:")
        for f in fails:
            print("  *", f)
        os.remove(args.out)
        return 1

    zip_sha = sha256_bytes(open(args.out, "rb").read())
    zip_size = os.path.getsize(args.out)
    with open(args.out + ".sha256", "w") as fh:
        fh.write(f"{zip_sha}  {os.path.basename(args.out)}\n")

    print(f"packed {len(names)} entries from {head} (namelist base {args.base})")
    print(f"  raw payload   {raw_bytes:,} B")
    print(f"  zip           {args.out}  {zip_size:,} B")
    print(f"  sha256        {zip_sha}")
    print(f"  sidecar       {args.out}.sha256")
    print(
        f"  namelist vs {args.base}: {len(moved_rows)} paths "
        f"({len(added)} A / {len(modified)} M), {len(shipped_moved)} shipped"
    )
    print("  zip verified entry-by-entry against the working tree; exclusions asserted absent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
