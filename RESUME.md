# RESUME.md — where the work stands, and how to pick it up

**As of 2026-10-07, end of the session-8 delivery (r8).** This file is the new-session door:
read it whole (it is sized to load), then read by range from its pointers — **do not load the big
documents whole** (`WO020-STATE.md` is ~600 lines, `CHECKLIST.md` ~2000, `SPARQ-PLAN.md` 1102;
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
poll floor, O-4 the user-data key file), slices **S1–S5**, and **nothing built yet**. Next
action, anywhere: **slice S1, the resizable instrument card** (sandbox-provable end to end).

## 2. What is OUTSTANDING (the honest list)

* **INC6 S1–S5** — the work order itself (`WO020-INC6-PLAN.md` §2 carries each slice's
  acceptance). S1 = `Node.size` + `Op::ResizeNode` + the layout + the corner handle + the
  half-size default; S2 = typing the manifest's `[[ui.panel.widgets]]` into the card's panel
  band; S3 = the key/cadence overrides store + the STREAMS dock tab; S4 = the live plane
  (launch registration → PLAY #58 discharged → the BrokerProvider driver thread → the paced
  guest draw at the D19 token 15 Hz — **this is the slice where panels start updating**); S5 =
  the card's per-panel RATE row + convergence.
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
* **The 1 GB wall stands:** anything pulling wasmtime (`--features instrument-host`) OOM-kills
  in `cranelift-assembler-x64`. That cell is DEVICE-FIRST-COMPILE by ruling — write it against
  the vendored API, desk-check, and say so in the delivery. Everything else (root workspace,
  `--features ui`, `streams`, `streams-net`, the observatory workspace, the python gates)
  builds and tests in the sandbox.
* **`.git` may not survive between turns.** The durable history is the bundles under `handoff/`
  (see §5). Recovery: clone `https://github.com/bitseq27/sparq` (published main `7ed2e08`),
  then `git fetch handoff/sparq-wo020-inc5r8.bundle main && git merge --ff-only FETCH_HEAD` —
  one fetch restores everything through the r8 documents seal. Commit + bundle EVERY slice.
* **Known sandbox-only red:** `sparq-app --features ui` test
  `ui::live::…level_follows` fails in this box (environment-sensitive timing; it fails
  identically on the pristine tree — proven). Device/MSVC runs it green. Not yours to fix.
* **Known standing red:** `token_audit` vs the §18 mockup SVGs (on main since before WO-020;
  `gates.bat`'s `design conformance` row). Not yours to fix either.

## 5. Artefacts (the r8 delivery, 2026-10-07)

* **`sparq-update-2026-10-07-wo020-inc5r8.zip`** — FULL-TREE pack (workspace root, one level
  above the repo; sha256 + size in `handoff/sha256sums-wo020-inc5r8.txt` and the delivery
  message). Records + the INC6 plan; **no code behaviour change** beyond test007.bat's echo
  words — device application is OPTIONAL; if applied, re-stamp
  `sparq-wo020-inc5r8-2026-10-07`. The r7 pack (`…inc5r7.zip`, sha `fef0480b…`) is what the
  device runs today; its preview.svg hand-delete step is DONE there (stage 6 regenerated it
  from the fresh component).
* **`handoff/sparq-wo020-inc5r8.bundle`** (+ `.sha256`) — `7ed2e08..HEAD` at the r8 documents
  seal, self-contained from the published main. Supersedes `sparq-wo020-inc5r7.bundle` and
  `sparq-wo020-session8.bundle` (kept as the increment record).
* **The packer** `handoff/make_pack_wo020.py` (reconstructed session 7, declared in its header):
  self-verifying — clean tree, entry count asserted against SYNC.md + the run sheet, the
  MUST-NOT-MOVE surfaces (WIT, `modules/`, the stamp, fixtures, goldens) diffed vs base,
  entry-by-entry zip↔tree hashes, named shas asserted inside. Usage is its docstring.

## 6. House rules (the ones that bite if forgotten)

Refuse in words, never silently · smallest honest patch · a gate nobody has seen fail is a gate
nobody trusts (prove failable by injection, revert byte-identical) · acceptance lines are
RECORDED, not invented · ceilings move up only with a measurement AND an operator ruling ·
commit + bundle every slice · `SYNC-STAMP.txt` never rides a pack and never moves from the
sandbox (the device re-stamps) · defects get numbered ledger rows in `CHECKLIST.md` (next free
number: **#100**) · design-token and plan movements are operator rulings, recorded where they
happen.
