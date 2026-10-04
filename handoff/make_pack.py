"""Build the round-5 delivery artefacts: overlay zip + sha256sums (bundle already created)."""
import subprocess, zipfile, hashlib, os

import sys; HOME = sys.argv[1]
REPO = os.path.join(HOME, "sparq")
OUT = os.path.join(HOME, "handoff")

files = subprocess.run(["git", "ls-files"], capture_output=True, text=True, cwd=REPO).stdout.splitlines()
print("tracked files:", len(files))

zp = os.path.join(OUT, "sparq-update-2026-10-04.zip")
with zipfile.ZipFile(zp, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for f in files:
        z.write(os.path.join(REPO, f), f)

names = set(zipfile.ZipFile(zp).namelist())
print("zip entries:", len(names), "size:", os.path.getsize(zp))
assert not any("target/" in n for n in names), "target leaked"
assert "Cargo.lock" not in names
for must in [
    "ROUND5-HANDOFF.md", "CHECKLIST.md", "SPARQ-PLAN.md", "MODULE-BUILD-GUIDE.md",
    "docs/adr/010-instrument-layer.md", "docs/api/instrument-wit/wit/world.wit",
    "tools/sparq-module-guest/src/lib.rs", "modules/out/main/sparqmod.toml",
    "reference/instrument-template/sparqmod.toml", "docs/api/instrument-wit/harness/smoke.mjs",
]:
    assert must in names, must
print("zip sanity: OK")

def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        for c in iter(lambda: fh.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()

lines = []
for p in [zp, os.path.join(OUT, "sparq-handoff-2026-10-04.bundle"), os.path.join(REPO, "ROUND5-HANDOFF.md")]:
    lines.append(f"{sha(p)}  {os.path.basename(p)}  ({os.path.getsize(p)} bytes)")
with open(os.path.join(OUT, "sha256sums.txt"), "w") as fh:
    fh.write("\n".join(lines) + "\n")
print("\n".join(lines))
