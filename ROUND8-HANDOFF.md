# ROUND8-HANDOFF.md — sixteenth session handoff: the WO-018 zero-dep half REBUILT from round 7's prose, compiled and measured green (2026-10-05)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's head). This round's
shape is the round-4 precedent repeating: **the previous session's code was lost with its
sandbox; only its prose reached origin; this session rebuilt the code from the prose and then
did what round 7 never could — compile it, test it, gate it, commit it.** Everything below is
measured on this box unless explicitly flagged otherwise.

---

## 0. READ THIS FIRST — state of the tree, delivery, provenance

**What this tree is:** a fresh clone of the re-publicised origin (`6681f87`, the operator's
"Fresh start — rebuilt git history" head, which carried the round-6 sealed state + round 7's
PROSE only) **plus five real commits** rebuilding and closing WO-018's zero-dependency half:
`450c38b` (contract-crate additions) · `7eee56b` (sparq-host-wasm) · `b3f2091` (the CLI wiring) ·
`51b25aa` (docs/CI/layout) · plus the records+pack commit(s) at the head. All gates measured
green AFTER the rebuild (§2). The working tree is clean; `Cargo.lock` regenerates (gitignored,
as always).

**The loss, confirmed with evidence (the round's reason to exist).** Origin's single ref had no
commit touching `crates/sparq-host-wasm`; `handoff/sparq-handoff-2026-10-05.bundle`'s head is
`15492fe` (round 6's seal — round 7 made zero commits BY DESIGN, so no bundle could carry its
code); no round-7 overlay zip exists anywhere; and the round-7 files that DID survive
(`ROUND7-HANDOFF.md`, `WO018-STATE.md`) are exactly the two the handoff says were "written with
certainty-of-content (no compiler needed for prose)". The CHECKLIST's ⚠ IN-FLIGHT paragraph and
`docs/instrument-host.md` did not survive. Conclusion: round 7's uncommitted tree died with its
sandbox before the operator's re-publish — ROUND5 §3.1's defect class, this time taking the
working tree itself instead of `.git`.

**Provenance / durability:** this round committed EARLY and OFTEN against exactly that defect
class: four increment commits, a bundle after each (`/home/user/handoff/sparq-r8-inc{1..4}.bundle`),
the closing bundle + full-tree overlay zip + sha256sums at pack time (§6). The local `.git`
survives snapshots (only `.git/config` and credentials are excluded), and `handoff/` artefacts
are tracked, so the round rides origin at the next push — **push was attempted and REFUSED (no
credentials in this sandbox)**; delivery to origin is the operator's `git push.bat` step.

**Operator rulings this round: none were needed.** No question tool call was made: the scope was
the operator's "continue with handoff 7" instruction, and the handoff + state card + ticket +
frozen contract fully determined the work. Every judgement call is listed in §4 for review; each
is reversible while no instrument exists.

---

## 1. What this round did

1. **Probed the environment** (recipe step 1): healthy — `/bin/true` spawns, loadavg `1/89` of
   1129 threads at open. The recipe's stop-condition never fired this round.
2. **Rebuilt the environment from nothing** (ROUND5 §7 + ROUND6 §7, ~10 min measured, not 25:
   the downloads were fast): rust 1.99.0 standalone → `/home/user/.cache/toolchain`, `pip install
   ziglang` + the `zigcc-direct` shim, env per `/home/user/.cache/env.sh`. NEW delta: the
   **MSVC std face** (`rust-std-1.99.0-x86_64-pc-windows-msvc`) was installed and USED (§2).
3. **Measured the pristine baseline BEFORE any edit** (the house rule): `cargo test --workspace`
   = **909/0 (+1 doctest ignored)** — the round-6 seal, exactly. The clone was sound.
4. **Read the frozen spec set** and rebuilt round 7's inventory (WO018-STATE.md's table is the
   authoritative checklist — every row is present in this tree): the five catalogue codes
   (25→30), `GPU_CLASSES` + drift/value pins + the field-table domain row, `check_gpu_class` at
   decode, the `sparq-host-wasm` crate (ceilings/package/validate + package_gate), the CLI
   (`sparq mod validate` / `sparq mod list`), `instruments/` + README, `docs/instrument-host.md`,
   the CI cells, the WO-018 status line. All seven ROUND7 §1 judgement calls were kept AS
   RECORDED (ceiling numbers, profiles, honour list, `HOST_MODULE_API`'s home, scene⇒gpu rule,
   CI shape, pre-registered codes).
