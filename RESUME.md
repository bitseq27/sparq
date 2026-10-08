# RESUME.md — where the work stands, and how to pick it up

**As of 2026-10-08, end of session 10 (the INC6 prep-sync DELIVERY; the S1–S5 record below stands).** This file is the new-session door:
read it whole (it is sized to load), then read by range from its pointers — **do not load the big
documents whole** (`WO020-STATE.md` is ~700 lines, `CHECKLIST.md` ~2100, `SPARQ-PLAN.md` 1102;
a session once died on exactly that — §1 of the old resume, kept because the lesson is standing).

---

## 1. The one-paragraph state

The Observatory (WO-020) is **device-accepted**: r7 went to SATURN, the operator applied it, and
`sparq mod validate instruments\observatory\` returned **GATE: PASS — the whole chain** for the
first time (stage 4 reproduced the harness pin `bdf59cdb…` bit-exactly over the WASM boundary on
component `4b29a5ae…`, the two runs' fuels EQUAL at 12 390 844; stage 5 inside the re-baselined
`max_fuel = 32 000 000`). The session-7 defect trio — **#97** the zoom-button shell hang (the
f32 `dash_segments` stall; the operator's own eyes confirmed: *"the zoom is working without
OOM"*), **#98** the stale checked-in component (predated INC4's `cell_ground` fix), **#99** the
golden stage's cold-vs-warm fuel comparison — are all fixed and **DEVICE-CONFIRMED**. The
operator then filed the window report (*"no controls… too big… only 4 panels… not updating…
NASA api key… poll rate per panel"*), which is commissioned as **WO-020 INC6 "the live
instrument"** — plan of record `WO020-INC6-PLAN.md`, four operator rulings recorded (O-1
resizable card defaulting to HALF size, O-2 STREAMS dock tab + card rate fields, O-3 the 10 s
poll floor, O-4 the user-data key file), slices **S1–S5**. **S1 LANDED SEALED 2026-10-08
(session 9, sandbox-proven end to end): the resizable instrument card — `Node.size` +
`Op::ResizeNode` (one undo per drag), the half-face default (the Observatory's card is now
1120×728, band 1088×560; the declared 2176×1120 maximum stays reachable and keeps the D6/R3
coverage rule), the class-L corner handle (`canvas/resize/{id}` in the audit), the 480×248…face
clamp in the op constructor, the 8-px snap on commit, FIT/FOCUS reading the size, the at-rest
wall scaling into the band, `project.md`'s reserved `size` line; gates green (workspace
**1047/0**, audit **PASS, 71 rows**), proven failable by three injections, bundled
(`handoff/sparq-wo020-inc6s1.bundle`). S2 LANDED SEALED the same session: the typed panel band
(D16) — the manifest's 27 widgets typed into `InstrumentDisplay.widgets`, painted at Full LOD
into the reserved band on D16's 44-px floor, enums through the EXISTING picker, toggles and
sliders through the undoable param door, the D7 gate evaluated per frame (12 visible at the
defaults), the label slot carrying the host's words with the at-rest sentence ("params edit;
the wall re-renders live when the loader runs" — never a frozen lie), the audit registering
every routed cell and nothing undeclared; **defect #100 found and fixed on the way** (the
manifest-parsed enums' zeroed range — no picker selection could ever change them; ledger row in
`CHECKLIST.md`); gates green (workspace **1054/0**, audit **PASS, 75 rows**), two more
injection proofs, bundled (`handoff/sparq-wo020-inc6s2.bundle`). S3 LANDED SEALED the same
session: the overrides store + the STREAMS dock tab (O-2/O-3/O-4, D18/D20) —
`sparq-streams/src/store.rs` (the user-data TOML beside the cache: the 10 s floor refuses in
words and keeps the old value, env wins over the file, the key's value has no reading door —
mask + state words only, masked `Debug`, atomic save, per-env-var sibling resolution), the
broker's `due`/stale-horizon reading override-then-registry, the CLI's every door composed
through the store, and the dock tab alive under the `streams` feature (fixture statuses under
an `AT REST — fixtures` header, the age in words, the cadence field with the dimmed registry
value beside the override, the masked KEY field disabled-with-words when the env governs;
smoke 58 drives it all through the recogniser + the key doors, the sentinel key grepped out of
every log line and frame text); gates green (workspace **1055/0**, streams **106/0**, audit
**PASS** in both feature faces — **80 rows** with `ui,streams`), three more injection proofs,
bundled (`handoff/sparq-wo020-inc6s3.bundle`). S4 LANDED SEALED the same session (its sandbox
half): the live plane's seams (plan D17/D19) — the D19 token (`instrument_display_hz = 15`)
ruled into layout.toml, the `LiveDisplay` seam + pacer (edits force the next frame; five
consecutive overruns bypass with §3's words, an edit re-arms), the live/at-rest band switch,
the stream driver (hermetic `poll_once` + the thin device thread), the provider swap (LIVE vs
`AT REST — fixtures` headers), and the DEVICE-FIRST-COMPILE launch registration
(`instrument_launch`: load_package → the display shelf + the registry's factory door — PLAY's
#58 lifts structurally; desk-checked call by call, `scripts/test008.bat` is its run sheet);
gates green (workspace **1061/0**, audit **PASS, 84 rows** with `ui,streams`), three more
injection proofs, bundled (`handoff/sparq-wo020-inc6s4.bundle`), and S5 LANDED SEALED the same
session: the card's RATE row + convergence (D20's second door — the host row on the info band's
precedent, its field writing the SAME per-stream override the tab does, its words naming the
governed stream + the shared-feed truth and following the CELL dropdown the same frame; the
card chrome grew 168→208, so the default card is 1120×768 and the maximum 2208×1328 — the O-1
BAND numbers untouched; the convergence sheet re-shot at Full LOD, sha `56e446d3…` recorded in
STATE, the live-wall PNG re-shoot rides the device round; the mockup audit re-run at its
unchanged standing set); gates green (workspace **1062/0**, audit **PASS, 88 rows** with
`ui,streams`), three more injection proofs, bundled (`handoff/sparq-wo020-inc6s5.bundle`).
**INC6's SANDBOX WORK IS COMPLETE (S1–S5).** The S3 live-key-flip and the S4 acceptance rows
[B]/[D]/[E] are DEVICE-PENDING and ride test008, as the plan scoped them.** Full record:
`WO020-STATE.md`'s INC6 section. **Session 10 DELIVERED it (prep sync):**
`sparq-update-2026-10-08-wo020-inc6.zip` — FULL-TREE, **495 entries**, sha256 + size in
`handoff/sha256sums-wo020-inc6.txt`; device stamp name **`sparq-wo020-inc6-2026-10-08`** (the
stamp does NOT ride; apply → re-stamp → build); durable history
`handoff/sparq-wo020-inc6-delivery.bundle` (+ `.sha256`) — `b2445d6..HEAD` at the artefacts
seal, COMMITTED (the r8 lesson), carrying the zip copy + sums inside. Next action, anywhere:
**the DEVICE round — apply + re-stamp + build, then test007's uninterrupted gates re-run
(session 8's [K]) + `scripts/test008.bat` + the [G]/[I] eyes + the convergence PNG re-shoot —
or operator rulings on the carried flags (the manifest's M vs the band's given S touch class;
the one-display-instance-per-module limit).**

## 2. What is OUTSTANDING (the honest list)

* **The INC6 DEVICE round** — the work order's sandbox slices **S1–S5 are ALL CLOSED** (session
  9; `WO020-STATE.md`'s INC6 section carries every record, number and judgment call) and
  **DELIVERED** (session 10's prep sync: the inc6 pack + the committed delivery bundle —
  `SYNC.md`'s top entry is the apply note; apply → re-stamp
  `sparq-wo020-inc6-2026-10-08` → build FIRST). What
  remains is the device's: **`scripts/test008.bat`** (the S4 launch words, PLAY with the
  Observatory — #58 gone, the cells updating / the ticker / PAUSE / the cell-swap-within-a-
  frame, the fuel watchdog's injection proof on a max_fuel=100000 copy, the STREAMS tab's LIVE
  rows, and S3's device half — the NASA key flipping neo/epic/power from 429/DEMO_KEY to the
  key's rate, with the on-device redaction findstr), the convergence **PNG** re-shoot with the
  live wall (the wo012 precedent; the sandbox SVG re-shoot is recorded by sha in STATE), plus
  session 8's still-open rows: the uninterrupted `gates.bat` re-run **[K]** and the **[G]/[I]**
  eyes. **Carried to the operator (NOT decided in the sandbox):** the manifest's declared
  touch_class M vs the band's given 44-px S floor (an M-sized band is a D6 movement — card
  +24 px), and the one-display-instance-per-module limit (a second node of a module stays at
  rest, in words).
* **Device [K]** — `gates.bat` on SATURN was CANCELLED mid-run (session 8: the `[FAIL] tests`
  row is every exit code `0xc000013a` = Ctrl+C, zero real failures). It wants one uninterrupted
  re-run; the 12 instrument-host runtime tests live in that row. The run sheet §2 now says
  DO NOT CANCEL with the evidence.
* **Device [G]/[I]** — airplane-mode words + the token re-theme, with screenshots (the run
  sheet's send-back list).
* **INC5b is discharged BY INC6 S4** (LATER.md's door says so) — do not build it twice.

## 3. Where the truth lives (read by range)

| Need | Read |
|---|---|
| The commission, the rulings, the slices' acceptance | `WO020-INC6-PLAN.md` — **whole, it is new and sized to load** |
| Sessions 7+8 (the defect evidence, the device PASS lines, the r7/r8 deliveries) | `WO020-STATE.md` — grep `session 7`/`session 8`/`Delivered as r7` |
| Defect ledger (#95–#99 with their DEVICE-CONFIRMED clauses) | `CHECKLIST.md` — grep `#97`, `#98`, `#99` |
| The device run book | `WO020-INC5-RUN-SHEET.md` — the r7/r8 revision notes + §1–§3 |
| What the shell does TODAY (at-rest wall, replay provider, picker) | `crates/sparq-app/src/ui/{shell_ui,canvas_ui,atrest,displaylist_egui}.rs` module headers |
| The loader/doors/stages (device-first-compile half) | `crates/sparq-host-wasm/src/runtime/` + `docs/instrument-host.md` §2–§4, §7 |
| The guest + its harness (the golden's native half) | `instruments-src/observatory/` + its `README.md` |
| The stream plane (broker/registry/windows/replay, the hermetic half) | `crates/sparq-streams/src/` module headers |
| Parked doors and supersessions | `LATER.md` — the `WO-020 INC5 doors` section |

## 4. Sandbox reality (it will have been wiped — this is expected, not a defect)

* **No Rust, no C linker, no curl, no xz, apt hangs.** The full install recipe — rustup via
  python `urllib`, **zig cc 0.17.0 as the linker** behind a two-line shim, toolchain kept at
  `/tmp/tools/{rustup,cargo}` so the 128 MB workspace snapshot is not eaten — is in
  `WO020-STATE.md`'s **Resume recipe** section (grep `Resume recipe`). It was measured working
  on 2026-10-07 (rustc 1.99.0; root `cargo test --workspace` **1034/0** in ~2 min warm).
  **Session 9's two shim fixes (measured; without them `ring` — hence `streams-net` — fails to
  build):** the zigcc shim must DROP cc-rs's `--target=x86_64-unknown-linux-gnu` argument (zig
  0.17 refuses the vendor spelling) and force its own `-target x86_64-linux-gnu`; and `ar` /
  `ranlib` must exist — two one-line shims over `zig ar` / `zig ranlib` on PATH. Also: keep
  `CARGO_TARGET_DIR` under /tmp (the repo's `target/` is snapshot-excluded anyway), and a
  RELEASE build of the egui stack OOMs in `read-fonts` on the 1 GB box — run `ui --audit` from
  the DEBUG build (the audit is headless; its numbers are opt-level-independent; measured the
  same 0 failures).
* **The 1 GB wall stands:** anything pulling wasmtime (`--features instrument-host`) OOM-kills
  in `cranelift-assembler-x64`. That cell is DEVICE-FIRST-COMPILE by ruling — write it against
  the vendored API, desk-check, and say so in the delivery. Everything else (root workspace,
  `--features ui`, `streams`, `streams-net`, the observatory workspace, the python gates)
  builds and tests in the sandbox.
* **`.git` may not survive between turns.** The durable history is the bundles under `handoff/`
  (see §5). Recovery (UPDATED session 10): clone `https://github.com/bitseq27/sparq` — the
  published main is **`b2445d6`**, the operator's Fresh-start re-publish of the r8 documents
  seal (it SUPERSEDES the `7ed2e08` + `sparq-update-2026-10-07-wo020-inc5r8.zip` +
  `sparq-wo020-inc5r8.bundle` route: those artefacts are unreachable from a fresh clone — the
  zip is a workspace-root pack that never rode git, the bundle was never committed — and
  `b2445d6`'s tree IS the r8 seal, verified by content: the r8 RESUME/PLAN/STATE documents, and
  `sync_check` red on exactly `scripts/test007.bat`, which is r8's own documented echo-words
  delta against the never-moved r7 stamp). The sessions-9+10 work sits on top: fetch
  **`handoff/sparq-wo020-inc6-delivery.bundle`** (prerequisite `b2445d6`; carries the WHOLE
  INC6 branch S1–S5 + the delivery documents + the artefacts commit — the committed zip copy +
  sums ride INSIDE it) and fast-forward. Session 10 note (honest, the 2nd recorded occurrence):
  `.git` was lost again between sessions 9 and 10; the recovery ran byte-identically via
  `handoff/sparq-wo020-inc6s5.bundle` (clone `b2445d6` → `git bundle verify` → fetch →
  `git reset`, tree untouched), and the lost S5 artefact commit was faithfully reconstructed
  (`0bc59ce`). Commit + bundle EVERY slice — it is what made both recoveries routine.
* **Known sandbox-only red:** `sparq-app --features ui` test
  `ui::live::…level_follows` fails in this box (environment-sensitive timing; it fails
  identically on the pristine tree — proven). Device/MSVC runs it green. Not yours to fix.
* **Known standing red:** `token_audit` vs the §18 mockup SVGs (on main since before WO-020;
  `gates.bat`'s `design conformance` row). Not yours to fix either.

## 5. Artefacts (the INC6 delivery, 2026-10-08; the S1–S5 slice seals; the r8 delivery, 2026-10-07)

* **`sparq-update-2026-10-08-wo020-inc6.zip`** — the INC6 FULL-TREE pack, **495 entries**
  (workspace root, one level above the repo; **the committed copy rides under `handoff/`** —
  the r8 lesson: a workspace-root-only artefact did not survive the re-publication; sha256 +
  size in `handoff/sha256sums-wo020-inc6.txt` and the delivery message). Carries BEHAVIOUR:
  S1–S5 + the records; the stamp does NOT ride — the device applies, re-stamps
  `--sync sparq-wo020-inc6-2026-10-08`, builds. `SYNC.md`'s top entry is the apply note
  (provenance, gates, the 32-path namelist, THE ASK).
* **`handoff/sparq-wo020-inc6-delivery.bundle`** (+ `.sha256`) — `b2445d6..HEAD` at the
  artefacts seal, prerequisite: any clone of the published main `b2445d6`. Carries the WHOLE
  INC6 branch (S1–S5 code + seals + the delivery documents + the artefacts commit with the
  zip copy + sums). Supersedes the s1–s5 slice bundles for recovery (all stay committed as the
  slice records, the inc1–4 pattern). Recovery from it was PROVEN this session (fresh clone →
  fetch → fast-forward, head matches).
* **`handoff/sparq-wo020-inc6s5.bundle`** (+ `.sha256`) — session 9's S5 seal: `b2445d6..HEAD`
  carrying the WHOLE INC6 branch (S1–S5, code + seals). Prerequisite: any clone of the
  re-published main `b2445d6`. Superseded for recovery by the delivery bundle above; it stays
  committed as the slice record — and it is what session 10's own `.git` recovery ran through
  (the lost 15th commit was reconstructed from it byte-identically, `0bc59ce`).
* **The convergence re-shoot (sandbox)** — `sparq ui --svg-out --review` with `ui,streams`,
  sha256 `56e446d3…`, 173 084 B: recorded in STATE's S5 section by sha + content grep; not
  checked in (token_audit scans `design/mockups/*.svg`; the PNG re-shoot with the LIVE wall is
  the device round's, the wo012 precedent).
* **`handoff/sparq-wo020-inc6s4.bundle`** (+ `.sha256`) — session 9's S4 seal: `b2445d6..HEAD`
  through S4.
* **`handoff/sparq-wo020-inc6s3.bundle`** (+ `.sha256`) — session 9's S3 seal: `b2445d6..HEAD`
  carrying the INC6 branch through S3.
* **`handoff/sparq-wo020-inc6s2.bundle`** (+ `.sha256`) — session 9's S2 seal: `b2445d6..HEAD`
  carrying the WHOLE INC6 branch so far (S1's code + seal, S2's code + seal). Prerequisite: any
  clone of the re-published main `b2445d6`. Supersedes `…inc6s1.bundle` for recovery (the S1
  bundle stays committed as the slice record, the inc1–4 pattern).
* **`handoff/sparq-wo020-inc6s1.bundle`** (+ `.sha256`) — session 9's slice seal:
  `b2445d6..HEAD` at the S1 documents seal (the code commit + the state/checklist/resume
  records). Prerequisite: any clone of the re-published main `b2445d6`.
* **`sparq-update-2026-10-07-wo020-inc5r8.zip`** — FULL-TREE pack (workspace root, one level
  above the repo; sha256 + size in `handoff/sha256sums-wo020-inc5r8.txt` and the delivery
  message). Records + the INC6 plan; **no code behaviour change** beyond test007.bat's echo
  words — device application is OPTIONAL; if applied, re-stamp
  `sparq-wo020-inc5r8-2026-10-07`. The r7 pack (`…inc5r7.zip`, sha `fef0480b…`) is what the
  device runs today; its preview.svg hand-delete step is DONE there (stage 6 regenerated it
  from the fresh component).
* **`handoff/sparq-wo020-inc5r8.bundle`** (+ `.sha256`) — `7ed2e08..HEAD` at the r8 documents
  seal, self-contained from the published main. Supersedes `sparq-wo020-inc5r7.bundle` and
  `sparq-wo020-session8.bundle` (kept as the increment record). **Session-9 note (honest):**
  this bundle and the r8 zip did NOT survive into the re-published main `b2445d6` (the pack was
  a workspace-root artefact; the bundle was never committed to the Fresh-start history) — they
  are unreachable from a fresh clone (the same holds for `handoff/make_pack_wo020.py` and
  `handoff/sha256sums-wo020-inc5r8.txt` named below/above). `b2445d6`'s tree IS the r8 seal
  (verified by content, §4), so nothing was lost; the durable history from here is `sparq-wo020-inc6s1.bundle` (above).
* **The packer** `handoff/make_pack_wo020.py` — **reconstructed a SECOND time in session 10
  and COMMITTED** (the original was lost in the re-publication; session 8's reconstruction was
  never committed and did not survive the sandbox — the header declares all of it):
  self-verifying — clean tree, the entry count AND the shipped namelist asserted three ways
  (tree == argv == the numbers printed in SYNC.md), the MUST-NOT-MOVE surfaces (WIT,
  `modules/`, the stamp, fixtures, `instruments/`, `instruments-src/`, mockups) diffed vs base,
  the r2 stamp-name rule, entry-by-entry zip↔tree hashes, the component's pinned sha asserted
  inside the zip, the must-ship set present. Usage is its docstring.

## 6. House rules (the ones that bite if forgotten)

Refuse in words, never silently · smallest honest patch · a gate nobody has seen fail is a gate
nobody trusts (prove failable by injection, revert byte-identical) · acceptance lines are
RECORDED, not invented · ceilings move up only with a measurement AND an operator ruling ·
commit + bundle every slice · `SYNC-STAMP.txt` never rides a pack and never moves from the
sandbox (the device re-stamps) · defects get numbered ledger rows in `CHECKLIST.md` (next free
number: **#101**) · design-token and plan movements are operator rulings, recorded where they
happen.
