# RDP-PREP-RUN-SHEET — one strictly COMPUTE-ONLY remote session on SATURN

**Rev 2 of this sheet, by operator ruling: the RDP session is strictly compute-only.**
No step below opens an audio device, and no step can close a device box — it closes the
COMPUTE boxes and makes the physical session shorter. This is exactly the workflow
WINDOWS.md endorses: "Develop over RDP, verify by offline render — **best available now** …
bit-exact and deterministic, so it is a *stronger* check than live playback for DSP work.
All of Phases B, C, E and most of D need no sound card."

Everything HAL, touch, listened or human moved to §8 (the physical-session run sheet) —
nothing was dropped, only re-homed.

**Source state used to write this sheet (GitHub `bitseq27/sparq`, head `52f13b9`, verified
byte-state in the source workspace):** the tree IS rev 3 — `SYNC-STAMP.txt` reads
`sync sync-ui-round-2026-10-01`, `fp 96f/2508905B`; 21 modules incl. `util/mult`,
`util/vca`, `mod/clk`, `mod/seq`; `modules/out/main/sparqmod.toml` present (defect #93's
file — it survived the repo rebuild). Rev-3 gate numbers per READ-ME-FIRST.txt / SYNC.md:
**832 tests (+1 ignored), `ui --audit` PASS 0 failures, selftest 9/9, goldens
`ba577186c988db21`.** Stale sheets at the root (`EXTRACT-AT-REPO-ROOT.txt`: 808/50/155;
`WO006-INC15-RUN-SHEET.md` §4: 761/17/25) are inc7c/inc15-era — the stamp and this sheet
win over both.

**Source-side pre-verification (this is what makes the device run a formality):** the
source clone was gate-verified on a clean Linux machine before this sheet shipped — see
§7 for the measured table. If SATURN's numbers differ from BOTH the expectation and §7,
that is a device-environment finding, not tree drift.

---

## 0. Hard rules for this session

1. **Strictly compute-only.** No `play`, no `soak`, no test004, no test006 J–K, no device
   enumeration runs "just to look". If a step would open an audio endpoint, it belongs to §8.
2. **No acceptance of any kind can close here** — WINDOWS.md: "the acceptance numbers only
   mean something at the physical machine." And the ADR-008 exit stays unfired regardless
   of anything this session shows (CHECKLIST, five times over: "do NOT fire it early").
3. **No feel judgments.** Over RDP the window renders through WARP (software) — "everything
   works, redraws are slower. The 60 fps number belongs to the physical machine"
   (WINDOWS.md L315). Any UI observation is labelled *WARP-render, provisional*.
4. **Digests back, full logs stay on the device** (standing protocol).

## 1. Connect + machine prep (2 min)

- [ ] RDP client audio settings: irrelevant — this session opens no stream. Leave them as
      they are; do NOT troubleshoot endpoints here (that is §8 step 0).
- [ ] No sleep/reboot mid-run: `powercfg /change standby-timeout-ac 0`; pause Windows
      Update for the session (a `gates.bat` full rebuild is 20–45 min).
- [ ] Disk: several GB free on the drive holding the tree (full rebuild + release artefacts
      + logs).
- [ ] Power plan High performance if trivially available — it does not gate compute results,
      but it keeps build times honest and the plan is already set for §8.

## 2. Bring the tree to rev 3 + the drift check (10 min)

