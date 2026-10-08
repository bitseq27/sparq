# Sync manifest — WO-020 INC6 "the live instrument": S1–S5 sandbox half + the device round (2026-10-08)

**Current bundle: `sparq-update-2026-10-08-wo020-inc6.zip` (a FULL-TREE pack, 495 entries —
overlays any tree at or after the r7 pack, which is what the device runs today; r8's documents
delta rides inside; built on the published main `b2445d6` + the 15 INC6 slice commits + this
delivery's documents commit).** Excluded by the standing rule: `.git/`, `target/`,
`Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and `SYNC-STAMP.txt` — **the stamp does NOT ride
and did not move from the sandbox**: after applying AT THE REPO ROOT, overwriting, re-run
`python tools\sync_check.py --write --sync sparq-wo020-inc6-2026-10-08` on the machine that
will own the tree. Until then `build.bat` and test007/test008's `[0]` name the mismatch in
words — EXPECTED, and `sync_check` against the device's r7-era stamp is red on exactly
**13 file(s) differ** (12 SIZE rows — the six `sparq-app` UI/streams files, the five
`sparq-ui/canvas` files, `scripts/test007.bat` — plus the src fingerprint
`106f/3202925B` vs the stamp's `103f/2948000B`) **+ 4 EXTRA warnings** (the new
`instrument_launch.rs`, `live_display.rs`, `streams_driver.rs`, `test008.bat`) — that list is
the definitive answer to "did the sync land"; the re-stamp clears it. The zip's sha256 is in
the delivery message and in `handoff/sha256sums-wo020-inc6.txt`. **Provenance:** sandbox
`main` off the published `b2445d6`; durable history in
`handoff/sparq-wo020-inc6-delivery.bundle` (`b2445d6..HEAD` at the artefacts seal — it carries
the whole INC6 branch INCLUDING the committed zip copy + sums; prerequisite: any clone of the
published main; `.sha256` sidecar; supersedes the five slice bundles for recovery, which stay
committed as the slice records); packer `handoff/make_pack_wo020.py` — **RECONSTRUCTED A SECOND
TIME this session** (the original was lost in the re-publication, session 8's reconstruction
was never committed; this one IS, the r8 artefacts lesson — its header declares it), all ten
gates ran green at the pack: clean tree, 495 == argv == this manifest, namelist 32 == argv ==
this manifest, MUST-NOT-MOVE unmoved vs `b2445d6`, the r2 stamp-name rule, entry-by-entry
zip↔tree hashes, the component `4b29a5ae…`/206 316 B asserted inside, the must-ship set
present.

**What it carries — BEHAVIOUR (unlike r8, which was records-only): the whole INC6 sandbox
half, slices S1–S5, each sealed + injection-proved in session 9** (full record:
`WO020-STATE.md`'s INC6 section). **S1** the resizable instrument card (O-1/D15): `Node.size`
+ `Op::ResizeNode` (ONE undo per drag), the HALF-face default (the Observatory's card, after
S5, 1120×768; band 1088×560), the class-L corner handle (`canvas/resize/{id}` in the audit),
the 480×248…declared-face clamp in the op constructor, the 8-px snap, FIT/FOCUS/marquee
reading the size, the at-rest wall scaling into the band. **S2** the typed panel band (D16):
the manifest's 27 `[[ui.panel.widgets]]` typed into `InstrumentDisplay.widgets`, painted at
Full LOD on the 44-px floor, enums through the EXISTING picker, toggles/sliders through the
undoable param door — **defect #100 found and fixed on the way** (manifest-parsed enums'
zeroed range). **S3** the overrides store + the STREAMS dock tab (O-2/O-3/O-4, D18/D20):
`sparq-streams/src/store.rs` (user-data TOML; the 10 s floor refuses in words and keeps the
old value; env wins over the file; the key has NO reading door — mask + state words only;
atomic save), the broker reading override-then-registry, the dock tab's honest rows. **S4**
the live plane's seams (D17/D19): the D19 token `instrument_display_hz = 15`, the `LiveDisplay`
seam + pacer (edits force; five consecutive overruns bypass with §3's words), the driver's
hermetic `poll_once` + the thin device thread, the provider swap (LIVE vs `AT REST —
fixtures`), and the **DEVICE-FIRST-COMPILE** launch registration (`instrument_launch`:
load_package → the display shelf + the registry's factory door — PLAY's #58 lifts
structurally; desk-checked call by call against the vendored API; the 1 GB sandbox refuses
wasmtime categorically, so THAT cell first compiles on the device — `scripts/test008.bat` is
its run sheet). **S5** the card's RATE row + convergence (D20's second door): the host row
writing the SAME per-stream override the tab does, its words naming the governed stream + the
shared-feed truth, following the CELL dropdown the same frame; chrome 168→208 (default card
1120×768, maximum 2208×1328 — the O-1 BAND numbers untouched); the convergence sheet re-shot
at Full LOD (sha `56e446d3…`, recorded in STATE; the PNG re-shoot WITH THE LIVE WALL is the
device round's). **Gates at the S5 seal (recorded, not invented):** workspace **1062/0**;
`sparq-app --features ui,streams` **56/1** (THE known sandbox-only env red,
`ui::live::…level_follows` — device/MSVC runs it green); streams(+net) **106/0**; observatory
**124/0** + goldens 6/6; `ui --audit` **PASS both faces** (`ui` 80 rows; `ui,streams` **88
rows**); fmt/clippy `-D warnings` green on six cells; fifteen injection proofs across S1–S5,
each reverted byte-identical. Python gates re-run in THIS delivery session: `check_text_io`
clean, `token_gen --check` rc 0, `unsafe_audit` clean, `module_docs --check` 24/24,
`token_audit` at its standing §18 mockup red (14 groups/298 — the IDENTICAL set, zero new),
`sync_check` the expected pre-re-stamp red named above. MUST-NOT-MOVE surfaces unmoved vs
`b2445d6` (the WIT, `modules/`, the stamp, the fixtures, `instruments/`, `instruments-src/`,
the mockups); the component ships byte-pinned: `instruments/observatory/observatory.wasm`
sha256 `4b29a5ae22ce67ddd7873d21ae4095ded570614dea6d0c697d1637edafa1050b`, 206 316 B, asserted
inside the pack by the packer.

**Namelist** (32 paths vs the published main `b2445d6` — 27 M / 5 A; the pack carries the whole
tree, this names what moved; `handoff/` artefacts do not ride): **A** —
`crates/sparq-app/src/ui/instrument_launch.rs` · `crates/sparq-app/src/ui/live_display.rs` ·
`crates/sparq-app/src/ui/streams_driver.rs` · `crates/sparq-streams/src/store.rs` ·
`scripts/test008.bat` (the S4 run sheet). **M (code, 15)** — `crates/sparq-app/src/streams.rs`
· `crates/sparq-app/src/ui/{canvas_ui,headless,mod,shell_ui,window}.rs` ·
`crates/sparq-streams/src/{broker,cache,lib,window}.rs` ·
`crates/sparq-ui/src/canvas/{connect,inspector,interact,layout,model}.rs`. **M (tokens, 6)** —
`design/tokens/layout.toml` (the D19 row) + the five generated faces. **M (records/docs, 5)** —
`WO020-STATE.md` · `CHECKLIST.md` · `RESUME.md` · `SYNC.md` · `docs/formats/project.md` (the
reserved `size` line). **M (scripts, 1)** — `scripts/test007.bat` (the [0] remedy + the [L]
re-stamp now name THIS delivery's stamp — the r8 echo-words precedent; found stale in this
session's prep, [L] would otherwise have re-stamped the device `sparq-wo020-inc5r3-<date>`).

**THE ASK — the DEVICE round (this pack exists for it):** apply at the repo root → re-stamp
`sparq-wo020-inc6-2026-10-08` → build (FIRST BUILD IS LONG, wasmtime compiles on SATURN —
session 2 proved it) → then: **(1)** `scripts\test007.bat` — the uninterrupted **[K]**
`gates.bat` re-run (session 8's row was CANCELLED, not failed: every `[FAIL]` was exit
`0xc000013a` = Ctrl+C, zero real failures; **DO NOT CANCEL** — the debug rebuild + wasmtime's
debug compile is long, not hung); **(2)** `scripts\test008.bat` — the S4 device half (the
launch words, PLAY with the Observatory — #58 gone, the living wall, the fuel-watchdog
injection on a max_fuel=100000 copy, the STREAMS tab LIVE + the NASA-key flip = S3's device
half, the on-device redaction findstr; send back `logs\test008.log` + the digest + the E/F/H
screenshots); **(3)** the **[G]/[I]** eyes (airplane-mode words, the token re-theme) with
screenshots; **(4)** the convergence **PNG** re-shoot with the live wall (the wo012
precedent); **(5)** operator rulings on the two carried flags: the manifest's declared
touch_class **M** vs the band's given 44-px **S** floor (an M-sized band is a D6 movement —
card +24 px), and the one-display-instance-per-module limit (a second node of a module stays
at rest, in words).

---

## Previous bundle — WO-020 INC5 r8: the session-8 records + the INC6 commission (2026-10-07)

**Current bundle: `sparq-update-2026-10-07-wo020-inc5r8.zip` (a FULL-TREE pack, 489 entries —
overlays any tree at or after the r7 pack; built on the published main `7ed2e08` + the
session-7/8 commits + the r8 documents seal).** Excluded by the standing rule: `.git/`,
`target/`, `Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and `SYNC-STAMP.txt` — **the stamp does
NOT ride**: if applied, re-run `python tools\sync_check.py --write --sync
sparq-wo020-inc5r8-2026-10-07` on the machine that will own the tree. The zip's sha256 is in
the delivery message and in `handoff/sha256sums-wo020-inc5r8.txt`. Provenance: sandbox `main`
off the published `7ed2e08`; durable history in `handoff/sparq-wo020-inc5r8.bundle`
(`7ed2e08..HEAD` at the documents seal, self-contained, `.sha256` sidecar — it supersedes the
r7 and session-8 bundles, which stay committed as the increment record); packer
`handoff/make_pack_wo020.py` (the reconstructed self-verifying gate).

**What it carries — RECORDS, not behaviour:** the device session-8 lines recorded in
`WO020-STATE.md` ([D] **GATE: PASS the whole chain** on r7: stage 4 `bdf59cdb…` ×2 bit-exact on
component `4b29a5ae…`, the two runs' fuels EQUAL at 12 390 844 — #98/#99 device-proven; stage 5
inside the ruled 32 000 000; [B] 20/23 + the two DEMO_KEY 429s; [H] p99 622 µs; the [K]
cancellation named as a cancellation — every exit code `0xc000013a`, zero test failures), the
**DEVICE-CONFIRMED clauses on ledger rows #97/#98/#99** (#97 by the operator's own words: "the
zoom is working without OOM"), test007.bat's [C] echo corrected to the component the pack
actually ships + the run sheet's do-not-cancel-[K] words, **`WO020-INC6-PLAN.md` — the
operator's window report commissioned as the next increment** ("the live instrument": the four
rulings O-1…O-4, D15–D20, slices S1–S5, superseding LATER.md's INC5b door), and **`RESUME.md`
REWRITTEN as the current new-session prompt** (the 2026-09-22 resume was four work orders
stale; git history keeps it).

**Device application is OPTIONAL** — nothing here changes a compiled byte except test007.bat's
echo text. The device stays correct at r7 + its own stamp; if r8 is applied, re-stamp r8. The
device's outstanding list is unchanged: the [K] gates re-run (uninterrupted — the debug rebuild
+ wasmtime's debug compile is LONG, not hung), the [G]/[I] eyes + screenshots.

**Namelist** (21 paths vs the r7 pack — 20 M / 1 A; the pack carries the whole tree, this names
what moved; `handoff/` artefacts do not ride): `WO020-INC6-PLAN.md` (**A** — the commission) ·
`RESUME.md` (rewritten) · `WO020-STATE.md`, `CHECKLIST.md`, `LATER.md`, `SYNC.md`,
`WO020-INC5-RUN-SHEET.md` (the records) · `scripts/test007.bat` (the [C] words + the [0]
remedy's stamp name).

**THE ASK:** none on the device beyond its outstanding list. The next pack carries **INC6 slice
S1 — the resizable instrument card** (O-1: drag-resizable, default half-size 1088×560 band,
clamped ¼-face → the declared 2176×1120; undoable; audit-registered handle), built and gated in
the sandbox before it ships.

---

## Previous bundle — WO-020 INC5 r7: the session-7 defect fixes (#97/#98/#99) + the max_fuel re-baseline (2026-10-07)

**Current bundle: `sparq-update-2026-10-07-wo020-inc5r7.zip` (a FULL-TREE pack, 488 entries —
overlays any tree at or after the r6 pack; built on the published main `7ed2e08` + the session-7
commit + this delivery's documents commit).** Excluded by the standing rule: `.git/`, `target/`,
`Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and `SYNC-STAMP.txt` — **the stamp does NOT ride**:
after applying, re-run `python tools\sync_check.py --write --sync sparq-wo020-inc5r7-2026-10-07`
on the machine that will own the tree; until then `build.bat` / `test006`'s `[00b]` name the
mismatch in words (EXPECTED). The zip's sha256 is in the delivery message and in
`handoff/sha256sums-wo020-inc5r7.txt`. **Provenance:** sandbox `main` off the published
`7ed2e08`; durable history in `handoff/sparq-wo020-inc5r7.bundle` (`7ed2e08..HEAD` at the
documents seal, `.sha256` sidecar; prerequisite: any clone of the published main); packer
`handoff/make_pack_wo020.py` — **RECONSTRUCTED this session** from the state card's description
of the lost original (the #96 precedent; its header declares it), same gates: clean tree at
pack time, the entry count asserted against this manifest and the run sheet, the MUST-NOT-MOVE
surfaces diffed vs base, entry-by-entry zip↔tree hashes, the component's sha asserted inside.

**ONE HAND-STEP ON APPLY:** delete `instruments\observatory\preview.svg` from the device tree —
an overlay zip cannot delete, and this pack DELIBERATELY ships without it: the checked-in copy
was the stale stage-6 sync-back from the OLD component (defect #98 — its bytes carry the
ground-over-body order the INC4 fix moved under). Stage 6 regenerates the file from the fresh
component at the next validate.

**What it carries:** the session-7 defect trio + one operator ruling — evidence and reasoning in
`WO020-STATE.md` (session 7) and the CHECKLIST ledger rows:
* **#97 — the operator's report, "sparq becomes unstable when the zoom buttons are used while the
  observatory is loaded."** `dash_segments` (the egui display-list painter) walked in f32 and
  stalled at EVERY toolbar zoom except exactly 1.0: rounding drift parks `phase` within an ulp
  of a dash boundary, the advance rounds back onto itself, and the shell either froze (gap
  branch) or grew zero-length segments until the OOM killer took the process (dash branch —
  reproduced: SIGKILL 3.5 s into the regression test on the old walk). All 130 dotted hairlines
  of the at-rest wall hit at once at every painted ladder value (0.8333…, 0.6944…, 0.5787…,
  0.4823…, 0.4019… down; 0.36, 0.432, 0.5184, 0.6221, 0.7465, 0.8958 up). Fixed: the walk
  computes in f64 + a totality guard; two regression tests pin the exact ladder values (proven
  failable). The SVG back end rides `stroke-dasharray` — no golden moves.
* **#98 — stage 4's hash off the pin (`5acfc1e6…` ≠ `bdf59cdb…`): the checked-in
  `observatory.wasm` (`3f775856…`) PREDATED INC4's `cell_ground` draw-order fix.** Repackaged
  around the rebuilt component **`4b29a5ae…`, 206 316 B** (the documented recipe: rustc 1.99.0 +
  wasm-tools 1.261.0); the manifest regenerated; the cross-boundary pin does NOT move —
  `bdf59cdb…` is the current core, harness goldens green (6/6).
* **#99 — stage 4's moved fuel (`12 707 802 vs 12 401 514`): the golden stage compared the
  FIRST draw against the second — a cold heap against a warm one.** wasmtime meters
  `memory.grow` per page; both numbers were fully deterministic (two independent loads each
  burned exactly 12 707 802 on their first draw). Fixed: one discarded warm-up draw before the
  measured pair; stage 5 KEEPS the cold draw on purpose (the declared budget's worst case); the
  failure words name the fuel-only class.
* **Ruling — `capabilities.max_fuel` 2 000 000 → 32 000 000**, operator ruling 2026-10-07, over
  the device's own stage-5 measurement (draw 12 707 802 cold / 12 401 514 warm; process blocks
  9 260; memory 1.2/3.2 MB of 64 MB) — recorded in `gen_manifest.py`, the package README and
  WO020-STATE session 7.

**Sandbox gates at the seal (MEASURED):** root **1034 / 0**; observatory workspace **124 / 0** +
harness goldens **6/6** (the pin holds) + fmt + clippy clean; `sparq-app --features ui`
**45 / 46** — the one red is the environment-sensitive `ui::live` level-follows test, which
fails identically on the PRISTINE tree in this sandbox (a host fact, not this pack's); fmt
clean; clippy `-D warnings` clean (default + ui cells); the python gates clean
(`check_text_io`, `make_coastline --check`, `gen_manifest --check`). The `instrument-host` cell
stays device-first-compile — the 1 GB wall re-confirmed from a third direction
(`cranelift-assembler-x64` OOM); the `stages.rs` patch is desk-checked against the adjacent
call patterns per the run sheet's §4 discipline. **The pre-existing red rides along:**
`token_audit` vs the §18 mockup SVGs (on main before this pack).

**Namelist** (15 paths vs git `7ed2e08` — 14 M / 1 D; the pack carries the whole tree, this
names what moved; the `handoff/` artefacts do not ride):
`crates/sparq-app/src/ui/displaylist_egui.rs` (#97) ·
`crates/sparq-host-wasm/src/runtime/stages.rs` (#99) ·
`instruments-src/observatory/{gen_manifest.py, package/sparqmod.toml, package/README.md}` (the
re-baseline) · `instruments/observatory/{sparqmod.toml, README.md, observatory.wasm}` (the
repackage; the component is `4b29a5ae…`) · `instruments/observatory/preview.svg` (**DELETED** —
the stale sync-back; the hand-step above) · `scripts/test007.bat` (step [0]'s remedy word names
the r7 re-stamp) · the records (`WO020-STATE.md`, `CHECKLIST.md`, `LATER.md`,
`WO020-INC5-RUN-SHEET.md`, `SYNC.md`).

**THE ASK — the operator's hands on SATURN:** apply → **delete the stale `preview.svg`** →
re-stamp `sparq-wo020-inc5r7-2026-10-07` → `scripts\gates.bat` → `scripts\test007.bat`. [D]
expects **GATE: COMPLETE + PASS**: stage 4's words `golden render ×2 bit-exact:
bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`, component `4b29a5ae…`, the
two runs' fuels EQUAL (warm); stage 5 inside the declared 32 M class; stage 6 rewriting
`preview.svg` (the package back to 5 of 5) + re-publishing the at-rest IR. [E/F] grew the zoom
eyes: with the Observatory on the canvas, walk the −/+ ladder through every value — the shell
must survive each and the graticules must stay dotted (#97's regression, by eye). Send back
`logs\test007.log` + `test007-digest.log` + `logs\gates.log` + the stage-4/5 words + the zoom
verdict.

---

## Previous bundle — WO-020 INC5 slices 1–2: the stream plane live + the instrument runtime (2026-10-07, r6)

**Current bundle: `sparq-update-2026-10-07-wo020-inc5r6.zip` (a FULL-TREE pack, 485 entries —
overlays any tree at or after the INC4 pack / git `96fe57d`; this session built on the
re-published main `a9dbf31`, which carries INC1–INC4).** Excluded by the standing rule:
`.git/`, `target/`, `Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and `SYNC-STAMP.txt` — **the
stamp does NOT ride**: after applying, re-run `python tools\sync_check.py --write --sync
sparq-wo020-inc5r6-2026-10-07` on the machine that will own the tree; until then `build.bat` /
`test006`'s `[00b]` name the mismatch in words (EXPECTED). The zip's sha256 is in the delivery
message and in `handoff/sha256sums-wo020-inc5r6.txt`. **Provenance:** sandbox branch `wo020-inc5`
off main `a9dbf31`; durable history in `handoff/sparq-wo020-inc5.bundle` +
`sparq-wo020-inc5-delivery.bundle` (`a9dbf31`..the delivery seal), each with a `.sha256` sidecar;
packer `handoff/make_pack_wo020.py` (self-verifying: entry-by-entry zip↔tree hashes, count
assertions against this manifest and the run sheet, MUST-NOT-MOVE surfaces diffed vs base).

**What it carries:** WO-020 **INC5 slices 1–2**. Slice 1 (MEASURED in the sandbox): `streams-net`
landed — `sparq-streams::{fetch,broker}` (the endpoint fill mirroring the recorder exactly, the
one-retry policy, key redaction; cadence floors counting attempts, replace-vs-accumulate as the
registry's new `accumulate` column — the three SWPC summary feeds), `HttpTransport` (ureq 3 +
rustls + webpki-roots — compiles AND runs on the 1 GB box), `BrokerProvider` behind the same
`StreamProvider` trait, the real `sparq streams probe [--live]|fetch|tail` verbs
(`record-fixtures` stays a refusal naming its owner: `tools/streams_record.py` — one owner of the
fixture format), the recorder's key-redaction fix, and the CI stream-plane cells. Slice 2
(WRITTEN against the pinned wasmtime's REAL API — the vendored generator run over the frozen WIT;
FIRST COMPILE IS THE DEVICE's / CI's): `sparq-host-wasm::runtime` — the loader (§2's order:
compile via wasmtime's own cache [the pin gained the `cache` feature: §2.1 with NO unsafe in
Tier-2 host code] → identity cross-check → prepare on both instances), the four doors (tokens =
the checked-in bundle verbatim; host = injected clock + the KERNEL's seed derivation + RT-violation
capture; assets = manifest-scoped, v1 `not-found` honestly; sources = D4 over the provider,
serving flag-stamped snapshots — the golden's parity rule), fuel→`BlockStatus::Overrun` (§3), the
memory limiter, `WasmInstrument` (the `Module` face) + `Registry::register_instrument` (the
SharedFactory door), the gate's runtime stages 3/4/5-measured/6 (smoke mirror, golden ×2,
fuel/memory vs the declared class, LOD ceilings + preview.svg through the SHELL's painter + the
at-rest publish), `validate::run_gated` wired into `sparq mod validate`, the noop fixture REBUILT
+ sha-pinned (`df14d18f781ea477596db552e60bdb9df06344bc673593c38148e28a3d546c65`, 53 747 B), the
§7 test plan as 12 integration tests, and `scripts/test007.bat` (the §15.2 run-sheet A–L) +
gates/build/CI cells. Sandbox gates at the seal: root **1034 / 0 / 1-ignored**; streams **96 / 0**
both cells; host-wasm+streams **28 / 0**; app streams-net **34 / 0**; observatory **124 / 0**;
fmt/clippy clean; every python gate; the zero-dep promise verified by `cargo tree`; LIVE: smoke
fetch green, probe **22/23 HTTP 200** + FIRMS KEY NEEDED in words. **Two honest states ride
along:** PLAY with an instrument still refuses in words (#58 — the launch wiring is INC5b, named
in LATER.md), and `token_audit` still fails the pre-existing §18 mockup SVGs (`gates.bat`'s
`design conformance` row red on exactly that — on main before this pack).

**Namelist** (58 paths vs git `a9dbf31` — 34 A / 24 M; the pack carries the whole tree, this
names what moved; the `handoff/` artefacts do not ride): `crates/sparq-host-wasm/` (10 — the
runtime's 5 modules, the 12-test integration file, the 4-file noop fixture, validate.rs,
sources.rs, Cargo.toml, lib.rs) · `crates/sparq-streams/` (6 — fetch.rs, broker.rs, registry.rs,
streams.toml, lib.rs, Cargo.toml) · `crates/sparq-module-api/src/registry.rs` (the
register_instrument door) · `crates/sparq-app/` (3 — instruments.rs, streams.rs, Cargo.toml) ·
`crates/sparq-ui/src/displaylist.rs` (the hygiene fmt wrap) · `scripts/` (3 — test007.bat new,
gates.bat, build.bat) · `.github/workflows/ci.yml` · `tools/streams_record.py` (the redaction
fix) · `docs/instrument-host.md` (the status line) · root docs (6 — `WO020-INC5-RUN-SHEET.md`
new; `WO020-STATE.md`, `CHECKLIST.md`, `SYNC.md`, `LATER.md`, `Cargo.toml`).

**THE ASK — the operator's hands on SATURN:** run `WO020-INC5-RUN-SHEET.md` (apply → re-stamp →
`gates.bat` → `test007.bat` A–L). The feature-on cells are the loader's FIRST COMPILE anywhere;
[D] is the first FULL hand-in gate (expect COMPLETE + PASS, the golden `bdf59cdb…` in stage 4's
words, `preview.svg` regenerated). Send back `logs\test007.log` + `test007-digest.log` +
`logs\gates.log` + screenshots of E/G/I. Then INC5b (launch registration, PLAY #58, the shell's
live-provider swap) closes the increment.

---

## Previous bundle — WO-020 The Observatory: INC1–INC4 (2026-10-06)



**Current bundle: `sparq-update-2026-10-06-wo020-inc4.zip` (a FULL-TREE pack, 471 entries —
defect #94's remedy stands: it overlays any tree at or after `sparq-update-2026-10-05-r8.zip` /
git `96fe57d` and does not depend on a base the receiver might not have).** Excluded from the
pack by the standing rule: `.git/`, `target/`, `Cargo.lock` (untracked by house rule; cargo
re-resolves on first build), `logs/`, `*.wav`, `handoff/` (the receiver's copies win), and
`SYNC-STAMP.txt` — **the stamp does NOT ride** (rev-3 pattern): after applying, re-run
`python tools\sync_check.py --write --sync sparq-wo020-inc4-2026-10-06` on the machine that will
own the tree; until then `scripts\build.bat` / `test006`'s `[00b]` name the stamp mismatch in
words, which is EXPECTED, not a failure. The zip's sha256 is recorded in the delivery message and
in `handoff/sha256sums-wo020.txt` (which rides the repo, not the zip). **Provenance:** sandbox
branch `wo020-observatory` off main `96fe57d` (the operator's Fresh-start re-publish, which
already carries WO-020 INC1+INC2, the plan and the §18 mockups); durable history in
`handoff/sparq-wo020-inc1…inc4.bundle` + the closing `sparq-wo020-delivery.bundle`
(`96fe57d`..the delivery commit), each with a `.sha256` sidecar; packer
`handoff/make_pack_wo020.py` (self-verifying: entry-by-entry zip↔tree hashes, count assertions
against this manifest and the run sheet, MUST-NOT-MOVE surfaces diffed vs base — it refuses to
write a pack whose documents would lie about it).

**What it carries:** WO-020 "The Observatory" (`dat/observatory`) **through INC4** — the stream
plane's hermetic half (INC1, on main already: `sparq-streams`, the 23-stream registry as data,
all 23 fixtures at HTTP 200), the contract additions (INC2, on main: ADR-011, the `stream`
binding, `E-STREAM-UNKNOWN`), **INC3 the guest**: the `instruments-src/observatory/` workspace
(pure core + wasm glue + native harness over one object, D13), the coastline asset (128 rings /
3 500 pts / 14 524 B), the generated manifest (26 params, 27 toolbar widgets, 16 stream
bindings), the **206 081 B component** (`wasm-tools validate` clean, `sparq:instrument/guest@1.1.0`),
the 4-file package `instruments/observatory/`, the pinned golden IR
(`bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`), and **INC4 the host**:
`sparq-ui::{json,displaylist}` (zero-dep painter core + SVG backend), the egui backend, the
dropdown picker (**round-4's OWED item 11 discharged**; 44 px rows, undoable), camera FOCUS, D6
wall-card sizing (2208×1288), the browser's instrument grouping, the at-rest wall in the shell
(`SPARQ_ATREST` / cache dir, WORDS when none), `sparq instrument render`. Sandbox gates at the
seal: root **1033 / 0 / 1-ignored**; observatory workspace **124 + 6**; `sparq-streams`
**75 / 0**; clippy clean in every runnable cell; fmt clean both workspaces; every python gate;
`ui --audit` **PASS**, coverage **97.5 %**; `mod validate` **PARTIAL exit 1 BY DESIGN** (stages
3/4/6 refuse in words — the runtime half is INC5); goldens untouched (`ba577186c988db21`); WIT
pins, the 24 module manifests and `SYNC-STAMP.txt` unmoved (asserted by the packer). **Two
honest states ride along:** `token_audit` still fails the pre-existing §18 mockup SVGs (14 groups
/ 298 occurrences — on main before this pack; `gates.bat`'s `design conformance` row will be red
on exactly that), and **PLAY with the Observatory on canvas refuses in words (#58)** until INC5's
wasmtime loader registers packages.

**Namelist** (95 paths vs git `96fe57d` — 61 A / 34 M; the pack carries the whole tree, this
names what moved; the 10 `handoff/` artefacts — inc3/inc4 + delivery bundles and sidecars, the
packer, the pack's committed copy, the sums — do not ride): `instruments-src/observatory/**` (34 —
the whole guest workspace: core 17, wasm glue 2, harness 9, package 3, `gen_manifest.py`, the
coastline asset) · `instruments/observatory/` (4 — the shipped package incl. the component) ·
`crates/sparq-ui/` (11 — `json.rs` + `displaylist.rs` + `canvas/picker.rs` new; browser, inset,
inspector, interact, layout, model, mod, lib touched) · `crates/sparq-app/` (11 — `ui/atrest.rs` +
`ui/displaylist_egui.rs` new; canvas_ui, headless, shell_ui, ui/mod, bridge, cli, instruments,
Cargo.toml, `tests/r3_controls.rs`) · `crates/sparq-host-wasm/` (3 — `sources.rs` new + the
`streams` feature) · `crates/sparq-module-api/` (2 — `ParamSpec.options` decode + manifest) ·
`crates/sparq-audio/tests/portless_instrument.rs` (the D5 proof, +9 tests) · `design/tokens/` (6
— additive `layout.marker.radius_s/m/l` + regenerated bundle) · `tools/` (5 —
`make_coastline.py`, `observatory_package.py`, `fetch_wasm_tools.py` new; `streams_record.py`
text-I/O fix, `token_gen.py` LUT lookups) · `reference/observatory/` (2 — the INC4 convergence
sheet + README) · root docs (6 — `WO020-INC4-RUN-SHEET.md` new; `SYNC.md`, `WO020-STATE.md`,
`CHECKLIST.md`, `LATER.md`, `.gitignore`).

**THE ASK:** apply → re-stamp → `scripts\gates.bat` (expect the single pre-existing
`design conformance` red, everything else ok) → the WO-020 device column of
`WO020-INC4-RUN-SHEET.md` §2 (first device measurements: streams 23, guest tests + budget block
exact-match, `mod validate` PARTIAL, the at-rest render via `SPARQ_ATREST`, the shell visual row,
the audit) → send back §3's list. The Windows at-rest cache-dir disagreement (harness
`%USERPROFILE%\.cache\…` vs app `%APPDATA%\…`) is recorded in `LATER.md` — use the env door; INC5
aligns it. Standing asks from ROUND8-HANDOFF §7 step 4 unchanged. **Next: WO-020 INC5**
(device-only: wasmtime loader, `BrokerProvider`, stage-6 at-rest publishing, the full validate
chain, `scripts\test007.bat` live acceptance) on a ≥ 2 GB host.

---

## Previous bundle — operator UI rounds: rev 4 (2026-10-02), the cable-node / living-wells round

**Rev 4 was `sparq-update-2026-10-02.zip` (operator round 4 of 2026-10-02 — a
FULL-TREE pack, defect #94's remedy: it overlays any tree at git `871d725` or later and does not
depend on a base the receiver might not have).** Excluded from the pack by rule: `.git/`,
`target/`, `Cargo.lock` (regenerates on first build), `logs/`, `*.wav`, and `SYNC-STAMP.txt` —
**the stamp does NOT ride in the pack** (rev-3 pattern): after applying, re-run
`python tools\sync_check.py --write --sync sparq-round4-2026-10-02` on the machine that will own
the tree; until then `scripts\build.bat` names the stamp mismatch in words, which is EXPECTED,
not a failure. **Provenance:** round-4's code was lost with the sandbox that wrote it (only
`ROUND4-HANDOFF.md` reached git); this pack is the REBUILD from that handoff on a clean clone,
re-measured end to end on Linux/rustc 1.99.0 — see `CHECKLIST.md`'s round-4 paragraph for the
three provenance notes (defect #95 pre-existing, mutation-hash baseline moved to
`8143e1ddfd8fb261` on this box, and defect #96: the pack's `modules/out/main/sparqmod.toml` is
a DECLARED RECONSTRUCTION — the #93 `out/`-drop trap re-struck the sealed tree and no byte-exact
copy survived; rebuilt from its generated doc + the `OutMain` implementation, proven
semantically identical at both layers, its stamp row moved while the fp did not).

**Rev 4 is the operator's 15-item round** (table in `UI-CHANGES-PLAN.md`): **cable nodes** —
hover ghost, tap-insert at identity (bit-identical render, audit smoke 54 pins it through the
real gesture door), drag up/down = amp / left/right = offset (the audio offset inert by the DC
rule), tap to remove, one undo per drag; the bridge gives them a voice (audio → invisible
`util/gain`, cv → `set_cv_trim`, control → param-mod, mult-collapse composed affinely,
event/data/spatial refused in words). **The executor adopts** (D1: unchanged nodes keep state
and allocations across a hotswap — adding a module while playing no longer rebuilds the world).
**The wells came alive**: clock division rings, seq walking lights, rand step bars (seed pinned
in both crates), the quantizer's 12-key keyboard with Custom-mode taps, the rms rolling graph,
the master's latched CLIP LED, the svf curve moving under its cv, the mult strip at quarter
width. **Three new modules** — `fx/fold`, `util/quant`, `mod/rand` — plus clk `phase`, seq
`step`, rms `slew`, delay `sync`: the first-party set is **24**, `module_docs` 24/24. **Tokens by
ruling**: event cables SOLID (D9; control wires stay dashed — mockup-review findings 25–26) and
ports float 6 px (D10). Gates: **899 tests (+1 ignored)**, goldens untouched
(`ba577186c988db21`, selftest 9/9, the three pinned exec renders exact), every runnable clippy
cell clean, every python gate, `ui --audit` **PASS, 0 failures, 59 [PASS] lines**,
`modules --strict` 24/24, `sync_check` OK at **162 files, `src 96f/2694477B`**. One declared
OWED piece: the dropdown picker (item 11's inspector list-picker). One declared defect: **#95**
(pre-existing ui-live glide-lag test; the ui-feature cell reads 28/1).

**Rev 4 namelist** (49 paths vs git `871d725`; the pack carries the whole tree, this names what
moved): `crates/sparq-audio/src/{executor,engine,modules}.rs` ·
`crates/sparq-audio/tests/{executor,mixer_cv,r4_modules,r4_new_modules}.rs` ·
`crates/sparq-kernel/src/sync/hotswap.rs` ·
`crates/sparq-ui/src/canvas/{model,layout,interact,inset,levels,mod}.rs` ·
`crates/sparq-app/src/bridge.rs` · `crates/sparq-app/src/ui/{canvas_ui,shell_ui,live,headless}.rs`
· `crates/sparq-app/tests/r3_controls.rs` · `modules/{fx/fold,util/quant,mod/rand}/sparqmod.toml`
(new) + bumped `modules/{ana/rms,mod/clk,mod/seq,util/delay,util/vca}/sparqmod.toml` ·
`docs/modules/` (8 regenerated: ana-rms, fx-fold, mod-clk, mod-rand, mod-seq, util-delay,
util-quant, util-vca) · `design/tokens/{colors,layout}.toml` + `generated/{tokens.rs,json,css}` +
`preview.html` · `docs/ui/gestures.md` (§4f, §6 event row, §7 round-4 list) ·
`design/mockups/mockup-review.md` (findings 25–26) · `scripts/test006.bat` (steps L–U, counts
moved) · `CHECKLIST.md` · `SYNC.md` · `README.md` · `UI-CHANGES-PLAN.md` (round-4 sheet) ·
`ROUND4-HANDOFF.md` (§0–§0e session record) · `SYNC-STAMP.txt` (re-stamped, NOT in the pack).

**THE ASK, round 4:** run `scripts\test006.bat` on SATURN — the new steps **L–U** are the
operator's eyes and ears: the clip LED on a hot render, the clock wheel turning under PLAY, the
seq/rand walking lights, the quantizer keyboard (Custom tap flips, preset tap refuses in words),
the cable node under a real finger (silent insert, audible amp drag, inert audio offset,
tap-remove, one undo), the mult strip, `vca` taking the lfo wire (the tremolo), `fold`'s bloom,
the rms graph + the svf curve moving, and the round-4 chrome (solid events, dashed controls,
6 px ports). Send back `test006-digest.log` + `logs\ui-digest.log`.

---

## Previous bundle — operator UI rounds 2026-10-01 (rev 3, the control-wire / junction-bus round)

**Rev 3 was `sparq-update-2026-10-01.zip` (rounds 1+2+3 of 2026-10-01, overlay
pack; NO patch base this rev — the sandbox lost its `.git` in a reset, so rev 3 is the file set,
not a diff; revs 1–2 patches remain valid for trees at those states).** It stacks on
`sync-wo014-inc7c.zip` (applied). Extract at the repo root, overwriting; the pack is
workspace-relative and touches nothing else. **The stamp does NOT move from the sandbox:**
`SYNC-STAMP.txt` still reads `sync sync-wo014-inc7c` / `fp 96f/2381733B` and the pack carries no
stamp — it is the cross-machine contract, so re-run `python tools/sync_check.py --write` on the
source machine after applying. Session-start discipline: `scripts\build.bat` will print the 24
changed filenames before cargo — that report is EXPECTED, not a failure. **One manifest moved:**
`syn/sine`'s frequency range is now 0.1 Hz – 10 kHz (operator ruling), so its sha moves and
`docs/modules/syn-sine.md` was regenerated — `module_docs --check` reads 17/17 matched.

**Rev 3 adds the third operator list:** control wires into every float parameter (small blue
sink dots in flight, dashed control wires, executor param-mod plan at one block of latency,
verdicts in words); `util/mult` the six-dot junction bus (role/type by first connection, one
source, bridge-collapsed); `util/vca`; the sequencer's sixteen step buttons; the scope accepting
any source class with the trace in the source's colour (audio outputs now publish waveforms on
the analysis ring); the 100 % zoom ceiling (`zoom_max` 1.0). First-party set **21 modules**,
`module_docs` 21/21. Gates: **832 tests (+1 ignored)**, goldens untouched, every runnable clippy
cell, every python gate, `ui --audit` PASS 0 failures with two new round-3 smokes (the bus
collapses and lights; the control wire applies).

**Rev 2 added the second operator list:** sine/polyblep at 10 Hz – 10 kHz; the clock family —
`mod/clk` (tempo clock, 4th/8th/16th/32nd trigger outs, sample-accurate grids, tempo edits
re-space from the next tick) and `mod/seq` (3–16 step trigger sequencer, bitmask fires at the
input event's sample) — taking the set to **19 modules** (`module_docs` 19/19); multi-choice ints
(3…8) as button rows; the card-slider capture dead-zone fix; zoom-relative card content; the
Main Out band's char-boundary clip + composed-to-fit second line + stereo main in; the lfo well
in control blue; and delete-with-splice (one Batch, one undo, verdict-checked splices). Gates:
**825 tests (+1 ignored)**, goldens untouched, every runnable clippy cell, every python gate,
`ui --audit` PASS 0 failures; five new clock/seq gate tests pin exact tick lists and zero
allocations in `process`.

What round 1 is, in one breath: the operator's UI list, each item in the compute-then-draw
discipline — log-mapped Hz sliders (the sine follows the mouse), ports floating 4 px beside the
window with covered-is-untouchable hit-testing, 2 px stripes, a WHITE selection (the token set's
one documented pure-white exemption), 32 px library tiles under per-group toggle switches, toggle
buttons for binary settings, the right-edge IN/OUT strip gone, `out/main` permanent and reading
the negotiated driver truth on its own info band, a 160 px scope screen with graticule and
measurements, and the mouse hand's bindings (wheel = canvas zoom, right-drag = pan, DEL = delete
the selection). `docs/ui/gestures.md` carries the rules (2b, 3c, 3d, 4c, 4e); the round's
screenshots live in `docs/ui/shots/`.

**Measured:** 818 tests (+10, +1 ignored) · goldens bit-identical (`ba577186c988db21` — the DSP
never moved) · selftest 9/9 · every clippy cell runnable in the sandbox (workspace, `ui`,
`ui-window`, `bootstrap-audio`) · every python gate · `ui --audit` PASS, 0 failures — the 5
viewport × 4 DPI matrix and all 41 smoke checks (same count as inc7c; smokes 12, 17, 36, 45 and 50
were RE-POINTED at the new behaviour, none removed).

**THE ASK, two lines:** wheel-zoom and right-drag pan with the real mouse on SATURN; and one fast
trim drag to hear that the toggle/log-map edits still glide.

---

## Previous bundle — WO-014 increment 7c (the one-pole glide: the fast-drag bumps, dead)

**Current bundle: `sync-wo014-inc7c.zip` (11 entries, listed below).** It stacks on
`sync-wo014-inc7b.zip` (applied). Extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. No drift-repair copy step. No device contract moves. Gate numbers: tests stay
**808** (the sweep gate replaced the 7b geometry gate; the gain unit test was rewritten to the
glide contract), smokes stay **50** (live smoke 27 now pumps past two glide taus — the lag is
declared, not hidden), the stamp moves.

**The stamp changes: see `SYNC-STAMP.txt`** (same **155-file** covered set, new byte count —
`core.rs`, `modules.rs`, `param_ramp.rs`, `headless.rs` moved). `Cargo.lock` stays SOFT and is
NOT in the zip. No manifest moved — 17/17.

## What this is

Your third ear-report named the shape error exactly: bumps under a FAST drag. The 7b ramp was
continuous but staircase-shaped — it reached each snapshot's target in one block (1.3 ms) and
held until the next UI snapshot (~16 ms), so a quick fader move produced sharp kinks at the
update rate. Kinks at 60 Hz are bumps.

The ramp is now a **one-pole glide** (`dsp::core::glide`, tau = `GLIDE_TAU_MS` = 25 ms): the
coefficient chases the moving target every sample (per frame on the multi-channel modules —
channel count still cannot change how far a glide travels, 7b's fix stands inside this one).
The envelope is smooth inside every block and continuous across every boundary; you get a
motorised-fader feel (~25 ms of lag, declared) instead of a staircase. A **snap at −80 dB**
lands settled edits EXACTLY on the target, so settled and static renders are the constant
multiply again — every checked-in golden stays bit-exact, and the first-block prime rule is
unchanged. Mute still cuts in exact zeros immediately.

The new gate drives a faster-than-any-hand drag (trim 1.0 → 0.0, one edit per block) and
asserts: no boundary jump beyond the signal's own slope anywhere in the sweep; block peaks fall
monotonically (no overshoot, no snap-back); both channels of every frame share one coefficient;
settled at zero the output is exact silence. On the staircase code the boundary jumps and the
hold-then-jump envelope fail it.

**Measured:** 808 tests (+1 ignored) · release goldens bit-identical (`ba577186c988db21`,
`dd975a24f03b19c1`, stress `7bb06379bd6845e5`) · selftest 9/9 · 17/17 · 434.8× realtime · every
clippy cell runnable in the sandbox · 5 python gates · `ui --audit` PASS, 50 smokes.

**THE ASK, one line:** drag the trim FAST again. The staircase kinks were the bumps; the glide
has none. Then, stage time permitting: test004 attempt 4 (still the WO-006 acceptance; the
ADR-008 exit stays unfired) and the gates digest at 808 / the stamp below.

**Waiting:** this bundle only.
Applied chain (newest first): `sync-wo014-inc7b.zip` (9), `sync-wo014-inc7.zip` (12),
`sync-wo012-inc6.zip` (27), `sync-wo012-inc5c.zip` (16), `sync-wo012-inc5b.zip` (21),
`sync-wo012-inc5.zip` (24), `sync-wo012-inc4.zip` (23, operator-confirmed: PLAY makes sound),
`sync-wo012-inc3.zip` (24), `sync-wo013-inc6.zip` (16), `sync-wo012-inc2.zip` (20),
`sync-wo008-inc7.zip` (17) — all operator-reported APPLIED — then `sync-wo006-inc15.zip` (15),
`sync-wo006-inc14.zip` (15), `sync-wo008-inc6.zip` (17), `sync-wo014-inc6.zip` (13),
`sync-wo013-inc5.zip` (20), `sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22),
`sync-wo014-inc4.zip` (12), `sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19),
`sync-wo008-inc4.zip` (38), `sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15),
`sync-wo006-inc13.zip` (10), `sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`,
`sync-p1b.zip`, `sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`,
`sync-wo012-inc1.zip`, `sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
EXTRACT-AT-REPO-ROOT.txt
README.md
SYNC-STAMP.txt
SYNC.md
WO014-INC7-PLAN.md
crates/sparq-app/src/ui/headless.rs
crates/sparq-audio/src/dsp/core.rs
crates/sparq-audio/src/modules.rs
crates/sparq-audio/tests/param_ramp.rs
tools/log_check.py
```


**Current bundle: `sparq-update-2026-10-06-wo020-inc4.zip` (a FULL-TREE pack, 471 entries —
defect #94's remedy stands: it overlays any tree at or after `sparq-update-2026-10-05-r8.zip` /
git `96fe57d` and does not depend on a base the receiver might not have).** Excluded from the
pack by the standing rule: `.git/`, `target/`, `Cargo.lock` (untracked by house rule; cargo
re-resolves on first build), `logs/`, `*.wav`, `handoff/` (the receiver's copies win), and
`SYNC-STAMP.txt` — **the stamp does NOT ride** (rev-3 pattern): after applying, re-run
`python tools\sync_check.py --write --sync sparq-wo020-inc4-2026-10-06` on the machine that will
own the tree; until then `scripts\build.bat` / `test006`'s `[00b]` name the stamp mismatch in
words, which is EXPECTED, not a failure. The zip's sha256 is recorded in the delivery message and
in `handoff/sha256sums-wo020.txt` (which rides the repo, not the zip). **Provenance:** sandbox
branch `wo020-observatory` off main `96fe57d` (the operator's Fresh-start re-publish, which
already carries WO-020 INC1+INC2, the plan and the §18 mockups); durable history in
`handoff/sparq-wo020-inc1…inc4.bundle` + the closing `sparq-wo020-delivery.bundle`
(`96fe57d`..the delivery commit), each with a `.sha256` sidecar; packer
`handoff/make_pack_wo020.py` (self-verifying: entry-by-entry zip↔tree hashes, count assertions
against this manifest and the run sheet, MUST-NOT-MOVE surfaces diffed vs base — it refuses to
write a pack whose documents would lie about it).

**What it carries:** WO-020 "The Observatory" (`dat/observatory`) **through INC4** — the stream
plane's hermetic half (INC1, on main already: `sparq-streams`, the 23-stream registry as data,
all 23 fixtures at HTTP 200), the contract additions (INC2, on main: ADR-011, the `stream`
binding, `E-STREAM-UNKNOWN`), **INC3 the guest**: the `instruments-src/observatory/` workspace
(pure core + wasm glue + native harness over one object, D13), the coastline asset (128 rings /
3 500 pts / 14 524 B), the generated manifest (26 params, 27 toolbar widgets, 16 stream
bindings), the **206 081 B component** (`wasm-tools validate` clean, `sparq:instrument/guest@1.1.0`),
the 4-file package `instruments/observatory/`, the pinned golden IR
(`bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`), and **INC4 the host**:
`sparq-ui::{json,displaylist}` (zero-dep painter core + SVG backend), the egui backend, the
dropdown picker (**round-4's OWED item 11 discharged**; 44 px rows, undoable), camera FOCUS, D6
wall-card sizing (2208×1288), the browser's instrument grouping, the at-rest wall in the shell
(`SPARQ_ATREST` / cache dir, WORDS when none), `sparq instrument render`. Sandbox gates at the
seal: root **1033 / 0 / 1-ignored**; observatory workspace **124 + 6**; `sparq-streams`
**75 / 0**; clippy clean in every runnable cell; fmt clean both workspaces; every python gate;
`ui --audit` **PASS**, coverage **97.5 %**; `mod validate` **PARTIAL exit 1 BY DESIGN** (stages
3/4/6 refuse in words — the runtime half is INC5); goldens untouched (`ba577186c988db21`); WIT
pins, the 24 module manifests and `SYNC-STAMP.txt` unmoved (asserted by the packer). **Two
honest states ride along:** `token_audit` still fails the pre-existing §18 mockup SVGs (14 groups
/ 298 occurrences — on main before this pack; `gates.bat`'s `design conformance` row will be red
on exactly that), and **PLAY with the Observatory on canvas refuses in words (#58)** until INC5's
wasmtime loader registers packages.

**Namelist** (95 paths vs git `96fe57d` — 61 A / 34 M; the pack carries the whole tree, this
names what moved; the 10 `handoff/` artefacts — inc3/inc4 + delivery bundles and sidecars, the
packer, the pack's committed copy, the sums — do not ride): `instruments-src/observatory/**` (34 —
the whole guest workspace: core 17, wasm glue 2, harness 9, package 3, `gen_manifest.py`, the
coastline asset) · `instruments/observatory/` (4 — the shipped package incl. the component) ·
`crates/sparq-ui/` (11 — `json.rs` + `displaylist.rs` + `canvas/picker.rs` new; browser, inset,
inspector, interact, layout, model, mod, lib touched) · `crates/sparq-app/` (11 — `ui/atrest.rs` +
`ui/displaylist_egui.rs` new; canvas_ui, headless, shell_ui, ui/mod, bridge, cli, instruments,
Cargo.toml, `tests/r3_controls.rs`) · `crates/sparq-host-wasm/` (3 — `sources.rs` new + the
`streams` feature) · `crates/sparq-module-api/` (2 — `ParamSpec.options` decode + manifest) ·
`crates/sparq-audio/tests/portless_instrument.rs` (the D5 proof, +9 tests) · `design/tokens/` (6
— additive `layout.marker.radius_s/m/l` + regenerated bundle) · `tools/` (5 —
`make_coastline.py`, `observatory_package.py`, `fetch_wasm_tools.py` new; `streams_record.py`
text-I/O fix, `token_gen.py` LUT lookups) · `reference/observatory/` (2 — the INC4 convergence
sheet + README) · root docs (6 — `WO020-INC4-RUN-SHEET.md` new; `SYNC.md`, `WO020-STATE.md`,
`CHECKLIST.md`, `LATER.md`, `.gitignore`).

**THE ASK:** apply → re-stamp → `scripts\gates.bat` (expect the single pre-existing
`design conformance` red, everything else ok) → the WO-020 device column of
`WO020-INC4-RUN-SHEET.md` §2 (first device measurements: streams 23, guest tests + budget block
exact-match, `mod validate` PARTIAL, the at-rest render via `SPARQ_ATREST`, the shell visual row,
the audit) → send back §3's list. The Windows at-rest cache-dir disagreement (harness
`%USERPROFILE%\.cache\…` vs app `%APPDATA%\…`) is recorded in `LATER.md` — use the env door; INC5
aligns it. Standing asks from ROUND8-HANDOFF §7 step 4 unchanged. **Next: WO-020 INC5**
(device-only: wasmtime loader, `BrokerProvider`, stage-6 at-rest publishing, the full validate
chain, `scripts\test007.bat` live acceptance) on a ≥ 2 GB host.

---

## Previous bundle — operator UI rounds: rev 4 (2026-10-02), the cable-node / living-wells round

**Rev 4 was `sparq-update-2026-10-02.zip` (operator round 4 of 2026-10-02 — a
FULL-TREE pack, defect #94's remedy: it overlays any tree at git `871d725` or later and does not
depend on a base the receiver might not have).** Excluded from the pack by rule: `.git/`,
`target/`, `Cargo.lock` (regenerates on first build), `logs/`, `*.wav`, and `SYNC-STAMP.txt` —
**the stamp does NOT ride in the pack** (rev-3 pattern): after applying, re-run
`python tools\sync_check.py --write --sync sparq-round4-2026-10-02` on the machine that will own
the tree; until then `scripts\build.bat` names the stamp mismatch in words, which is EXPECTED,
not a failure. **Provenance:** round-4's code was lost with the sandbox that wrote it (only
`ROUND4-HANDOFF.md` reached git); this pack is the REBUILD from that handoff on a clean clone,
re-measured end to end on Linux/rustc 1.99.0 — see `CHECKLIST.md`'s round-4 paragraph for the
three provenance notes (defect #95 pre-existing, mutation-hash baseline moved to
`8143e1ddfd8fb261` on this box, and defect #96: the pack's `modules/out/main/sparqmod.toml` is
a DECLARED RECONSTRUCTION — the #93 `out/`-drop trap re-struck the sealed tree and no byte-exact
copy survived; rebuilt from its generated doc + the `OutMain` implementation, proven
semantically identical at both layers, its stamp row moved while the fp did not).

**Rev 4 is the operator's 15-item round** (table in `UI-CHANGES-PLAN.md`): **cable nodes** —
hover ghost, tap-insert at identity (bit-identical render, audit smoke 54 pins it through the
real gesture door), drag up/down = amp / left/right = offset (the audio offset inert by the DC
rule), tap to remove, one undo per drag; the bridge gives them a voice (audio → invisible
`util/gain`, cv → `set_cv_trim`, control → param-mod, mult-collapse composed affinely,
event/data/spatial refused in words). **The executor adopts** (D1: unchanged nodes keep state
and allocations across a hotswap — adding a module while playing no longer rebuilds the world).
**The wells came alive**: clock division rings, seq walking lights, rand step bars (seed pinned
in both crates), the quantizer's 12-key keyboard with Custom-mode taps, the rms rolling graph,
the master's latched CLIP LED, the svf curve moving under its cv, the mult strip at quarter
width. **Three new modules** — `fx/fold`, `util/quant`, `mod/rand` — plus clk `phase`, seq
`step`, rms `slew`, delay `sync`: the first-party set is **24**, `module_docs` 24/24. **Tokens by
ruling**: event cables SOLID (D9; control wires stay dashed — mockup-review findings 25–26) and
ports float 6 px (D10). Gates: **899 tests (+1 ignored)**, goldens untouched
(`ba577186c988db21`, selftest 9/9, the three pinned exec renders exact), every runnable clippy
cell clean, every python gate, `ui --audit` **PASS, 0 failures, 59 [PASS] lines**,
`modules --strict` 24/24, `sync_check` OK at **162 files, `src 96f/2694477B`**. One declared
OWED piece: the dropdown picker (item 11's inspector list-picker). One declared defect: **#95**
(pre-existing ui-live glide-lag test; the ui-feature cell reads 28/1).

**Rev 4 namelist** (49 paths vs git `871d725`; the pack carries the whole tree, this names what
moved): `crates/sparq-audio/src/{executor,engine,modules}.rs` ·
`crates/sparq-audio/tests/{executor,mixer_cv,r4_modules,r4_new_modules}.rs` ·
`crates/sparq-kernel/src/sync/hotswap.rs` ·
`crates/sparq-ui/src/canvas/{model,layout,interact,inset,levels,mod}.rs` ·
`crates/sparq-app/src/bridge.rs` · `crates/sparq-app/src/ui/{canvas_ui,shell_ui,live,headless}.rs`
· `crates/sparq-app/tests/r3_controls.rs` · `modules/{fx/fold,util/quant,mod/rand}/sparqmod.toml`
(new) + bumped `modules/{ana/rms,mod/clk,mod/seq,util/delay,util/vca}/sparqmod.toml` ·
`docs/modules/` (8 regenerated: ana-rms, fx-fold, mod-clk, mod-rand, mod-seq, util-delay,
util-quant, util-vca) · `design/tokens/{colors,layout}.toml` + `generated/{tokens.rs,json,css}` +
`preview.html` · `docs/ui/gestures.md` (§4f, §6 event row, §7 round-4 list) ·
`design/mockups/mockup-review.md` (findings 25–26) · `scripts/test006.bat` (steps L–U, counts
moved) · `CHECKLIST.md` · `SYNC.md` · `README.md` · `UI-CHANGES-PLAN.md` (round-4 sheet) ·
`ROUND4-HANDOFF.md` (§0–§0e session record) · `SYNC-STAMP.txt` (re-stamped, NOT in the pack).

**THE ASK, round 4:** run `scripts\test006.bat` on SATURN — the new steps **L–U** are the
operator's eyes and ears: the clip LED on a hot render, the clock wheel turning under PLAY, the
seq/rand walking lights, the quantizer keyboard (Custom tap flips, preset tap refuses in words),
the cable node under a real finger (silent insert, audible amp drag, inert audio offset,
tap-remove, one undo), the mult strip, `vca` taking the lfo wire (the tremolo), `fold`'s bloom,
the rms graph + the svf curve moving, and the round-4 chrome (solid events, dashed controls,
6 px ports). Send back `test006-digest.log` + `logs\ui-digest.log`.

---

## Previous bundle — operator UI rounds 2026-10-01 (rev 3, the control-wire / junction-bus round)

**Rev 3 was `sparq-update-2026-10-01.zip` (rounds 1+2+3 of 2026-10-01, overlay
pack; NO patch base this rev — the sandbox lost its `.git` in a reset, so rev 3 is the file set,
not a diff; revs 1–2 patches remain valid for trees at those states).** It stacks on
`sync-wo014-inc7c.zip` (applied). Extract at the repo root, overwriting; the pack is
workspace-relative and touches nothing else. **The stamp does NOT move from the sandbox:**
`SYNC-STAMP.txt` still reads `sync sync-wo014-inc7c` / `fp 96f/2381733B` and the pack carries no
stamp — it is the cross-machine contract, so re-run `python tools/sync_check.py --write` on the
source machine after applying. Session-start discipline: `scripts\build.bat` will print the 24
changed filenames before cargo — that report is EXPECTED, not a failure. **One manifest moved:**
`syn/sine`'s frequency range is now 0.1 Hz – 10 kHz (operator ruling), so its sha moves and
`docs/modules/syn-sine.md` was regenerated — `module_docs --check` reads 17/17 matched.

**Rev 3 adds the third operator list:** control wires into every float parameter (small blue
sink dots in flight, dashed control wires, executor param-mod plan at one block of latency,
verdicts in words); `util/mult` the six-dot junction bus (role/type by first connection, one
source, bridge-collapsed); `util/vca`; the sequencer's sixteen step buttons; the scope accepting
any source class with the trace in the source's colour (audio outputs now publish waveforms on
the analysis ring); the 100 % zoom ceiling (`zoom_max` 1.0). First-party set **21 modules**,
`module_docs` 21/21. Gates: **832 tests (+1 ignored)**, goldens untouched, every runnable clippy
cell, every python gate, `ui --audit` PASS 0 failures with two new round-3 smokes (the bus
collapses and lights; the control wire applies).

**Rev 2 added the second operator list:** sine/polyblep at 10 Hz – 10 kHz; the clock family —
`mod/clk` (tempo clock, 4th/8th/16th/32nd trigger outs, sample-accurate grids, tempo edits
re-space from the next tick) and `mod/seq` (3–16 step trigger sequencer, bitmask fires at the
input event's sample) — taking the set to **19 modules** (`module_docs` 19/19); multi-choice ints
(3…8) as button rows; the card-slider capture dead-zone fix; zoom-relative card content; the
Main Out band's char-boundary clip + composed-to-fit second line + stereo main in; the lfo well
in control blue; and delete-with-splice (one Batch, one undo, verdict-checked splices). Gates:
**825 tests (+1 ignored)**, goldens untouched, every runnable clippy cell, every python gate,
`ui --audit` PASS 0 failures; five new clock/seq gate tests pin exact tick lists and zero
allocations in `process`.

What round 1 is, in one breath: the operator's UI list, each item in the compute-then-draw
discipline — log-mapped Hz sliders (the sine follows the mouse), ports floating 4 px beside the
window with covered-is-untouchable hit-testing, 2 px stripes, a WHITE selection (the token set's
one documented pure-white exemption), 32 px library tiles under per-group toggle switches, toggle
buttons for binary settings, the right-edge IN/OUT strip gone, `out/main` permanent and reading
the negotiated driver truth on its own info band, a 160 px scope screen with graticule and
measurements, and the mouse hand's bindings (wheel = canvas zoom, right-drag = pan, DEL = delete
the selection). `docs/ui/gestures.md` carries the rules (2b, 3c, 3d, 4c, 4e); the round's
screenshots live in `docs/ui/shots/`.

**Measured:** 818 tests (+10, +1 ignored) · goldens bit-identical (`ba577186c988db21` — the DSP
never moved) · selftest 9/9 · every clippy cell runnable in the sandbox (workspace, `ui`,
`ui-window`, `bootstrap-audio`) · every python gate · `ui --audit` PASS, 0 failures — the 5
viewport × 4 DPI matrix and all 41 smoke checks (same count as inc7c; smokes 12, 17, 36, 45 and 50
were RE-POINTED at the new behaviour, none removed).

**THE ASK, two lines:** wheel-zoom and right-drag pan with the real mouse on SATURN; and one fast
trim drag to hear that the toggle/log-map edits still glide.

---

## Previous bundle — WO-014 increment 7c (the one-pole glide: the fast-drag bumps, dead)

**Current bundle: `sync-wo014-inc7c.zip` (11 entries, listed below).** It stacks on
`sync-wo014-inc7b.zip` (applied). Extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. No drift-repair copy step. No device contract moves. Gate numbers: tests stay
**808** (the sweep gate replaced the 7b geometry gate; the gain unit test was rewritten to the
glide contract), smokes stay **50** (live smoke 27 now pumps past two glide taus — the lag is
declared, not hidden), the stamp moves.

**The stamp changes: see `SYNC-STAMP.txt`** (same **155-file** covered set, new byte count —
`core.rs`, `modules.rs`, `param_ramp.rs`, `headless.rs` moved). `Cargo.lock` stays SOFT and is
NOT in the zip. No manifest moved — 17/17.

## What this is

Your third ear-report named the shape error exactly: bumps under a FAST drag. The 7b ramp was
continuous but staircase-shaped — it reached each snapshot's target in one block (1.3 ms) and
held until the next UI snapshot (~16 ms), so a quick fader move produced sharp kinks at the
update rate. Kinks at 60 Hz are bumps.

The ramp is now a **one-pole glide** (`dsp::core::glide`, tau = `GLIDE_TAU_MS` = 25 ms): the
coefficient chases the moving target every sample (per frame on the multi-channel modules —
channel count still cannot change how far a glide travels, 7b's fix stands inside this one).
The envelope is smooth inside every block and continuous across every boundary; you get a
motorised-fader feel (~25 ms of lag, declared) instead of a staircase. A **snap at −80 dB**
lands settled edits EXACTLY on the target, so settled and static renders are the constant
multiply again — every checked-in golden stays bit-exact, and the first-block prime rule is
unchanged. Mute still cuts in exact zeros immediately.

The new gate drives a faster-than-any-hand drag (trim 1.0 → 0.0, one edit per block) and
asserts: no boundary jump beyond the signal's own slope anywhere in the sweep; block peaks fall
monotonically (no overshoot, no snap-back); both channels of every frame share one coefficient;
settled at zero the output is exact silence. On the staircase code the boundary jumps and the
hold-then-jump envelope fail it.

**Measured:** 808 tests (+1 ignored) · release goldens bit-identical (`ba577186c988db21`,
`dd975a24f03b19c1`, stress `7bb06379bd6845e5`) · selftest 9/9 · 17/17 · 434.8× realtime · every
clippy cell runnable in the sandbox · 5 python gates · `ui --audit` PASS, 50 smokes.

**THE ASK, one line:** drag the trim FAST again. The staircase kinks were the bumps; the glide
has none. Then, stage time permitting: test004 attempt 4 (still the WO-006 acceptance; the
ADR-008 exit stays unfired) and the gates digest at 808 / the stamp below.

**Waiting:** this bundle only.
Applied chain (newest first): `sync-wo014-inc7b.zip` (9), `sync-wo014-inc7.zip` (12),
`sync-wo012-inc6.zip` (27), `sync-wo012-inc5c.zip` (16), `sync-wo012-inc5b.zip` (21),
`sync-wo012-inc5.zip` (24), `sync-wo012-inc4.zip` (23, operator-confirmed: PLAY makes sound),
`sync-wo012-inc3.zip` (24), `sync-wo013-inc6.zip` (16), `sync-wo012-inc2.zip` (20),
`sync-wo008-inc7.zip` (17) — all operator-reported APPLIED — then `sync-wo006-inc15.zip` (15),
`sync-wo006-inc14.zip` (15), `sync-wo008-inc6.zip` (17), `sync-wo014-inc6.zip` (13),
`sync-wo013-inc5.zip` (20), `sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22),
`sync-wo014-inc4.zip` (12), `sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19),
`sync-wo008-inc4.zip` (38), `sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15),
`sync-wo006-inc13.zip` (10), `sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`,
`sync-p1b.zip`, `sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`,
`sync-wo012-inc1.zip`, `sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
EXTRACT-AT-REPO-ROOT.txt
README.md
SYNC-STAMP.txt
SYNC.md
WO014-INC7-PLAN.md
crates/sparq-app/src/ui/headless.rs
crates/sparq-audio/src/dsp/core.rs
crates/sparq-audio/src/modules.rs
crates/sparq-audio/tests/param_ramp.rs
tools/log_check.py
```
