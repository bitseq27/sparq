#!/usr/bin/env python3
"""make_pack_wo020.py — the WO-020 delivery packer: a self-verifying pre-pack gate.

RECONSTRUCTED A SECOND TIME for the INC6 delivery (session 10, 2026-10-08) from
WO020-STATE.md's 2026-10-06 delivery-table description and RESUME.md §5 — the
original rode the sealed tree and was lost in the re-publication; session 8's
reconstruction was never committed and did not survive the next sandbox. THIS
copy is committed (the r8 artefacts lesson: what is not in git is not durable).

It refuses to write a pack whose documents would lie about it. Gates 1–6 run
before the zip is built; 7–10 run on the in-memory zip BEFORE a byte is written
to disk. Any red = NO pack anywhere and the refusal named in words.

  1. the working tree is clean vs HEAD (git status --porcelain empty);
  2. the entry count (git ls-files minus handoff/ and SYNC-STAMP.txt) equals
     BOTH the command-line expectation AND the number printed in SYNC.md's top
     "Current bundle" entry — grepped from the document, not passed in;
  3. the shipped namelist (git diff --name-status BASE..HEAD minus handoff/)
     equals the counts printed in SYNC.md's "Namelist" line (N paths, M/A);
  4. the MUST-NOT-MOVE surfaces are unmoved vs BASE (the WIT, modules/, the
     stamp, reference/fixtures, instruments/, instruments-src/, design/mockups/);
  5. the r2 rule: the new stamp name is not the one SYNC-STAMP.txt carries;
  6. every file to be packed exists on disk;
  7. the exclusion rules are asserted absent from the zip (.git/, target/,
     Cargo.lock, logs/, *.wav, handoff/, SYNC-STAMP.txt);
  8. entry-by-entry sha256(zip bytes) == sha256(tree bytes), every entry;
  9. the component's pinned sha256 + size asserted INSIDE the zip;
 10. the must-ship set is present (the door documents, both run sheets, the new
     INC6 sources, the #93-trap files, the component).

Writes (only on full green): the zip at the workspace root (one level above the
repo), the committed copy under handoff/, and handoff/<SUMS> with the
sha256sum-checkable line for the handoff copy (cd handoff && sha256sum -c <SUMS>).

Usage (from the repo root):
  python3 handoff/make_pack_wo020.py BASE ZIP_NAME STAMP_NAME ENTRIES NAMELIST SUMS
  e.g.
  python3 handoff/make_pack_wo020.py b2445d6 \
      sparq-update-2026-10-08-wo020-inc6.zip sparq-wo020-inc6-2026-10-08 \
      495 32 sha256sums-wo020-inc6.txt

Exit 0 = every gate green + the three artefacts written. 1 = refused, in words.
"""
import hashlib
import io
import os
import re
import subprocess
import sys
import zipfile

COMPONENT = "instruments/observatory/observatory.wasm"
COMPONENT_SHA = "4b29a5ae22ce67ddd7873d21ae4095ded570614dea6d0c697d1637edafa1050b"
COMPONENT_SIZE = 206316

MUST_NOT_MOVE = (
    "docs/api/instrument-wit/",
    "modules/",
    "SYNC-STAMP.txt",
    "reference/fixtures",
    "instruments/",
    "instruments-src/",
    "design/mockups/",
)

MUST_SHIP = (
    "SYNC.md",
    "RESUME.md",
    "WO020-STATE.md",
    "CHECKLIST.md",
    "WO020-INC6-PLAN.md",
    "scripts/test007.bat",
    "scripts/test008.bat",
    "crates/sparq-app/src/ui/instrument_launch.rs",
    "crates/sparq-app/src/ui/live_display.rs",
    "crates/sparq-app/src/ui/streams_driver.rs",
    "crates/sparq-streams/src/store.rs",
    "modules/out/main/sparqmod.toml",
    "instruments/observatory/sparqmod.toml",
    COMPONENT,
)


def refuse(why: str) -> "NoReturn":  # noqa: F821 - the refusal IS the exit
    print(f"PACKER REFUSED: {why}")
    sys.exit(1)