5. **Ran the resume recipe's gates** (§2), committed in the recipe's four increments with a
   bundle after each, and probed the wasmtime half (§3).

**What was NOT rebuilt:** the CHECKLIST ⚠ paragraph (superseded by this close — the state it
described no longer exists) and round 7's exact prose for `docs/instrument-host.md` (lost; the
rebuild carries the same content obligations — §2–§4 design, §8 blockage + recipe — sourced
from ROUND7-HANDOFF §1/§3/§4, the WIT README's decisions, and the ticket. It is a declared
RECONSTRUCTION in its own header, on the `out/main` manifest precedent).

---

## 2. Verification matrix — measured on this box

| gate | round-6 seal | round 8 (this round, after the rebuild) |
|---|---|---|
| pristine baseline | 909/0/1-ignored | **909/0/1-ignored — reproduced exactly, before any edit** |
| `cargo test --workspace` | 909/0/1-ignored | **925 / 0 / 1-ignored** (+16 = the state card's itemised expectation EXACTLY: manifest drift-pin 1, decode gpu 1, ceilings 2, package 5, package_gate 5, instruments parse 2; round 7's "≈927" was loose arithmetic on its own list) |
| fmt | clean | `cargo fmt --all` applied (first pass over the rebuilt code) then `--check` CLEAN |
| clippy `-D warnings` (workspace, all targets) | clean | **CLEAN** (two lints fixed on the way: a `matches!` shape and the test table's `type_complexity`, allowed with the api_snapshot precedent) |
| MSVC crosscheck cells (default features) | clean | **CLEAN and MEASURED here**: module-api, host-wasm, music, kernel, audio, ui, app — all `--target x86_64-pc-windows-msvc --all-targets` (the std face installed this round; clippy does not link) |
| MSVC cells hal-wasapi / ui-window | clean (round 6) | NOT re-run (heavy `windows`-crate compiles; this round touched neither surface) — carry-forward, device re-runs in gates.bat |
| python gates | clean | **CLEAN**: check_text_io ✓, token_gen --check ✓ (0 writes), token_audit ✓, unsafe_audit ✓, module_docs --check **24/24** ✓, sync_check --self-test 11/11 ✓ |
| `sync_check` (bare) | FAIL BY DESIGN (11 files) | **FAIL BY DESIGN — 15 files** (round 6's 11 movers + this round's contract/CLI edits); the device re-stamps (§7) |
| WIT hash pins | pinned at freeze | **UNTOUCHED and passing** — `docs/api/instrument-wit/wit/*` did not change; the freeze holds (the hash pins prove the rebuild touched no frozen surface) |
| CLI smoke | — | `sparq mod list --strict` on the repo's own `instruments/` → 0 packages, **exit 0** (the CI smoke step, measured); `sparq mod validate reference/instrument-template/` → stage 1 PASS (the frozen template passes every static instrument rule), stage 2 FAIL in words (source, not package — correct), runtime stages REFUSED, exit 1; a legal five-file package in /tmp → PARTIAL, **exit 1** ("a partial gate is not a hand-in") |
| ui --audit | carry-forward | unchanged — this round's only UI-adjacent surface is the CLI (no painter, no geometry) |
| wasmtime probe | — | **MEASURED this round** (§3): dependency tree green to `cranelift-codegen`, then OOM SIGKILL — the blockage is the sandbox's 1.06 GB, categorically |

Round-4 device obligations stand untouched (test006 A–U, test004 attempt 4, defect #95, the
dropdown picker); round 6 and 7 added none that this round did not discharge or carry explicitly.

---

## 3. The wasmtime probe, measured (the runtime half's pacing risk)

The retry recipe (`docs/instrument-host.md` §8) ran with every mitigation (`-j 1`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`, separate `CARGO_TARGET_DIR`, zigcc-direct).
Result: wasmtime **49.0.2 resolved and locked cleanly** (the exact pin's metadata is proven
valid); ~60 dependency crates checked green — including `cranelift-assembler-x64`, round 7's
victim, which the debug=0 setting saved — until **`cranelift-codegen` was SIGKILLed (OOM) at
4 m 21 s** on the 1.06 GB MemTotal, at `cargo check` (the cheapest signal that exists). A 3 GB
swapfile was built and REFUSED (`swapon`: EPERM — no CAP_SYS_ADMIN in the container). Loadavg
stayed healthy throughout (`1/537`): this was memory, not round 7's thread exhaustion.

**Ruling this produces:** the runtime half is deferred to a ≥ 2 GB host — the device
(`scripts\gates.bat` after §7) or CI (ubuntu-latest), both of which clear the measured floor.
The feature-on CI cells stand as recorded (never run to completion anywhere; risk narrowed and
documented in §8's note, with the MSVC fallback). **The zero-dep half ships regardless — it is
green here, which is the ticket's acceptance criterion 2.**

---

## 4. Judgement calls made this round (flagged for review; reversible while no instrument exists)

Kept from round 7 (its §1, all seven — the rebuild's fidelity claim): the ceiling numbers and
anchors; the launch-vs-strict profile split; the static capability-honour list; `HOST_MODULE_API
= 1`'s home in `sparq-host-wasm::validate`; scene⇒gpu_class-above-none; the CI shape; the five
pre-registered codes. NEW this round:

1. **`sparq mod validate` exits NON-ZERO on PARTIAL.** The gate's promise is "passes validate ⇒
   loads at launch"; while stages 3/4/6 refuse, the CLI must not claim the promise was kept. The
   words say exactly which half ran. (Consequence: no CI step can use `mod validate` as a green
   gate until the runtime half lands — the smoke step is `mod list --strict` instead, which is
   what round 7's CI decision recorded anyway.)
2. **`validate::schema_check` is public** — stage 1's static rules as a bare report, so the
   launch door (`mod list`, later the browser) and the gate run the SAME rules in the same
   order. The CLI is a voice, never a second implementation.
3. **Loose files in `instruments/` are named in words** (`package::loose_files`), with
   `README.md` exempt as the folder's own layout artefact — a misdropped package (five files
   without their directory) is a visible refusal, never a silent ignore.
4. **Role matching is case-sensitive** (`readme.md` is Unknown, with words): a distributed
   artefact does not get two spellings of one role.
5. **Unrecognised files are `E-VALUE-MALFORMED:package/<name>`** (within the cap): the frozen
   catalogue has no package-file-role code, and pre-registering a sixth v1.1 code was NOT this
   round's call — the existing derived vocabulary carries it actionably.
6. **Stage-1 decode failures use the generic words** ("{n} schema failure(s), verbatim below"):
   the decoder's own lines print beneath, so the author loses nothing; round 7's exact phrasing
   was not recorded.
7. **The MSVC crosscheck cell for the feature** stays as round 7's decision recorded it, with
   the unmeasured-risk note and the documented fallback in `docs/instrument-host.md` §8 — the
   honest shape: the decision is kept, its risk is stated, the fallback is pre-agreed in writing.
8. **`instruments/README.md` ships in the repo** (the layout artefact the state card lists);
   `sparq mod list` on the repo's own folder therefore reports 0 packages and exit 0 — the CI
   smoke's expected shape.

---

## 5. The runtime half — what the next round does (WO-018 increment 2)

Design: `docs/instrument-host.md` §2 (loader), §3 (fuel/epoch↔watchdog), §4 (two-instance
ordering — freeze debt #2 CLOSED as spec; hot reload). Test plan: §7 (smoke.mjs's Rust mirror,
the sha-pinned noop-wasm fixture rebuilt from the SDK per the WIT README §7 recipe, fuel→watchdog
through the real executor, hot reload at the boundary, launch-loading, the template end-to-end
under the 60 s bar). **Requirement: a ≥ 2 GB host** (§3's measurement). Order of work: the §8
retry recipe's step 2 (`cargo check --features instrument-host`) FIRST — it is the pacing risk
and it is now known to be memory-bound, not code-bound; then runtime.rs; then the fixture; then
the wiring. Acceptance criteria 1/3/4/5 of the ticket close there; criterion 2 is already green.

---

## 6. Git artefacts

```
6681f87  clone head (the operator's "Fresh start" re-publish: round-6 seal + round-7 prose)
450c38b  WO-018 (rebuilt): contract-crate additions                     ┐
7eee56b  WO-018 (rebuilt): sparq-host-wasm — the zero-dependency half   │ this round, bundled
b3f2091  WO-018 (rebuilt): sparq mod validate / list — the CLI wiring   │ after each
51b25aa  WO-018 (rebuilt): docs, CI cells, layout                       ┘
<head>   records (CHECKLIST/WO018-STATE/this file) + the round pack
```

Author `sparq-agent <agent@localhost>` (the recorded placeholder). `/home/user/handoff/`:
`sparq-handoff-2026-10-05-r8-final.bundle` (main through the artefacts commit — the durable
one), `sparq-update-2026-10-05-r8.zip` (FULL tree, the defect-#94 remedy — depends on no base
you might not have; `handoff/` excluded from the zip to keep it lean), `sha256sums-r8.txt`.
Per-increment safety bundles were created after each commit (ROUND5 §3.1's standing rule) and
PRUNED once the final bundle verified — they are superseded, and the snapshot budget is not
infinite. The closing bundle + zip + sums are committed under `sparq/handoff/` so they ride
origin at the next push. **Push refused here (no credentials) — the operator's `git push.bat` is
the delivery.**

---

## 7. Device-apply steps (Windows, SATURN)

1. Pull or apply: `git pull` if the operator pushed from the sandbox side; otherwise extract
   `sparq-update-2026-10-05-r8.zip` AT THE REPO ROOT, overwriting (excluded from the pack:
   `.git`, `target/`, `Cargo.lock`, logs, `*.wav`, `SYNC-STAMP.txt`, `handoff/` — your copies win).
2. Re-stamp: `python tools\sync_check.py --write --sync sparq-wo018-rebuild-2026-10-05` (until
   then, build.bat/test006's [00b] name the mismatch in words — EXPECTED, not a failure).
3. `scripts\gates.bat`. Expected: **925 tests (+1 ignored), 0 failed**; fmt/clippy clean;
   `module_docs` 24/24; `sparq mod list --strict` exit 0; the digest MOVES (new test count, new
   src fingerprint — both unmeasured on Windows by design, this is their first device
   measurement). The device also gets the FIRST full measurement of the feature-on cells
   (`cargo clippy -p sparq-host-wasm --features instrument-host --all-targets -- -D warnings` +
   `cargo test -p sparq-host-wasm --features instrument-host`) — if wasmtime refuses the MSVC
   face, apply `docs/instrument-host.md` §8's documented fallback and record the narrowing there.
4. The standing asks are unchanged: test006 A–U, test004 attempt 4, defect #95 triage, the
   dropdown picker.

---

## 8. Environment recipe deltas (this round's findings, on top of ROUND5 §7 + ROUND6 §7)

* The MSVC std face is cheap and worth installing even in a Linux sandbox:
  `rust-std-1.99.0-x86_64-pc-windows-msvc.tar.gz` + `install.sh --prefix=…` → the whole
  default-feature crosscheck family runs (clippy does not link; no MSVC toolchain needed).
* `swapon` is EPERM in this container class — a memory ceiling cannot be relieved from inside;
  the floor for wasmtime work is a host with ≥ 2 GB free (measured, §3).
* No curl/wget in this sandbox generation: python `urllib.request.urlretrieve` is the download
  door (the rust tarballs, ~400 MB, took 7 s — the network is not the bottleneck).
* Everything else stands: zigcc-direct, `-j 2` for the zero-dep tree (it fits comfortably),
  `CARGO_HOME` under `.cache`, node 20 preinstalled.