The device tree at `Q:\morphosis\code\sparq` is at inc7c (CHECKLIST header: "stamp still
inc7c"; test006 stopped at the sync gate as designed). Rev 3 shipped as an overlay with no
stamp — but GitHub head now carries the full rev-3 tree WITH the re-written stamp, so the
cleanest landing is a fresh clone, not the overlay:

- [ ] `ren Q:\morphosis\code\sparq sparq-inc7c` (keeps its `logs\` history)
- [ ] `git clone https://github.com/bitseq27/sparq Q:\morphosis\code\sparq`
      (no git on the box → run `scripts\setup.bat` from the OLD tree first; it installs the
      toolchain. Fallback if clone is impossible: apply `sparq-update-2026-10-01.zip` rev 3
      over the old tree, then `python tools\sync_check.py --write --sync
      sync-ui-round-2026-10-01` — READ-ME-FIRST §2.)
- [ ] Carry the history: `xcopy /e /i Q:\morphosis\code\sparq-inc7c\logs
      Q:\morphosis\code\sparq\logs`
- [ ] **The drift check itself** (defect #86's class — "the pushed tree can drift from the
      sealed one"):
      `cd /d Q:\morphosis\code\sparq && scripts\synccheck.bat`
      Expected: **`sync_check: OK - ... sync ui-round-2026-10-01 ... src 96f/2508905B`**
      (the `sent unsent` line in the stamp is the delivery marker, not a file hash).
      - FAIL names files → **STOP, do not build.** Send `logs\synccheck.log` back; the tree
        that landed is not the tree that was sealed.

## 3. Gates — the compute box the checklist header is waiting on (20–45 min, unattended)

```bat
scripts\gates.bat
```

- [ ] First run after a sync does a FULL rebuild — `gates.bat` deletes cargo's fingerprints
      on purpose (defects #41/#49: zip-synced timestamps otherwise re-serve stale results).
      `build.bat` printing its changed-file report is likewise expected (SYNC.md), not a
      failure.
- [ ] Expected verdicts (rev-3 numbers; the same table the source machine measured in §7):
      - **832 passed / 0 failed / 1 ignored** (`cargo test --workspace`)
      - `cargo fmt --check` clean; clippy cells clean (workspace, `ui`, `bootstrap-audio`;
        `ui-window` clean single-job — the README's OOM note is a 1 GB-sandbox artefact,
        not a code issue; SATURN has the RAM)
      - goldens bit-identical: **selftest PASS 9/9**, hash **`ba577186c988db21`**
      - pinned evidence renders (durations are part of the hashes — defect #85):
        `mod-demo` 1 s → `1621e1f65b1b64e1` · `drum-demo` 2 s → `f2303f13aa0cf299` ·
        `demo` 2.8 s → `53de3b1f3f40e3c9`
      - modules **21/21** strict; `module_docs --check` 21/21
      - **`ui --audit` PASS, 0 failures** (5 viewport × 4 DPI matrix + 43 checks incl. the
        two round-3 smokes)
      - stress evidence line: **`7bb06379bd6845e5` · 2 383 swaps · 7 617 refused**
      - python gates PASS: `token_gen --check`, `token_audit`, `unsafe_audit`,
        `module_docs --check` (the `just python-gates` four)
- [ ] **Send back: `gates-digest.log`.** This closes the header's open "gates digest" box at
      the REV-3 numbers and formally retires the stale "777 / `src 94f/2188595B` / 37
      smokes" expectation written when the device sat at inc4.

## 4. Headless screenshots — the operator's-eye box, provisional pass (5 min)

```bat
cargo run --release -p sparq-app --features ui -- ui --svg-out docs\ui\shots\rdp-prep-default.svg --width 2560 --height 1600
cargo run --release -p sparq-app --features ui -- ui --svg-out docs\ui\shots\rdp-prep-review.svg  --width 2560 --height 1600 --review
```

- [ ] Vector output, WARP-irrelevant (headless) — safe compute evidence for the "operator's
      eye against `docs/ui/shots/`" box. The final look is still owed on the real screen
      (§8 step 4); label these `rdp-prep-*`.

## 5. OPTIONAL — test006's mechanical subset (30 min; only if §3 is green)

`scripts\test006.bat` is re-runnable (dated banner per run) and the physical session will
re-run it whole, so a partial honest run now costs nothing and pre-proves the sync gate and
the one mechanical claim. Rules for a compute-only pass:

| Step | Run? | Note |
|---|---|---|
| [00b] sync gate | ✅ must PASS | proves §3's tree the interactive way |
| A–E (browser, inspector, re-patch, baseline render) | ✅ | offline render, no device |
| mouse paragraph (wheel zoom, right-drag pan, DEL) | ✅ | RDP forwards mouse; this is SYNC.md round-1's ASK line 1 — answer feel questions *WARP-render, provisional* |
| [06]/[07] slider edit changes `canvas-render.wav` hash | ✅ | the script's one mechanical claim: same hash = "the slider is a lie", in its own words |
| I (cv wire lighting from its own value) | ✅ | offline-driven visual |
| F (rename sheet) | ⚠️ function yes, feel no | keys forward over RDP; "real keys" stays a §8 box |
| G (two-finger scroll), H (LOD walk vs mockup on the stage screen) | ❌ | answer DEFERRED-PHYSICAL; H needs the real screen |
| J–K (PLAY on wasapi-shared, live edit heard) | ❌ **strictly compute-only** | do NOT tap PLAY; answer honestly (n / deferred) — the log line will say so, and §8's re-run supersedes it with LISTENED answers |

- [ ] **Send back: `test006-digest.log` + `logs\ui-digest.log`** with a cover line listing
      which steps ran and which are DEFERRED-PHYSICAL.

## 6. OPTIONAL — dispatch-bench C/D (compute-class CPU evidence, 10 min)

- [ ] `tools\dispatch-bench` C/D numbers for ADR-009 (CHECKLIST item 7's last compute piece).
      CPU-bound, travels over RDP; label the row *RDP session, High-performance plan* in the
      devlog. The stage-machine re-measure still belongs to §8.

## 7. Source-side pre-verification table (filled on the source machine, Linux, clean clone)

> Filled in by the source session that wrote rev 2 of this sheet (2026-10-02, Linux x86-64,
> clean clone of `52f13b9`, rustc 1.99.0, single-job — the 1 GB-sandbox discipline).
> Operator ruling: tests-only pass at the source; the release cells below are the DEVICE's
> `gates.bat` boxes, where they run fast — they are deliberately not pre-run here.

| Gate | Source result (clean clone of `52f13b9`) |
|---|---|
| `sync_check` vs `SYNC-STAMP.txt` | **OK** — `sync sync-ui-round-2026-10-01`, fp `96f/2508905B` matches; 158/159 files byte-exact + `Cargo.lock` soft-missing (gitignored, regenerates on first build — SATURN starts the same way; its `synccheck.bat` will say OK too) |
| `cargo fmt --check` | **CLEAN** |
| `cargo test --workspace` | **832 passed / 0 failed / 1 ignored** (36 suites; the ignored one is `sparq-kernel/src/alloc.rs`'s doc test — the known "+1 ignored") |
| python gates (token_gen --check, token_audit, unsafe_audit, module_docs --check) | **ALL PASS** — 0 files to write; audit clean (3 svg + 27 sources); unsafe clean (11 allowlisted paths / 123 .rs scanned); module_docs **21/21** |
| goldens: selftest 9/9 `ba577186c988db21` + the three pinned exec renders | deferred to device (§3) |
| `ui --audit` | deferred to device (§3) |
| clippy (workspace, ui, bootstrap-audio) | deferred to device (§3) |

MSVC-vs-Linux note: goldens are bit-exact across platforms by design (that is the project's
determinism claim); a hash that differs on SATURN but matches here is a device finding.

### 7b. Round-4 state (filled on the source machine, Linux, clean clone of `871d725` + the rebuild)

> Filled in by the round-4 source session (2026-10-02, Linux x86-64, clean clone of `871d725` —
> the rev-3 codebase + round-4 docs — with ALL round-4 code rebuilt from `ROUND4-HANDOFF.md`,
> rustc 1.99.0, single-job, the 1 GB-sandbox discipline). Unlike the rev-2/3 table above, this
> session ran every gate that runs on Linux, release cells included; only the WASAPI/device cells
> stay the machine's.

| Gate | Source result (round-4 state, this box) |
|---|---|
| `sync_check` vs `SYNC-STAMP.txt` | **OK** — re-stamped `sparq-round4-2026-10-02`, 162 files, `src 96f/2694477B`; the pack carries no stamp (the device re-stamps after applying) |
| `cargo fmt --all --check` | **CLEAN** |
| `cargo clippy` — workspace, `ui`, `bootstrap-audio` cells, `--all-targets -D warnings` | **0 diagnostics** (the `ui-window` cell stays device-only) |
| `cargo test --workspace` | **899 passed / 0 failed / 1 ignored** (the ignored one is `sparq-kernel/src/alloc.rs`'s doc test — the known "+1 ignored") |
| `cargo test -p sparq-app --features ui` | **28 passed / 1 failed** — the 1 is **defect #95**, PRE-EXISTING on the pristine clone (stash-proven): the `ui::live` glide-lag test; operator triage owed |
| python gates (token_gen --check, token_audit, unsafe_audit, module_docs --check, check_text_io) | **ALL PASS** — token_gen writes 0; audit clean; unsafe clean; module_docs **24/24**; text-io clean |
| goldens: `sparq selftest --golden` | **9/9, `ba577186c988db21`** — the DSP never moved (ran on THIS box, release build) |
| the three pinned exec renders | **EXACT** — `mod-demo` 1 s `1621e1f65b1b64e1`, `drum-demo` 2 s `f2303f13aa0cf299`, `demo` 2.8 s `53de3b1f3f40e3c9` |
| `sparq ui --audit` | **PASS, 0 failures — 59 [PASS] lines** (58 carried + smoke 54, the cable-node render hashes) |
| `sparq modules --strict` | **24/24** |
| mutation-stress hash | **`8143e1ddfd8fb261`** on this box — pristine-vs-rebuilt identical (stash-proven); the handoff's `7bb06379bd6845e5` was the lost sandbox's product and does not reproduce here |

## 8. Send-back package (one folder, digests only)

```
gates-digest.log                 (THE box: 832 / ba577186c988db21 / 21 modules / audit PASS)
test006-digest.log + ui-digest   (if §5 ran; with the ran/deferred cover line)
rdp-prep-*.svg                   (§4 shots)
dispatch-bench C/D numbers       (if §6 ran, labelled RDP-class)
cover note, 3 lines:             synccheck verdict · anything FAILED verbatim · minutes spent
```

## 9. THE PHYSICAL-SESSION RUN SHEET (everything this session may NOT do)

Written so each line is a 5-minute box, not a debugging session. Order: ears and fingers
first, the long unattended soak last (it runs while the room is empty).

0. **Machine prep + census:** UMC 204HD powered + default endpoint; both exclusive boxes
   ticked; High-performance plan; sleep off; speakers AND headphones; every other audio app
   closed; `scripts\devices.bat` — confirm **OUT 1-2 (BEHRINGER UMC 204HD 192k)** and NOT a
   `Remote Audio` default (WINDOWS.md §RDP: one device, name containing "Remote", locked to
   44.1 kHz stereo = a remote session is in the way — log out of RDP entirely, console
   session only). `rt setup` must NOT say `NO-mmcss`.
1. **test004, the whole run** (run sheet `WO006-INC15-RUN-SHEET.md` §1 stays current for
   shapes): [03] caps — MULTI-rate exclusive envelope containing 48000 (a lone 96000 = the
   driver refuses 48 kHz exclusive; a finding, send the caps block) · [04] exclusive tone,
   LISTENED: no `adjusted` line, open line `48000 Hz · 2 ch · i24-in-32 (converting) ·
   device period 480 fr (10.00 ms) · sparq block 64 fr`, no `NEGOTIATED` line, latency
   ~10.67 ms, drift within a few hundred ppm, 0 late wakes, `play: PASS`, clean tone for
   the full 10 s · [05]/[06] unplug + recovery, in exclusive · failure reading is
   pre-written in run sheet §3 (triple returns → #92 rate-independent → push-mode exclusive
   is the named lever, its own increment).
2. **Touch protocols (Phase-0 DoD):** 12-module patch built entirely by touch **< 3 min**
   (timer running) · real-finger canvas test · palm rejection · two-finger inspector scroll
   (test006 G) · DPI matrix walk on the real monitors · 60 fps on the stage device ·
   100 % of touch targets ≥ 44 px (the audit says so; the finger confirms).
3. **test006 whole, re-run:** F under real keys · H LOD walk vs `design-mode.svg` ON THE
   STAGE SCREEN (write the diff list into the H answer) · J–K LISTENED: PLAY audible on
   wasapi-shared, wires animating while it plays, live slider edit heard without stopping ·
   the standing ASKs: one fast trim drag (hear the inc7c glide — no staircase kinks);
   wheel/right-drag with the real mouse; drag an LFO onto a gain knob's dot and hear the
   knob become a bias; clock → seq → percussion, listened; a mult split lighting like its
   source (rev-3 asks) · WO-009 LISTENED tempo sweep.
4. **Identity + human protocols (WO-004, "unrun" per CHECKLIST):** blind identity check ·
   3 m distance review (tape measure) · dark-room review · glove test · monochrome check ·
   the real-screen look at the converged chrome vs the `rdp-prep-*` shots from §4 — the
   screenshot that reads as "scientific software" (Phase-0 DoD).
5. **Hardware numbers:** WO-001 sheets (`docs/hardware/stage-baseline.md`) — DPC baseline
   WITHOUT any remote session · WO-014 `<15 % of a core` benchmark (stage-machine
   acceptance — "a sandbox CPU ratio does not transfer") · WO-008 loaded soak (200 modules,
   30 min, zero xruns) · WO-009 30-min drift run on the new delivered-vs-wall metric (#76 —
   old drift logs are not comparable) · dispatch-bench C/D re-measure to replace §6's
   RDP-class row.
6. **THE ACCEPTANCE: test004 [07] — the 2 h zero-xrun EXCLUSIVE soak at the rate [04]
   proved** (prompt default; 96 kHz stretch after, if the machine is free — quirk-table
   evidence either way). On PASS: **WO-006 acceptance closes → ADR-008 exit FIRES** (only
   now: delete the cpal bootstrap, HAL becomes `play`'s default) and the ratify-or-strike
   decision on the amended acceptance wording (run sheet §2) is written down against the
   digest.
7. **WO-015, the music:** record the Phase 0 study — 45–90 s made only in sparq, built by
   touch, reproducible from project + journal — and the ranked pain log (the touch session's
   annoyances, ranked; that list becomes the Phase 1 backlog).
8. **WO-016, the gate:** devlog published; every number above measured and recorded; Phase 1
   backlog written from the pain log. **That is Phase 0 wrapped.**

**Machine-day checklist:** interface + speakers/headphones · tape measure · gloves · a
dark-hour slot · timer · 3–4 h block (2 h of it is the soak) · this sheet on a second
device.
