# WO-020 INC5 slices 1–2 — SATURN run sheet (the stream plane live + the instrument runtime)

**Sync bundle:** `sparq-update-2026-10-07-wo020-inc5r6.zip` (**485 entries**, FULL-TREE — overlays
any tree at or after the INC4 pack / git `96fe57d`; this session's base is the re-published main
`a9dbf31`, which already carries INC1–INC4) → extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. Excluded by rule (**your copies win**): `.git\`, `target\`, `Cargo.lock`, `logs\`,
`*.wav`, `handoff\`, and `SYNC-STAMP.txt` — **the stamp does NOT ride**: re-stamp on this machine
(step 2). The zip's sha256 is in the delivery message and in `handoff\sha256sums-wo020-inc5.txt`
(check with `certutil -hashfile <zip> SHA256` before extracting). Persistent history (sandbox
side): `handoff\sparq-wo020-inc5.bundle` + `sparq-wo020-inc5-delivery.bundle` (`a9dbf31`..the
delivery seal), each with a `.sha256` sidecar. Companions: `WO020-STATE.md` (the INC5 section
carries the full build record + the environment verdict), `CHECKLIST.md`, plan of record
`WO020-OBSERVATORY-PLAN.md`, design `docs/instrument-host.md`.

**Namelist vs `a9dbf31`:** 58 paths (34 A / 24 M), 38 shipped. The A paths: the runtime
(`crates/sparq-host-wasm/src/runtime/*` — 5 files), its 12-test integration file, the pinned noop
fixture (`tests/fixtures/noop/` — 4 files), the stream plane's live half
(`crates/sparq-streams/src/{fetch,broker}.rs`), `scripts/test007.bat`, this run sheet, the packer.

---

## 0. What this pack is — and the two honest states it carries

**Slice 1 (MEASURED in the sandbox):** `streams-net` landed — the live transport (ureq 3 +
rustls), the broker's scheduling core (cadence floors, replace-vs-accumulate as registry data,
last-good cache seeding), `BrokerProvider` behind the same trait the replay provider wears, and
the real `sparq streams probe [--live] | fetch | tail` verbs. Sandbox evidence: a real TLS fetch
of swpc.kp green; a live probe **22/23 HTTP 200** + FIRMS KEY NEEDED in words.

**Slice 2 (WRITTEN against the pinned wasmtime's real API; FIRST COMPILE IS YOURS):**
`sparq-host-wasm::runtime` — the loader (identity cross-check, prepare-before-anything on both
instances, fuel→Overrun, the memory limiter, wasmtime's own compile cache so no unsafe enters
Tier-2 host code), the four doors, and the gate's runtime stages (3 smoke / 4 golden ×2 /
5 measured / 6 visual + `preview.svg` + the at-rest publish). `sparq mod validate` runs the FULL
chain when built with `instrument-host`. The sandbox refuses wasmtime compilation categorically
(1 GB; `docs/instrument-host.md` §8, re-measured this session from two directions) — so the
feature-on cells below are the first compile. That is the recorded plan, not a surprise.

**Honest state 1 — INC5b is NOT in this pack:** PLAY with an instrument on the canvas still
refuses in words (#58), and the shell still shows the at-rest wall (now fed by stage 6's
publish), not a live-running guest. The registry's `register_instrument` door and the `Module`
face exist and are tested; the launch wiring (discovery → registration → executor adoption →
the shell's live-provider swap) is the next slice, named in `LATER.md`.

**Honest state 2 — the pre-existing red rides along:** `token_audit` still fails the §18 mockup
SVGs (on main before this pack; `gates.bat`'s `design conformance` row will be red on exactly
that, 14 groups / 298 occurrences).

## 1. Apply + re-stamp

1. **Extract** the zip at the repo root, overwriting.
2. **Re-stamp:** `python tools\sync_check.py --write --sync sparq-wo020-inc5r6-2026-10-07`
   (until then, `build.bat` / `test006`'s `[00b]` name the mismatch in words — EXPECTED).

## 2. The standing gates + the new cells

`scripts\gates.bat` — it now carries the INC5 rows itself: clippy+tests for
`sparq-host-wasm --features instrument-host` (the loader's first compile — expect a few minutes;
wasmtime is exact-pinned `=49.0.2` so the cost is stable), the stream-plane cells
(`streams`/`streams-net`), and the app cell. Expected: root **1034 / 0 / 1-ignored**;
`sparq-streams` **96 / 0** (both feature cells); `sparq-host-wasm --features streams` **28 / 0**;
app `streams,streams-net` **34 / 0**; and — first run anywhere — `--features instrument-host`
**12 runtime tests** (`tests/instrument_runtime.rs`, the §7 test plan). If wasmtime's compile
OOMs even on SATURN, that is a host fact to record in `WO020-STATE.md`, not a code defect (§8's
rule).

## 3. The device acceptance run — `scripts\test007.bat`

**Re-stamp (§1.2) BEFORE test007** — `build.bat` REFUSES on a stamp mismatch (defect #78), and
every later step would then run against the STALE exe (`unknown command streams/mod` — exactly
what the first device run hit, 2026-10-07; test007's new step [0] fails fast on it with the
remedy printed).

The §15.2 run-sheet as a script: **[A]** build (features now include
`instrument-host,streams,streams-net`) · **[B]** `sparq streams probe --live` — all 23 statuses
recorded (DONKI stays dead; §3.3's asterisks clear or become device facts; KEY NEEDED rows are
D14 working) · **[C]** the sealed component is in the pack · **[D=J]** `sparq mod validate
instruments\observatory\` — expect **GATE: COMPLETE + PASS** now: smoke, golden ×2 bit-exact with
the harness's pinned sha `bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`
recorded in stage 4's words, measured fuel/memory inside the declared class (stage 5), and
stage 6 regenerating `instruments\observatory\preview.svg` (the package becomes 5 of 5 files) +
publishing the at-rest IR · **[E/F]** the shell eyes (at-rest wall per honest state 1; dropdown/
SOLO/FOCUS on the canvas card) · **[G]** airplane mode: STALE at 3× → OFFLINE at 10× in words,
audio unaffected, restore → LIVE (drive it with `sparq streams fetch/tail` + the shell) ·
**[H]** `ui --headless 120` frame histogram · **[I]** the token re-theme (change
`color.signal.audio`, re-run `token_gen`, rebuild: the wall's phosphor traces re-theme with ZERO
package bytes changed) · **[K]** `gates.bat` · **[L]** re-stamp.

**Send back:** `logs\test007.log` + `test007-digest.log` + `logs\gates.log` + `logs\ui-digest.log`
+ screenshots of E/G/I + the stage-4/5 words from [D] (they carry the measured fuel/memory and
the golden hash — the acceptance lines are recorded, not invented).

## 4. If the first compile finds defects

The loader was written against the real generator's expansion (recipe in `WO020-STATE.md`'s INC5
section) and the vendored 49.0.2 sources, but it has never met rustc. Any compile error is a
device-session fix: smallest honest patch, re-run the cell, record the finding in
`WO020-STATE.md` ("found by building it" — the house rule). The 12 runtime tests then run the
§7 plan for real: smoke 1:1 against `smoke.mjs`'s assertions, the golden anchor across the
boundary, fuel→Overrun, the identity/port refusals, the doors in isolation, and the full chain
end-to-end on the noop fixture.
