# CHECKLIST.md — session handoff (live document)

**Purpose:** the first file a new session reads. Says what is done, what is in flight, and what to do
next. Updated at the end of every session (and mid-session when state changes). This file supersedes
`RESUME.md`'s handoff role (RESUME.md is the 2026-09-22 snapshot; keep it as history). For deep
history see `PHASE0-WORKORDERS.md` §2.1 (status table) and its build-log sections — read **by line
range**, never whole (see `RESUME.md` §1 for why).

**Last updated:** 2026-09-26 (WO-014 inc 2 built, sealed, zipped — and SATURN has **applied all three previous bundles**: wo013-inc3, wo006-inc13, wo008-inc3)

---

## Current position

- **Device state:** the user confirmed 2026-09-26 that **all three bundles are applied on
  SATURN** (wo013-inc3 → wo006-inc13 → wo008-inc3). The pending device runs are unchanged:
  `test004` (exclusive acceptance — the #79 fix), `test006` (canvas window session), `gates.bat`
  (**expected test count is now 559** — log_check's baseline moves with it), and the WO-008
  **loaded soak** (200 modules, 30 min) when the HAL and the machine are both free.
- **WO-014 increment 2 is BUILT and sandbox-green** — the six AUDIO-domain modules
  (`syn/noise` seeded+`reset`, `syn/polyblep` (stable id, bandlimited-additive impl per #12 —
  the manifest says so in words), `flt/svf` five modes, `util/delay` with the set's first
  **parametric latency** (`"param:time"`), `fx/bitcrush` seeded per-channel dither,
  `util/panner` two laws): **9 first-party modules total**, `modules --strict` 9/9, six module
  goldens + a five-module chain golden checked in, per-module 5 000-call zero-alloc gates, the
  **polyblep aliasing acceptance measured through the registry build: −166.8 dB vs the −60 dB
  bar** (printed by the test every run), the delay echo landing at exactly its declared sample.
  **`tools/module_docs.py`** ships the generated-docs acceptance (9 docs, `--check` wired into
  justfile + ci.yml + gates.bat, proven failable pre-ship; check_text_io caught its cp1252
  console gap pre-ship — #37's class). **559 tests** (was 549), goldens UNCHANGED
  (`ba577186c988db21` / `0f5c3e86c7f117a9` / `d46736fd9a1c48a1` — the demo renders identically
  with a 3× registry), selftest 8/8, audit PASS 19 smokes, all clippy cells clean, 6 Python
  gates clean. The **eight modules that did NOT ship are declared waiting on contracts**:
  membrane/env-ad/lfo/clk-div/mixer (event/cv/multi-port → contract v1 + WO-009), tap/scope
  (ring publication), out/main (ship with the canvas master-handover so the badge never lies).
  Sealed as **`sync wo014-inc2`**, bundle **`../sync-wo014-inc2.zip`**.
- **WO-008 increment 3 is BUILT and sandbox-green** (tasks 4–7): the boundary-swap `Engine`
  (`sparq-audio::engine` — clock inheritance, retire-at-boundary, master-vanish refuses in words;
  single-owner by declaration — the cross-thread hot-swap primitive is parked kernel work), the
  **10 000-mutation stress** (`tests/mutation_stress.rs`: 10 001 blocks · 7 203 swaps · 2 797
  refusals · **0 audio-path allocations** · hash `b42068ec7b206789` bit-identical across
  debug/release), kernel **per-path latency** (`latency_map`/`path_latency`/`max_path_latency`,
  cached on version+edit-count+block_frames, 3 hand-computed reference graphs) + the
  **raw/compensated switch** (per-edge fan-in alignment, chains byte-identical, goldens pin the
  Raw default), the **watchdog** (3 consecutive overruns ⇒ auto-bypass + bounded journal;
  passthrough-or-silence; streak resets on any good block), and the **determinism harness**
  (`sparq_audio::determinism` — it caught a real HashMap-iteration-order nondeterminism in the
  stress schedule on day one; fixed pre-ship, recorded in the build log). **549 tests** (was 526,
  +23), all clippy cells clean (default/ui/bootstrap/MSVC×3), release goldens + selftest 8/8 with
  identical hashes, `ui --audit` PASS 19 smokes, 4 Python gates clean. **Multi-port `AudioCtx`
  (contract v1) deliberately NOT done** — a contract change deserves its own increment; non-audio
  edges stay refused in words. Sealed as **`sync wo008-inc3`** (stamp regenerated + verified, self-test re-proven
  failable), bundle **`../sync-wo008-inc3.zip`** (15 entries, namelist-checked).
- **WO-006 increment 1.3 is BUILT and sandbox-green** (the defect-#79 exclusive device-period
  fix): new ungated `sparq-kernel::hal::period` (pure ladder, 7 Linux unit tests incl. the exact
  test004 device shape), `open_exclusive` executes it with a fresh client + alignment two-step per
  candidate and a full period probe table on refusal, pump unchanged (the FIFO already decouples
  period from block — the 2 h shared soak proved this ratio at duration). **526 tests** (was 519),
  all 7 runnable clippy cells clean (MSVC hal cells re-checked fresh after touch), release goldens
  pass, selftest 8/8 with determinism `0f5c3e86c7f117a9` identical, 4 Python gates clean. Sealed as
  **`sync wo006-inc13`**, stamp **`src 83f/1484219B`** (124-file covered set verified,
  self-test re-proven failable); bundle
  **`../sync-wo006-inc13.zip`** (10 entries, namelist-checked); device contract = the existing
  `scripts\test004.bat` (header/hint text updated) + `WO006-INC13-RUN-SHEET.md`. Defect **#79 is
  now LOGGED** (table row in the inc-1.3 build-log entry; arithmetic in `docs/hal/windows-notes.md`
  §4c).
- **WO-013 increment 3 is BUILT and sandbox-green** (browser + inspector + wire re-patch, details
  in the build-log entry in `PHASE0-WORKORDERS.md`, search "increment 3"). Sealed as
  **`sync wo013-inc3`**, stamp **`src 82f/1473629B`** (123-file covered set verified;
  `sync_check.py --self-test` re-proven failable). Bundle: **`../sync-wo013-inc3.zip`** (19 entries,
  namelist checked against `SYNC.md`). Device contract: `scripts\test006.bat` +
  `WO013-INC3-RUN-SHEET.md`.
- Measured at the inc-3 seal: **519 tests passing** (+1 ignored, was 474) · fmt clean · clippy clean in
  default / `ui` / `bootstrap-audio` / MSVC×3 cells · native `ui-window`/gles cell clean at `-j 1`
  with dev debuginfo off (**new finding:** the recorded "naga OOM" is parallel-job pressure at
  1 GB, not naga itself) · `sparq ui --audit` **PASS, 0 failures, 19 smokes** (3 new) ·
  `selftest --golden` **8/8**, golden `ba577186c988db21` matching, determinism `0f5c3e86c7f117a9`
  identical, 525.9× realtime (relaxed sandbox codegen — SATURN uses the repo profile) ·
  `modules --strict` + `exec` PASS (rms tap 0.16621882 cross-validated) · 4 Python gates clean ·
  goldens unchanged (untouched patches render at manifest defaults, bit-identical to inc2).
- MSVC × `ui-window` cell: **still unrunnable anywhere** — the `windows` crate's rustc is
  OOM-killed at 1 GB (environment, not code; CI has no such cell either).

## 🔴 THE BLOCKER (device track — waiting on SATURN, not on code)

**WO-006 exclusive acceptance = the `test004` re-run** with `sync-wo006-inc13.zip` applied.
History: `test004` attempt 1 (2026-09-24, inc 1.2 build) passed the caps checkpoint (#77's format
fix verified on hardware) and failed every rung at `Initialize (exclusive): HRESULT 0x88890020` =
`AUDCLNT_E_INVALID_DEVICE_PERIOD` — the open asked the 64-fr block (666.7 µs @ 96 kHz) as the
device period against a 10 ms engine (**defect #79**, now logged and fixed in inc 1.3 as above).
On the re-run, the [04] open line should read
`i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr`; a second
`INVALID_DEVICE_PERIOD` against a 10 ms ask would mean the driver wants a number it is not
reporting — the new period probe table in the error is the diagnosis either way. When [04]+[05]
pass and the **2 h zero-xrun soak @ 96 kHz/64 exclusive** finishes clean: WO-006 acceptance
closes → ADR-008 exit fires → **next sandbox increment: delete the cpal bootstrap, HAL becomes
`play`'s default** (that deletion is gated on this device run — do not do it early).

## Next session's task list, in order

**Device track (when SATURN is reachable — both bundles are sealed and waiting):**
1. Apply `sync-wo013-inc3.zip` then `sync-wo006-inc13.zip`; run `scripts\test004.bat` (the
   exclusive acceptance — pass shape in `WO006-INC13-RUN-SHEET.md`) and `scripts\test006.bat`
   (the inc-3 window session; step [07] is the mechanical claim: slider edit must change the
   canvas-render.wav hash). Wanted back: `test004.log`, `test006.log`, `logs\ui.log`.
2. If test004's 2 h soak passes → **WO-006 acceptance closes** → sandbox increment: ADR-008 exit
   (delete the cpal bootstrap, HAL becomes `play`'s default), then WO-006 increment 2 backlog
   (ASIO, duplex, round-trip measurement, STA retry for the #75 property-store `E_ACCESSDENIED`,
   clock-drift re-derivation per #76).

**Sandbox track (buildable now, in value order):**
3. **Multi-port `AudioCtx` (contract v1)** — now THE unblocking increment: cv/event payloads
   travel, the executor stops refusing non-audio edges, and it unlocks in one stroke: the eight
   remaining WO-014 modules, the rms→filter modulation demo (acceptance), WO-013's live wire
   levels (with the ring publication), and the canvas cv wires. Coordinate with the
   compat-matrix mirror deletion (the `port.rs` vs `compat-matrix.toml` dedup carried since
   WO-007). Contract change → increment ADR-005/module-api-v1 carefully; the throwaway-module
   gate and the API-surface snapshot both exist to catch exactly this kind of edit.
4. **WO-009 clocks + transport v0** (depends on WO-008 — now built): three clocks, event queue,
   sample-accurate dispatch; unblocks `mod/lfo` sync + `mod/clk-div` + the delay's tempo-sync
   parameter path.
5. **Kernel hot-swap primitive** (ADR-009 d3's literal pointer swap, epoch retirement) — the
   allowlisted-`unsafe` increment that makes `Engine` cross-thread and publishes meters/analysis
   through the lock-free rings (decision 8).
6. **WO-013 increment 4** — live wire levels **only with** the executor taps (do not fake them);
   else rename text entry (shared with the browser's provisional keyboard feed), inspector
   scrolling, LOD visual iteration vs `design-mode.svg`.
7. Studio-session evidence still open: real-finger canvas touch-test, DPI matrix walk
   (WO-012/WO-013), WO-001 hardware sheets, dispatch-bench C/D numbers for ADR-009, and the
   WO-008 **loaded soak** (200 modules, 30 min, zero xruns — the WO's hardware acceptance).

## Done (most recent first)

- [x] **WO-014 inc 2** (2026-09-26, this session) — the audio-domain six (9 modules total),
      aliasing acceptance −166.8 dB measured, six+one goldens, zero-alloc gates, docs generator
      + new CI/gates stage, `set_phase` on the osc. Sealed `sync wo014-inc2`.
- [x] **WO-008 inc 3** (2026-09-25, this session) — tasks 4–7: Engine + 10k mutation stress +
      latency map & compensated fan-in alignment + watchdog auto-bypass + determinism harness
      (caught a HashMap-order nondeterminism pre-ship). Sealed `sync wo008-inc3`; device work:
      the loaded soak (needs SATURN + HAL).
- [x] **WO-006 inc 1.3** (2026-09-25, this session) — the defect-#79 exclusive device-period
      ladder (`hal/period.rs` pure + Linux-tested; `open_exclusive` rewired; #79 logged with its
      arithmetic in windows-notes §4c; test004.bat text updated; sealed `sync wo006-inc13`,
      10-entry zip, run sheet). Device-verified: NOT yet — that is test004's re-run.
- [x] **WO-013 inc 3** (2026-09-25, this session) — module browser + fuzzy search
      (`sparq-ui::canvas::browser`), inspector with touch sliders + per-node param state
      (`Op::SetParam`, one drag = one undo step, bridge renders node state), wire-end re-patch
      (grab handles in `layout`, verdict on the post-removal graph, refusals restore byte-exact),
      shell routing + provisional keyboard feed, painter for all three, 3 new audit smokes,
      test006.bat + run sheet + SYNC.md + zip. Caught in-development by the new tests and fixed
      before shipping: slider coalescing kept the wrong `from` (undo restored the first waypoint);
      browser NO-MATCH row reserved by `height()` but not hittable (tap closed the sheet silently).
      Live wire levels deliberately NOT done (need WO-008 taps — parked in LATER.md, not faked).
- [x] WO-013 inc 2b — delivery-path fix (#78); **test005 PASS on SATURN 2026-09-25 13:37** (bridge
      verified on hardware, operator listened)
- [x] WO-013 inc 2 — the bridge; WO-014 inc 1 — module batch + `sparq exec`; WO-008 inc 1–2 —
      graph core + executor; WO-006 inc 1.2 — integer exclusive rungs (#77 fix; probe side
      verified on hardware, open side blocked by the INVALID_DEVICE_PERIOD failure above)
- [x] P1/P1B/P1C device runs — first MSVC builds green (golden bit-identical, selftest 8/8, 1059× realtime)
- [x] WO-007 complete — module contract as data + code; WO-012 inc 1 — gesture core + egui shell;
      WO-013 inc 1 — canvas model + painter; WO-000…WO-005 — workspace, tokens, ADRs, mockups,
      Phase A/B/C1 (DSP toolkit, music systems)

## Environment notes (sandbox) — re-read every session

- **`.git` is NOT durable in this workspace** (observed twice on 2026-09-25: snapshot cycles drop
  it — `.git/config` is a credential-excluded path, and the whole dir vanishes). Do not rely on
  git for state; the integrity tool is `sync_check.py` (content hashes) and the seal is the zip.
  Re-init for in-session diffs if useful, expect it gone next session.
- Toolchain wipes **mid-session, without warning** (happened again 2026-09-25 during WO-006 work:
  `/opt`, `target/`, apt packages AND `.git` all vanished; `"$ARENA_WORKSPACE"` files survived intact —
  `sync_check.py` byte-verified the tree afterwards). Re-init git after a wipe if diffs matter:
  `git init && git add -A && git commit` on the sealed state, so the increment shows as a diff.
  Lives in `/opt`, deliberately outside the snapshot. Restore (~80 s with warm apt, ~4.5 min cold):
  ```bash
  apt-get update && apt-get install -y curl ca-certificates gcc libasound2-dev pkg-config
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
       --default-toolchain stable --component rustfmt,clippy
  # GOTCHA (hit 2026-09-25): the installer uses $HOME — in this sandbox that can be /tmp, so it
  # lands in /tmp/.cargo. Check, then move: mv /tmp/.cargo /opt/cargo; mv /tmp/.rustup /opt/rustup
  export RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
  rustup target add x86_64-pc-windows-msvc   # cross-LINT only; never links
  ```
- Every cargo call needs the exports above (shell state does not persist between tool calls).
- **1 GB RAM is the binding constraint.** `ui-window`/wgpu tree: build/clippy with `-j 1` and
  `CARGO_PROFILE_DEV_DEBUG=0` (proven this session). `ui --audit` + goldens: run release with
  `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` — SATURN builds with
  the repo profile. The MSVC×ui-window cell does not fit even single-job (`windows` crate OOM).
- Background processes do NOT survive between tool calls — run long builds synchronously.
- Default workspace build has zero third-party deps → `cargo test --workspace` is cheap (~40 s).
- Gates sequence (`just gates` equivalent, plain commands): `cargo fmt --all --check` → clippy
  cells → `cargo test --workspace` → 4 python gates (`tools/check_text_io.py`, `token_audit.py`,
  `unsafe_audit.py`, `sync_check.py`) → release golden tests → `selftest --golden` → `ui --audit`.
- **After any product-code change, re-seal:** `python3 tools/sync_check.py --write --sync <name>
  --sent <date>` then `--quiet` (verify) and `--self-test` (prove failable), rebuild the zip, and
  keep heading = list = contents counts equal in `SYNC.md` (the off-by-one class is checked, not
  hoped). New `scripts/*.bat` files ARE stamp-covered (covered set 123 files); `fp` counts only
  the five src roots (82f).
- Python text I/O must be explicit `encoding="utf-8"` (+`newline="\n"` on write) —
  `check_text_io.py` gates it. `.bat` files: pure ASCII, CRLF, no unescaped parens in echo text
  inside blocks (#42/#69).
- Avoid literal workspace paths inside Python source (sandbox rewrites them); use relative + cwd.

## Open defects / debt (tracked in PHASE0-WORKORDERS.md §2.2)

- **#79 — LOGGED + fixed in sandbox (inc 1.3), device-verified: pending** (test004 re-run).
- #69 — `build.bat` cmd-parser death: probe ships, culprit statement not yet named (stays open)
- compat-matrix mirror in `port.rs` vs `docs/api/compat-matrix.toml` — read at discovery, delete
  the mirror (carried WO-007 → WO-008, still open)
- WO-002: two typefaces still `chosen = ""`; WO-004: human protocols unrun; ADR-009 still `draft`
- `sparq-app/src/modules.rs`: no unit tests (CLI smoke only); `sparq-module-api`: no golden render
  of its own
- WO-013 inc 3 v0 limits (all parked in `LATER.md` with reasons): no numeric param entry, no
  inspector scrolling, no enum/text/blob editing (manifest v1 `options[]` first), single-selection
  inspector only, provisional keyboard feed for the browser, no browser scroll gesture
- WO-008 inc 3 declared limits (parked in `LATER.md` §WO-008): single-owner Engine until the
  kernel hot-swap primitive lands; compensation aligns PLAIN fan-in arms only; watchdog→UI red
  hairline is a WO-013-side painter increment; meters not yet ring-published cross-thread
- MSVC × `ui-window` clippy cell: never run anywhere (sandbox OOM, no CI cell)
- Device-side baselines that MOVE with this increment: gates.log test count **559** (log_check),
  `sparq modules` count **9** — the run sheets' expected outputs say so; SATURN's next gates run
  will show them