def git(*args: str) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    if r.returncode != 0:
        refuse(f"git {' '.join(args)} -> rc {r.returncode}: {r.stderr.strip()}")
    return r.stdout


def sha_bytes(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def main() -> int:
    if len(sys.argv) != 7:
        print(__doc__)
        return 2
    base, zip_name, stamp_name, entries_s, namelist_s, sums_name = sys.argv[1:7]
    try:
        want_entries, want_namelist = int(entries_s), int(namelist_s)
    except ValueError:
        refuse(f"ENTRIES/NAMELIST must be integers, got {entries_s!r}/{namelist_s!r}")

    if not os.path.isfile("SYNC-STAMP.txt") or not os.path.isdir(".git"):
        refuse("run from the repo root (SYNC-STAMP.txt and .git must be here)")

    # -- gate 1: clean tree ------------------------------------------------
    dirty = git("status", "--porcelain").strip()
    if dirty:
        refuse(f"working tree not clean vs HEAD:\n{dirty}")
    print("gate 1 PASS: working tree clean vs HEAD")

    # -- gate 2: entry count, three ways -----------------------------------
    tracked = git("ls-files").splitlines()
    pack = [f for f in tracked if not f.startswith("handoff/") and f != "SYNC-STAMP.txt"]
    sync_txt = open("SYNC.md", encoding="utf-8").read()
    m = re.search(
        r"\*\*Current bundle: `(sparq-update-[^`]+\.zip)` \(a FULL-TREE pack, (\d+) entries",
        sync_txt,
    )
    if not m:
        refuse("SYNC.md's top 'Current bundle' entry does not match the expected shape")
    if m.group(1) != zip_name:
        refuse(f"SYNC.md names the zip {m.group(1)!r}, argv names {zip_name!r}")
    sync_entries = int(m.group(2))
    if not (len(pack) == want_entries == sync_entries):
        refuse(
            f"entry count disagreement: tree {len(pack)}, argv {want_entries}, "
            f"SYNC.md {sync_entries}"
        )
    print(f"gate 2 PASS: {len(pack)} entries — tree == argv == SYNC.md")

    # -- gate 3: shipped namelist vs BASE, three ways ----------------------
    diff = git("diff", "--name-status", f"{base}..HEAD").splitlines()
    shipped = []
    for line in diff:
        parts = line.split("\t")
        status, path = parts[0], parts[-1]
        if not path.startswith("handoff/"):
            shipped.append((status, path))
    n_m = sum(1 for s, _ in shipped if s == "M")
    n_a = sum(1 for s, _ in shipped if s == "A")
    if n_m + n_a != len(shipped):
        refuse(f"namelist has statuses other than M/A: {sorted(set(s for s, _ in shipped))}")
    m2 = re.search(
        r"\*\*Namelist\*\* \((\d+) paths vs the published main `[^`]+` — (\d+) M / (\d+) A",
        sync_txt,
    )
    if not m2:
        refuse("SYNC.md's 'Namelist' line does not match the expected shape")
    sync_n, sync_m, sync_a = int(m2.group(1)), int(m2.group(2)), int(m2.group(3))
    if not (len(shipped) == want_namelist == sync_n and n_m == sync_m and n_a == sync_a):
        refuse(
            f"namelist disagreement: tree {len(shipped)} ({n_m} M / {n_a} A), "
            f"argv {want_namelist}, SYNC.md {sync_n} ({sync_m} M / {sync_a} A)"
        )
    print(f"gate 3 PASS: namelist {len(shipped)} paths ({n_m} M / {n_a} A) — tree == argv == SYNC.md")

    # -- gate 4: MUST-NOT-MOVE surfaces unmoved vs BASE --------------------
    moved = [
        p
        for _, p in shipped
        if any(p == s.rstrip("/") or p.startswith(s) for s in MUST_NOT_MOVE)
    ]
    if moved:
        refuse(f"MUST-NOT-MOVE surfaces moved vs {base}: {moved}")
    stamp_diff = git("diff", "--stat", f"{base}..HEAD", "--", "SYNC-STAMP.txt").strip()
    if stamp_diff:
        refuse(f"SYNC-STAMP.txt differs vs {base}: {stamp_diff}")
    print(f"gate 4 PASS: MUST-NOT-MOVE surfaces unmoved vs {base} (incl. the stamp)")

    # -- gate 5: the r2 rule — a second zip must not share the first's stamp name
    stamp_txt = open("SYNC-STAMP.txt", encoding="utf-8").read()
    if stamp_name in stamp_txt:
        refuse(f"stamp name {stamp_name!r} is the one SYNC-STAMP.txt already carries (r2 rule)")
    print(f"gate 5 PASS: {stamp_name!r} differs from the stamp the sandbox carries (r2 rule)")

    # -- gate 6: every packed file exists ----------------------------------
    missing = [f for f in pack if not os.path.isfile(f)]
    if missing:
        refuse(f"files tracked but missing from disk (the #93 trap?): {missing}")
    print(f"gate 6 PASS: all {len(pack)} packed files exist on disk")

    # -- build the zip in memory -------------------------------------------
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f in pack:
            z.write(f, f)
    data = buf.getvalue()
    zf = zipfile.ZipFile(io.BytesIO(data))
    names = zf.namelist()

    # -- gate 7: exclusion rules asserted absent ---------------------------
    bad = [
        n
        for n in names
        if n.startswith(("handoff/", ".git/", "target/", "logs/"))
        or n in ("SYNC-STAMP.txt", "Cargo.lock")
        or n.endswith(".wav")
    ]
    if bad:
        refuse(f"excluded content leaked into the zip: {bad[:10]}")
    if sorted(names) != sorted(pack):
        refuse("zip namelist != computed pack list")
    if len(names) != want_entries:
        refuse(f"zip holds {len(names)} entries, expected {want_entries}")
    print(f"gate 7 PASS: exclusions absent; zip holds exactly {len(names)} entries")

    # -- gate 8: entry-by-entry zip<->tree hashes ---------------------------
    for n in names:
        zb = sha_bytes(zf.read(n))
        with open(n, "rb") as fh:
            tb = sha_bytes(fh.read())
        if zb != tb:
            refuse(f"zip<->tree hash mismatch on {n}")
    print(f"gate 8 PASS: {len(names)}/{len(names)} entry hashes zip == tree")

    # -- gate 9: the component, pinned, INSIDE the zip ----------------------
    comp = zf.read(COMPONENT)
    if sha_bytes(comp) != COMPONENT_SHA or len(comp) != COMPONENT_SIZE:
        refuse(
            f"component inside the zip is {sha_bytes(comp)[:16]}…/{len(comp)} B, "
            f"pinned {COMPONENT_SHA[:16]}…/{COMPONENT_SIZE} B"
        )
    print(f"gate 9 PASS: component {COMPONENT_SHA[:16]}… / {COMPONENT_SIZE} B asserted inside the zip")

    # -- gate 10: the must-ship set ----------------------------------------
    absent = [f for f in MUST_SHIP if f not in names]
    if absent:
        refuse(f"must-ship files absent from the zip: {absent}")
    print(f"gate 10 PASS: all {len(MUST_SHIP)} must-ship files present")

    # -- write the artefacts -------------------------------------------------
    zip_sha = sha_bytes(data)
    root_out = os.path.abspath(os.path.join(os.pardir, zip_name))
    handoff_out = os.path.join("handoff", zip_name)
    sums_out = os.path.join("handoff", sums_name)
    with open(root_out, "wb") as fh:
        fh.write(data)
    with open(handoff_out, "wb") as fh:
        fh.write(data)
    with open(sums_out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(f"{zip_sha}  {zip_name}\n")
    print(f"PACK WRITTEN (every gate green):")
    print(f"  {root_out}  ({len(data)} B)")
    print(f"  {handoff_out}  (the committed copy)")
    print(f"  {sums_out}")
    print(f"  sha256 {zip_sha}")
    print(f"  size   {len(data)}")
    print(f"  entries {len(names)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
